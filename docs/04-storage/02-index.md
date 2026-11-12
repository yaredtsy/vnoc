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
    ├── 6.1 where the file lives
    ├── 6.2 settings, and two processes on one file
    ├── 6.3 keeping it true: the start-up check
    └── 6.4 call trees: four steps, in order
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
- The file is `.vnoc/local/cache/index.sqlite` once `.vnoc/` exists (§6.1). It is under `local/`, so it is never committed.
- `PRAGMA user_version` is the schema version. Mismatch or corruption → delete the file and rebuild. There are **no migrations**, ever, because nothing in
  it is irreplaceable.
- It **loads into** the same in-memory `Index` at start and is written behind it. The memory index stays the thing requests read from. SQLite makes
  start-up fast; it does not replace the struct.

That last point is worth stressing: the common mistake is to move every read to SQL. Keep reads in memory; use the database as a saved copy.

One rule decides what may go in the file: **everything in it can be rebuilt from the code and the committed files.** Docs, tasks and anything a person
wrote never go in it.

§6.1–6.4 were added 2026-11-12, from a review of an outside proposal for a call-tree cache. Nothing in them is built.

### 6.1 Where the file lives

| Situation | File |
|---|---|
| `.vnoc/` exists | `.vnoc/local/cache/index.sqlite` |
| no `.vnoc/` yet (nothing attached: the root is not chosen, `01-vnoc-folder.md` §1) | VS Code's per-workspace `storageUri`, which the extension passes to the core. Call trees work before the first attach, so they need a place too |

- **Model the files on disk, not branches.** A branch switch is the same event as an edit to many files (§6.3). Branches differ in few functions, so
  little is recomputed.
- **The cache's paths are relative to the driver's root**, the opened folder (`14-call-trees.md` §6.3), not the git root. A cache made with one folder
  opened is not read with another folder opened.
- **Git worktrees** have their own paths, so each gets its own file with no coordination. A new worktree starts cold; copying another worktree's file
  into it is safe, because the start-up check (§6.3) finds what differs.

