# 05.05 Finding a node again

```
resolving
├── 1. the rule in 0.1: marker → anchor → Missing
├── 2. the states a documented node can be in
├── 3. duplicates (one ID in two places)
├── 4. stale anchors, and why 0.1 never rewrites them by itself
├── 5. Relink
└── 6. what later releases add
```

## 1. The rule in 0.1

Run for every node folder in `.vnoc/nodes/`, after every index change:

```
resolve(id, node.toml):

  1. MARKER   does the index have a marker with this ID?
                one place     → FOUND there (by marker)
                several       → §3 picks one

  2. ANCHOR   does node.toml's (kind, path, qualname, occurrence) exist in the index?
                yes, and that node carries NO other V-NOC marker → FOUND there (by name)
                yes, but it carries a DIFFERENT V-NOC ID          → not ours: go to 3
                no                                                 → go to 3

  3. MISSING
```

The check in step 2 ("a different ID") matters. Without it: you move a documented function to another file, then write a new function with the old name in
the old place and document it too. The first node's stale anchor would point at the second node, and one function would show two nodes' docs.

> **🦀 Rust note: this is one small pure function.**
> ```rust
> pub fn resolve(id: &NodeId, record: &NodeRecord, index: &Index) -> Resolved {
>     if let Some(at) = index.marker_of(id, record) {                 // step 1 (+ §3)
>         return Resolved::Found { at, by: FoundBy::Marker };
>     }
>     if let Ok(anchor) = record.toml.location() {
>         match index.marker_at(&anchor) {                            // which ID, if any, is written at that place?
>             None if index.exists(&anchor) => return Resolved::Found { at: anchor, by: FoundBy::Anchor },
>             _ => {}                                                 // someone else's ID, or nothing there
>         }
>     }
>     Resolved::Missing
> }
> ```
> "Pure" means it reads its arguments and returns a value: no disk, no driver, no clock. Pure functions are the easiest code to test. Write a table of
> small cases (marker found, marker lost, anchor stolen, nothing) and you have covered the heart of the product in fifty lines of test.

## 2. The states

```
                 ┌────────────── found again (marker or anchor) ──────────────┐
                 ▼                                                            │
   FOUND by marker      solid link icon                                       │
        │  marker removed from the code, anchor still valid                   │
        ▼                                                                     │
   FOUND by name        outlined link icon · action: Write ID                 │
        │  renamed / moved / deleted                                          │
        ▼                                                                     │
   MISSING              listed under "Missing" · actions: Relink, Remove ─────┘
        │  Remove
        ▼
   in the trash         commands: Restore removed doc, Empty trash
```

- **None of these states is stored.** They are computed from the code every time. Switch to a branch where the function does not exist and the node is
  Missing; switch back and it is Found. Nothing on disk changed, so `git status` stays clean.
- **0.1 does not interrupt.** No notification, no prompt. A "Missing (2)" group appears in the sidebar and that is all. This is why 0.1 does not need the
  plan's Missing vs "not on this branch" rule, `gix`, or a `.git` watcher: those exist to avoid a pop-up that 0.1 does not have.

## 3. Duplicates

Copy-paste a documented function, copy a documented folder, cherry-pick a commit: the same ID now appears in two places.

Which one is the node? First rule that gives an answer:

1. the copy whose location equals the anchor in `node.toml`
2. else the copy that comes first by path, then by line

The other copy is listed under **Problems**: "this ID is also at `src/copy.py::parse_order` · [Go there]". 0.1 changes nothing by itself. The user deletes
the extra line (or the extra `.vnoc-id`).

Giving the copy a fresh ID automatically (the plan's VS-D71 repair) is a source edit nobody asked for; leave it for a later release, behind a click.

## 4. Stale anchors

After a rename, the marker still finds the node, but `node.toml` still holds the old path. Should V-NOC rewrite it?

`node.toml` is a committed file. Rewriting it automatically means: you rename a function, and a file under `.vnoc/` shows up in `git status` by itself.
Worse, on a branch switch V-NOC could "fix" anchors back and forth. The plan has four careful rules (VS-D74) for when a rewrite is safe.

**0.1 takes the simple road: never rewrite an anchor automatically.**

| | |
|---|---|
| Is a stale anchor a problem? | Not while the marker exists: step 1 wins and the anchor is not consulted. |
| When does it matter? | Only if the marker is later lost. Then the node goes Missing instead of "found by name". |
| How does it get fresh? | **Relink** rewrites it. So does a command **"V-NOC: Update stored locations"**, which rewrites every stale anchor at once and tells the user to commit the result. **The command waits for 0.1.x** (decided 2026-11-11). |
| What about a node with no marker (`"no"`)? | Its anchor is all it has, and it changes only by Relink. |

The command gives the same end state as the plan's automatic rules, with the user choosing the moment. Automatic rewriting can be added in 0.4 without
any format change.

## 5. Relink

The manual fix for a Missing node, and the only "matching" in 0.1.

```
Missing ▸ parse_order (src/orders.py) ▸ Relink
   └── a picker with the nodes of the same kind (functions for a function, folders for a folder), searchable
         └── pick  src/order_parsing.py::parse_payload
               ├── node.toml: path, qualname, occurrence ← the new location           (atomic write)
               ├── markers on, target has no marker → write this ID there, id_in_code = "yes"
               └── target already has a DIFFERENT V-NOC ID, or another node already resolves to it (found by name)
                   → stop: "that node already has its own docs".
                   (merging two nodes is a 0.4 item; in 0.1 the user moves the .md files by hand if they want that)
```

It can also start from the new code: **Attach a Missing Doc Here…** on a node lists the Missing nodes of its kind and makes the same call. The ID is
written before `node.toml`, as in attach. Build plan: [`../11-missing-relink-remove.md`](../11-missing-relink-remove.md) §3.

`kind` in `node.toml` ([`../04-storage/01-vnoc-folder.md`](../04-storage/01-vnoc-folder.md) §3) is what lets the picker show the right list when the node
itself no longer exists.

## 6. What later releases add

Each row plugs in as a new step between ANCHOR and MISSING. Steps 1–3 above do not change.

| Release | Step added | Finds |
|---|---|---|
| 0.1.x | **prefix retry**: a marked file or folder moved → retry the anchors under it ([`03-files.md`](03-files.md) §5) | unmarked nodes inside a moved file or folder |
| 0.4 | file-rename events from VS Code | unmarked files renamed in the editor |
| 0.4 | same file content at a new path; git rename detection | unmarked files moved outside the editor |
| 0.4 | body hash, then similarity (asks when unsure) | unmarked functions renamed or moved |
| 0.4 | Missing vs "not on this branch"; grouped notifications; archive | quieter handling of branch switches once there are pop-ups |

All of it is designed and tuned in spike S4 (`spikes/s4-git/spike-match`). When you reach 0.4, that code is the starting point, and it will need the saved
"before" state, which is the natural moment for SQLite.

Next: [`../06-drivers.md`](../06-drivers.md)
