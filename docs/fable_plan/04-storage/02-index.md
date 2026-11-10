# 04.02 The index: in memory now, SQLite later

```
index
├── 1. what it holds
├── 2. the Rust shape: a struct with private fields
├── 3. sharing it between requests (Arc + RwLock)
├── 4. keeping it fresh
│   ├── at start: full build
│   ├── on change: only the changed files
│   └── known limit: unsaved text
├── 5. struct now, trait later
└── 6. when to add SQLite, and what it will look like
```

## 1. What it holds

Everything V-NOC knows about the project **right now**, computed from the code and from `.vnoc/`:

```
Index
├── files        every file in scope → its outline (classes, functions, ranges, markers found)
├── folders      every folder in scope → the ID in its .vnoc-id, if any
├── markers      ID → where that marker was found in the code            (built from the two above)
├── records      ID → the node folder in .vnoc/: node.toml + doc file names
└── resolved     ID → Found(location, how) | Missing                     (built from markers + records)
```

Today the core has none of this: `vnoc/structure` asks the driver about every file on every call and throws the result away (review C3).

## 2. The Rust shape

```rust
// crates/vnoc/src/index/mod.rs

pub struct Index {
    revision: u64,                                   // +1 on every change; the UI uses it to know "my copy is old"
    files:    BTreeMap<RepoPath, FileEntry>,         // BTreeMap = sorted by key, so the tree comes out in order
    folders:  BTreeMap<RepoPath, FolderEntry>,
    markers:  HashMap<NodeId, Vec<LocationKey>>,     // a Vec: copy-paste can put one ID in two places
    records:  HashMap<NodeId, NodeRecord>,
}

pub struct FileEntry {
    pub stamp:   FileStamp,          // mtime + size, to notice changes cheaply
    pub file_id: Option<NodeId>,     // from the `# FileID:` comment
    pub defs:    Vec<DefEntry>,      // flat, in source order (what the driver's outline returned)
}

pub struct NodeRecord {
    pub toml: NodeToml,              // the parsed node.toml
    pub docs: Vec<String>,           // file names of the *.md files in the node folder
}

pub enum Resolved {
    Found { at: LocationKey, by: FoundBy },
    Missing,
}
pub enum FoundBy { Marker, Anchor }  // drives the solid / outlined link icon in the UI
```

> **🦀 Rust note: private fields are the interface.**
> The fields of `Index` have no `pub`. Code outside the `index` module cannot read or change them; it can only call the methods you give it:
> ```rust
> impl Index {
>     pub fn revision(&self) -> u64 { … }
>     pub fn tree(&self) -> Node { … }                                // what the sidebar and canvas draw
>     pub fn resolve(&self, id: &NodeId) -> Resolved { … }
>     pub fn docs_of(&self, id: &NodeId) -> &[String] { … }
>     pub fn id_at(&self, loc: &LocationKey) -> Option<NodeId> { … }  // "does this function already have an ID?"
>     pub fn missing(&self) -> Vec<NodeId> { … }
>
>     pub fn replace_file(&mut self, path: RepoPath, entry: FileEntry) { … }   // &mut self = this one changes the index
>     pub fn remove_file(&mut self, path: &RepoPath) { … }
>     pub fn put_record(&mut self, id: NodeId, record: NodeRecord) { … }
> }
> ```
> `&self` methods only read. `&mut self` methods change. The compiler will not let anyone hold a reader while a writer runs. That list of methods **is**
> the storage interface. When SQLite arrives, it implements the same list (§5).

**One design rule for these methods:** they speak in domain words (`resolve`, `docs_of`), never in storage words (`get_row`, `query`). If a caller needs
something new, add a method. Do not add a method that returns the raw maps.

## 3. Sharing it between requests

The core handles several requests at once (the sidebar asks for the tree while the canvas asks for docs). They all need the same index.

> **🦀 Rust note: `Arc<RwLock<T>>`.**
> - `Arc<T>` ("atomically reference-counted") lets several owners hold the same value. Cloning an `Arc` copies a pointer, not the data.
> - `RwLock<T>` lets **many readers or one writer** in at a time.
>
> ```rust
> #[derive(Clone)]
> pub struct SharedIndex(Arc<RwLock<Index>>);
>
> // reading: many handlers at once
> let tree = shared.0.read().unwrap().tree();
>
> // writing: one at a time, readers wait
> shared.0.write().unwrap().replace_file(path, entry);
> ```
> **The rule that keeps this safe with async code: never hold the lock across an `.await`.** Take the lock, copy out what you need, release it, *then*
> await. In practice: do the slow work first (ask the driver for an outline), and take the write lock only for the final, instant `replace_file`.
> ```rust
> let outline = client.outline_file(&root, &path).await?;   // slow, no lock held
> shared.0.write().unwrap().replace_file(path, outline.into());   // instant
> ```
> Use `std::sync::RwLock` here, not tokio's. The std one is faster, and because of the rule above you never need to await while holding it.

## 4. Keeping it fresh

### At start: a full build

```
1. find the git root, read .vnoc/vnoc.toml (or defaults)
2. walk the files in scope                      (scan.rs, as today; extensions come from the driver)
3. for each file: ask the driver for its outline
4. for each folder: read .vnoc-id if it exists  (05-identity/04)
5. read every .vnoc/nodes/*/*/node.toml and list its docs
6. build `markers`, then resolve every record
7. revision = 1; tell the extension: vnoc/indexChanged
```

Steps 2–4 are the slow part. The extension should not wait for them: `activate()` returns at once, the tree shows "reading the code…", and fills in when
`indexChanged` arrives.

### On change: only what changed

```
VS Code file watcher (extension)                core
  save / create / delete / rename
        │  collect for ~200 ms (debounce)
        ▼
  vnoc/filesChanged { changed: [...], deleted: [...] }  ─────►  for each changed file in scope:
                                                                   stamp same as before? skip
                                                                   else outline it, replace_file
                                                                for each deleted: remove_file
                                                                a path under .vnoc/ → reload that node's record
                                                                a .vnoc-id → reload that folder
                                                                rebuild markers + resolve; revision += 1
  refresh the sidebar, tell the webview  ◄──── vnoc/indexChanged { revision }
