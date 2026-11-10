# 05.00 — The orchestrator: overview

`vnoc-sync` turns "the disk changed" into "one `WritePlan`, applied". This
folder is the heart of the plan. This page is the map, and each step has its own file.

---

## The pipeline, with the data that flows between steps

```
SyncRequest { project, scope: Full | Paths(Vec<RelPath>), reason }
│
├─ 1 SCAN ─────────────► ScanResult        { items: [ScanItem{path, kind, hash, size}] }      02-scan.md
├─ 2 SNAPSHOT ─────────► Snapshot          { structure rows, seq, data_version }               03-snapshot.md
├─ 3 IDENTIFY ─────────► Identified        { items + IdRead (found/missing/unsupported) }      04 + ids
├─ 4 STRUCTURE DIFF ───► StructureDiff     { new, changed, moved, unchanged, missing* }        04-structure-diff.md
├─ 5 PARSE ────────────► Outlines          { per changed/new file: [DefOutline] | problem }    05-code-element-diff.md
├─ 6 CODE DIFF ────────► CodeDiff          { new, changed, moved, unchanged, missing* }        05-code-element-diff.md
├─ 7 RECONCILE ────────► Reconciled        { moves (incl. cross-file), tombstones, resurrections, dup-id fixes }   06
├─ 8 CALLS ────────────► CallChanges       { layer 1 ops, layer 2 ops }                         07-call-graph-sync.md
├─ 9 PLAN ─────────────► WritePlan                                                              03-storage/03
├─10 APPLY ────────────► ApplyOutcome      (the ONLY write; one commit)
├─11 PERSIST IDS ──────► PersistReport     (in-source writes, watcher told)                    04-drivers-and-ids/04
└─12 EMIT ─────────────► SyncReport + events                                                    10-progress-and-events.md

* "missing" = candidates for removal. NOT deleted. Step 7 decides.
```

### The two rules that shape everything

1. **Steps 1–9 are read-only.** They produce data. Step 10 is the only write. (Atomicity, testability.)
2. **Classification is global, writing is not.** Steps 4–7 see *the whole sync* before deciding
   anything is gone. That's the fix for "removed in batch 1, re-added in batch 2" (L3). Parsing (5)
   and analysis (8) can be windowed and parallel; the *decisions* wait for all of them.

## The code shape

```rust
// crates/vnoc-sync/src/run.rs

pub struct SyncCtx {                         // everything a run needs, cheap to clone (all Arcs)
    pub project: ProjectRecord,
    pub graph: Arc<dyn ProjectGraph>,
    pub drivers: Drivers,
    pub ids: Arc<dyn IdStrategy>,
    pub events: broadcast::Sender<SyncEvent>,
    pub cfg: SyncConfig,
}

pub async fn run_sync(ctx: &SyncCtx, req: SyncRequest, cancel: CancellationToken)
    -> Result<SyncReport, SyncError>
{
    let scan      = scan::run(ctx, &req.scope).await?;                    // 1
    let snap      = snapshot::load(ctx, &req.scope).await?;               // 2
    let ident     = identify::run(ctx, &scan).await?;                     // 3
    let sdiff     = structure::diff(&ident, &snap);                       // 4  (pure, sync fn!)
    check(&cancel)?;
    let outlines  = parse::run(ctx, &sdiff, &cancel).await?;              // 5  (windowed, parallel)
    let code_snap = snapshot::code(ctx, sdiff.files_to_compare()).await?;
    let cdiff     = code::diff(ctx.ids.as_ref(), &outlines, &code_snap);  // 6  (pure)
    let rec       = reconcile::run(&sdiff, &cdiff, &snap, &code_snap, req.seq()); // 7 (pure)
    check(&cancel)?;
    let calls     = calls::run(ctx, &rec, &cancel).await?;                // 8
    let plan      = plan::build(&rec, &calls, &snap);                     // 9  (pure)
    let outcome   = apply_with_retry(ctx, plan, &snap).await?;            // 10
    let persisted = ctx.ids.persist(rec.minted_ids()).await?;             // 11
    Ok(emit::report(ctx, outcome, persisted, &rec))                       // 12
}
```

