# 05.10 — Progress and events

What the UI sees while and after a sync runs. Python emitted Socket.IO events
(`progress.py`, `project:updated`). v2 uses gRPC server-streaming (06-api), fed by a
`broadcast` channel.

---

## Event types

```rust
pub enum SyncEvent {
    Started   { project: ProjectId, seq: u64, scope: ScopeKind, reason: SyncReason },
    Phase     { project: ProjectId, seq: u64, phase: Phase, done: u32, total: u32, current: Option<String> },
    Warning   { project: ProjectId, seq: u64, warning: SyncWarning },         // mass tombstone, failed files
    Committed { project: ProjectId, seq: u64, counts: PlanCounts, commit: Option<CommitRef> },
    Finished  { project: ProjectId, seq: u64, report: SyncReportSummary },
    Failed    { project: ProjectId, seq: u64, error: String, retry_in: Option<Duration> },
    GraphChanged { project: ProjectId, changed: ChangedSummary },            // for UI cache invalidation
}

pub enum Phase { Scan, Identify, Parse, Diff, Calls, Apply, PersistIds }
```

`ChangedSummary` tells the UI **what** to refetch: the set of containers whose children changed
(the `parent`s and `owner`s touched by the plan) plus "call trees of these entries changed". The
UI re-queries just those (with React Query, invalidate those keys). Python's `project:updated`
made the UI reload everything.

## Plumbing

```
run_sync ──► ctx.events: broadcast::Sender<SyncEvent>  (capacity 256)
                 │
                 ├──► API stream for client 1  (WatchProject RPC)
                 ├──► API stream for client 2
                 └──► a `watch::Sender<SyncStatus>` for "current status" (late subscribers get the latest)
```

- **Throttling:** `Phase` events at most every 100 ms per project (Python had a similar throttle in
  `ProgressTracker.emit`). `Started`/`Committed`/`Finished`/`Failed` are always sent.
- **Slow clients:** `broadcast` drops the oldest messages for a receiver that lags
  (`RecvError::Lagged(n)`). The API stream then sends the current `watch` status and continues.
  A slow browser can never slow down the sync.

> **🦀 Rust note: why `broadcast` + `watch` together?**
> `broadcast` delivers *every* event to every subscriber, which is right for a live progress bar.
> But a client that connects mid-sync has missed `Started`. `watch` holds only the *latest*
> value, so the new client reads it once to catch up, then follows the broadcast. Two small
> channels, each doing one job well.

## SyncReport (stored and returned)

```rust
pub struct SyncReport {
    pub seq: u64, pub scope: ScopeKind, pub duration: Duration,
    pub counts: PlanCounts,                     // creates / patches / moves / tombstones / resurrections
    pub calls: CallStats,                        // entries re-solved, entries cut off at step C, call writes
    pub files_failed: Vec<(RelPath, String)>,
    pub warnings: Vec<SyncWarning>,
    pub phase_ms: Vec<(Phase, u32)>,             // from tracing spans
}
```

The last N reports are kept in memory per project (and the summary goes into the commit message).
`calls.entries_cut_off` is the number that shows step C of 07 working.

## Exit criteria

- A UI test client subscribed during an import receives `Started`, monotonically increasing `Phase` progress, `Committed`, `Finished`.
- A client that connects mid-sync gets the current status immediately.
- A deliberately slow client doesn't change sync duration.

Next: [`../06-api/01-grpc-connect.md`](../06-api/01-grpc-connect.md)
