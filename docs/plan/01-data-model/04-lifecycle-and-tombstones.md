# 01.04 — Lifecycle and tombstones

Decision D6: nothing that came from disk is hard-deleted by a sync. It's
**tombstoned**: marked as gone, with all its data kept. Later it's either
**resurrected** (it came back = it was a move) or **collected** (GC, really gone).

---

## States

```
                 seen on disk
   (new) ───────────────────────────► Active ◄─────────────────┐
                                        │                      │
                     not seen anywhere  │                      │ seen again (same ID)
                     at end of a sync   ▼                      │  → resurrect (maybe moved)
                                    Tombstoned ────────────────┘
                                        │
                     GC: older than 7 days AND ≥ 20 syncs since (P11)
                                        ▼
                                    Collected (hard delete; docs re-targeted, see below)
```

```rust
pub enum Status {
    Active,
    Tombstoned { since_seq: u64, at: Timestamp, last_path: RelPath },
}
```

`last_path` is kept for the UI ("deleted: `src/old.py`") and for debugging.

## Rules

1. **Only the reconcile step makes tombstones** (05-orchestrator/06), after the whole sync has been
   parsed. No other step deletes anything.
2. **A tombstone hides its subtree.** If file F is tombstoned, its classes/functions are
   tombstoned in the same plan, with the same `since_seq`. Queries for the UI filter out non-Active nodes.
3. **Resurrection restores the subtree as it was, then sync updates it.** A resurrected file is
   then diffed like any other file, so functions that really disappeared while it was gone are
   tombstoned at that point.
4. **Documents are never collected with their node.** At GC time, documents whose target is
   collected are re-targeted to `Root/root` with a field `orphaned_from: { kind, qname, last_path }`.
   The UI can show "notes for deleted code".
5. **User-only nodes are not tombstoned.** A Group the user deletes is gone immediately (they asked).
   If a group becomes empty because all its children were tombstoned, it stays (empty) until the
   user deletes it. Sync never deletes groups.
6. **Call nodes:** a `Call` with *no user data* (not in a group, no documents, no theme) is
   hard-deleted immediately when it leaves the tree. It's derived data and will be re-derived if
   it comes back. A `Call` *with* user data is tombstoned like everything else. This keeps the
   tombstone table from filling up with call-tree churn.

## Why "N syncs AND N days"?

- **Days only:** a project you don't open for a week would collect everything that was mid-move.
- **Syncs only:** an active project with a watcher syncs hundreds of times a day, so 20 syncs could be 10 minutes.
- **Both:** the tombstone must survive a while *and* a number of chances to reconnect. Both numbers are per-project settings.

## GC

- Runs as a separate job, not inside a sync, so syncs stay fast and GC can batch.
- Its own commit: `"gc: collected N nodes (seq ≤ S)"`.
- Triggered after a sync if `tombstones > threshold`, or on a timer, or from the API ("empty trash").

---

## Exit criteria

- Tests: cut a function in sync 1, paste it in sync 2: it's resurrected with its docs and group intact.
- Test: tombstone → GC → documents re-targeted to Root with `orphaned_from`.
- Test: a tombstoned file's children are hidden from `children(parent)` queries.

Next: [`05-rust-domain-types.md`](05-rust-domain-types.md)
