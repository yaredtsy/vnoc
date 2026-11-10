# 00.02 — Decision log

Each decision has a **status**:

- ✅ **decided**: you chose it (2026-11-04).
- 🟡 **proposed — confirm**: my default. The plan is written assuming it, but it's cheap to change *now* and expensive later. Please say yes/no.
- 🔓 **open**: deliberately left undecided, with the hook that keeps it open.

When a decision changes, don't delete the row. Add a new row that says "supersedes Dn",
so the history stays readable (same idea as an ADR, an Architecture Decision Record).

---

## Decided

| # | Decision | Why |
|---|---|---|
| D1 ✅ | **New v2 schema**, no compatibility with Python DBs. | Lets us fix the model (Root, parent-on-child, deterministic call IDs). Old projects get re-imported. |
| D2 ✅ | **`LanguageDriver` trait**: pylspt in-process, TS/JS out-of-process JSON-RPC. | In-process = no serialisation, salsa cache shared across syncs. Out-of-process stays possible for languages not written in Rust. |
| D3 ✅ | **gRPC / Connect** for the frontend. | Typed contracts generated for both Rust and TS; server-streaming for progress. |
| D4 ✅ | **Two-layer call tree**. | See L4 in [`01-lessons-from-python.md`](01-lessons-from-python.md). |
| D5 ✅ | **In-source IDs** (Python approach) **behind `IdStrategy`**. | They survive moves done outside the IDE. The trait keeps us from being locked in. |
| D6 ✅ | **Deferred deletes + tombstones + GC.** | Fixes cross-batch and cross-sync moves (L3). |
| D7 ✅ | Only external service: **language drivers**. | The agent, logs, tests etc. are deferred. |
| D8 ✅ | **DB behind traits.** TerminusDB first. | L6. |
| D9 ✅ | **Real `Root` node.** | L2. |

---

## Proposed — please confirm

