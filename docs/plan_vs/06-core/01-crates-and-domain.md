# 06.01: Crates and boundaries

> **Status:** ✅ short on purpose · **Program:** Rust core · **Milestones:** M0

**Purpose:** The crate list and the rules between them. **The domain types are not described here: they live in `crates/vnoc-domain`, with the format in their doc comments** (start at `src/id.rs`).

**Builds on:** [02-07 §1 repo layout](../02-technology/07-repo-and-drivers.md) · VS-D67 (async-lsp for the core), VS-D68 (drivers choose their own transport), VS-D79 (the node ID).

## Crates

| Crate | One job | Exists |
|---|---|---|
| `vnoc-domain` | `NodeId`, `LocationKey`, `RepoPath`, `NodeToml`; no I/O | ✅ M0 |
| `vnoc-driver-protocol` | the Rust types and helpers for the wire in [07-01](../07-drivers/01-driver-protocol.md); `PROTOCOL_VERSION` | M0 |
| `vnoc-store` · `-sqlite` · `-memory` | the cache behind a trait; the SQLite adapter; a test adapter | M1 |
| `vnoc-userdata` | reading and writing `.vnoc/` files (node folders, docs, trash) | M3 |
| `vnoc-driver-host` | find, copy, spawn and talk to drivers | M1 |
| `vnoc-sync` | scan, diff, reconcile (VS-D74's rule); ladder step 1 | M4 |
| `vnoc-protocol` | editor ↔ core messages (+ generated TS types) | M0 |
| `vnoc-app` | the use cases | M1+ |
| `vnoc` | the core binary | M0 |
| `drivers/python` | `vnoc-driver-python`: a thin adapter around pylspt (its **own Cargo workspace**, own `rust-toolchain.toml` and lockfile) | M1 |

Crates are created when their milestone starts, not before.

## Boundaries (the rules that keep it simple)

```
vnoc-domain ◄── vnoc-store ◄── vnoc-store-sqlite
     ▲  ▲            ▲
     │  └── vnoc-userdata
     │            vnoc-sync ──► vnoc-driver-host ──► vnoc-driver-protocol
     └──────────── vnoc-app ──► (all of the above) ◄── vnoc (binary) ──► vnoc-protocol
```

1. **`vnoc-domain` depends on no other V-NOC crate, and does no I/O.** It may use `serde`, `blake3`, `thiserror`, `toml_edit`.
2. **Dependencies point toward the domain.** Nothing below `vnoc-app` knows about editors, transports or tokio.
3. **Async only at the edge** (`vnoc` and `vnoc-app`'s entry): `async-lsp` and tokio stay out of the library crates. The library crates are plain blocking code.
4. **Async is for I/O only** (VS-D67): CPU work (parsing, hashing) goes through `spawn_blocking` or a thread pool; **SQLite writes go to the one writer thread**; a debug check warns when a handler holds a tokio worker for more than about 50 ms.
5. **The core never writes a file a user may have open.** It returns edits for the editor to apply; it writes directly only the cache, `node.toml` files, `trashed.toml` and `.vnoc/.gitignore` (B8).
6. **A driver depends on `vnoc-driver-protocol` and never on `vnoc-domain`:** the core picks IDs, the driver picks places. (Tests may use the domain crate to check a driver against golden values.)
7. **pylspt depends on no V-NOC crate** (VS-D65). `drivers/python` is the only place that knows both.
8. **Errors:** `thiserror` enums in the library crates, `anyhow` only in binaries.

## MVP vs stable

| MVP (0.x) | Stable (1.0) |
|---|---|
| all of it (M0) | unchanged; the P-list items arrive as fields and modules, not new crates |

## Questions settled

- **tokio or plain threads for the core?** Settled by S1/C3: `async-lsp` on tokio for the core (VS-D67); drivers stay on threads (VS-D68).
