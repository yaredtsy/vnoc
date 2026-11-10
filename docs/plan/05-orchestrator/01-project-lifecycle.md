# 05.01 — Project lifecycle

Everything that happens before and around syncs: create, open, close, delete, rebuild.

---

## States

```
          create                 open                    close / idle timeout
 (none) ─────────► Registered ─────────► Open ◄──────────────────────────┐
                       ▲                  │  watcher, scheduler,          │
                       │ close            │  driver actors running        │
                       └──────────────────┘                               │
 delete: Registered|Open ──► stop everything ──► drop graph DB ──► unregister
```

A `ProjectHandle` exists only while **Open**. It owns the per-project tasks:

```rust
pub struct ProjectHandle {
    pub ctx: SyncCtx,                            // 05-orchestrator/00
    scheduler: SchedulerHandle,                  // mpsc sender into the scheduler task
    watcher: Option<WatcherHandle>,
    drivers: Drivers,                            // pylspt actor lives here
    shutdown: CancellationToken,                 // cancels all of the above
}
```

`vnoc-app` keeps `DashMap<ProjectId, Arc<ProjectHandle>>` (or `RwLock<HashMap<…>>`) of open projects.

> **🦀 Rust note: shutting down cleanly with `Drop`?**
> It's tempting to stop tasks in `impl Drop for ProjectHandle`. But `drop` can't be `async`,
> so it can't *wait* for a sync to finish. The pattern instead: an explicit
> `async fn close(self)` that cancels the token, then `join`s the task handles. `Drop` is
> only a safety net (cancel the token, don't wait). Same idea for the pylspt actor thread:
> send `Cmd::Shutdown`, then `join()` it.

## Create project

Input: `name`, `local_path`, settings (ID strategy, languages, ignore extras).

```
1. validate: path exists, is a dir, not already registered (canonicalised path)
2. db_name = slug(name) (+ timestamp suffix if taken, same as Python's bootstrap)
3. store.create_project_graph(db_name)     → DB + schema v2 + Root/root + SyncState{seq:0}
4. registry.put(ProjectRecord{ id, name, local_path, db_name, settings, created_at })
5. open (below)
6. scheduler.request(Full, reason=Initial)  → first sync = the import
7. return immediately with the project; the UI follows the import through progress events
```

If step 3 succeeds and step 4 fails, drop the DB (compensating action) so there are no orphan DBs.
On startup, also reconcile registry ↔ databases: list DBs with the vnoc schema marker that aren't
in the registry and log them (don't auto-delete).

### The first sync is just a sync

There's no special "import" code path. An empty graph + a full scan means everything is
`new`, so the plan is all creates. One code path to test. The first sync also:

- assigns IDs to everything without one (that's most things in a fresh project),
- runs call analysis for **all** entries (07), windowed with progress,
- triggers `optimize` afterwards (03-storage/05).

## Open project

```
1. registry.get(id)
2. graph = store.open(db_name)       (checks schema version, fails loudly on mismatch)
3. start drivers for enabled languages (pylspt actor: open_project(local_path) → warm salsa)
4. start scheduler task
5. start watcher (05-orchestrator/09)
6. scheduler.request(Full, reason=Open)  — catches changes made while we were closed
```

## Close project

Cancel the token → scheduler finishes or cancels the current sync (never mid-apply) → watcher
stops → driver actors shut down → handle removed.

## Delete project

Close → `store.drop_project_graph(db_name)` → `registry.delete(id)`. Source files are **not touched**.
The in-source IDs stay in the code, which is harmless and means re-creating the project later
re-attaches to them. 🟡 Optional later: a "strip IDs" tool.

## Rebuild (escape hatch)

"Something is wrong, rebuild from scratch": drop and re-create the graph DB, keep the registry
entry, run a full sync. Since IDs are in the source, functions/files get **the same IDs back**,
but user data (docs, groups) is lost, so the UI must warn. Better: export user-owned data first
(documents, groups, themes keyed by NodeId) and re-apply after the rebuild. 🔓 Deferred, but the
NodeId-keyed design makes it possible.

## Project settings (stored in the registry record)

```rust
pub struct ProjectSettings {
    pub id_strategy: IdStrategyKind,          // InSource (D5)
    pub languages: Vec<LanguageId>,           // which drivers to start
    pub extra_ignores: Vec<String>,           // on top of .gitignore
    pub store_source_text: bool,              // P13
    pub tombstone_gc: GcPolicy,               // P11
    pub call_limits: CallOptions,             // pylspt Limits (depth, budget, deadline)
}
```

## Exit criteria (M3)

- Create → close → open → delete round trip with a fixture project. After delete there is no DB, no registry entry, and source files are unchanged.
- Kill the server during create step 3/4: on restart, no orphan registry entry, and an orphan DB is logged.

Next: [`02-scan.md`](02-scan.md)
