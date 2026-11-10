# V-NOC Rust Backend — Plan

> **What this is:** the top-down plan for rebuilding the V-NOC backend in Rust.
> The static analyser (`pylspt`) is mostly done. This plan covers everything
> *around* it: the data model, storage, the sync orchestrator, the API and the
> crates we use.
>
> **How to read it:** top to bottom. Every folder starts with the *why*, then
> the *what*, then the *how*. Each file ends with **exit criteria**: how you know
> that step is done.
>
> **Style:** written to teach. Wherever a Rust idea shows up for the first time
> (newtypes, traits, `async` in traits, `Arc`, channels, error enums, …) there is
> a box marked **🦀 Rust note** that explains it with a small example. Skip the
> boxes if you already know the concept.

---

## The old codebase in one paragraph

The Python backend (`~/Documents/Programing/ide/v-noc/src/backend`) works, but it
grew without a plan. The pain points were
written down in `v-noc/plan/parser-orchestrator-refactor/01-current-state-analysis.md`
and are summarised in [`00-foundations/01-lessons-from-python.md`](00-foundations/01-lessons-from-python.md).
The short list:

1. **Groups break resync.** Anything a user put in a group looks "moved" on the next sync and is pulled back out.
2. **No root node.** Top-level folders/files have `parent = null`, so "give me the top level" is a slow negation query.
3. **Moves look like delete + create.** If a function leaves file A (batch 1) and shows up in file B (batch 2), batch 1 deletes it, batch 2 creates a new one, and all its docs and groups are lost.
4. **The call tree is fully expanded per function.** When `B` changes, every tree that contains `B` is stale, and there is no cheap way to know which ones actually changed.
5. **Too many commits, read-modify-write updates, full-document snapshots.** TerminusDB gets slower with every sync.
6. **Storage is everywhere.** WOQL is built in services, repos and the parser. Changing the DB would mean rewriting everything.

---

## Decisions already taken (2026-11-04)

| # | Topic | Decision | Where it lives |
|---|---|---|---|
| D1 | Schema | **New v2 schema.** No compatibility with Python DBs. | [`01-data-model/`](01-data-model/) |
| D2 | Analyser link | **`LanguageDriver` trait.** pylspt runs in-process; TS/JS stays a JSON-RPC process. | [`04-drivers-and-ids/`](04-drivers-and-ids/) |
| D3 | Frontend API | **gRPC / Connect.** | [`06-api/`](06-api/) |
| D4 | Call tree | **Two layers.** Context graph = source of truth; tree = materialised view with deterministic IDs. | [`05-orchestrator/07-call-graph-sync.md`](05-orchestrator/07-call-graph-sync.md) |
| D5 | Stable IDs | **Keep the Python approach** (docstring `ID:`, `FileID:`, `FolderID` in `__init__.py`) **behind an `IdStrategy` trait**, so other strategies can plug in later. | [`04-drivers-and-ids/04-id-strategy.md`](04-drivers-and-ids/04-id-strategy.md) |
| D6 | Deletes | **Deferred to the end of the sync, then tombstoned.** Hard delete only by garbage collection. | [`05-orchestrator/06-moves-and-tombstones.md`](05-orchestrator/06-moves-and-tombstones.md) |
| D7 | Services | The only external service for now: **language drivers**. | [`04-drivers-and-ids/`](04-drivers-and-ids/) |
| D8 | DB | **Abstract the DB.** TerminusDB is the first adapter, not the architecture. | [`03-storage/`](03-storage/) |
| D9 | Root | **A real `Root` node** in every project graph. | [`01-data-model/01-node-kinds.md`](01-data-model/01-node-kinds.md) |

Smaller defaults I picked are marked **"proposed — confirm"** in
[`00-foundations/02-decision-log.md`](00-foundations/02-decision-log.md). Please look through them.

**Deferred on purpose** (kept open in the design, not planned in detail):
logs (`vn_logger`), conversations, tasks, agent/walkthrough, tests, playgrounds,
versioning UI, the "no-ID" mode, a migration tool from v1 DBs.
See [`07-roadmap/03-deferred.md`](07-roadmap/03-deferred.md).

