# 06.02: The editor ↔ core protocol

> **Status:** ✅ short, written with the walking skeleton: **only what exists is specified**; the rest is reserved · **Program:** Rust core (+ extension, webview) · **Milestones:** M0–M6

**Purpose:** The messages between the extension and the core. Code: `crates/vnoc/src/protocol.rs` (types) and `server.rs` (handlers).

**Builds on:** [02-02 running the sidecar](../02-technology/02-running-rust-sidecar.md) · [02-03 message bridge](../02-technology/03-frontend-webview.md) · VS-D67 (the core is `async-lsp` on tokio).

## The wire

- **LSP-framed JSON-RPC on the core's stdin / stdout** (`Content-Length` headers). The extension uses `vscode-languageclient` only as the transport and calls `sendRequest("vnoc/…")`; none of the language features.
- **stdout is the protocol and nothing else.** Logs go to stderr (`VNOC_LOG=debug`). The core's tests fail a run on any byte outside a frame.
- **Positions** are the same as on the driver wire ([07-01](../07-drivers/01-driver-protocol.md)): 0-based line, UTF-16 `character`.
- **The extension never starts a driver.** It tells the core where the driver is, once, in `initialize`.

## Lifecycle (VS-D67)

`initialize` → `initialized` → requests → `shutdown` → `exit`.

- `initialize` carries **`initializationOptions: { driverPath }`**. The extension resolves the path (a setting, or the build folder in development) and, in **Restricted Mode, ignores workspace settings** for it (`restrictedConfigurations`), so a cloned repo cannot choose which program runs. `VNOC_DRIVER` is a fallback for terminals and tests.
- **An unknown notification is logged and ignored; an unknown request answers `-32601`.** The core never exits over a message it does not understand (async-lsp's default would stop its loop). Tested with the real binary.
- A handler that holds a tokio worker for more than ~50 ms is logged (`watchdog::timed`); blocking work goes through `spawn_blocking`.
- **A request that takes no parameters must accept all three ways a client says so**: absent, `null` and `[]` (`vscode-languageclient` sends `[]`). Found by the first real client.

## Requests that exist

| Method | Params | Result |
|---|---|---|
| `vnoc/ping` | none | `{ pong, pid, version }` |
| `vnoc/structure` | `{ root }` (absolute path of the opened folder) | `{ root: Node, stats, truncated, failed }`: the folders, files, classes and functions under `root`, read through the driver's `vnoc/outline` |

`Node` is `{ id, kind: "folder" \| "file" \| "class" \| "function", name, path, qualname, range, syntax_error, children }`. A def's `id` is its **location key** (`path::qualname#n`, from `vnoc-domain`); it is not a `NodeId`. `stats.folders` counts folders under the root. A file the driver cannot read is listed in `failed` and the rest of the tree is still returned. Files in scope: every `.py` under the folder, honouring `.gitignore`, hidden folders skipped (the real scope rule, `vnoc.toml` `include` / `exclude`, comes with M1).

**The webview never talks to the core.** It posts `{ type: "req", id, method }` to the extension host; the host relays to the core and posts `{ type: "res", id, result | error }` back. The webview also posts `{ type: "rendered", visible, hidden }` once it has drawn, which the extension's test suite uses to prove the whole chain ran.

## Reserved (from the outline; not built)

`tree.children`, `node.get`, `node.code.range`, `callTree`, `callTree.expand`, `docs.list / add / remove / move`, `node.archive / relink / remove`, `sync.now`, `keys.translate`, `settings.effective`; notifications `graphChanged`, `missingChanged`, `syncCount`, `progress`, `configError`; later `ids.*`, `playground.*`. **Results that change files are edits** (`TextEdit[]` / file operations) the extension applies through `WorkspaceEdit`; never a path the core already wrote (VS-P24).

## Versioning

Free to change until 1.0 (only one machine depends on it), then additive only.

## Questions still open

- `ts-rs` or `specta` for generating the TypeScript types (today `webview/src/types.ts` is written by hand and says so).
- One `graphChanged` with a diff, or invalidate-and-refetch?
- Does the whole structure travel in one answer (fine for the skeleton), or does the tree load lazily (M1's sidebar)?
