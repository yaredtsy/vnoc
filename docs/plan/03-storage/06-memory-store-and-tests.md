# 03.06 — The memory store and the contract tests

## Why a second adapter on day one

An abstraction with one implementation is just indirection. You find out it leaks
the day you try to add the second one. The memory store forces the traits to be honest
from the start, and it pays for itself as the test double for all of sync and app.

## Design

```rust
pub struct MemoryGraph {
    inner: tokio::sync::Mutex<State>,      // one lock = trivially atomic apply
}

struct State {
    nodes:    FxHashMap<NodeId, Node>,
    by_parent: FxHashMap<NodeId, FxHashSet<NodeId>>,   // index: children (UI)
    by_owner:  FxHashMap<NodeId, FxHashSet<NodeId>>,   // index: logical children (sync)
    by_file:   FxHashMap<FileId, FxHashSet<NodeId>>,
    reaches:   FxHashMap<NodeId, FxHashSet<NodeId>>,   // func → entries that reach it
    side:      SideDocs,                               // contexts, summaries, docs, source text
    state:     SyncState,
    commits:   Vec<CommitMeta>,                        // so tests can assert "exactly 1 commit"
}
```

`apply` = lock, check all `expect`s and the `StateCas` **first**, then mutate and update
the indexes, then run `check_invariants()` (I1–I8 from 01-data-model/03) **in debug builds**.
A violated invariant panics in tests, right at the plan that caused it.

> **🦀 Rust note: `tokio::sync::Mutex` vs `std::sync::Mutex`.**
> Use `std::sync::Mutex` when you lock, do quick work and unlock **without `.await`
> in between**. It's faster. Use `tokio::sync::Mutex` when you must hold the lock across an
> `.await`. Holding a std mutex guard across `.await` can deadlock, and it makes the future
> non-`Send`, so the compiler stops you. The memory store's `apply` is all in-memory with no awaits
> inside, so `std::sync::Mutex` would also work. We use tokio's only because the trait methods
> are async anyway and it keeps the code obviously safe if someone adds an await later.
> Either way, make it a conscious choice.

## The contract test suite

One set of tests, **generic over the store**, run against both adapters:

```rust
// crates/vnoc-store/src/contract.rs   (behind a "contract-tests" feature)
pub async fn run_all<S: GraphStore>(make: impl Fn() -> S) {
    create_graph_has_root(&make()).await;
    apply_is_atomic_on_precondition_failure(&make()).await;
    apply_twice_same_seq_is_idempotent(&make()).await;
    patch_does_not_touch_user_fields(&make()).await;
    children_vs_owner_children_with_groups(&make()).await;
    tombstoned_hidden_from_children(&make()).await;
    entries_reaching_reverse_lookup(&make()).await;
    paging_children(&make()).await;
    // … one per behaviour the sync relies on
}
```

```rust
// crates/vnoc-store-memory/tests/contract.rs
#[tokio::test] async fn memory_contract() { vnoc_store::contract::run_all(MemoryStore::new).await }

// crates/vnoc-store-terminus/tests/contract.rs  (needs docker; #[ignore] by default, run in CI)
#[tokio::test] #[ignore] async fn terminus_contract() { run_all(|| terminus_from_env()).await }
```

> **🦀 Rust note: generic test functions.**
> `run_all<S: GraphStore>` is compiled once per concrete `S`, as if you had
> written the suite twice by hand. That's monomorphisation. `impl Fn() -> S` is a
> factory closure, so every test gets a fresh, empty store.

When a third adapter (Postgres, …) arrives, it passes this suite or it isn't done.
That's what "easy to change the DB" means in practice (D8).

## Exit criteria

- The suite has a test for every behaviour sync and app depend on. Rule: a sync bug caused by store behaviour gets a contract test first.
- `MemoryStore` passes it in `cargo test` (no docker). Terminus passes it in CI.

Next: [`../04-drivers-and-ids/01-language-driver-trait.md`](../04-drivers-and-ids/01-language-driver-trait.md)