---

## Reading order

```
plan/
│
├── README.md ............................................ you are here
│
├── 00-foundations/ ...................................... WHY
│   ├── 01-lessons-from-python.md ........................ what broke, with file:line evidence
│   ├── 02-decision-log.md ............................... every decision, its reason, its status
│   └── 03-architecture-overview.md ...................... the layers and how data flows through them
│
├── 01-data-model/ ....................................... WHAT we store
│   ├── 00-overview.md ................................... the graph at a glance
│   ├── 01-node-kinds.md ................................. Root, Folder, File, Class, Function, Group, …
│   ├── 02-identity.md ................................... NodeId, deterministic call IDs
│   ├── 03-edges-parent-and-owner.md ..................... ★ the trick that makes groups transparent
│   ├── 04-lifecycle-and-tombstones.md ................... Active → Tombstoned → (Resurrected | Collected)
│   └── 05-rust-domain-types.md .......................... the structs/enums, explained
│
├── 02-workspace-and-crates/ ............................. HOW the code is organised
│   ├── 01-cargo-workspace.md ............................ crates, what depends on what, and why
│   ├── 02-third-party-crates.md ......................... tokio, tonic, reqwest, ignore, notify, …
│   └── 03-errors-logging-config.md ...................... thiserror vs anyhow, tracing, config
│
├── 03-storage/ .......................................... HOW we store (DB-agnostic)
│   ├── 00-overview.md
│   ├── 01-store-traits.md ............................... the abstraction boundary
│   ├── 02-repositories.md ............................... read models + the write plan
│   ├── 03-write-plan-and-unit-of-work.md ................ one sync = one atomic change
│   ├── 04-terminus-adapter.md ........................... TerminusDB specifics, kept in one crate
│   ├── 05-terminus-best-practices.md .................... batching, commits, branches, optimize
│   └── 06-memory-store-and-tests.md ..................... the second adapter that keeps us honest
│
├── 04-drivers-and-ids/ .................................. HOW we read code
│   ├── 01-language-driver-trait.md
│   ├── 02-pylspt-in-process.md
│   ├── 03-jsonrpc-driver.md ............................. TS/JS (and future languages)
│   └── 04-id-strategy.md ................................ in-source IDs today, open for others
│
├── 05-orchestrator/ ..................................... ★ THE CORE ★
│   ├── 00-overview.md ................................... the pipeline, top to bottom
│   ├── 01-project-lifecycle.md .......................... create / open / delete / rebuild
│   ├── 02-scan.md ....................................... walk the disk, hash, ignore
│   ├── 03-snapshot.md ................................... lean read of what the DB believes
│   ├── 04-structure-diff.md ............................. folders + files: new / changed / moved / gone
│   ├── 05-code-element-diff.md .......................... classes + functions, across batches
│   ├── 06-moves-and-tombstones.md ....................... ★ the "removed then re-added" fix
│   ├── 07-call-graph-sync.md ............................ ★ the two-layer call tree, change propagation
│   ├── 08-groups.md ..................................... groups are invisible to sync
│   ├── 09-scheduler-and-watcher.md ...................... file events → coalesced, serialized syncs
│   └── 10-progress-and-events.md ........................ what the UI sees while a sync runs
│
├── 06-api/ .............................................. HOW others talk to us
│   ├── 01-grpc-connect.md ............................... tonic + gRPC-Web + connect-es
│   ├── 02-proto-layout.md ............................... services and messages
│   └── 03-application-services.md ....................... the layer between API and storage
│
└── 07-roadmap/ .......................................... WHEN
    ├── 01-milestones.md ................................. ordered, each one testable on its own
    ├── 02-testing-strategy.md
    └── 03-deferred.md ................................... what we left open, and the hooks we kept
```

---

## Start here

→ [`00-foundations/01-lessons-from-python.md`](00-foundations/01-lessons-from-python.md)
