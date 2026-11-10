# 01.02: Relation to the old plan (`plan/`)

`plan/` was for a long-running server: TerminusDB, gRPC, a web frontend. Most of the **Rust core**
(scan, diff, reconcile, the pylspt driver, the two-layer call tree) doesn't care where it runs, so it carries over.
What changes is **everything at the edges**: storage, the API, the watcher, packaging and the identity UX.

Legend: **✅ reuse** (read it as-is) · **✏️ change** (still the base, with the differences listed) · **❌ drop** (doesn't apply).

---

## File by file

### `plan/00-foundations/`
| File | | Notes |
|---|---|---|
| 01-lessons-from-python.md | ✅ | Every lesson still applies (groups, root node, cross-batch moves, call-tree churn). The "too many commits" lesson becomes "one SQLite transaction per sync". |
| 02-decision-log.md | ✏️ | Superseded where [`01-decision-log.md`](01-decision-log.md) says so. Still valid: P5 (xxh3), P7 (relative `/` paths), P9 (parent on child), P10 (call IDs), P14 (pylspt actor, now *inside* the Python driver process), P15 (thiserror/anyhow/tracing), P20 (tree budget), P25 (no symlinks). |
| 03-architecture-overview.md | ✏️ | The layering rule ("arrows point down", dependency inversion) is the same. The top boxes change: `vnoc-server`/`vnoc-api`/tonic → `vnoc-sidecar` (stdio). See `02-technology/01`. |

### `plan/01-data-model/`
| File | | Notes |
|---|---|---|
| 00-overview.md | ✅ | |
| 01-node-kinds.md | ✏️ | Root/Folder/File/Class/Function/Call are the same. **Group: deferred** (VS-P15). **SourceText: dropped** (VS-D1). **Document** moves out of the DB into `.vnoc/nodes/` files. New: **Anchor** (VS-P4). |
| 02-identity.md | ✏️ | Call-ID hashing and the duplicate-ID policy stay. Changed: permanent IDs are **derived by hash** at creation (VS-D50; random UUIDs before), kept in the docstring `ID:` line (the driver's job, VS-D54) and/or a `node.toml` folder name; every node also has a **location key** `path::qualname` (SQLite only); **folders never get in-code IDs** (drops plan/P18's FolderID). See `02-technology/05` §1–2. |
| 03-edges-parent-and-owner.md | ✅ (later) | Only needed once groups come back. Keep `parent`+`owner` in the schema so they can. |
| 04-lifecycle-and-tombstones.md | ✏️ | Tombstones + GC become **Missing → Archived/Removed**, user-driven, only for nodes with a permanent ID. Other nodes are simply dropped (they're cache). Archive expiry after N days is new (VS-D45). The "documents are never collected" rule is kept. |
| 05-rust-domain-types.md | ✅ | Plus `Anchor`, `AnchorState`, `IdMode`, `InjectPolicy`. |

### `plan/02-workspace-and-crates/`
| File | | Notes |
|---|---|---|
| 01-cargo-workspace.md | ✏️ | Replaced by the monorepo layout in [`../02-technology/07-repo-and-drivers.md`](../02-technology/07-repo-and-drivers.md) (VS-D27). Drop `vnoc-store-terminus`, `vnoc-proto`, `vnoc-api`, `vnoc-server`. Add `vnoc-store-sqlite`, `vnoc-userdata`, `vnoc-protocol`, `vnoc-driver-protocol`, `vnoc-driver-host`, the `vnoc` binary, and `drivers/python`. The `pylspt` cargo-feature trick is replaced by `default-members` (pylspt is only a dependency of the driver). |
| 02-third-party-crates.md | ✏️ | Drop tonic/prost/reqwest. Add rusqlite, an LSP/JSON-RPC crate (`lsp-server` or `async-lsp`/`tower-lsp-server`, **[check]** in phase 2), `toml`, `ts-rs`/`specta`. Keep tokio, `ignore`, `xxhash-rust`, `tracing`. `notify` only for a future CLI mode. |
| 03-errors-logging-config.md | ✏️ | Errors: same. Logging: `tracing` to **stderr**, which the extension shows in an Output channel (stdout is the protocol!). Config: `vnoc.toml`. |

### `plan/03-storage/`
| File | | Notes |
|---|---|---|
| 00-overview.md, 01-store-traits.md, 02-repositories.md | ✅ | The trait boundary is exactly what makes SQLite a swap. |
| 03-write-plan-and-unit-of-work.md | ✅ | "One sync = one `WritePlan` = one atomic apply" → one SQLite transaction. Easier than Terminus branches. |
| 04-terminus-adapter.md, 05-terminus-best-practices.md | ❌ | Replaced by a SQLite adapter doc in phase 2. |
| 06-memory-store-and-tests.md | ✅ | The contract test suite runs against memory + SQLite. |

### `plan/04-drivers-and-ids/`
| File | | Notes |
|---|---|---|
| 01-language-driver-trait.md | ✅ | |
| 02-pylspt-in-process.md | ✏️ | **No longer in-process** (VS-D28 supersedes plan/D2). The actor design moves unchanged into `vnoc-driver-python`, the driver's own `main`. Lazy call trees use pylspt's `render_depth` limit (`pylspt/src/interp/mod.rs`). **[check]** whether solving is also bounded, or only rendering. |
| 03-jsonrpc-driver.md | ✏️ | Becomes **the** driver protocol for every language, Python included, over **stdio** instead of HTTP (supersedes plan/P3). Plus a protocol-version handshake (VS-D29.1). |
| 04-id-strategy.md | ✏️ | The trait stays. Changes: modes `on-attach` / `all` / `none` + `[[path]]` rules + per-node `id_in_code` + scope rule; removal by exact recorded span only; `persist` returns **text edits** for the extension to apply with `WorkspaceEdit` (VS-D16), never writes files; new functions get IDs inside the user's save (`onWillSaveTextDocument`); the watcher-expectation trick becomes "recognise the events caused by our own edits". |

### `plan/05-orchestrator/`
| File | | Notes |
|---|---|---|
| 00-overview.md, 02-scan.md, 03-snapshot.md, 04-structure-diff.md, 05-code-element-diff.md | ✅ | The pipeline is the same. |
| 01-project-lifecycle.md | ✏️ | No registry DB. A project = a folder with `.vnoc/`, or a quick look. |
| 06-moves-and-tombstones.md | ✏️ | Reconcile is the same; what happens to leftovers differs: dropped (no user data), **not on this branch**, or **Missing**: see VS-D74. Plus a new signal: VS Code rename events. plan/P21's "mass tombstone" guard is replaced by VS-D74. |
| 07-call-graph-sync.md | ✏️ | Layer 1 / layer 2 stay. Layer 2 is materialised **only for opened branches** (lazy). Invalidation by `reaches` stays. |
| 08-groups.md | ❌ (later) | VS-P15. |
| 09-scheduler-and-watcher.md | ✏️ | The scheduler actor (debounce, coalesce, one queue) stays, plus a **manual mode** that only counts changes (VS-D22). The watcher is replaced by file + git-HEAD events from the extension (VS-P9). |
| 10-progress-and-events.md | ✏️ | Same events, sent as JSON-RPC notifications (`$/progress`, `vnoc/graphChanged`). |

### `plan/06-api/`
| File | | Notes |
|---|---|---|
| 01-grpc-connect.md, 02-proto-layout.md | ❌ | Replaced by the JSON-RPC protocol (phase 2). |
| 03-application-services.md | ✅ | The use-case layer sits between the protocol and the store, same as before. |

### `plan/07-roadmap/`
| File | | Notes |
|---|---|---|
| 01-milestones.md | ❌ | Replaced by [`../04-roadmap/02-milestones.md`](../04-roadmap/02-milestones.md). Several milestones carry over (M1, M4–M8). |
| 02-testing-strategy.md | ✅ | Plus extension tests (`@vscode/test-electron`) in phase 2. |
| 03-deferred.md | ✏️ | Merged into VS-O*. |

Next: [`../02-technology/01-architecture-at-a-glance.md`](../02-technology/01-architecture-at-a-glance.md)