Notice which steps are plain `fn` (no `async`): **4, 6, 7, 9**. They're pure functions from
data to data. No DB, no driver, no clock. All the tricky logic (moves, groups, tombstones,
duplicate IDs) lives there, and it can be unit-tested with hand-written inputs in microseconds.
The `async` steps only fetch data and hand it over.

> **🦀 Rust note: "functional core, imperative shell".**
> Keep decisions in pure functions that take `&Input` and return `Output`. Keep I/O in a thin
> async shell around them. In Rust this pays off twice: tests need no runtime or mocks, and the
> borrow checker is happiest when functions borrow inputs immutably and return fresh owned data,
> with no shared mutable state to reason about.

> **🦀 Rust note: cancellation with `CancellationToken`.**
> A newer sync request (or project close) cancels the running one. `tokio_util::sync::CancellationToken`
> is a cheap, clonable flag: `cancel.cancel()` on one side, `cancel.is_cancelled()` or
> `cancel.cancelled().await` on the other. We check it **between** steps and inside the long
> loops (parse windows, analysis). We **never** cancel during step 10 (apply), because a half-sent commit
> isn't something we want to reason about. Once apply starts, the run finishes.

## Full vs scoped syncs

| | Full | Scoped (`Paths`) |
|---|---|---|
| when | project open, manual "resync", periodic safety net (default 30 min), after a driver restart | watcher events |
| scan | whole tree | only the given paths (+ their parent folders, for owner checks) |
| "missing" | anything in the snapshot not seen | only snapshot items **under the scoped paths** |
| moves across scope | detected | half-seen (e.g. only the delete side) → tombstone now, **resurrected** when the other half is synced (06) |

Scoped syncs are what make the watcher cheap. Tombstones make them safe.

## Concurrency inside a run

```
parse:     files ──chunks(256)──► buffer_unordered(4) ──► driver.outline(chunk)
analysis:  entries ──chunks(64)──► buffer_unordered(4) ──► driver.call_graphs(chunk)
           (pylspt then fans each chunk out over rayon threads internally)
```

`futures::stream::iter(chunks).map(|c| driver.outline(c)).buffer_unordered(4)` gives at most 4
batches in flight. No semaphore, no `Mutex`. Each batch returns its own `Vec`, and they're
concatenated.

## Memory bound

The biggest thing held for the whole run is the list of outlines for changed files. That's
small (names, spans, hashes, no source text). Source text is never held by the orchestrator:
the driver reads files itself, and `SourceText` side docs (P13) are built per window and handed
straight to the plan. For a first import of a huge project, the plan is the largest object
(~10 MB per 100k nodes), which is acceptable.

## Files in this folder

| File | Step |
|---|---|
| [`01-project-lifecycle.md`](01-project-lifecycle.md) | before any sync: create / open / close / delete / rebuild |
| [`02-scan.md`](02-scan.md) | 1 |
| [`03-snapshot.md`](03-snapshot.md) | 2 |
| [`04-structure-diff.md`](04-structure-diff.md) | 3–4 |
| [`05-code-element-diff.md`](05-code-element-diff.md) | 5–6 |
| [`06-moves-and-tombstones.md`](06-moves-and-tombstones.md) | 7 ★ |
| [`07-call-graph-sync.md`](07-call-graph-sync.md) | 8 ★ |
| [`08-groups.md`](08-groups.md) | how groups stay out of the way (all steps) |
| [`09-scheduler-and-watcher.md`](09-scheduler-and-watcher.md) | who calls `run_sync`, and when |
| [`10-progress-and-events.md`](10-progress-and-events.md) | 12, and progress during the run |

## Exit criteria for the orchestrator as a whole

- An end-to-end test suite of **scenarios** (fixture project + list of disk edits + expected graph), run on the memory store:
  add/edit/delete/rename/move files, folders, functions; cut/paste across files in one sync and across two syncs;
  copy-paste duplicates; grouped items moved on disk; a syntax error mid-edit; a call added deep in a chain.
- Every scenario also asserts **commit count** and that **no user-owned field changed**.
