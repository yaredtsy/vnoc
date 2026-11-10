# 05.03 — Step 2: snapshot

**Input:** the store. **Output:** what the DB currently believes, minimal fields only.

```rust
pub struct Snapshot {
    pub seq: u64,                         // SyncState.seq at read time (for StateCas)
    pub at: CommitRef,                    // the commit we read from (consistency)
    pub structure: Vec<StructureRow>,     // 03-storage/02
    pub by_id:   FxHashMap<NodeId, usize>,    // index into `structure`
    pub by_path: FxHashMap<RelPath, usize>,   // for fallback matching and scoped syncs
}
```

## Consistency: read everything from one commit

Snapshot queries (structure now, code rows later in step 6) must see **the same** version of the graph.
The Terminus adapter resolves `main` to a commit once and reads `…/commit/<id>` for every snapshot
query of this run (03-storage/05, "Read from a fixed commit"). The memory store just clones its state.

## What's in it

- **All** Folder/File rows, Active **and** Tombstoned. Tombstoned rows are needed so step 7 can
  resurrect them.
- **Not** classes/functions (too many). Those are loaded per file in step 6, only for files that
  changed (`snapshot_code(files)`).
- **Not** calls. Step 8 loads them level by level, only where hashes differ.

For a scoped sync, the structure snapshot is still loaded completely (it's small: ~10k rows ≈
1–2 MB), because a scoped change can still be a move to or from anywhere.

## Why not keep the snapshot in memory between syncs?

Tempting (zero DB reads), but the DB can change outside a sync: user edits through the API,
GC, a rebuild. A stale cache would produce wrong plans. Reading 2 MB per sync is cheap; a wrong
diff is not. 🔓 If profiling ever says otherwise, cache it keyed by commit id (valid only while
`main` is still at that commit).

## Exit criteria

- 100k-node graph: `snapshot::load` < 1 s on Terminus.
- A test where a user edit lands between the two snapshot queries: the run still reads one consistent commit.

Next: [`04-structure-diff.md`](04-structure-diff.md)
