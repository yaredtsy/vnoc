# 03.01 — The store traits

---

## The traits

```rust
// crates/vnoc-store/src/lib.rs

/// Opens per-project graphs. One per process; picked by vnoc-server from config.
#[async_trait::async_trait]
pub trait GraphStore: Send + Sync + 'static {
    async fn create_project_graph(&self, db_name: &DbName) -> Result<(), StoreError>; // schema + Root
    async fn drop_project_graph(&self, db_name: &DbName) -> Result<(), StoreError>;
    async fn open(&self, db_name: &DbName) -> Result<Arc<dyn ProjectGraph>, StoreError>;
    fn registry(&self) -> Arc<dyn ProjectRegistry>;
    /// `Some` only if the backend supports branches/commits (Terminus does).
    fn versioned(&self, db_name: &DbName) -> Option<Arc<dyn Versioned>> { None }
}

/// One project's graph. All reads + the single write.
#[async_trait::async_trait]
pub trait ProjectGraph: Send + Sync {
    // ---- the one write ----
    async fn apply(&self, plan: WritePlan, meta: CommitMeta) -> Result<ApplyOutcome, StoreError>;

    // ---- reads, grouped by use case (02-repositories.md) ----
    async fn snapshot_structure(&self) -> Result<Vec<StructureRow>, StoreError>;
    async fn snapshot_code(&self, files: &[FileId]) -> Result<Vec<CodeRow>, StoreError>;
    async fn code_rows_by_ids(&self, ids: &[NodeId]) -> Result<Vec<CodeRow>, StoreError>; // moved-in / resurrect
    async fn get_nodes(&self, ids: &[NodeId]) -> Result<Vec<Node>, StoreError>;
    async fn children(&self, parent: &NodeId, q: ChildQuery) -> Result<Page<Node>, StoreError>;
    async fn ancestors(&self, id: &NodeId) -> Result<Vec<Node>, StoreError>;
    async fn entries_reaching(&self, funcs: &[NodeId]) -> Result<Vec<NodeId>, StoreError>;
    async fn call_level(&self, owners: &[NodeId]) -> Result<Vec<CallRow>, StoreError>;
    async fn contexts(&self, keys: &[ContextKey]) -> Result<Vec<ContextRow>, StoreError>;
    async fn tombstones(&self, q: TombstoneQuery) -> Result<Vec<TombstoneRow>, StoreError>;
    async fn documents_for(&self, target: &NodeId) -> Result<Vec<Document>, StoreError>;
    async fn sync_state(&self) -> Result<SyncState, StoreError>;
}

#[async_trait::async_trait]
pub trait ProjectRegistry: Send + Sync {
    async fn list(&self) -> Result<Vec<ProjectRecord>, StoreError>;
    async fn get(&self, id: &ProjectId) -> Result<Option<ProjectRecord>, StoreError>;
    async fn put(&self, rec: ProjectRecord) -> Result<(), StoreError>;
    async fn delete(&self, id: &ProjectId) -> Result<(), StoreError>;
}

/// Optional capability: TerminusDB has it, Postgres would not.
#[async_trait::async_trait]
pub trait Versioned: Send + Sync {
    async fn branches(&self) -> Result<Vec<Branch>, StoreError>;
    async fn create_branch(&self, name: &str, from: &CommitRef) -> Result<(), StoreError>;
    async fn log(&self, branch: &str, limit: usize) -> Result<Vec<CommitInfo>, StoreError>;
    async fn diff(&self, a: &CommitRef, b: &CommitRef) -> Result<GraphDiff, StoreError>;
    // push/pull/remotes later (deferred, 07-roadmap/03)
}
```

### Why this split?

- **`GraphStore` vs `ProjectGraph`**: v1 used one TerminusDB database per project
  (`db_name` in `ProjectSchema`). We keep that. The store opens a handle per project, and the
  handle is what sync holds. A Postgres adapter could map a project to a schema or to a `project_id` column; nobody above cares.
- **`Versioned` is optional.** Version control is a Terminus *feature*, not a storage
  *requirement*. If it were in `ProjectGraph`, every future adapter would have to fake it.
  Code that needs it asks `store.versioned(db)` and handles `None` (the UI hides the versioning panel).
  This is the **capability** pattern: small optional traits instead of one big trait with
  `unimplemented!()` methods.
- **No `query(woql: Value)` escape hatch.** The day someone adds one, Terminus leaks into
  the app layer. If a new use case needs a new read, add a method and implement it in both adapters.

---

> **🦀 Rust note: `async fn` in traits, and why we use `#[async_trait]` here.**
>
> Since Rust 1.75 you can write `async fn` in a trait directly:
>
> ```rust
> trait ProjectGraph { async fn sync_state(&self) -> Result<SyncState, StoreError>; }
> ```
>
> Under the hood, each implementation returns its **own anonymous future type**.
> That's zero-cost, but it has two consequences:
>
> 1. **No `dyn`.** `Arc<dyn ProjectGraph>` needs every method to return the *same* type
>    for all implementations, and anonymous future types differ per impl. So a trait with
>    native `async fn` is not "dyn-compatible" (formerly "object safe").
> 2. **`Send` is not guaranteed.** tokio's multi-threaded runtime moves futures between
>    threads, so they must be `Send`, and a native async trait can't promise that for every impl.
>
> We **want** `dyn` here, because the store is chosen at runtime from config
> (`kind = "terminus"`). The `async_trait` macro rewrites each method to return
> `Pin<Box<dyn Future<Output = …> + Send + '_>>`, a single boxed type, so `dyn` works
> and `Send` is enforced. The cost is one heap allocation per call, which is nothing next to an
> HTTP round trip to TerminusDB.
>
> **Rule of thumb for this project:**
> - Trait chosen at **runtime**, called across I/O → `#[async_trait]` + `Arc<dyn Trait>`.
> - Trait used in a **hot loop**, chosen at compile time → native `async fn` / generics.
>
> (Alternative to know about: "enum dispatch", i.e. `enum AnyStore { Terminus(T), Memory(M) }`
> with a `match` in each method. It's fast and needs no macro, but every new adapter edits that enum.)

> **🦀 Rust note: `Send + Sync + 'static`.**
> - `Send`: the value may be **moved** to another thread.
> - `Sync`: `&value` may be **shared** between threads (so `Arc<T>` can be cloned across tasks).
> - `'static`: it doesn't borrow anything short-lived, so it can live as long as the server.
>
> `Arc<dyn ProjectGraph>` is handed to many tasks at once (API handlers, the sync, the GC),
> so the trait requires all three. The `reqwest::Client` inside the Terminus adapter is
> already `Send + Sync` (and cheap to clone), so this costs us nothing.

---

## What crosses the boundary

Only `vnoc-domain` types plus the row/plan types defined in `vnoc-store`. **Never**:
`serde_json::Value` of WOQL, Terminus IRIs (`terminusdb:///data/…`), commit IDs as raw
strings (wrapped in `CommitRef`), or HTTP status codes (mapped to `StoreError`).

## Exit criteria

- `vnoc-store` depends on `vnoc-domain`, `async-trait`, `thiserror` and `serde`, and nothing else.
- Both adapters compile against the traits. The contract suite (03-storage/06) runs against both.

Next: [`02-repositories.md`](02-repositories.md)
