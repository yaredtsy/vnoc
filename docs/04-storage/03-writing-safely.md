# 04.03 Writing safely: who writes what, and surviving a crash

```
writing
├── 1. who is allowed to write what
├── 2. atomic writes (never a half-written file)
├── 3. the attach flow, step by step
│   └── what a crash leaves behind at each step
├── 4. remove and restore
└── 5. the userdata module: use cases, not file paths
```

## 1. Who is allowed to write what

| What | Who writes | How | Why |
|---|---|---|---|
| **Source files** (the `ID:` line, `# FileID:`) | **VS Code**, never Rust | the driver plans a text edit, VS Code applies it (`WorkspaceEdit`) | the file may be open with unsaved changes; only VS Code can edit it without a conflict, and undo keeps working |
| **`.vnoc/` new files** (`node.toml`, a new doc, `vnoc.toml`) | **the core** | atomic write (§2) | nobody has them open yet |
| **Doc content** | **the user**, in a VS Code editor tab | normal typing and saving | it is just a Markdown file |
| **`.vnoc-id`** | **the core** | atomic write | it is V-NOC's own file, not source code; no driver, no editor |
| **Moving a node folder** (Remove, Restore) | **VS Code** | `WorkspaceEdit.renameFile` | a doc inside may be open and dirty; the editor follows the move (spike S3) |

The short rule: **the core creates, VS Code edits and moves.**

## 2. Atomic writes

A plain `std::fs::write` truncates the file and then writes the new bytes. If the process dies in between (a crash, the laptop lid, VS Code closing),
the file is left empty or half-written. For `node.toml` that means a documented node whose anchor is gone.

The fix is the same on every operating system: write the new content to a **temporary file in the same folder**, then **rename** it over the real name.
A rename within one folder is a single step for the file system: other programs see either the old file or the new one, never a mix.

> **🦀 Rust note: an atomic write.**
> ```rust
> // crates/vnoc/src/userdata/atomic.rs
> use std::{fs, io::Write, path::Path};
>
> pub fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
>     let dir = target.parent().expect("a file has a parent folder");
>     fs::create_dir_all(dir)?;
>     // same folder as the target: a rename across two disks is a copy, and a copy is not atomic
>     let tmp = dir.join(format!(".{}.tmp-{}", target.file_name().unwrap().to_string_lossy(), std::process::id()));
>     {
>         let mut f = fs::File::create(&tmp)?;
>         f.write_all(bytes)?;
>         f.sync_all()?;              // ask the OS to really put the bytes on the disk before we rename
>     }                               // the file is closed here (end of the block): Windows needs that before a rename
>     fs::rename(&tmp, target)        // the atomic step
> }
> ```
> The `tempfile` crate has `NamedTempFile::persist`, which does the same with more edge cases covered. Either is fine. What matters is that **every** write
> under `.vnoc/` goes through this one function.

For a file that must **not** already exist (a new doc), use `OpenOptions::new().write(true).create_new(true)`: it fails if the name is taken, instead of
silently overwriting someone's doc.

Leftover `.*.tmp-*` files from a crash are deleted at the next start-up.

## 3. The attach flow

"Attach doc" on a node. This is the one flow in 0.1 that touches all four storage places, so it is worth walking slowly.

```
user: Attach doc  (node = src/orders.py::OrderService.parse_order, title = "Why we parse twice")

 1  Does this node already have an ID?            index.id_at(location)   (a marker in code, or a node.toml anchor)
      yes → use it, jump to step 5
 2  Pick the ID                                   first free ID for this location            (05-identity/01)
 3  Markers on, and this node can carry one?
      function / class → driver: planIds(text, id) → edits
                         VS Code applies the edits
      file             → driver: planFileId(text, id) → edits → VS Code applies
      folder           → core writes <folder>/.vnoc-id (atomic)
      result: marker written → id_in_code = "yes"
              could not be written (syntax error in that function, edit refused) → id_in_code = "no"   ← still attach!
      markers off      → id_in_code = "no"
 4  First attach in this project? Create .vnoc/, vnoc.toml, .gitignore
 5  Create the node folder, write node.toml      (atomic)      — skipped if the folder exists
 6  Create the doc file with its frontmatter     (create_new)
 7  Update the index, send vnoc/indexChanged
 8  The extension opens the new doc in an editor tab
```

