# 00.03 — Architecture overview

The whole system on one page. Every box below is a Rust crate (or an external
process), and every arrow is "depends on" or "talks to".

---

## Layers

```
                    ┌──────────────────────────────────────────┐
                    │   Frontend (React)  — connect-es client   │
                    └───────────────────┬──────────────────────┘
                                        │ gRPC-Web (HTTP/1.1 or 2)
┌───────────────────────────────────────▼──────────────────────────────────────┐
│ vnoc-server  (binary)   config · wiring · startup · shutdown                  │
│                                                                               │
│ ┌───────────────────────────────────────────────────────────────────────────┐ │
│ │ vnoc-api      tonic services. THIN: decode request → call app → encode     │ │
│ └───────────────────────────────┬───────────────────────────────────────────┘ │
│ ┌───────────────────────────────▼───────────────────────────────────────────┐ │
│ │ vnoc-app      use cases: ProjectService, GroupService, GraphQueryService,  │ │
│ │               DocumentService, SyncService (asks the scheduler)           │ │
│ └──────────┬──────────────────────────────────────┬─────────────────────────┘ │
│ ┌──────────▼──────────────────────┐   ┌────────────▼──────────────────────────┐ │
│ │ vnoc-sync  ★ orchestrator       │   │ vnoc-store  (TRAITS ONLY)             │ │
│ │  scheduler · watcher            │──►│  GraphStore · repositories · WritePlan │ │
│ │  scan · snapshot · diff         │   └────────────▲──────────────────────────┘ │
│ │  reconcile · call layers · plan │                │ implements                  │
│ └──────────┬──────────────────────┘   ┌────────────┴───────────┬──────────────┐ │
│ ┌──────────▼──────────────────────┐   │ vnoc-store-terminus    │ vnoc-store-  │ │
│ │ vnoc-drivers                    │   │ (reqwest, WOQL, docs)  │ memory(tests)│ │
│ │  LanguageDriver · IdStrategy    │   └────────────┬───────────┴──────────────┘ │
│ │  ├─ python  (pylspt, in-proc)   │                │ HTTP                        │
│ │  └─ jsonrpc (TS/JS, out-of-proc)│                ▼                             │
│ └──────────┬──────────────────────┘        ┌──────────────┐                      │
│            │                               │  TerminusDB  │                      │
│ ┌──────────▼──────────────────────┐        └──────────────┘                      │
│ │ vnoc-domain  pure types: ids,   │  ◄── everyone depends on this; it depends    │
│ │  nodes, edges, errors, hashes   │      on nothing of ours                      │
│ └─────────────────────────────────┘                                              │
└──────────────────────────────────────────────────────────────────────────────────┘
             │ JSON-RPC over HTTP
             ▼
     ┌──────────────────┐
     │ TS/JS driver     │  (Bun, existing: v-noc/src/lsp/ts_js)
     └──────────────────┘
```

### The dependency rule

Arrows only point **down**. In particular:

- `vnoc-domain` knows nothing about the DB, drivers or gRPC.
- `vnoc-sync` knows the `vnoc-store` **traits**, never `vnoc-store-terminus`.
- Only `vnoc-server` (the binary) picks the concrete store and drivers and plugs them together.

This is called **dependency inversion**: high-level code (sync) defines what it
needs as a trait, and low-level code (Terminus) implements it. It's what makes
"swap the DB" (D8) a matter of adding one crate.

> **🦀 Rust note: why crates and not modules?**
> In Rust a *crate* is the unit of compilation and the unit of visibility
> (`pub(crate)`). If `vnoc-sync` doesn't list `vnoc-store-terminus` in its
> `Cargo.toml`, it **cannot** import it, so the compiler enforces the dependency rule.
> Modules inside one crate can't give you that guarantee. A side benefit is
> faster incremental builds: changing the Terminus adapter doesn't recompile the
> sync crate. pylspt (with vendored ruff) is heavy, so keeping it in its own corner
> (`vnoc-drivers`, behind a feature) matters for build times.

---

## The main flow: a sync

```
file saved ─► watcher ─► scheduler (debounce, coalesce, one queue per project)
                                 │
                                 ▼
                         SyncRun (vnoc-sync)
   ┌──────────────────────────────────────────────────────────────────────┐
   │ 1 SCAN        disk → {path, kind, content_hash}                       │
   │ 2 SNAPSHOT    DB → {id, path, hash, owner, status} (lean read)        │
   │ 3 IDENTIFY    IdStrategy: read IDs; list things that need a new ID    │
   │ 4 DIFF        structure (folders/files) by ID, owner-based            │
   │ 5 PARSE       changed files → outlines (classes/functions) via driver │
   │ 6 DIFF        code elements, across ALL files of this sync            │
   │ 7 RECONCILE   removed ∩ appeared = moved; leftovers → tombstone       │
   │ 8 CALLS       layer 1 (context graph) → layer 2 (tree), only changed  │
   │ 9 PLAN        everything above → one WritePlan (pure data)            │
   │10 APPLY       store.apply(plan) → ONE atomic commit                    │
   │11 ASSIGN IDS  IdStrategy writes IDs into new things (watcher muted)   │
   │12 EMIT        progress + "graph changed" events to the UI             │
   └──────────────────────────────────────────────────────────────────────┘
```

The details are in [`05-orchestrator/00-overview.md`](../05-orchestrator/00-overview.md). Two properties
to remember already:

1. **Steps 1–9 never write to the DB.** They build a plan. So a crash halfway leaves the DB untouched,
   and you can unit-test the whole pipeline by checking the plan it produces.
2. **Step 10 is the only write**, and it's atomic.

> Why is ID assignment (step 11) *after* apply? Ordering problem: if we write IDs into
> source files first and then the DB write fails, the files have IDs the DB has never
> seen. That's harmless (next sync sees them as new), but noisy. The other way round,
> the DB knows the ID first and the file catches up. Details and the alternative in
> [`04-drivers-and-ids/04-id-strategy.md`](../04-drivers-and-ids/04-id-strategy.md).

---

## Runtime shape (what runs where)

```
tokio runtime (multi-threaded)
├── tonic server task (handles API requests, many concurrent)
├── per project:
│   ├── watcher task      (notify → filtered events → scheduler channel)
│   ├── scheduler task    (owns the sync queue for this project)
│   └── pylspt actor      (dedicated OS thread, owns the salsa db)
│        └── analysis fan-out on salsa snapshots (rayon / spawn_blocking)
└── shared: store client (reqwest connection pool), driver clients
```

> **🦀 Rust note: tasks vs threads.**
> A tokio *task* is a lightweight green thread: thousands are cheap, and they
> must not block (no long CPU loops, no blocking I/O), or they stall every
> other task on that worker. CPU-heavy work (parsing, analysis, hashing) goes
> to **real threads**: `tokio::task::spawn_blocking`, a `rayon` pool, or a
> dedicated `std::thread`. The rule of thumb is that waiting is done in tasks and
> computing is done on threads.

Next: [`../01-data-model/00-overview.md`](../01-data-model/00-overview.md)
