# 07.02 — Testing strategy

The design is built so most tests need **no DB, no driver, no runtime**: steps 4, 6, 7, 9 of
the sync and the `rules::*` are pure functions. The pyramid:

```
                 ┌───────────────┐
                 │  E2E (few)    │  real TerminusDB (docker) + real pylspt + fixture project + gRPC client
                 ├───────────────┤
                 │  Scenarios    │  memory store + real pylspt + fixture project + scripted disk edits
                 ├───────────────┤
                 │  Contract     │  same suite against memory and Terminus stores
                 ├───────────────┤
                 │  Unit (many)  │  pure functions: diffs, reconcile, P17, hashes, ID parsing
                 └───────────────┘
```

## 1. Unit tests (pure)

- **Table tests** for every diff/reconcile case listed in 05-04, 05-05, 05-06, 05-08.
- **Golden values** for `call_id`, `subtree_hash`, `outline_hash` (hard-coded hex). Catches accidental hash changes, which would silently re-ID everything.
- **Property tests** (`proptest`):
  - `NodeId` round-trip through `Display`/`FromStr`.
  - Random sequences of group operations + disk moves on a random tree keep invariants I1–I8.
  - "Diff is idempotent": applying the plan, then diffing again against the same disk → empty plan.

## 2. Contract tests (03-06)

Every behaviour sync relies on, against both stores. New store bug → new contract test first.

## 3. Scenario tests: the main safety net

A small DSL so scenarios read like stories:

```rust
#[tokio::test]
async fn cut_paste_across_two_syncs_keeps_docs() {
    let mut s = Scenario::python_fixture("simple_pkg").await;       // memory store + real pylspt
    s.sync().await;
    let f = s.find_function("pkg.a.parse");
    s.add_document(&f, "important note").await;

    s.edit("pkg/a.py", |src| remove_def(src, "parse"));
    s.sync_paths(["pkg/a.py"]).await;                               // scoped: only the delete side
    assert!(s.node(&f).is_tombstoned());

    s.edit("pkg/z.py", |src| append_def(src, PARSE_WITH_SAME_ID));
    s.sync_paths(["pkg/z.py"]).await;
    let n = s.node(&f);
    assert!(n.is_active());
    assert_eq!(n.owner, s.find_file("pkg/z.py"));
    assert_eq!(s.documents_for(&f).len(), 1);                       // the docs survived
    s.assert_commits(3);                                            // one per sync
    s.assert_user_fields_unchanged();                               // always checked
}
```

Every scenario automatically asserts: invariants hold, commit count, user fields unchanged, and
**running one more full sync produces an empty plan** (convergence).

## 4. Parity with the Python backend (optional but useful)

For the fixture corpus, compare the v2 graph (structure + code elements + merged call trees) with what the
Python backend produced, modulo known intended differences (Root, IDs format, group behaviour).
pylspt already has a parity harness for call trees (`pylspt/plan/04-build/03-transport-and-parity.md`).

## 5. E2E

A handful: create project → import → `GetChildren(Root)` over gRPC → edit a file on disk → `WatchProject`
delivers `Committed` + `GraphChanged` → `GetChildren` shows the change. Run in CI with docker.

## 6. Performance tracking

A `bench/` with three fixed projects (small ~200 files, medium ~2k, large ~10k). Record per milestone:
first import time, no-op sync time, single-file edit sync time, hot-utility edit (calls), peak RSS, commits.
Regressions > 20% fail CI (🟡 once numbers are stable).