**A failed marker does not block the attach.** Tracking is optional by design: the doc is attached by path + name (`id_in_code = "no"`), the user is told
"the ID could not be written into the code (syntax error); this doc is attached by name", and the node gets a "Write ID" action for later.

### What a crash leaves behind

The order above is chosen so that stopping after any step leaves something harmless:

| Stopped after | On disk | What happens next time |
|---|---|---|
| step 3 | a marker in code, no node folder | Nothing breaks: a marker without a folder is an unused ID. Attach again → step 1 finds the marker → same ID → continues. |
| step 5 | node folder with `node.toml`, no doc | The node shows with 0 docs. Attach again adds the doc. |
| step 6 | everything | The index is rebuilt at start; the doc is there. |

The opposite order (files first, marker second) would leave a `node.toml` that says `id_in_code = "yes"` with no ID in the code. That is recoverable too
(it is the "ID was removed from code" case), but it shows the user a warning for something they did not do. Marker first is quieter.

## 4. Remove and restore

**Revised 2026-11-11** ([`../11-missing-relink-remove.md`](../11-missing-relink-remove.md) §5): `trashed.toml` is written **before** the move, and
Restore works out where the node goes from its ID.

```
Remove (on a Missing node, after a confirmation that lists the files)
  1  core: write .vnoc/local/trash/<timestamp>_<id>/trashed.toml (atomic)       a crash after this leaves an empty entry, the node untouched
  2  VS Code: rename  .vnoc/nodes/<shard>/<id>/  →  .vnoc/local/trash/<timestamp>_<id>/nodes/<shard>/<id>/
       a doc of this node open with unsaved changes moves with it (S3)
  3  index: drop the record

Restore (command "V-NOC: Restore removed doc" → pick from the list; empty entries are removed when the list is made)
  1  target = nodes/<shard>/<id>, from the ID, never from trashed.toml
     target folder exists again? → stop, say so, leave the trash copy alone   (never overwrite)
  2  VS Code: rename back
  3  index: load the record; it resolves to Found or Missing like any other

Empty trash (command, with a confirmation that shows the size)
  the ONLY place in V-NOC that permanently deletes something a person wrote
```

If a marker is still in the code after Remove, leave it. It is harmless, and it means Restore re-attaches at once.

## 5. The `userdata` module: use cases, not file paths

The idea is from `plan_vs` (the "no `pub fn write(path, bytes)`" note) and it is the right one. Restated for 0.1:

```rust
// crates/vnoc/src/userdata/mod.rs      — the ONLY code that knows what is inside .vnoc/

pub struct UserData { root: PathBuf }        // root = <git root>/.vnoc

impl UserData {
    pub fn load_all(&self) -> LoadReport                                   // every node folder → records + a list of problems
    pub fn is_taken(&self, id: &NodeId) -> bool                            // node folder exists (nodes/ or trash/)
    pub fn create_node(&self, id: &NodeId, toml: &NodeToml) -> Result<()>  // step 5
    pub fn create_doc(&self, id: &NodeId, title: &str, hint: &str) -> Result<PathBuf>   // step 6; returns the new file
    pub fn set_anchor(&self, id: &NodeId, to: &LocationKey) -> Result<()>  // Relink
    pub fn set_id_in_code(&self, id: &NodeId, v: IdInCode) -> Result<()>
    pub fn trash_plan(&self, id: &NodeId) -> MovePlan                      // WHERE things move; VS Code does the move
}
```

> **🦀 Rust note: why this shape protects you.**
> Callers say **what** should happen (`create_doc`), never **where** (`write(".vnoc/nodes/7K/…")`). The path layout lives in one private file
> (`userdata/layout.rs`). Three things follow:
> 1. A rule like "sharded by the last two characters" cannot be broken by a typo somewhere else, because nowhere else builds such a path.
> 2. When private docs arrive (0.5), `create_doc` gains a `side` argument and one function decides between `nodes/` and `local/nodes/`. No caller changes
>    its logic.
> 3. Tests need only a temp folder: `UserData { root: tmp }`, call methods, look at the files.

`load_all` returns problems instead of failing:

```rust
pub struct LoadReport {
    pub records:  Vec<(NodeId, NodeRecord)>,
    pub problems: Vec<Problem>,        // "nodes/7K/XYZ/node.toml: no `path`" — shown in the UI, one line each
}
```

One broken file from a bad merge must never stop the other two hundred docs from loading.

Next: [`04-growth-path.md`](04-growth-path.md)
