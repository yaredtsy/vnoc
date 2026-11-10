# 04.02 — The Python driver: pylspt in-process

`vnoc-drivers` (feature `python`) wraps pylspt as a library. No JSON-RPC, no
process, no serialisation.

---

## What pylspt gives us today (verified in `~/Documents/Programing/rust/pylspt/src`)

| Need | pylspt API | Notes |
|---|---|---|
| open a project | `db::open_project(dir) -> ProjectDatabase` | a ty/salsa database |
| outline of a file | `defs::definitions(db, file) -> Vec<DefNode>` | IDs, kinds, spans, nesting |
| inject def IDs | `inject::inject_ids(&mut db, file, path, write: bool)` | has a **dry-run** mode and refuses files that don't parse. That's exactly the guard we want. |
| MRO | `mro.rs` | |
| call graph per entry | `interp::call_graph(db, file, entry: &str, opts) -> Option<GraphOut>` | contexts + edges, stable IDs |
| rendered tree | `interp::call_wire(...) -> (FrameJson, Stats)` + `wire::merge_for_output` | merged by target, like v-noc |
| stable identities | `stable::StableDef`, `canon::StableContextId { func, digest }` | layer-1 keys |

Gaps to fill in pylspt (small, listed in 07-roadmap/01, milestone M5):

- **`FileID` / `FolderID`** reading and writing (Python: `vnoc_lsp_python/file_folder_ids.py`). pylspt has only def IDs.
- **`body_hash`** per def (hash of the def's text minus the ID line). Can be computed in the adapter from spans + source text.
- **Entry by identity, not name**: `call_graph` takes `entry: &str`. **[check]** whether a qname is unique enough
  (`StableDef::Path { nth }` exists for duplicates). If not, add an overload that takes a definition.
- **`reaches`**: the set of functions in `GraphOut.contexts`. Derived in the adapter, no pylspt change.

## The actor: one thread owns the database

salsa's rule: **reads** take `&db` and can run in parallel on clones; **writes** (telling it a
file changed) need `&mut db`, and they cancel in-flight reads. The cleanest way to respect
that in Rust is to give the database **one owner** and talk to it with messages.

```
             (tokio side)                         (dedicated OS thread)
 PythonDriver ──mpsc::Sender<Cmd>──────────────►  PylsptActor { db: ProjectDatabase }
   outline(files)   Cmd::Outline{files, reply}      │
   call_graphs(..)  Cmd::Graphs{entries, reply}     │  for reads: let snap = db.clone();
   files_changed    Cmd::Changed{changes, reply}    │     rayon::scope(|s| … on snap clones …)
                ◄──oneshot reply─────────────────── │  for writes: File::sync_path(&mut db, …)
```

```rust
enum Cmd {
    Changed { changes: Vec<FileChange>, reply: oneshot::Sender<Result<(), DriverError>> },
    Outline { files: Vec<RelPath>,      reply: oneshot::Sender<Vec<FileOutline>> },
    Graphs  { entries: Vec<EntryRef>, opts: CallOptions, reply: oneshot::Sender<Vec<EntryGraph>> },
    InjectIds { files: Vec<RelPath>, reply: oneshot::Sender<Vec<InjectResult>> },   // used by IdStrategy
    Shutdown,
}

fn actor_loop(mut db: ProjectDatabase, rx: std::sync::mpsc::Receiver<Cmd>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Changed { changes, reply } => { apply_changes(&mut db, &changes); let _ = reply.send(Ok(())); }
            Cmd::Outline { files, reply } => {
                let snap = db.clone();                          // cheap snapshot
                let out = files.par_iter()                      // rayon: parallel on CPU threads
                    .map_with(snap, |db, f| outline_one(db, f))
                    .collect();
                let _ = reply.send(out);
            }
            …
        }
    }
}
```

> **🦀 Rust note: why an actor instead of `Arc<RwLock<ProjectDatabase>>`?**
> pylspt's own transport plan suggests the lock (`04-build/03-transport-and-parity.md`).
> That works too. The actor is proposed (P14) because:
> - There's **no lock to forget** and no way to hold a write lock across an `.await` by accident.
> - **Ordering is explicit.** Commands run in the order sent, so "files changed, *then* outline"
>   can't be reordered.
> - salsa cancellation (a write cancelling reads) can only happen *between* commands, which we
>   control, so we never have to handle a cancelled query mid-sync.
>
> The bridge between the two worlds: the tokio side sends with a **std** channel (or
> `tokio::sync::mpsc` + `blocking_recv` on the thread) and awaits a `tokio::sync::oneshot` reply.
> `oneshot::Sender::send` is not async, so it works fine from a plain thread.

> **🦀 Rust note: `map_with(snap, …)`.**
> rayon's `map_with` gives each worker thread its own **clone** of `snap`, so each thread
> has its own `&ProjectDatabase`. That's how salsa expects parallel reads to happen. If you used
> `map(|f| outline_one(&snap, f))` instead, every thread would share one `&snap`, which only
> compiles if the db is `Sync`. **[check]** ty's `ProjectDatabase` `Send`/`Sync` bounds in M5,
> and pick whichever compiles cleanly.

## Keeping salsa warm across syncs

The actor lives as long as the project is open. So the second sync re-parses only the files
the watcher said changed (`Cmd::Changed`), and salsa re-computes only queries that depend on
them. This is the main speed win over the Python driver, which rebuilt Jedi state per request
(`call_resolver.py` creates a new `JediProjectManager` per call).

## Memory

A ty database for a big project can be hundreds of MB. One per **open** project; close the
actor when the project is closed in the UI (and on idle timeout, configurable).

## Exit criteria (M5)

- `PythonDriver` passes the driver test suite: outline of the pylspt fixture corpus matches golden JSON.
- `files_changed` + `outline` on 1 changed file in a 2k-file project takes < 50 ms (warm).
- `call_graphs` for all functions of a changed file runs in parallel (CPU > 100% in a profile).

Next: [`03-jsonrpc-driver.md`](03-jsonrpc-driver.md)
