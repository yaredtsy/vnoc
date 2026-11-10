# 03.05 — TerminusDB best practices (the rulebook)

TerminusDB is an **immutable, layered** store, like git for triples. Every commit adds
a layer (the triples added and removed). Reads have to look through the stack of
layers until it's optimised. Almost every rule below follows from that.

---

## Commits

| Rule | Why |
|---|---|
| **One logical operation = one commit.** A sync is one commit (03-storage/04, small or squashed). A user action is one commit. | Fewer layers means faster reads and a readable history. Python made many commits per sync (L5). |
| **Commit messages are structured.** `sync #142: +12 ~40 →3 ✝5 ↺1 (2.3s)` (create, patch, move, tombstone, resurrect). `user: move File/… into Group/…`. `gc: collected 48 (seq ≤ 120)`. | The history becomes a sync log you can read in the Terminus console. |
| **Author** is `vnoc-sync`, `vnoc-gc` or the user's name. | Lets you filter user changes from machine changes in the versioning UI later. |
| **Never commit an empty plan.** `if plan.is_empty() { return Ok(Noop) }` | Empty commits still add layers. Most watcher-triggered syncs change nothing. |

## Writes

| Rule | Why |
|---|---|
| **Patch triples, never replace whole documents** for nodes. | Replace rewrites every triple of the doc (bigger layer) and races with user fields. |
| **No read-modify-write.** If you need the old value, it's already in the snapshot; use `expect` for safety. | Halves round trips; avoids lost updates. |
| **Batch by payload size, not by item count.** Measure JSON bytes while encoding; cut chunks at the budget. | A chunk of 5000 functions with huge spans is a different size from 5000 tiny folders. Python had byte-chunking commented out (`phase_processor.py:135-141`). |
| **Use `TerminusDB-Data-Version`** on direct writes to `main`. | Free optimistic concurrency. |
| **Gzip request bodies** for big chunks (`reqwest` `gzip` feature + `Content-Encoding`). **[check]** server support. | Sync payloads are very repetitive JSON. |

## Layers and optimisation

| Rule | Why |
|---|---|
| **Optimise after big syncs**: call `optimize` on the branch (and `_commits`, `_meta`) when the layer count since the last optimise exceeds a threshold (start with 50), or after a first import. | Collapses the layer stack so reads go back to being fast. Python never optimised. |
| **Run optimise in the background**, not inside the sync's critical path. | It can take a while on big graphs, and the sync result is already committed. |
| **Squash side branches** (big-plan path) so `main` gets one commit per sync. | Same reason. |

## Reads

| Rule | Why |
|---|---|
| **Select only the fields you need** (`select` + specific triples). | Full documents are expensive to assemble. |
| **Avoid negation** (`not(...)`) on hot paths. | Negation means scanning. The Root node (R2) exists to remove the main negation Python needed. |
| **Prefer denormalised links over long `path()` queries.** `file`, `owner`, `entry` fields exist so hot queries are one hop. | Path queries over `parent*` are fine for breadcrumbs (short), not for "all functions of a file". |
| **Lookups by object must be indexed.** `children = triple(?c, parent, X)`. | **[check]** and benchmark in M2. The design depends on it (01-data-model/03). |
| **Page large results** (`limit`/`offset` or keyset by id). | The UI asks for children of huge folders. |
| **Read from a fixed commit for consistency** when a reader needs several queries (the sync snapshot): resolve `main` to a commit once, then query `…/local/commit/<id>`. | The URL helper already supports `commit/<ref>` (`mixins.py::_branch_base`). Without it, a user edit between two snapshot queries could give an inconsistent picture. |

## Databases and branches

| Rule | Why |
|---|---|
| **One database per project** (as in v1). | Isolation, independent history, easy delete. |
| **A separate `registry` database** for the project list (P6). | Keeps project metadata out of project histories. |
| **`main` is the truth.** Side branches are short-lived (`sync-<seq>`) or user-created (versioning, later). | Simple mental model. |
| **Clean up orphan `sync-*` branches on startup** (left behind by a crash). | Otherwise they pile up. |

## Connections and retries

| Rule | Why |
|---|---|
| One `reqwest::Client` per process, with timeouts (connect 3 s; request 60 s, configurable per call for big chunks). | Pooling; no hung syncs. |
| Retry **only** `Unavailable` errors, max 3, exponential backoff with jitter. | Retrying a `Rejected` (our bug) just repeats the bug. |
| Writes are retried safely thanks to `StateCas` / data-version (03-storage/03). | Idempotency. |

---

## Exit criteria

- A 10k-file import produces exactly **1** commit on `main`.
- 100 no-op watcher syncs produce **0** commits.
- After the M2 benchmark import, `optimize` is triggered automatically and read latency drops back to baseline.

Next: [`06-memory-store-and-tests.md`](06-memory-store-and-tests.md)
