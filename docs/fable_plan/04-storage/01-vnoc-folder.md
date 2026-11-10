# 04.01 The `.vnoc/` folder: what users commit

This is the frozen part of V-NOC. It builds on `plan_vs/02-technology/04-storage-and-git.md` and keeps its layout. What changes here is only **how little
of it 0.1 creates**, plus one suggested field (`kind`).

```
.vnoc/
├── 1. where it goes
├── 2. the 0.1 layout
├── 3. the files, one by one
│   ├── vnoc.toml
│   ├── .gitignore
│   ├── node.toml
│   ├── docs (*.md)
│   └── local/trash/
├── 4. when it is created
└── 5. rules for every file in here
```

## 1. Where it goes

One `.vnoc/` per project, at the **git root**. If there is no git, at the opened folder.

Finding the git root needs no git library: start at the opened folder and walk up until a folder contains an entry named `.git`.

> **🦀 Rust note: walking up.**
> ```rust
> fn git_root(start: &Path) -> Option<&Path> {
>     // `ancestors()` yields start, its parent, its grandparent… up to the file-system root
>     start.ancestors().find(|dir| dir.join(".git").exists())
> }
> ```
> `.exists()` and not `.is_dir()`: in a git *worktree* or *submodule*, `.git` is a small **file** that points elsewhere.

All paths V-NOC stores are relative to this root, with `/` separators. That is what `RepoPath` already enforces.

## 2. The 0.1 layout

```
<git root>/
├── .vnoc/
│   ├── vnoc.toml                      committed    project settings
│   ├── .gitignore                     committed    one line: local/
│   ├── nodes/                         committed
│   │   └── 7K/                                     shard = last 2 characters of the ID
│   │       └── 0Q8V5W4T2F7KX9M3B6D1RZ4H7K/        one folder per documented node
│   │           ├── node.toml                       where the node is + its state
│   │           ├── why-we-parse-twice.md           a doc
│   │           └── edge-cases.md                   another doc
│   └── local/                         NOT committed (ignored by .vnoc/.gitignore)
│       └── trash/
│           └── 20261110T142233_0Q8V…H7K/           a removed node, restorable
│
├── src/
│   ├── billing/
│   │   ├── .vnoc-id                   committed    folder marker        (05-identity/04)
│   │   └── invoice.py                              # FileID: …  at the top  (05-identity/03)
│   └── orders.py                                   ID: …  in a docstring   (05-identity/02)
```

Not in 0.1, but the space is reserved (do not use these names for anything else):
`.vnoc/local.toml`, `.vnoc/local/nodes/`, `.vnoc/local/cache/`.

## 3. The files, one by one

### `vnoc.toml`

```toml
# .vnoc/vnoc.toml
format = 1                 # the only required key

# everything below is optional; when absent, the default applies
[index]
include = ["src", "tests"]     # default: the whole repo
exclude = ["src/generated"]    # on top of .gitignore, which is always honoured

[ids]
default = "on-attach"          # "on-attach" (default): attaching writes a marker · "none": never write markers
# style = "docstring"          # reserved for 0.4 ("docstring" | "comment"); 0.1 does not read it
```

- `format` is the **one version gate** of the whole on-disk format. A V-NOC that reads a number bigger than it knows opens the project read-only.
- 0.1 writes only `format = 1`. It reads `[index]` and `[ids] default` if you add them by hand.
- Keys it does not know are kept (same `toml_edit` approach as `node.toml`).

### `.gitignore`

```gitignore
local/
```

Written once, when `.vnoc/` is created. V-NOC never edits the repo's root `.gitignore`.

### `node.toml`

Already implemented in `crates/vnoc-domain/src/node_toml.rs`. I suggest **one** added field before 0.1 freezes: `kind`.

