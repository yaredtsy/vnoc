# 07.01 — Milestones

Ordered so that **each one is testable on its own** and nothing waits on the frontend until
M10. The memory store lets sync work (M4–M8) start before the Terminus adapter is complete.

```
M0 workspace ─► M1 domain + store traits + memory store ─┬─► M2 Terminus adapter ─────────────┐
                                                         │                                   │
                                                         └─► M3 lifecycle ─► M4 structure ───┤
                                                                            sync + IDs       │
                                                     M5 pylspt driver ─► M6 code sync ──────┤
                                                                         + groups            │
                                                                    M7 calls L1 ─► M8 L2 ───┤
                                                                                             ▼
                                                              M9 watcher/scheduler ─► M10 gRPC + UI
                                                                                       M11 TS driver
```

---

### M0: workspace skeleton (02-workspace-and-crates)
- Turn `vnoc_playground` into the workspace from 02-01. Empty crates compile.
- Path dependency on pylspt behind the `python` feature. `cargo tree -d` is clean.
- CI: `cargo fmt --check`, `clippy -D warnings`, `cargo test`.
- ✅ **Done when:** 02-01 exit criteria pass.

### M1: domain + store traits + memory store (01, 03-01..03, 03-06)
- `vnoc-domain`: `NodeId`, `Node`, `NodeData`, `NodePatch`, hashes, `rules::*` invariant checks.
- `vnoc-store`: traits, row types, `WritePlan`, the **contract test suite**.
- `vnoc-store-memory` passing the suite, with `check_invariants` after each apply.
- ✅ **Done when:** contract suite green on memory; golden hash tests green.

### M2: TerminusDB adapter (03-04, 03-05)
- Client, schema v2, encode plans, small + big apply paths, lean reads, optimize trigger.
- Docker compose with TerminusDB for tests.
- ✅ **Done when:** contract suite green on Terminus; the 100k benchmark and kill-tests from 03-04 pass;
  the object-indexed `parent` lookup is measured (**the design's riskiest assumption, so do this early**).

### M3: project lifecycle (05-01)
- Registry, create/open/close/delete, `ProjectHandle`, `ProjectWriter`.
- ✅ **Done when:** lifecycle round trip + crash-during-create tests pass.

### M4: structure sync + ID strategy (05-02, 05-03, 05-04, 05-06 structure part, 04-04)
- Scan, snapshot, identify (`InSourceIds` for `FileID`/`FolderID`: port `file_folder_ids.py` into pylspt or `vnoc-drivers`),
  structure diff, reconcile (fallback pairing, tombstones, cascade), plan, apply, persist IDs.
- No code elements yet: files are leaves.
- ✅ **Done when:** structure scenario tests pass (rename/move/delete/copy folder/branch switch/permission denied),
  1 commit per sync, 0 commits for no-op syncs.

### M5: pylspt driver (04-01, 04-02)
- `LanguageDriver` trait, `Drivers` registry, `PythonDriver` with the actor, batched `outline`.
- pylspt additions: FileID/FolderID read/write (if not done in M4), `body_hash`, entry by identity **[check]**.
- ✅ **Done when:** driver test suite passes; outline of the fixture corpus matches goldens.

### M6: code-element sync + groups (05-05, 05-06, 05-08, 06-03 GroupService)
- Parse step, code diff, cross-file moves, duplicate IDs, resurrection, the P17 rule.
- `GroupService` (without API): create/move/delete groups.
- ✅ **Done when:** the cut/paste walkthroughs in 05-06 pass (same sync and across two syncs), grouped items survive
  10 syncs, and no user-owned field ever changes in any scenario.

### M7: call graph layer 1 (05-07 steps A–C)
- `call_graphs` in the driver, `CallContext` + `EntrySummary`, `entries_reaching`, the step-C cut-off.
- ✅ **Done when:** edit 1 of the worked example → 0 call writes; the hot-utility benchmark is measured and recorded.

### M8: call graph layer 2 (05-07 step D)
- Deterministic call IDs, Merkle hashes, level-by-level diff, call groups, per-entry budget.
- ✅ **Done when:** edit 2 → exactly the expected writes; call-tree goldens match `pylspt-dev` merged output;
  grouped call with docs survives remove/re-add.

### M9: watcher + scheduler (05-09, 05-10)
- notify + filter + scheduler actor + safety-net timer + events.
- ✅ **Done when:** 05-09 exit criteria pass (debounce, no lost events, no self-triggered syncs).

### M10: gRPC API + frontend integration (06)
- `vnoc-proto`, `vnoc-api`, `vnoc-server` wiring; buf codegen for the frontend; swap the React app's REST/Socket.IO
  calls for connect-web clients, screen by screen (project list → tree → groups → docs → progress).
- ✅ **Done when:** the React app runs a full create → import → browse → group → edit-file → see-update loop against the Rust backend.

### M11: TS/JS driver (04-03)
- `JsonRpcDriver`, process supervision, batched methods in the Bun driver.
- ✅ **Done when:** the TS fixture passes the driver suite; the kill-Bun-mid-sync test passes.

---

## After M11 (see 03-deferred)
GC job polish, versioning UI (`Versioned` trait), logs, tests, playgrounds, conversations/tasks, agent integration,
lazy call materialisation, other ID strategies, second DB adapter, v1 → v2 import tool.