```

> **🧩 VS Code note: the watcher.** `vscode.workspace.createFileSystemWatcher("**/*")` reports create / change / delete for files under the opened folders,
> including changes made outside VS Code (git, the terminal). A `git checkout` can fire thousands of events in a burst, which is why the extension collects
> them for a moment and sends **one** message. If `.vnoc/` is above the opened folder, watch it with a `RelativePattern` based on the git root (already
> tested in spike S1).

**A safety net:** if the batch is very large (say more than 500 paths), do not process it file by file. Run the full build again. It is simpler and it
cannot drift.

### Known limit in 0.1: unsaved text

The index describes files **as saved on disk**. The code shown in a card is read from VS Code's document (so it includes unsaved typing), but the *range* of
the function comes from the index. With unsaved edits above a function, the card shows shifted lines.

- **0.1:** accept it. When the document is dirty, show a small "unsaved changes · save to refresh" hint on the card.
- **0.3 (canvas editing needs it anyway):** the extension sends the buffer text on change; the core outlines the *text* (the driver already accepts `text`
  instead of a file) and keeps it as an overlay on top of the saved entry.

## 5. Struct now, trait later

You chose "in memory first, SQLite later behind the same interface". The cheapest way to get there is **not** to write the trait first.

> **🦀 Rust note: what a trait is, and why to wait.**
> A **trait** is a list of methods that several types promise to have:
> ```rust
> pub trait IndexStore {
>     fn resolve(&self, id: &NodeId) -> Resolved;
>     fn docs_of(&self, id: &NodeId) -> Vec<String>;
>     // …
> }
> impl IndexStore for MemoryIndex { … }
> impl IndexStore for SqliteIndex { … }
> ```
> A trait written before the second implementation exists is a guess. You will guess wrong about what SQLite needs (owned values instead of references,
> errors on every call, transactions), and then you maintain a wrong abstraction.
>
> With private fields (§2), your struct **already is** the interface. The day SQLite arrives:
> 1. copy the `pub fn` signatures of `Index` into `trait IndexStore`,
> 2. write `impl IndexStore for Index` (move the bodies),
> 3. change callers from `Index` to `impl IndexStore` or `Arc<dyn IndexStore>`.
>
> That is an afternoon, and the trait will be right because it was written from real use.

Two small habits now make that afternoon easy:

- Return **owned** values from read methods where it is cheap (`Vec<String>`, not `&[String]`). A database cannot hand out references into itself.
- Let methods that might one day touch a disk return `Result`. Changing `T` to `Result<T, _>` across a codebase later is the tedious part.

## 6. When to add SQLite, and what it will look like

**Add it when a measurement says so,** not on a date. The three triggers:

| Trigger | How you will notice |
|---|---|
| Start-up is too slow | the full build on a real 1,000–10,000 file repo takes longer than you can stand |
| Call-tree results are expensive to recompute | 0.2: expanding the same function after a restart is slow |
| You need "before" data | 0.4: the matching ladder compares the last known state with the current one, so the last state must survive a restart |

**What it will be** (unchanged from `plan_vs/02-technology/04-storage-and-git.md` §8, which is sound):

- `rusqlite` with the `bundled` feature, WAL mode, one writer thread.
- The file is `.vnoc/local/cache/index.sqlite`. It is under `local/`, so it is never committed.
- `PRAGMA user_version` is the schema version. Mismatch or corruption → delete the file and rebuild. There are **no migrations**, ever, because nothing in
  it is irreplaceable.
- It **loads into** the same in-memory `Index` at start and is written behind it. The memory index stays the thing requests read from. SQLite makes
  start-up fast; it does not replace the struct.

That last point is worth stressing: the common mistake is to move every read to SQL. Keep reads in memory; use the database as a saved copy.

Next: [`03-writing-safely.md`](03-writing-safely.md)
