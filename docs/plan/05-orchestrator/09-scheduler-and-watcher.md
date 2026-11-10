# 05.09 — Scheduler and watcher: who calls `run_sync`, and when

---

## The pieces

```
notify (OS events, its own thread)
   │  raw events
   ▼
EventFilter  ── drops: ignored paths (.gitignore + defaults), editor temp files (~, .swp, .#*, 4913),
   │            our own ID writes (expected hash, 04-drivers-and-ids/04), directories-only noise
   │  FsEvent { path, kind }
   ▼
Scheduler (one tokio task per project, an ACTOR that owns the queue)
   ├── inputs:  FsEvent (from filter), SyncRequest (from API: "resync", project open, timer)
   ├── debounce: wait until quiet for 500 ms, but never longer than 5 s after the first event
   ├── coalesce: union of paths → Scope::Paths(…); any Full request → Full
   ├── run:     one sync at a time per project
   └── dirty:   events during a run are queued (never dropped); when the run ends, run again
```

## The scheduler as an actor

```rust
pub enum SchedMsg {
    Fs(FsEvent),
    Request { req: SyncRequest, done: Option<oneshot::Sender<Result<SyncReport, SyncError>>> },
    Shutdown,
}

async fn scheduler_loop(ctx: SyncCtx, mut rx: mpsc::Receiver<SchedMsg>) {
    let mut pending = PendingScope::default();         // union of what's waiting
    let mut waiters = Vec::new();                       // oneshot senders to answer after the run
    loop {
        // 1. wait for the first message, then keep collecting until quiet (or max wait)
        // 2. take `pending` → SyncRequest, run it (the run itself is awaited here)
        // 3. while it runs, new messages pile up in `rx`: nothing is lost
        // 4. answer waiters, loop
    }
}
```

> **🦀 Rust note: `tokio::select!` for debounce.**
> The debounce loop waits on *two* things at once: "a new message" and "the quiet timer fired".
>
> ```rust
> let quiet = tokio::time::sleep(Duration::from_millis(500));
> tokio::pin!(quiet);
> loop {
>     tokio::select! {
>         Some(msg) = rx.recv() => { pending.add(msg); quiet.as_mut().reset(Instant::now() + QUIET); }
>         _ = &mut quiet => break,                       // quiet for 500 ms → go
>         _ = &mut max_wait => break,                    // 5 s since first event → go anyway
>     }
> }
> ```
>
> `select!` polls every branch and runs the first that's ready. `tokio::pin!` is needed because
> `sleep` must not move in memory while it's being polled, and `reset` lets one timer be reused
> instead of creating a new one per event.

> **Why one task owns the queue:** no `Mutex<Option<RunningSync>>`, no "is a sync running?" flag
> to forget to clear. The scheduler task *is* the single place where "one sync at a time" is true,
> by construction. This is the same reasoning as the `ProjectWriter` (03-storage/03) and the pylspt
> actor (04-drivers-and-ids/02).

## Rules (fixing L8)

| Rule | Python had |
|---|---|
| Events keep their paths, and the sync is scoped to them | every save → full rescan |
| The watcher **never pauses**; events during a sync are queued | paused during sync → lost events |
| Our own ID writes are filtered by expected hash | ID writes re-triggered syncs |
| Ignored and temp files are filtered before the scheduler | temp files triggered syncs |
| One serialised queue per project | API resync and watcher could overlap |
| A full **safety-net sync** runs every 30 min while open (🟡), plus on open | none |

## Renames from the OS

`notify` reports renames as `(from, to)` pairs on some platforms and as delete + create on
others. We **don't rely on** the rename info: both paths go into the scope, and ID-based
diffing plus tombstones (06) figure it out. Platform differences stay in the watcher.

## Overflow

If the OS event queue overflows (`notify` reports `Rescan`), or the pending path set is larger
than 2,000 paths, switch the scope to `Full`. A full sync is always correct, just slower.

## Exit criteria (M9)

- Save a file 20 times in 2 s → exactly 1 sync (debounce + coalesce) with a 1-path scope.
- Edit a file **during** a sync → a second sync runs right after and picks it up.
- ID persistence after a sync triggers **no** further sync.
- `git checkout` of 3k files → a few syncs at most (overflow → Full), with mass-tombstone warnings as expected.

Next: [`10-progress-and-events.md`](10-progress-and-events.md)
