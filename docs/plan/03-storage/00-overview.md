# 03.00 — Storage overview

Goal (D8, R6): **TerminusDB is an adapter, not the architecture.** You should be
able to add Postgres or SurrealDB by writing *one crate*, without touching sync,
app or API code.

---

## The shape in one picture

```
                vnoc-sync, vnoc-app
                       │ uses only traits
                       ▼
     ┌────────────────────────────────────────────────┐
     │ vnoc-store                                     │
     │                                                │
     │  trait ProjectGraph   (one project's graph)    │
     │    ├── reads   : snapshot_*, children, get, …  │  ← "repositories" = typed read methods
     │    └── write   : apply(WritePlan, CommitMeta)  │  ← THE ONLY WRITE
     │                                                │
     │  trait ProjectRegistry (list of projects)      │
     │  trait GraphStore      (opens ProjectGraphs)   │
     │  trait Versioned       (optional: branches…)   │
     │                                                │
     │  struct WritePlan  (pure data: creates, patches, deletes, side docs)
     └──────────────┬──────────────────┬──────────────┘
                    │ implements       │ implements
          vnoc-store-terminus    vnoc-store-memory
```

## The three ideas

1. **One write method.** Every change, whether a 50k-node sync or a user renaming a group,
   is a `WritePlan` passed to `apply`. An adapter implements writing **once**, atomically.
   Python had ~49 places that wrote and committed (L5); v2 has one.
   → [`03-write-plan-and-unit-of-work.md`](03-write-plan-and-unit-of-work.md)

2. **Reads are use-case shaped, not table shaped.** Instead of a generic `get_all(type)` that
   returns full documents, the trait has `snapshot_structure()` returning exactly the 7 fields
   the diff needs. Each adapter makes that fast in its own way (WOQL `select`, SQL `SELECT`, …).
   → [`01-store-traits.md`](01-store-traits.md), [`02-repositories.md`](02-repositories.md)

3. **Two adapters from day one.** The memory store is the reference implementation, the
   test double, and the proof that the traits don't leak Terminus concepts. A shared
   *contract test suite* runs against both.
   → [`06-memory-store-and-tests.md`](06-memory-store-and-tests.md)

## Files

1. [`01-store-traits.md`](01-store-traits.md): the traits, and how `async` + `dyn` work together in Rust.
2. [`02-repositories.md`](02-repositories.md): the read methods, grouped by use case.
3. [`03-write-plan-and-unit-of-work.md`](03-write-plan-and-unit-of-work.md): `WritePlan`, `CommitMeta`, preconditions, idempotency.
4. [`04-terminus-adapter.md`](04-terminus-adapter.md): schema v2 JSON, the HTTP client, mapping plans to WOQL.
5. [`05-terminus-best-practices.md`](05-terminus-best-practices.md): commits, branches, squash, optimise, batching, query tips.
6. [`06-memory-store-and-tests.md`](06-memory-store-and-tests.md): the second adapter and the contract tests.
