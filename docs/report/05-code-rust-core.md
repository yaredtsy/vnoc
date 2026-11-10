# 05. Code: the Rust core (`crates/`)

The root `Cargo.toml` is a **workspace** with `members = ["crates/*"]`. It excludes `spikes/`, `drivers/` (its own workspace) and the plan folders.
The root `src/main.rs` is a leftover `Hello, world!` from `cargo new`; it does nothing.

## Crates: planned vs built

The plan (`plan_vs/06-core/01-crates-and-domain.md`) has 10 crates, each created **when its milestone starts**. Three exist:

| Crate | Job | Status |
|---|---|---|
| `vnoc-domain` | `NodeId`, `LocationKey`, `RepoPath`, `NodeToml`, `persist()`; **no I/O** | ✅ built, final MVP format, tested |
| `vnoc-driver-protocol` | Rust types for the core ↔ driver wire | ✅ handshake + `outline`; other methods are names only |
| `vnoc` | the core binary (editor-facing server) | 🟡 walking skeleton |
| `vnoc-store`, `-sqlite`, `-memory` | the cache behind a trait | ❌ M1 |
| `vnoc-driver-host` | find, copy, spawn drivers | ❌ M1 (a skeleton version lives in `vnoc/src/driver_host.rs`) |
| `vnoc-userdata` | reading/writing `.vnoc/` files | ❌ M3 |
| `vnoc-sync` | scan, diff, reconcile | ❌ M4 |
| `vnoc-protocol` | editor ↔ core messages + generated TS types | ❌ (today in `vnoc/src/protocol.rs`, TS written by hand) |
| `vnoc-app` | the use cases | ❌ M1+ |

**Boundary rules** (06-01): `vnoc-domain` depends on no V-NOC crate and does no I/O; async (tokio) only in the binary; `thiserror` in libraries, `anyhow` only in binaries;
**the core never writes a file a user may have open**; a driver never depends on `vnoc-domain`.

---

## `vnoc-domain`: the frozen formats ✅

This is the most "final" code in the repo: these formats are promised from the MVP on, because people will commit `.vnoc/`.
**The spec is in the doc comments**; start at `src/id.rs`.

| File | What it defines |
|---|---|
| `src/id.rs` | **`NodeId`**: a `u128` newtype. Made by BLAKE3 `derive_key("v-noc node id v1", path\0qualname\0occurrence)`, first 16 bytes, Crockford base32, 26 chars, first char 0–7. Shard = last 2 chars. Lenient reading (`I`/`L`→1, `O`→0). Also **`ProvisionalId`**: a hash you can compare but **cannot store or serialize** |
| `src/location.rs` | **`RepoPath`** (repo-relative, `/` only, refuses `..`, absolute, backslash; keeps Unicode bytes exactly as git records them) and **`LocationKey`** (`path::qualname#n`) |
| `src/node_toml.rs` | **`NodeToml`**: `path`, `qualname`, `id_in_code`, `status`, `archived_at`, `occurrence`. Edited with `toml_edit` so **unknown fields, comments and key order survive** |
| `src/persist.rs` | **`persist()`**: the *only* way to create a `NodeId` for a new node. It steps past taken IDs, **writes the ID into the code first** (through an `IdWriter` trait), then returns it |

> **🦀 Rust note: why types like `NodeId` and `ProvisionalId`?**
> A **newtype** (`struct NodeId(u128)`) is a wrapper the compiler treats as a different type from `u128`, so you can't pass a random number where an ID is expected.
> Making the constructor private means the only ways to *get* a `NodeId` are parsing one that already exists or calling `persist()`. So "an ID that exists in a
> `.vnoc/` folder but was never written into code" can't be expressed. `ProvisionalId` has no `Serialize` impl, so a hash that was never persisted **can't** end up in a file:
> the compiler refuses it.

