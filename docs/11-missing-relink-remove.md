# 11. Step 5: Missing, Relink, Remove

Written 2026-11-11, from the Step 5 plan the coding session proposed, after a review. It is the build plan for **Step 5** of
[`02-mvp-and-releases.md`](02-mvp-and-releases.md). The rules it builds on are in [`05-identity/05-finding-nodes-again.md`](05-identity/05-finding-nodes-again.md)
and [`04-storage/03-writing-safely.md`](04-storage/03-writing-safely.md) §4. Where this file is more exact, it wins.

```
Step 5
├── 1. the goal
├── 2. core: Missing and Problems become visible · kind in node.toml
├── 3. Relink (two ways in, one core call)
├── 4. Write ID
├── 5. Remove · Restore · Empty trash
├── 6. UI: the "Needs Attention" view · where each action lives
├── 7. messages
├── 8. order of work, each with its test
├── 9. what you decided
└── 10. draft rows for your decision log
```

## 1. The goal

A documented node that you rename, move or delete is never lost: you can see it, attach it again, or clean it up.
Nothing is written into the repo without a click. No file is ever left half-written.

## 2. Core: Missing and Problems become visible

**Today** `resolve` works out which nodes are Missing, but `index/mod.rs` keeps only the found ones. Broken `node.toml` and `.vnoc-id` files go to the
log only. Duplicate IDs are not reported at all: `resolve` picks one copy silently.

**Step 5:**

| What | Rule |
|---|---|
| Missing | `vnoc/structure` gains `missing: [{ id, kind, path, qualname, docs }]`, the anchor as `node.toml` says it, sorted by path. Only records V-NOC **looked for** are listed: see "Missing means looked for" below. |
| Problems | `vnoc/structure` gains `problems: [{ text, file?, line? }]`: where to go when the user clicks. `text` names paths relative to the repo, not absolute. `file` can be a folder (a node folder whose name is not an ID). |
| Duplicates | An ID in two places: the copy at the anchor wins, else the first by path, then line (already the rule in `resolve`). Each other copy becomes a problem that names the node and the copy: "`src/orders.py::parse_order`: its ID is also at `src/copy.py::parse_order`", with the place to go. A copy on the second of two same-named defs ends in `#2`. **Nothing is changed.** |
| `kind` in `node.toml` | `"function"` / `"class"` / `"file"` / `"folder"` (a method is `"function"`: the driver has no method kind). Written on attach and on Relink. A file without it reads as unknown and is **never rewritten by itself**. An unknown value is kept, like `IdInCode::Other`. |
| `kind` and matching | `kind` is for display and the Relink list only. `resolve` does not compare it, so old files and future kinds keep working. |

`kind` is the one format change in 0.1, and it freezes with 0.1. It is additive: an older V-NOC ignores it (rule 1 in `node_toml.rs`).

### Missing means looked for

The records come from the whole repo (`.vnoc/` at the git root). The scan covers less: only the opened folder, no git-ignored files, at most
20,000 files, and not a file the driver failed on. A record whose code was never scanned is **unknown**, not Missing. Listing it would offer
Remove on docs whose code is still there.

```
a record that resolve() calls Missing is listed only when
  · its anchor path was scanned     an outlined file, or a folder the walk found: the ID and the name were looked for and are not there
  · or its anchor path is not on disk     the file or the folder is gone
else it is unknown: not in the tree, not in the list (as before Step 5)
```

- "Scanned" does not include a file in `failed`: the driver gave no outline, so nothing was looked for.
- A path that differs from a scanned path **only by case** counts as scanned. On macOS and Windows the old spelling of a renamed file still
  "exists", and without this a node found by name would turn unknown and show nowhere.
- The disk check runs in `refresh` (`index/build.rs`), beside `load_all` and off the index lock. It hands the index the anchor paths that exist
  but were not scanned. `resolve_all` stays pure. Only records whose path was not scanned cost a filesystem call.
- The anchor path is untrusted (trap 10): it is checked as a `RepoPath` under the git root, and only its existence is read.
- What it does not catch: code that **moved into** a place the scan does not cover. The old path is gone, so the node is listed. Remove goes to
  the trash and Restore brings it back, so nothing is lost.

## 3. Relink

One core call, `vnoc/relink { id, to }`, where `to` is a location key in the index.

```
relink(id, to):
  refuse, with the reason, when
    · `to` carries a different V-NOC ID             "that node already has its own docs"
    · another node already resolves to `to`         same message (a node found by name has no ID in the code, so the first check misses it)
    · `kind` is known and `to` is another kind      (the lists below never offer it; the core checks anyway)
  1  markers on? write the ID at `to`, the same way attach does it:
       function / class / file → the driver plans it, the editor applies it
       folder                  → the core writes .vnoc-id
     a failed write does not block: id_in_code = "no", and the user is told (same as attach)
  2  node.toml: path, qualname, occurrence, kind ← `to`   (one atomic write, toml_edit keeps unknown keys)
```

**The ID is written first, then `node.toml`**, the same order as attach (`04-storage/03` §3, "marker first is quieter").

