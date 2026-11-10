# 04.01 — The `LanguageDriver` trait

A driver turns source files into **outlines** (classes/functions with IDs and spans)
and **call graphs**. The orchestrator never knows whether the driver is pylspt running
in-process or a Bun process across HTTP (D2).

---

## What Python's driver protocol looked like

`v-noc/src/backend/app/core/parser/drivers/protocol.py`: `parse_file(file, content, resolve_mro)`,
`resolve_calls(file, calls)`, `read_or_inject_file_id(file)`, `read_or_inject_folder_id(folder)`,
`initialize`, `shutdown`, `is_alive`. Problems:

- **One file per call** → one HTTP round-trip per file, per phase.
- **Parse twice** (phase 1 with MRO, phase 2 without).
- **IDs mixed into the driver** and written on a read path (L7).
- **Calls resolved per scope** from a list of call positions the backend sent back.
  pylspt doesn't need that any more (`pylspt/src/nodes.rs`: the `Call` node kind was retired).

## v2 trait

```rust
// crates/vnoc-drivers/src/driver.rs

#[async_trait::async_trait]
pub trait LanguageDriver: Send + Sync {
    fn language(&self) -> LanguageId;
    fn handles(&self, path: &RelPath) -> bool;               // by extension, cheap, no I/O

    /// Tell the driver which files changed on disk since last time (it keeps its own cache).
    async fn files_changed(&self, changes: &[FileChange]) -> Result<(), DriverError>;

    /// Outlines for many files in one call. Never writes to disk.
    async fn outline(&self, files: &[RelPath]) -> Result<Vec<FileOutline>, DriverError>;

    /// Call graphs for many entries in one call.
    async fn call_graphs(&self, entries: &[EntryRef], opts: &CallOptions)
        -> Result<Vec<EntryGraph>, DriverError>;

    async fn health(&self) -> Health;
    async fn shutdown(&self);
}
```

The data that comes back:

```rust
pub struct FileOutline {
    pub path: RelPath,
    pub result: Result<Vec<DefOutline>, ParseProblem>,   // a parse error is DATA (02-03: "per-file failures are data")
}

pub struct DefOutline {
    pub source_id: Option<String>,   // the docstring `ID:` if present, as read (no prefix)
    pub kind: DefKind,               // Class | Function(FnKind)
    pub name: String,
    pub qname: String,
    pub span: Span,
    pub body_hash: Hash128,          // hash of the def's text with the ID line removed
    pub bases: Vec<BaseRef>,         // classes only (MRO)
    pub children: Vec<DefOutline>,   // nesting
}

pub struct EntryRef { pub entry: NodeId, pub file: RelPath, pub qname: String }

pub struct EntryGraph {
    pub entry: NodeId,
    pub contexts: Vec<ContextOut>,   // layer 1: deduped by ContextKey
    pub tree: TreeOut,               // the rendered, merged tree (layer 2 input)
    pub reaches: Vec<NodeId>,        // every project function reachable
    pub stats: CallStats,            // cuts, gaps
}
```

## Driver registry

```rust
pub struct Drivers { by_lang: Vec<Arc<dyn LanguageDriver>> }
impl Drivers {
    pub fn for_path(&self, p: &RelPath) -> Option<&Arc<dyn LanguageDriver>> { … }
    /// Group a batch of files by driver, so each driver gets ONE batched call.
    pub fn partition<'a>(&self, files: &'a [RelPath]) -> Vec<(Arc<dyn LanguageDriver>, Vec<&'a RelPath>)>;
}
```

Files no driver handles are still Files in the graph (with `language = "unknown"`), just with no
children. A README or `.toml` shows up in the tree like it did before.

## Rules

1. **Batch everything.** The orchestrator calls `outline` with windows of N files (default 256), not one by one.
2. **Drivers never write user files.** Writing IDs is the `IdStrategy`'s job (04).
3. **A driver may be slow or crash.** The orchestrator applies a timeout per call. On failure, the files in
   that call are marked "failed this sync" (treated as unchanged, nothing tombstoned).
4. **Parse once.** `outline` returns everything the structure and code diffs need, and `call_graphs` doesn't
   need outlines passed back in, because the driver has its own cache.

> **🦀 Rust note: `Result` inside a `Vec`.**
> `Vec<FileOutline>` where each item holds a `Result` means "the call worked, but
> some files didn't". Compare with `Result<Vec<…>>`, where one bad file would fail the whole batch.
> Choosing *where* the `Result` goes is how Rust expresses partial failure in the type itself.

Next: [`02-pylspt-in-process.md`](02-pylspt-in-process.md)
