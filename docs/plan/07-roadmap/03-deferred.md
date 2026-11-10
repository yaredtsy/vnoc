# 07.03 — Deferred (on purpose), and the hooks that keep each one open

You said logs, conversations and tasks can wait. This page lists everything that is **not**
planned in detail, and for each one, what in the current design keeps the door open. The goal
is that adding any of these later is "add a crate/service", not "change the core".

---

| Feature | Python code today | Hook in v2 | Notes for when we get there |
|---|---|---|---|
| **Logs** (`vn_logger`) | `api/json_rpc/entrypoint.py::register_logs_batch`, `log_service.py` | logs attach by **`NodeId`** (the docstring `ID:` that `@context_logger(function_id=…)` already uses). Same IDs in v2, so the user's decorators keep working. | Its own store trait (`LogStore`). High-volume append-only data probably belongs in a different DB (ClickHouse/SQLite/Postgres) than the graph, which the store abstraction allows. Ingestion as a gRPC or HTTP endpoint. |
| **Conversations / agent / walkthrough** | `app/agent/*`, `app/walkthrough/*` (LangGraph, Python) | stays a **Python service** that calls our gRPC API (it's just another client). Graph reads via `GraphService`. | If the agent needs to write (grouping, docs), it uses the same `GroupService`/`DocumentService` RPCs, so it gets the same rules and the same `ProjectWriter`. |
| **Tasks** | `task_service.py` (1k lines) | attaches by `NodeId` | Re-design when needed. |
| **Tests** (pytest interceptor, coverage) | `test_service.py`, `plugins/pytest_interceptor` | test results attach by `NodeId`; a runner is an external worker | |
| **Playgrounds / sandbox** | `play_ground_service.py`, `sandbox/code_run.py` | attaches by `NodeId` | Running user code must be sandboxed in a separate process; never inside the backend. |
| **Versioning UI** (branches, commits, diff, remotes) | `api/v1/versioning/*`, `terminus_client/remote.py` | the optional **`Versioned`** trait (03-01) | Sync always writes to `main`. Branch-aware sync (sync into a user branch) needs a design pass. |
| **Other ID strategies** (O1) | — | **`IdStrategy`** trait + `ProjectSettings.id_strategy` (04-04) | Manifest and fingerprint strategies are sketched in 04-04. |
| **Second DB adapter** (O2) | — | `GraphStore`/`ProjectGraph` traits + the contract suite (03-06) | Postgres sketch: one `nodes` table (id, kind, parent, owner, file, status, jsonb data) + indexes on parent/owner/file; recursive CTE for ancestors; `apply` = one transaction. |
| **Lazy call materialisation** (O3) | — | layer 1 is stored (05-07) | Render layer 2 on demand from stored contexts; step D runs only for opened branches. |
| **v1 → v2 import** (O4) | — | goes through `WritePlan` like a sync | Read the Python DB, map `FolderSchema`/… to v2 nodes (IDs keep their UUIDs, so files match), carry over groups/docs/themes. Only if users need old projects. |
| **"Strip IDs" tool** | — | `IdStrategy` | Remove `ID:`/`FileID:`/`FolderID` lines when a user leaves V-NOC. |
| **Auth / multi-user** | none | `CommitMeta.author` already exists | Today it's local-only (bind to 127.0.0.1). |
| **Rebuild with user-data export** | — | user data is keyed by `NodeId` | Export docs/groups/themes, rebuild, re-import (05-01). |

## Questions to revisit later

- Should `Document` support rich content (JSON) instead of plain text? (Python: `data: str`.)
- Should call-site precision (two calls to `f` in one function as two nodes) come back as an option (P10)?
- Should child **ordering** be stored (manual order in groups)? Currently unordered, like Python.