### 6.2 Settings, and two processes on one file

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;
```

- Two V-NOC windows on one folder, or later the CLI / MCP (`futureplan/02-agent-access`), share one file. WAL allows many readers and one writer; each
  process has its own writer thread and the others wait up to `busy_timeout`.
- Two processes can compute the same entry at once. Write with `INSERT OR IGNORE` on the key: either one wins. This is safe **only** for results that
  are the same whoever computes them (§6.4, the rules).
- WAL does not work on network file systems, and has failed on some Docker bind mounts on macOS. If `PRAGMA journal_mode = WAL` does not answer `wal`,
  use the default journal; if the file cannot be opened at all, run without the cache. A missing cache is slower, never wrong.
- Requests do not query SQLite on each lookup: memory first, SQLite on a miss, new rows sent to the one writer thread in batches.

### 6.3 Keeping it true: the start-up check

While the window is open, the file watcher (`extension/src/core.ts` `watchFiles`) already sees every change on disk: an editor save, an agent writing
through a CLI, a `git pull`, a branch switch. What is missing is what changed **while V-NOC was not running**. Today the in-memory index needs no check
because it is rebuilt from nothing at start; a saved copy does.

| Piece | Rule |
|---|---|
| table | `file(path, mtime, size, content_hash)`, one row per file in scope |
| at start | compare each file's `(mtime, size)` with its row; hash the content only when they differ; a file whose hash differs (or is new, or is gone) is a change, handled as the watcher would handle it |
| a stamp that moved, same content | not a change (a `git checkout` rewrites mtimes) |
| unsaved text | **never written.** A result computed from buffer text (the 0.3 overlay, §4) stays in memory only |

### 6.4 Call trees: four steps, in order

The proposal stored one row per `(function, context)` with call edges between rows, so a path is a walk over shared rows and an edit marks the rows
above it by walking the edges backwards. The idea is right, and the analyzer already works that way in memory: one slot per `ContextId`
(`analyzer_core::graph::ContextId { func, digest }`, `language_driver_rs` `interp/canon.rs`), only the parameters that matter in the key
(`relevant.rs`), and an early stop when a summary's return value did not change (`same_svals`). But the core never sees a summary: the driver sends
only the tree (`14-call-trees.md` T1). So most of the speed comes from simpler steps first. Take each step only when the one before it is measured
and found too slow.

| # | Step | Where | Makes fast |
|---|---|---|---|
| **1** | measure (`14-call-trees.md` step 7.6): time opening the project **apart from** the walk, for a first and a second request | — | — (decides the rest) |
| **2** | keep the project open in the driver between requests; drop the analyzer's summary memo on **any** file change | driver / analyzer, memory only | a second request in the same session |
| **3** | the core saves **merged trees** in SQLite (key below) | core, SQLite | the same tree after a restart, with no analyzer work at all |
| **4** | the analyzer saves **summaries** with their edges, and an edit dirties only what depends on it | `language_driver_rs`, its own SQLite | re-expanding after an edit |

**Step 3's key:**

```
entry (file, qualname, occurrence)
+ the hash of every file the analyzer READ for this request (content hashes from §6.3)
+ the hash of environment_inputs (pyproject.toml, the venv, lock files: 06-drivers.md tier 3)
+ the driver's name and version, and the request's options (depth)
```

- **"Files read"** means every file ty read to answer the request, **including module-resolution misses**: the paths ty looked for and did not find.
  A miss is a dependency too: creating `pkg/foo.py` where an import found nothing before changes the tree.
- That set is not available today. It needs the analyzer to record what it read and the driver to send it with the tree (a new field on the
  `vnoc/callTree` result, additive). **Until it exists, key on the hash of all the project files' stamps** (§6.3's table, every row). Any change to
  any file then misses every saved tree: correct, and still instant after a restart with no edits.

**Step 4: what it needs, in this order.** The first item comes before any table design.

1. **A summary with no `'db` handles.** Today `FunctionResult` holds `Definition<'db>`, `Type<'db>` and `ContextId`, which are salsa handles and
   interner indexes that mean nothing outside one request. It needs a form made of `StableDef`, `StableType`, sites and stable context IDs only, the way
   `canon.rs` already writes a key. Until that form exists there is nothing to put in a table.
2. **Reading a stored key back into values.** `Canon` is `Serialize` only; walking a function again from its stored key (`Walk::materialize`) needs
   the `Fp` values back.
3. **Dependencies per file, not per function body.** A summary depends on what ty infers, and that changes with other files (an imported annotation, a
   base class, a re-export in `__init__.py`, module-level code). Each summary records the files it read, misses included, as step 3 does.
4. **Recursion is one unit.** Functions that call each other are solved together in one fixpoint (`Parked`, `Resolving`, `Heads` in `solver.rs`); a
   change to one recomputes the whole group.
5. **The early stop compares everything a caller reads**: `returns`, `effects`, `status`. Writes to `self` outside `__init__` are not modelled, so they
   cannot be compared.
6. Then the tables: summaries, edges, dependencies, a last-used time.

**Rules for every saved call-tree result**, step 3 or 4:

| Rule | Why |
|---|---|
| never save a result that was cut or marked `incomplete` | a cut depends on the request's history (`contexts_per_func`, `max_contexts` and the deadline are counted per request); it is not the same whoever computes it, so `INSERT OR IGNORE` would keep a wrong one |
| the analyzer's version and its precision options (`fp_depth`, `max_rounds`, `constructor_effects`) are in the key or the file's header; a change drops the cache | the same code with other options gives another tree |
| a last-used time on every row, and a size limit, from the first version | each edit makes new keys and the old rows are never read again |
| a jump target is stored as a range | the canvas opens it as is |

Next: [`03-writing-safely.md`](03-writing-safely.md)
