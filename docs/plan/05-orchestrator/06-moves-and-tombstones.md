# 05.06 — Step 7: reconcile. Moves, tombstones, resurrections

> ★ This is the fix for: *"a function/file/folder moved; batch 1 saw it gone and deleted
> it, batch 2 saw it new and created it, all its data was lost"* (L3).

---

## Why the problem disappears by construction

Python's problem had two causes:

1. **Decisions per batch.** Batch 1 decided "gone" without knowing what batch 2 would find.
2. **Writes per batch.** That wrong decision was committed before batch 2 could correct it.

v2 removes both:

1. Steps 4 and 6 diff **by ID over the whole sync**. A thing that moved is one ID with a new
   owner, so it's `moved` from the start and never appears as `missing`.
2. Nothing is written until step 10, and even then "gone" means **tombstoned**, not deleted.
   If the other half of the move shows up in a *later* sync, the node is resurrected with all its data.

So step 7 has three remaining jobs: pair up the things that have **no ID** (fallback), decide
what's **really gone**, and **cascade** tombstones down the tree.

---

## Input and output

```rust
pub fn reconcile(s: &StructureDiff, c: &CodeDiff, snap: &Snapshot, code: &CodeSnapshot,
                 seq: u64) -> Reconciled;

pub struct Reconciled {
    pub structure: StructureDiff,            // with fallback moves folded in
    pub code: CodeDiff,
    pub tombstone: Vec<Tombstone>,           // { id, last_path, since_seq: seq }
    pub resurrect: Vec<NodeId>,
    pub warnings: Vec<SyncWarning>,          // e.g. "mass tombstone: 40% of project"
    pub minted: Vec<MintedId>,               // for step 11 (persist)
}
```

## Job 1: fallback pairing for things without IDs

Only `Unsupported` items (non-code files, non-package folders; P18) reach this.

```
files:   missing file rows  ×  unidentified new files
         pair when content_hash equal (exactly one candidate on each side; ambiguous → no pair)
folders: missing folder rows ×  unidentified new folders
         pair when the set of child IDs (as seen this sync) equals the old set of owner-children
         (or ≥ 80% overlap, 🟡 tunable). Run AFTER files, so child IDs are settled.
pairs   → moved (or renamed). The node keeps its DB ID.
unpaired new  → new (minted ID, DB-only since they can't carry one)
unpaired missing → candidates for tombstone (job 2)
```

## Job 2: what's really gone

```
gone_structure = structure.missing  −  (anything paired in job 1)
gone_code      = code.missing       −  (ids that appear anywhere in code.desired)  # always empty
                                                                                   # when diffed by ID; a defensive check
```

The defensive line is cheap and catches a whole class of future bugs (a diff that forgets to
look across files). If it ever removes something, log a `warn!`. It means steps 4–6 have a bug.

## Job 3: cascade

When a container is gone, so is everything under it, **except** what was seen elsewhere in this
sync (moved out before the delete).

```
for f in gone_structure (folders first, top-down):
    for each descendant d of f (by owner, from snapshot + code rows of tombstoned files):
        if d was seen in this sync (it moved out): skip
        else: tombstone(d)
```

Tombstoning a file needs its classes/functions, which may not be in the code snapshot (the file
wasn't parsed, it's gone). So one extra read: `snapshot_code(tombstoned files)`, done by the
async shell before calling `reconcile` (the pure function gets it as input).

Tombstoned call trees: when a Function is tombstoned, its `EntrySummary` and its call nodes are
removed (calls are derived, 01-data-model/04 rule 6). Call nodes with user data are tombstoned
instead. Step 8 handles this, because a removed function is a "change" for every entry that reached it.

## Job 4: resurrection

```
resurrect = structure.resurrected ∪ code.resurrected
```

For each: status → Active, and the usual `moved`/`changed` patches apply on top. Invariant I8
(active nodes have active owners) holds automatically: a resurrected node's owner comes from the
disk *now*, and that owner is either active or being created/resurrected in the same plan.

**Children of a resurrected file:** they were tombstoned together with it. The file is re-parsed
(it's in `files_to_parse` because it's resurrected), so step 6 decides for each child: present
means resurrected, absent means it stays tombstoned. Nothing is resurrected blindly.

## Safety valve: mass tombstones

`git checkout other-branch` can make half the project "disappear" in one sync. With tombstones
that's fine: switching back resurrects everything, docs and groups included. That's a strong
reason for D6. We still:

- emit `SyncWarning::MassTombstone { count, percent }` when > 25% of nodes and > 200 nodes are tombstoned
  in one sync (the UI shows "Large change detected (branch switch?)"),
- and **GC never collects tombstones from a mass-tombstone sync until the user confirms**
  (🟡 proposed), so a long-lived branch can't silently erase user data from `main`'s code.

## Walkthrough: the original bug

```
before:  a.py: parse() [ID p1]     z.py: (nothing)      Function/p1 has 2 docs, in Group G under a.py
user:    cut parse() from a.py, paste into z.py, save both (watcher fires once)

scan:      a.py hash changed, z.py hash changed
parse:     a.py → []           z.py → [parse() ID p1]
code diff: desired p1 (owner z.py, file z.py); snapshot p1 (owner a.py)  → moved
           missing: {} (p1 was seen)
reconcile: nothing to tombstone
plan:      patch p1 { owner: z.py, file: z.py, parent: z.py (G was local to a.py → leaves G, P17), span }
result:    p1 keeps its ID, its 2 docs and its theme; one commit.

the same edit saved as two syncs (a.py first, z.py 1 s later, scoped):
sync 1:    a.py → [], p1 missing → tombstoned (docs kept)
sync 2:    z.py → [p1] → snapshot has p1 Tombstoned → resurrected + moved
result:    the same as above, one sync later.
```

## Exit criteria

- Both walkthrough variants as end-to-end scenario tests (memory store).
- Folder deleted, but one of its files was moved out first in the same sync: the folder and the
  remaining files are tombstoned, the moved file is not.
- README.md moved between folders (no ID): paired by hash, same DB ID afterwards.
- Branch-switch scenario: 60% tombstoned → warning emitted; switch back → 100% resurrected, docs intact.

Next: [`07-call-graph-sync.md`](07-call-graph-sync.md)