Two ways in, both end in `vnoc/relink`:

| From | What the user does | The list |
|---|---|---|
| a Missing row | **Relink…** → pick the new place | every node of the same kind (all nodes when the kind is unknown), searchable; the R8 search QuickPick |
| the new code (a tree row, a card, the editor's V-NOC submenu) | **Attach a Missing Doc Here…** → pick the doc | the Missing nodes of that kind (and the unknown ones); if there are none of that kind, say so |

## 4. Write ID

An action on a node **found by name** (the tree already says so: `link: "name"`) while `[ids] default` is not `"none"`. It covers both cases:
the ID line was deleted from the code, and the ID could not be written at attach (a syntax error). It writes the ID the same way Relink step 1 does,
then sets `id_in_code = "yes"`.

## 5. Remove · Restore · Empty trash

**Remove is only for Missing nodes in 0.1** (as `04-storage/03` §4 says). To drop a doc from a node that still exists, delete its `.md` file.

```
Remove (a Missing row → Remove…)
  0  confirm: list the doc files, and say:
       "The docs move to V-NOC's trash on this computer. The deletion shows in git; commit it to remove them for everyone."
  1  core  vnoc/trash { id }   → refuses an ID that is not in the index's `missing` list now (the core holds the rule, not only the menu)
                                 writes .vnoc/local/trash/<YYYYMMDDTHHMMSS>_<id>/trashed.toml (atomic, UTC time)
                                 returns { from, to }:  .vnoc/nodes/<shard>/<id>/  →  .vnoc/local/trash/<entry>/nodes/<shard>/<id>/
  2  VS Code  WorkspaceEdit.renameFile(from, to)   (an open doc follows the move, spike S3)
  3  the .vnoc/** watcher → the index drops the record
```

`trashed.toml` is written **before** the move. A crash or a refused rename between 1 and 2 leaves an **empty entry** (a `trashed.toml` with no node
folder) and the node untouched. Nothing is lost. Empty entries are removed whenever the trash is listed.

```
Restore ("V-NOC: Restore Removed Doc…" → pick)
  1  core  vnoc/trashList        → entries, newest first: { entry, id, removedAt, docs }; empty entries are removed here
  2  core  vnoc/restorePlan { entry } → { from, to }; `to` is worked out from the ID (nodes/<shard>/<id>), never read from trashed.toml
         `to` exists already → refuse: "this node already has docs", leave the entry alone   (never overwrite)
  3  VS Code renameFile(from, to); the entry is now empty and goes at the next listing
```

- The same ID can have two entries (remove, restore, remove again). The list shows both. Restoring one makes the other refuse (step 2).
- `trashed.toml` is not trusted: `.vnoc/local/` is git-ignored, so it never arrives by `git pull`, but it is a plain file anyone can edit, and the ID
  alone says where the node belongs. Check that the entry's folder name, the `id` in it and the folder inside all agree; skip the entry if not.
- If an ID line is still in the code, leave it. Restore then finds the node again at once.

```
Empty Trash ("V-NOC: Empty Trash")
  confirm, with the number of entries and the size
  workspace.fs.delete(trash, { recursive: true, useTrash: true })   → the OS trash where there is one: one more chance
  the ONLY permanent delete in V-NOC
```

## 6. UI

### The "Needs Attention" view

One native view, `vnoc.attention`, under Structure, with two groups. `when: vnoc.hasAttention`: it is there only when something is in it.

```
NEEDS ATTENTION
├── Missing (2)
│   ├── ƒ parse_order        src/orders.py · 2 docs
│   └── ▭ src/billing        folder · 1 doc
└── Problems (1)
    └── ⚠ this ID is also at src/copy.py::parse_order
```

| Row | Click | Right-click (`view/item/context`) and hover buttons |
|---|---|---|
| Missing | opens its docs: the docs tab pinned to the ID (`vnoc/docs` takes any ID), titled with the stored name | Relink… · Remove… · Open Docs |
| Problem | goes to the place: the other copy for a duplicate, the broken file for a broken file. When `file` is a folder, reveal it in the Explorer; do not open it as a document | — |
| Missing (group title) | — | view title `…` menu: Restore Removed Doc… · Empty Trash |

A Missing node is not in the tree, so it can never be selected, focused or shown on the canvas. The session does not change; `treeChanged` already
drops a selected or focused node that went Missing. Add a session test for it.

### Where each action lives

| Action | Missing row | Tree row + card (both lists, `menus.test.ts`) | Editor V-NOC submenu | Command palette |
|---|---|---|---|---|
| Relink… | ✓ | | | |
| Remove… | ✓ | | | |
| Open Docs | ✓ | (already there) | | |
| Attach a Missing Doc Here… | | ✓ when `vnoc.hasMissing` | ✓ | |
| Write ID | | ✓ on rows found by name (`contextValue` flag `byName`, card `nodeContext().byName`) | ✓ | |
| Restore Removed Doc… · Empty Trash | view title | | | ✓ |

The row + card actions follow [`10-ui-rework/04-menus.md`](10-ui-rework/04-menus.md): both lists, the same commands, `category: "V-NOC"`, and
`menus.test.ts` grows with them. Actions on the Missing view are not in the parity test (there is no card for them); add them to its exceptions list.

## 7. Messages

Add each to the typed map in `shared/messages.ts` (and its Rust twin in `crates/vnoc/src/protocol.rs`).

| Message | Direction | Params → result |
|---|---|---|
| `vnoc/structure` | extension → core | + `missing`, + `problems` (§2) |
| `vnoc/relink` | extension → core | `{ id, to }` → `{ written: "id" \| "anchorOnly", notice? }` or an error with the reason |
| `vnoc/writeId` | extension → core | `{ id }` → `{ notice? }` |
| `vnoc/trash` | extension → core | `{ id }` → `{ from, to }`, or an error when the ID is not Missing |
| `vnoc/trashList` | extension → core | `{}` → `[{ entry, id, removedAt, docs }]` |
| `vnoc/restorePlan` | extension → core | `{ entry }` → `{ from, to }` or an error |

Remove and Restore do the move in the extension: the core plans, VS Code moves (`04-storage/03` §1). The core's Rust tests cover the plans and the
refusals. The real-window suite covers the moves.

## 8. Order of work

Each step leaves a working extension and runs the full test list from [`10-ui-rework/05-build-order.md`](10-ui-rework/05-build-order.md) §4.

| # | Build | Proof |
|---|---|---|
| **5.1** | `kind` (read, write on attach, unknown kept) · `missing` and `problems` in `vnoc/structure` · duplicates as problems | Rust: `node.toml` round-trips with and without `kind` and with an unknown one · a missing record is listed · a duplicate is a problem and the winner is unchanged |
| **5.1b** | after the review of 5.1 (commit `step 5.1b: …`, before 5.2): "Missing means looked for" (§2) · problem texts: the duplicate names its node and the occurrence, paths relative to the repo | Rust, one test each: a record outside the opened folder, in a `failed` file, past the cap and in a git-ignored file is **not** listed while its file is on disk, and **is** listed once the file is gone · a case-only rename stays listed · `lifecycle.rs`: `vnoc/structure` returns `missing` and `problems` as JSON |
| **5.2** | the Needs Attention view, `vnoc.hasAttention`, `vnoc.hasMissing`, clicks (a problem on a folder is revealed, not opened) | window test: **delete** a documented function (a rename is followed by its ID, so it would not go Missing), and it shows under Missing; put it back and it leaves · session test: a selected node that goes Missing is dropped |
| **5.3** | Relink from both ways in, with all three refusals | Rust: each refusal · the anchor and `kind` rewritten, unknown keys kept · window test: relink a Missing function to another, and its badge appears there |
| **5.4** | Remove · Restore · Empty Trash, the empty-entry cleanup | Rust: the plans, `vnoc/trash` refuses an ID that is not Missing, the derived `to`, the refusal on an existing folder, an entry that disagrees with itself is skipped · window test: **Remove → Restore gives back the same bytes** (the Step 6 no-data-loss test, written now) |
| **5.5** | Write ID · every new command in its menus · `menus.test.ts` | window test: delete an `ID:` line, run Write ID, and the node is found by marker again · `menus.test.ts` passes |

## 9. What you decided

| Question | Answer |
|---|---|
| `kind` in `node.toml` | **Yes** (S1). Frozen with 0.1. |
| A Problems list in 0.1 | **Yes**, as the second group of the Needs Attention view. |
| Who moves the node folder | **VS Code** (`renameFile`), as `04-storage/03` §1 already said. The core plans. `trashed.toml` is written first. |
| Remove on a node that still exists | **No** in 0.1. Missing only. |
| "V-NOC: Update stored locations" (`05-identity/05` §4) | **Later** (0.1.x). While the ID is in the code the stale anchor is never read, and the command only makes `git status` noisy. Relink covers what 0.1 needs. |
| "Attach a Missing Doc Here…" | **Yes, in 0.1.** It is the way people think about it ("it went here"), and it is the same core call as Relink. |

## 10. Draft rows for your decision log

Paste what you accept into [`08-decisions-to-log.md`](08-decisions-to-log.md); I did not edit it.

| Draft row | Decision | Supersedes |
|---|---|---|
| **S1** | accepted: `kind` in `node.toml`, written on attach and Relink, never rewritten by itself, not used for matching | — |
| **M5a** | Missing and Problems share one native view, "Needs Attention", shown only when it has rows | U9 (a Missing view alone) |
| **M5b** | Remove writes `trashed.toml` first, then VS Code moves the folder; Restore works out the destination from the ID | the order in `04-storage/03` §4 |
| **M5c** | Remove is for Missing nodes only in 0.1 | — |
| **M5d** | "Update stored locations" waits for 0.1.x | `05-identity/05` §4 (0.1) |
| **M5e** | A record is Missing only when its anchor path was scanned or is gone from disk; otherwise it is unknown and not listed. `vnoc/trash` refuses an ID that is not Missing | — |
