# 02.02: Running the Rust code: the sidecar (VS-D10)

## The idea in one paragraph

We compile the Rust core into a normal executable called `vnoc`. The extension ships the right
build for the user's OS inside its package. When V-NOC is needed, the extension starts it as a
**child process** and talks to it through its **stdin and stdout**: the extension writes a JSON request
to the process's stdin, and Rust writes the JSON reply to its stdout. Logs go to **stderr**. This is
exactly how VS Code talks to rust-analyzer, ruff, pyright and most language servers.

```
extension host                                   vnoc (child process)
     │  stdin  ──►  Content-Length: 87\r\n\r\n{"jsonrpc":"2.0","id":4,"method":"vnoc/callTree", ...}
     │  stdout ◄──  Content-Length: 512\r\n\r\n{"jsonrpc":"2.0","id":4,"result":{...}}
     │  stdout ◄──  Content-Length: 70\r\n\r\n{"jsonrpc":"2.0","method":"vnoc/graphChanged","params":{...}}
     │  stderr ◄──  2026-11-05T10:00:01Z INFO sync: 3 files changed     → shown in the "V-NOC" Output panel
```

Three kinds of message (that's all JSON-RPC 2.0 is):

| Kind | Has `id`? | Gets a reply? | Example |
|---|---|---|---|
| request | yes | yes, a `result` or an `error` with the same `id` | `vnoc/children {node}` → list of children |
| notification | no | no | `vnoc/filesChanged`, `vnoc/graphChanged`, `$/progress` |
| cancel | (a notification) | no | `$/cancelRequest {id: 4}`, sent when the user collapses a tree before it loads |

The `Content-Length` header is **LSP framing**: it tells the reader how many bytes the JSON is, so
messages can contain newlines and can be read reliably from a byte stream.

## Proposed: speak LSP's framing through `vscode-languageclient` (VS-P7) 🟡

`vscode-languageclient` is Microsoft's npm package for launching language servers. We'd use it
even though we're "not really a language server", because it gives us for free:

- spawning the binary, **auto-restart on crash** (with a limit), clean shutdown;
- request/notification typing, cancellation tokens, the `$/progress` protocol;
- a "trace" setting that logs every message to the Output panel (`vnoc.trace.server`), which is great for debugging.

Our methods are custom (`vnoc/...`) and sent with `client.sendRequest("vnoc/children", params)`.
Later, the same server can also answer **real LSP requests** (`textDocument/codeLens`, `textDocument/hover`)
for FR-47/FR-48, with no extra plumbing. Other editors (VS-O4) can launch the same binary.

The alternative is the lower-level `vscode-jsonrpc` package: same wire format, but we'd write the
process management ourselves. **Decide in phase 2** after a 1-day spike.

## The Rust side

```
main()
 ├─ parse args:  vnoc lsp            ← what the extension runs   (later: vnoc mcp, vnoc index)
 ├─ init tracing → stderr
 ├─ connection = stdio transport (reader thread + writer thread)
 └─ loop over incoming messages:
      request      → spawn a task; the reply is sent when it's done (many in flight at once)
      notification → route (filesChanged → scheduler channel)
      cancel       → trip that task's CancellationToken
```

Crate choice for the transport is a phase-2 spike **[check]**: `lsp-server` (used by rust-analyzer; sync,
crossbeam channels, minimal) vs `async-lsp` / `tower-lsp-server` (async, tokio, more batteries).

> **🦀 Rust note: why stdout must stay clean.**
> In a sidecar, **stdout is the protocol**. A single stray `println!("debug")` corrupts the
> stream: the extension reads `debug` where it expected `Content-Length:` and drops the connection.
> So:
> - log with `tracing` and configure its writer to `std::io::stderr`;
> - `#![deny(clippy::print_stdout)]` at the crate root makes `println!` a compile error;
> - the same holds **inside each driver** (the core reads the driver's stdout as protocol too): pylspt or anything it vendors must not print. **[check]** in the spike.
>
> ```rust
> tracing_subscriber::fmt()
>     .with_writer(std::io::stderr)   // never stdout
>     .with_ansi(false)                // the Output panel doesn't render colours
>     .init();
> ```

> **🦀 Rust note: one typed source for the protocol (NFR-42).**
> Every request is a pair of Rust structs with `serde` derives, and `ts-rs` (or `specta`) generates the
> matching TypeScript types at build time:
>
> ```rust
> #[derive(Serialize, Deserialize, TS)]
> #[ts(export)]                           // writes bindings/ChildrenParams.ts
> #[serde(rename_all = "camelCase")]     // Rust snake_case ↔ JS camelCase
> pub struct ChildrenParams { pub node: NodeId, pub include_missing: bool }
> ```
>
> The extension and the webview import those generated `.ts` files, so if a field is renamed in Rust,
> the TypeScript build fails instead of the UI silently breaking at runtime.

## Lifecycle

| When | What |
|---|---|
| Extension activates (folder has `.vnoc/vnoc.toml`, or a V-NOC command is run) | spawn `vnoc lsp`, send `initialize` with: workspace folders, `storageUri` (quick look), `isTrusted`, the current **git HEAD** per repo (from `vscode.git`), the **proxy** settings, and the path of the bundled driver files |
| Workspace becomes trusted | `vnoc/trustChanged` → source edits and playgrounds are enabled |
| HEAD moves | `vnoc/headChanged{repo, commit}` (VS-D15) |
| VS Code window closes | `shutdown` + `exit`; the core finishes the current SQLite transaction, stops its drivers and exits |
| Core crashes | auto-restart (max 5 in 3 min), status bar "V-NOC restarting"; the cache survives, so restart is fast |
| Driver crashes | the **core** restarts it (the extension doesn't even see it); files in flight are retried |
| Extension update | new binary on the next window reload; a cache with an old `schema_version` is rebuilt (NFR-13) |

The **core talks to drivers the same way** the extension talks to the core: a child process, stdio, JSON-RPC with `Content-Length` framing,
with a protocol-version handshake. The same stdout rule applies inside drivers. Details: [`07-repo-and-drivers.md`](07-repo-and-drivers.md).

## Where the binary comes from

The VSIX for `darwin-arm64` contains `bin/vnoc` built for `aarch64-apple-darwin` (plus, in v1, the bundled Python driver, VS-D30), and so on.
The extension finds it with `vscode.Uri.joinPath(context.extensionUri, "bin", "vnoc")` (plus `.exe` on Windows). Building and publishing
one VSIX per platform is in [`06-distribution-and-integrations.md`](06-distribution-and-integrations.md).
For development, `vnoc.server.path` points at `target/debug/vnoc`. It's listed in `restrictedConfigurations`, so an untrusted repo can't set it (VS-D18).

Next: [`03-frontend-webview.md`](03-frontend-webview.md)