| # | Proposal | Why | Alternative |
|---|---|---|---|
| P1 🟡 | Async runtime: **tokio**. | tonic, reqwest and axum all use it; it's the de-facto standard. | async-std (fading), smol. |
| P2 🟡 | gRPC server: **`tonic` + `tonic-web`** (gRPC-Web). Frontend: **`@connectrpc/connect-web`** with its **gRPC-Web transport**. | tonic is the mature Rust gRPC stack. Browsers can't speak raw gRPC; gRPC-Web is what tonic-web serves and what connect-web can talk to. | A native *Connect-protocol* Rust server crate (lets you `curl` JSON). **[check]** maturity before choosing. |
| P3 🟡 | TS/JS driver transport: **keep JSON-RPC over HTTP** (what the Bun driver already speaks). | No rewrite of the TS driver. The `LanguageDriver` trait hides it anyway. | Move drivers to gRPC too (one protocol for everything). |
| P4 🟡 | TerminusDB client: **our own thin client on `reqwest`** (document API + WOQL JSON). | **[check]** There is no maintained official Rust client. We only need ~10 endpoints. | Generate one from the OpenAPI spec, if a good one exists. |
| P5 🟡 | Content hash: **xxh3-128** (`xxhash-rust`). | pylspt already uses it. Very fast. 128 bits makes accidental collisions practically impossible. | blake3 (cryptographic, a bit slower). |
| P6 🟡 | Project registry (the list of projects): its own **`registry` database**, behind a `ProjectRegistry` trait. | Same as Python (ProjectSchema in a main DB), but behind a trait so a local file/SQLite can replace it. | A `~/.vnoc/projects.toml` file. |
| P7 🟡 | Paths stored **relative to the project root, `/`-separated**, as derived data (not identity). | Projects can move on disk; Windows/macOS paths compare the same. | Absolute paths (Python did a mix). |
| P8 🟡 | **One `Group` type** with a `family` field (`Structure` / `Code` / `Call`) instead of three schemas. | One repo, one set of rules. The family limits what it can hold. | Keep three types. |
| P9 🟡 | **Parent link lives on the child** (`parent` + `owner`), with **no child sets on parents**. | A move is one field write. No read-modify-write. Groups become transparent (see 01-data-model/03). | Child sets on parents (Python). |
| P10 🟡 | **Call node ID = hash(parent node ID, target definition ID).** Siblings with the same target merge, keeping `call_count` (like Python). | The same call in the same place always gets the same ID, so docs/groups on calls survive re-analysis. | Include the call-site span: more precise, but IDs change whenever lines shift. |
| P11 🟡 | Tombstone GC: collect when **older than 7 days AND seen by ≥ 20 later syncs**. Configurable per project. | Gives cut/paste across sessions time to reconnect. | Time only, or count only. |
| P12 🟡 | Atomic sync: **write the sync on a temporary branch, then squash + fast-forward `main`**. Small syncs: a single WOQL transaction directly on `main`. | Readers never see half a sync. A failed sync is deleted with its branch. **[check]** exact endpoints in 03-storage/05. | Several commits straight on main (Python). |
| P13 🟡 | File source text stored in a separate `SourceText` document, **on by default**, can be turned off per project. | Needed later for remote/versioned projects; kept off the tree so tree loads stay small. | Never store code in the DB. |
| P14 🟡 | pylspt's database is owned by **one actor thread per project**; work is fanned out to cheap salsa snapshots. | salsa writes need `&mut db`; an actor serialises writes without locks. | `Arc<RwLock<ProjectDatabase>>` (pylspt's own transport plan). |
| P15 🟡 | Errors: **`thiserror`** enums in library crates, **`anyhow`** only in the binary. Logging: **`tracing`**. | Standard Rust practice; explained in 02-workspace-and-crates/03. | `snafu`, `log`. |
| P16 🟡 | Config: **TOML file + env overrides** via `figment` (or plain `serde` + `toml`). | Simple, typed. | `config` crate. |
| P17 🟡 | **Grouped item moved on disk:** if its group belonged to the *old* place, it leaves the group; if the group is a cross-cutting "virtual folder", it stays. (01-data-model/03, 05-orchestrator/08) | A group inside `src/` is about organising `src/`. A virtual folder shouldn't break when files move. | Always leave the group, or always stay. |
| P18 🟡 | **`FolderID` via `__init__.py` only in Python package folders** (folders that already have `.py` files). Other folders and non-code files use **fallback matching** (path, then content hash / child set). (04-drivers-and-ids/04) | Python created an `__init__.py` in *every* folder, even in non-Python projects. | Keep Python's behaviour exactly. |
| P19 🟡 | `Code` and `Call` groups are always **local** (same owner as their children). Only `Structure` groups can be virtual folders. (05-orchestrator/08) | A group of methods from different classes doesn't have an obvious meaning in the tree. | Allow virtual groups for every family. |
| P20 🟡 | **Per-entry call-tree budget: 5,000 materialised nodes** (marked `Truncated` beyond). (05-orchestrator/07) | Expanded trees can explode; storage and sync time grow with them. | pylspt's default 100k. |
| P21 🟡 | **GC never collects tombstones from a "mass tombstone" sync** (> 25% and > 200 nodes) without user confirmation. (05-orchestrator/06) | A long-lived git branch must not silently erase user data. | Treat them like any other tombstone. |
| P22 🟡 | **Safety-net full sync every 30 min** while a project is open; every 10th full sync ignores the mtime cache and re-hashes. (05-orchestrator/02, 09) | Catches missed watcher events and mtime-preserving tools. | Only on open / manual. |
| P23 🟡 | Timestamps with **`jiff`**. | Cleaner API, correct time zones. | `chrono`. |
| P24 🟡 | TS types for the driver JSON-RPC protocol **generated from the Rust types** (`ts-rs` or `specta`). (04-drivers-and-ids/03) | One source of truth for both ends. | Hand-written JSON Schema. |
| P25 🟡 | The scanner **does not follow symlinks**. | Avoids cycles and indexing files outside the project. | Follow them, with cycle detection. |
| P26 🟡 | At GC time, documents of collected nodes are **re-targeted to `Root`** with `orphaned_from` info. (01-data-model/04) | We never lose user writing. | Delete them with the node. |

---

## Open (with the hook that keeps them open)

| # | Open question | The hook |
|---|---|---|
| O1 🔓 | Other ID strategies (sidecar manifest, fingerprint matching, "no-ID" mode). | `IdStrategy` trait, `ProjectSettings.id_strategy` enum. 04-drivers-and-ids/04. |
| O2 🔓 | A second database (Postgres, SurrealDB, Neo4j, SQLite for local mode). | `GraphStore` + repository traits; the memory store proves the boundary. 03-storage/01. |
| O3 🔓 | Lazy call-tree materialisation (only build branches the UI opens). | Layer 2 is derived from layer 1, so it can be built eagerly or lazily without touching sync. 05-orchestrator/07. |
| O4 🔓 | Migration tool from v1 (Python) DBs. | Import goes through the same `WritePlan` as a sync. 07-roadmap/03. |
| O5 🔓 | Logs / tests / playgrounds / conversations / tasks / agent. | Everything attaches to nodes by `NodeId`, never by path. 07-roadmap/03. |

Next: [`03-architecture-overview.md`](03-architecture-overview.md)