**Tests:** 15 unit + 3 golden-ID + 8 `node_toml` tests, all pass. The golden IDs come from an **independent oracle** (`tests/golden/gen_golden.py`: Python's own `blake3`,
a hand-written encoder, and qualnames from Python's compiler), so a bug in our Rust encoder can't hide by agreeing with itself.

## `vnoc-driver-protocol` ✅ (small)

`PROTOCOL_VERSION = 1`, method names (`vnoc/outline`, `vnoc/planIds`, `vnoc/callTree`, `vnoc/expand`, `vnoc/environmentInputs`), and types for the handshake
and `outline` (`Position` = 0-based line + UTF-16 column, `Range`, `Def`, `DefKind`). The wire spec itself is `plan_vs/07-drivers/01-driver-protocol.md`; the types follow it.
Types for `planIds` / `callTree` / `expand` are not written yet.

## `vnoc`: the core binary 🟡

| File | What it does |
|---|---|
| `main.rs` | starts tracing (logs to **stderr**), runs the `async-lsp` main loop on stdin/stdout |
| `server.rs` | the `Router`: `initialize` (reads `driverPath`), `initialized`, `shutdown`, **`vnoc/ping`**, **`vnoc/structure`**; catch-alls so an unknown notification is logged and an unknown request gets `-32601` (async-lsp would otherwise stop) |
| `protocol.rs` | the request/response types for `ping` and `structure` |
| `driver_host.rs` | starts the driver at a given path, does the handshake (checks the protocol version and the `outline` capability), sends one request at a time. `kill_on_drop`: the driver never outlives the core. If the connection breaks, the driver is forgotten and the next request starts a fresh one |
| `scan.rs` | lists every `.py` under the opened folder, honouring `.gitignore`, skipping hidden folders (uses the `ignore` crate). Max 20,000 files |
| `structure.rs` | turns the driver's flat list of defs into a tree (folder → file → class → function), keyed by `LocationKey` so redefinitions stay unique |
| `watchdog.rs` | wraps every handler; warns if one poll holds a tokio worker > 50 ms (blocking work must go through `spawn_blocking`) |

**What `vnoc/structure` does today:** scan the folder → ask the driver for each file's outline (one file at a time) → build the tree → keep it in memory → return it with counts and timings.
A file the driver can't read is listed in `failed`; the rest still comes back.

> **🦀 Rust note: async only for I/O.**
> The core runs on tokio. An `async fn` that does a long CPU loop blocks the worker thread and every other task on it. So walking the disk and building the tree run in
> `tokio::task::spawn_blocking(...)`, which uses a separate thread pool. `watchdog::timed` catches mistakes: it measures each `poll` of a handler's future.

### Not built yet in the core

SQLite cache · real scope from `vnoc.toml` · lazy tree loading (today the whole tree goes in one answer) · parallel driver requests · driver lookup/copy/restart · reading `.vnoc/` · attach docs ·
`persist()` wired to a real writer · git (gix) · Missing / not on this branch · call trees · notifications to the editor (`graphChanged` etc.).
The reserved method list is in `plan_vs/06-core/02-editor-protocol.md` ("Reserved").

---

## Tests

```sh
cargo test --workspace --no-fail-fast     # needs drivers/python built first (the lifecycle tests use the real driver)
```

| Suite | Result |
|---|---|
| `vnoc` unit tests (scan, structure, watchdog) | ✅ 11 / 11 |
| `vnoc` `tests/lifecycle.rs` (the real binary + real driver: lifecycle, unknown messages, ping, structure) | ⚠ **3 / 4**: `the_structure_of_a_folder_comes_from_the_driver` fails |
| `vnoc-domain` (unit, golden IDs, node_toml) | ✅ 26 / 26 |

**Why the one test fails (not a code bug):** there is an **untracked, empty file `crates/vnoc-domain/tests/fixtures/te.py`** (created 2026-11-08 10:04, probably while trying the
extension on the fixtures folder). The test expects exactly 1 Python file in that folder and now finds 2 (`left: 2, right: 1`). The extension's own test (`skeleton.test.ts`) expects
`files: 1` too, so it would fail the same way. **Fix:** delete `te.py` (I didn't, because it's your file).

Next: [`06-code-python-driver.md`](06-code-python-driver.md)
