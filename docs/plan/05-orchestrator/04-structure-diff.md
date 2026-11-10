# 05.04 — Steps 3–4: identify and diff the structure (folders and files)

**Input:** `ScanResult`, `Snapshot`, `IdStrategy`. **Output:** `StructureDiff`.
Step 3 (identify) is async (it reads IDs from files). Step 4 is a **pure function**.

---

## Step 3: identify

```rust
let reads: Vec<IdRead> = ctx.ids.read_structure_ids(&scan.items).await?;   // batched, parallel, no writes
```

In-source strategy: Python package folders → read `FolderID` from `__init__.py`; `.py` files → read
`FileID` from the module docstring; everything else → `Unsupported` (04-drivers-and-ids/04, P18).

**Only for items that need it.** If an item's `(size, mtime, hash)` hasn't changed since the
last sync *and* the snapshot has an Active row at the same path with `id_pending = false`,
reuse that row's ID without opening the file. That alone removes the "RPC per file per sync"
cost from Python (L5 analysis, section 4: `read_or_inject_file_id` for every scanned path).

## Step 4: the diff

```rust
pub fn diff(ident: &Identified, snap: &Snapshot, scope: &Scope) -> StructureDiff;

pub struct StructureDiff {
    pub new:         Vec<Seen>,                       // ID not in snapshot (or freshly minted)
    pub resurrected: Vec<Seen>,                       // ID in snapshot as Tombstoned
    pub moved:       Vec<Move>,                       // owner changed
    pub renamed:     Vec<Rename>,                     // same owner, name/path changed
    pub content:     Vec<FileId>,                     // file hash changed → parse
    pub unchanged:   usize,                           // just a count
    pub missing:     Vec<NodeId>,                     // in snapshot + in scope, not seen. CANDIDATES ONLY
    pub unidentified:Vec<Seen>,                       // Unsupported items not matched by path (fallback later)
    pub dup_ids:     Vec<DupId>,                      // same ID seen more than once
}

pub struct Seen { pub id: NodeId, pub path: RelPath, pub owner: NodeId, pub kind: NodeKind,
                  pub hash: Option<Hash128>, pub minted: bool }
pub struct Move { pub id: NodeId, pub from_owner: NodeId, pub to_owner: NodeId,
                  pub new_path: RelPath, pub parent_was_group: bool }
```

### Algorithm

```
sort scanned items by depth (folders before their contents)
disk_id: map path → NodeId   (filled as we go; "" → Root/root)

for item in items (top-down):
    owner = disk_id[parent_path(item.path)]        # known: parents come first

    id = match read(item):
        Found(k)    → NodeId(kind, k)
        Missing     → snapshot row at same path with id_pending? reuse it : mint (minted = true)
        Unsupported → snapshot Active row at same path & kind? reuse it : unidentified (fallback, step 7)

    record (id → occurrence) for duplicate detection
    disk_id[item.path] = id

    match snapshot.by_id.get(id):
        None                         → new
        Some(row) if row.tombstoned  → resurrected (+ moved/renamed/content as below)
        Some(row):
            row.owner != owner       → moved   (owner-based: groups are invisible here, 08-groups.md)
            row.path  != item.path   → renamed (same owner, new name)
            row.hash  != item.hash   → content
            else                     → unchanged

missing = snapshot Active rows within scope whose id was not seen
duplicates: resolve with the policy in 01-data-model/02 (keep the one matching the snapshot,
            the others become `new` with minted ids)
```

Three details:

1. **Owner, not parent.** `row.owner != owner` is the whole group story for structure. A file in a
   group has `parent = Group`, `owner = its folder`. If it's still in that folder on disk, it's
   unchanged, and its group membership is never touched (L1 fixed).
2. **Missing is not deleted.** It's a list of candidates. Step 7 decides (moves across the sync,
   tombstones for the rest).
3. **Content change ≠ structure change.** A file whose bytes changed is listed in `content` and
   goes to the parser. Whether its classes/functions changed is step 6's question.

### Folder renames and `path`

`path` is stored on every Folder/File (P7) for fast lookups and UI. The trade-off: renaming
`src/` → `lib/` changes the `path` of every descendant, so the plan carries one small patch per
descendant (only the `path` triple). For 5k files that's 5k tiny patches in one commit, which is
acceptable for a rare event. The alternative, storing only `name` and deriving paths from the
owner chain, makes renames O(1) but every path lookup O(depth). 🔓 Revisit if huge renames are slow.

### What gets parsed next (input to step 5)

```
files_to_parse = new ∪ resurrected ∪ content     (only files a driver handles)
```

A **moved** file with an unchanged hash is **not** re-parsed: its classes/functions keep their
IDs and their `file` field (same FileId), so only the file's own `owner`/`path` change.

## Tests (table-driven, pure)

| Case | Expected |
|---|---|
| file edited | `content` |
| file renamed in same folder | `renamed` |
| file moved to other folder (has FileID) | `moved` |
| file moved, **was in a group under the old folder** | `moved` with `parent_was_group = true` (step 9 applies the P17 rule) |
| file in a group, untouched | `unchanged` ✔ (the Python bug) |
| folder copied (FolderID duplicated) | one keeps the ID, the copy is `new` with a minted id |
| file deleted | `missing` (not tombstoned yet) |
| file appears, no FileID yet | `new`, `minted = true` |
| `id_pending` file still without ID | reuses the pending ID, not a second mint |
| README.md moved (Unsupported) | `missing` + `unidentified` → paired by content hash in step 7 |
| permission-denied folder | its rows are neither `missing` nor anything else (scan skipped it) |

## Exit criteria

All rows above pass as unit tests of the pure `diff` function, with no store or driver involved.

Next: [`05-code-element-diff.md`](05-code-element-diff.md)
