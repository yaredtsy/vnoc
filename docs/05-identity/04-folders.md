# 05.04 Folders: the `.vnoc-id` tracker file

Your idea: track a folder the way macOS keeps `.DS_Store`, with a small hidden file inside the folder. This replaces the plan's earlier rule
("folders never get an ID on disk; a folder is recognised by the hashes of its children"). It is a clear improvement: simpler to build and more reliable.

```
folder markers
├── 1. the idea, and why it beats the old rule
├── 2. the file
├── 3. when it is created (only on attach — F2)
├── 4. how the core reads it
├── 5. every case, in one table
├── 6. side effects to know about
└── 7. later: private projects
```

## 1. The idea

```
src/
├── billing/
│   ├── .vnoc-id          ← "this folder is node 0Q8V5W4T2F7KX9M3B6D1RZ4H7K"
│   ├── invoice.py
│   └── tax.py
└── utils/                ← nobody attached anything here: no file
    └── text.py
```

Rename `billing/` to `invoicing/`, move it under `core/`, do it with `git mv` or in the terminal while VS Code is closed: the file travels inside the
folder, so the ID travels too.

| | Old rule: match by children's hashes | **`.vnoc-id`** |
|---|---|---|
| Rename the folder | works if the files inside did not change | **always works** |
| Rename the folder *and* edit files in the same commit | may fail → Missing | **always works** |
| Needs a saved "before" state (SQLite) | yes | **no** |
| Code to write | a matching step with thresholds | read one small file |
| Cost | none on disk | one tiny committed file per **documented** folder |

It also needs no language driver, which makes it the easiest of the three markers to build.

## 2. The file

Name: **`.vnoc-id`**, directly inside the folder it identifies.

```toml
# V-NOC folder ID. It keeps this folder's docs attached when the folder is renamed or moved.
# Safe to commit. Do not copy this file into another folder.
id = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K"
```

- **TOML with one key**, not a bare line of text. The two comment lines tell a curious teammate what it is, and a later version can add a key without
  breaking 0.1 (same "ignore unknown keys, keep them" rule as every other V-NOC file).
- The leading dot hides it in `ls`, in Finder and in most file listings. On Windows a leading dot does not hide a file; that is acceptable, and V-NOC should
  not set the Windows "hidden" attribute (git does not carry it, so it would differ between clones).
- **It is committed.** A teammate needs it for their V-NOC to follow the folder.

This is the fifth frozen format (with the ID text, `node.toml`, doc frontmatter and the in-code markers).

## 3. When it is created

**Only when you attach something to that folder** (decision F2), and only if markers are on (`[ids] default = "on-attach"`).

So a project with three documented folders has three `.vnoc-id` files. Setup creates none. Browsing creates none.

It is written by the **core**, with the atomic write from [`../04-storage/03-writing-safely.md`](../04-storage/03-writing-safely.md) §2. It is not source
code and nobody has it open, so it does not need to go through VS Code.

## 4. How the core reads it

During the scan, for every folder the walker visits, check for the file:

> **🦀 Rust note: one extra check per folder.**
> Your scan uses the `ignore` crate with `.hidden(true)`, which skips hidden files, so `.vnoc-id` never appears in the walk. Keep that setting (it is what
> keeps `.git/` out) and look for the marker explicitly when the walker yields a directory:
> ```rust
> for entry in walker.flatten() {
>     if entry.file_type().is_some_and(|t| t.is_dir()) {
>         let marker = entry.path().join(".vnoc-id");
>         if let Ok(text) = std::fs::read_to_string(&marker) {       // most folders: a quick "not found"
>             match folder_marker::parse(&text) {
>                 Ok(id) => folders.push((rel_path(entry.path()), id)),
>                 Err(e) => problems.push(Problem::bad_marker(marker, e)),   // reported, never fatal
>             }
>         }
>         continue;
>     }
>     // … files, as today
> }
> ```
> The walker already skips ignored folders (`node_modules`, `target`), so they are never checked.

The extension's file watcher must also report changes to files named `.vnoc-id`, so that a rename done outside VS Code is noticed while it is running.
A `**/*` watcher already does.

Each `(path, id)` goes into the index's `markers` map, exactly like a docstring ID.

## 5. Every case

| What happens | What V-NOC sees | Result |
|---|---|---|
| Folder renamed or moved (VS Code, `git mv`, terminal, any tool) | the marker at a new path | **Found**, docs still attached |
| Folder renamed on another branch, then merged | same | Found |
| Folder deleted | no marker, anchor path gone | **Missing** → Relink or Remove |
| Only `.vnoc-id` deleted (by hand, by a clean-up script) | no marker, but the anchor path still exists | Found "by name" (outlined icon) + a "Write ID again" action |
| Folder **copied** (`cp -r billing billing2`) | the same ID in two folders | the folder whose path equals the anchor in `node.toml` keeps the ID. The copy is listed under Problems: "duplicate folder ID · [Delete the copy's marker]". V-NOC never deletes it by itself. |
| Folder **split** (half the files move to a new folder) | the marker stays where the file stays | the docs stay with the folder that kept `.vnoc-id` |
| Two documented folders **merged** into one | two `.vnoc-id` cannot both exist at one path; one overwrote the other, or git reports a conflict | the surviving ID's docs are Found; the other node is Missing → Relink to the merged folder (then it is two IDs on one node, shown as two doc groups; a real merge is a 0.4 item) |
| All code files removed, `.vnoc-id` left behind | a folder that contains only the marker | Found, with a hint: "this folder is empty except for its V-NOC marker" |
| `.vnoc-id` with broken content | parse error | Problems list; the node falls back to its anchor |
| The folder is git-ignored | never scanned | cannot be documented with a marker; attach falls back to anchor-only |

## 6. Side effects to know about

Small, but better learned here than from a user:

- **git tracks the folder even when it has no other files.** Git ignores empty folders; a folder with `.vnoc-id` is not empty. Harmless, and the hint in
  the table above covers it.
- **VS Code's Explorer shows dot-files.** Users will see `.vnoc-id` in documented folders. A user who minds can add `"**/.vnoc-id": true` to
  `files.exclude`. V-NOC should not change that setting on its own; mention it in the README.
- **Python packaging.** The file is not importable and does not affect `import`, namespace packages or pytest discovery (unlike `__init__.py`, which is
  why the plan refused to touch that). It can end up inside a source distribution if the project packages "all tracked files"; `global-exclude .vnoc-id` in
  `MANIFEST.in` removes it. Worth one line in the user guide.
- **Docker / deploy copies** carry it along. It is about 150 bytes of text.
- **Search and grep** tools that honour hidden-file rules skip it.

## 7. Later: private projects

When private docs arrive (0.5), a `.vnoc-id` would be the one visible trace of V-NOC in a repo whose team does not use it. The answer is already in the
model: in solo mode the default is `[ids] default = "none"`, so no marker of any kind is written, and folder docs are attached by path. A user who wants
folder tracking without committing the file can list `.vnoc-id` in `.git/info/exclude` (a personal, uncommitted ignore file). Nothing to build for 0.1.

Next: [`05-finding-nodes-again.md`](05-finding-nodes-again.md)