```toml
# .vnoc/nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/node.toml
kind       = "function"                  # NEW (suggested): "function" | "class" | "file" | "folder"
path       = "src/orders.py"             # anchor: where it was last seen (repo-relative)
qualname   = "OrderService.parse_order"  # anchor: the language's own name; "" for a file or folder
id_in_code = "yes"                       # "yes" | "no" | "never": is there a marker?
status     = "active"                    # "active" | "archived"
# occurrence  = 2                        # only when not 1
# archived_at = 2026-11-06               # only when archived
```

**Why add `kind`.** Today a file and a folder look the same in `node.toml` (both have `qualname = ""`). While the thing exists, V-NOC can look at the disk
to tell them apart. When it is **Missing**, it cannot, and that is exactly when the UI needs to say "the *folder* `src/billing` is missing" and offer only
folders in the Relink picker. It is also the field a second language will need ("method", "struct", "interface"…), so treat unknown values the way
`IdInCode::Other` does: keep them, show a generic icon.

Adding it is not a format change (rule 1 in your own file: readers ignore unknown fields). Old files without `kind` are read as "unknown, work it out from
the disk".

What is **not** in `node.toml`, on purpose: Missing (computed), hashes (index), timestamps of "last seen" (would change on every sync and dirty git).

### Docs

```markdown
---
node: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
hint: src/orders.py::OrderService.parse_order
---
# Why we parse twice

The upstream API sends…
```

- `node` repeats the folder's ID so a doc that someone moved by hand can be noticed. **The folder wins** if they disagree.
- `hint` is for humans reading a pull request. It is never used to find anything.
- **The folder is the doc list.** There is no index file to keep in sync: every `*.md` in the node folder is a doc of that node.
- **File name** = a slug of the title. Rules for the slug, so it is a legal file name on all three operating systems:
  - drop `< > : " / \ | ? *` and control characters; spaces become `-`; lower-case ASCII letters; keep other Unicode letters
  - no leading dot, no trailing dot or space; at most 60 characters
  - empty after cleaning, or a Windows reserved name (`con`, `nul`, `aux`, `prn`, `com1`…) → `doc`
  - if the name is taken, add `-2`, `-3`…
- The title shown in the UI is the first `# heading` in the file, else the file name. The user can rename the file freely.

### `local/trash/`

```
.vnoc/local/trash/20261110T142233_0Q8V5W4T2F7KX9M3B6D1RZ4H7K/
├── trashed.toml          node = "0Q8V…", removed_at = 2026-11-10T14:22:33Z, from = "nodes/7K/0Q8V…H7K"
└── nodes/7K/0Q8V…H7K/    the node folder, exactly as it was
```

Remove **moves** the node folder here. Restore moves it back and never overwrites. Nothing here is ever deleted without a click on "Empty trash".
Spike S3 proved this on macOS.

## 4. When it is created

**Nothing is written until the first attach.** Opening a project, browsing the tree and the canvas create no file anywhere.

On the first "Attach doc", in this order:

```
.vnoc/                     create
.vnoc/vnoc.toml            write   format = 1
.vnoc/.gitignore           write   local/
.vnoc/nodes/<shard>/<id>/  create  (+ node.toml + the doc)
```

`.vnoc/local/` appears the first time something is removed.

This replaces the plan's separate "Quick look" mode and the guided setup for 0.1. There is nothing to set up: the default scope is "every file git does not
ignore".

## 5. Rules for every file in here

1. **Readers ignore unknown keys; writers keep them.** (TOML via `toml_edit`; frontmatter: rewrite only the lines you own.)
2. **Everything is untrusted input.** These files arrive by `git pull` from other people. `RepoPath::new` already refuses `..` and absolute paths; apply the
   same suspicion to doc file names and to anything in `trashed.toml`. Never follow a symlink out of `.vnoc/`.
3. **A broken file breaks one node, not the project.** A `node.toml` that does not parse → that node is listed under "Problems" with the file path; every
   other node loads normally.
4. **No file is ever written in place.** [`03-writing-safely.md`](03-writing-safely.md) §2.
5. **Folder names are IDs in canonical form** (26 characters, upper case). A folder under `nodes/` whose name is not a valid ID is ignored and reported once.

Next: [`02-index.md`](02-index.md)
