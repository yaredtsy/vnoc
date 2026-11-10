# 05.01: S1 hello core

**Goal:** a VS Code command starts a Rust binary, asks it something over stdio, and shows the answer in a webview with one xyflow node.
**Answers:** B5 (LSP framing + `vscode-languageclient`), C3 (Rust transport crate), C6 (`#:schema` in TOML), C18 (watching above the opened folder).
**Time box:** 2 days. **Code:** `spikes/s1-hello-core/`.

## Build

1. **Generate an extension.** `npx --package yo --package generator-code -- yo code` → *New Extension (TypeScript)*, bundler **esbuild**. Press **F5**: a second
   VS Code window (the *Extension Development Host*) opens with your extension loaded. This loop (edit → F5) is how all extension work goes.
2. **A Rust binary `spike-core`** using the `lsp-server` crate. It answers `initialize` and one custom request `vnoc/ping` → `{ pong: true, pid }`. Logs go to **stderr**
   with `tracing`; nothing else may touch stdout (`#![deny(clippy::print_stdout)]`).
3. **Start it from the extension** with `vscode-languageclient`: `ServerOptions = { command: <path to spike-core>, transport: TransportKind.stdio }`, then
   `await client.start()` and `client.sendRequest("vnoc/ping")`. Show the reply in a notification first.
4. **A webview panel** (`vscode.window.createWebviewPanel`) loading a **Vite + React + @xyflow/react** bundle through `webview.asWebviewUri`, with a CSP and a nonce
   (02-03 "webview rules"). The webview posts `{type: "req", method: "vnoc/ping"}`; the host relays it to the core and posts the result back. Draw it as **one node**.
5. **Save and restore state:** `vscode.setState({…})` in the webview, `registerWebviewPanelSerializer` in the host. Close and reopen VS Code: the panel comes back.
6. **Workspace Trust:** declare `capabilities.untrustedWorkspaces = { supported: "limited", restrictedConfigurations: ["vnoc.server.path"] }`. Open the folder in
   Restricted Mode and check the extension still activates and `workspace.isTrusted` is `false`.
7. **Watch above the opened folder (C18):** open `repo/sub/` as the workspace, create a `RelativePattern(<repo root>, ".vnoc/**")` watcher, touch a file in `repo/.vnoc/`.
   Does the event arrive? Does it work in Restricted Mode?
8. **`#:schema` (C6):** a `vnoc.toml` with `#:schema ./schema.json` on line 1; check autocomplete with Even Better TOML and with Tombi.
9. **C3:** write the same ping server once with `async-lsp` (or `tower-lsp-server`) and compare: lines of code, how cancellation and notifications look, compile time.

> **🧩 VS Code note: why `vscode-languageclient` even though we're not a language server.**
> It's just a very good JSON-RPC-over-stdio client: it starts the process, restarts it if it crashes, frames messages (`Content-Length:` headers), handles
> request IDs and cancellation, and shows traffic in the Output panel when `vnoc.trace.server` is on. We use its custom-request API (`sendRequest("vnoc/…")`)
> and ignore the language features.

## Measure

- Time from activation to the first `pong` (cold and warm).
- `spike-core` release binary size (stripped).
- Webview bundle size (React + xyflow).

## Pass / fail

- ✅ `pong` appears **in a canvas node** in under ~1 s after activation, and survives a VS Code restart.
- ✅ Restricted Mode: activates, no crash, the setting `vnoc.server.path` is ignored.
- ✅ C18, C6, C3 each have a one-paragraph answer below.

## Findings

*Partly done (2026-11-05). Code is built and compiles; GUI checks are still open.*

| Measure | Result |
|---|---|
| `spike-core` release binary | 1.05 MB, **0.87 MB stripped** (`lsp-server` + `tracing-subscriber`) |
| Core alone: spawn → initialize → ping → shutdown | ~3 ms |
| Webview bundle (React + xyflow) | JS 335 KB (109 KB gzip) + CSS 16 KB |
| Extension bundle (with `vscode-languageclient`) | 800 KB, unminified |
| `cargo clippy` with `deny(print_stdout)` | clean |

**Panel restore works (checked in VS Code).** Two rules we learned the hard way. They go into `08-extension/`:
1. A panel type that can be restored needs `activationEvents: ["onWebviewPanel:<viewType>"]`. Without it the extension isn't started on restore and the panel hangs on its loading screen.
2. `activate()` must register `registerWebviewPanelSerializer` **before** any `await`. Start the core in the background and have every request wait on a `ready` promise.

**C18 answered: yes.** With `repo/sub/` opened, `createFileSystemWatcher(new RelativePattern(<parent>, ".vnoc/**"))` delivered create, change and delete
for a file in `repo/.vnoc/` (Trusted mode). Not yet checked: whether events still arrive in Restricted Mode. The watcher must be created in `activate()`,
so the extension has to be active first.

**Restricted Mode:** the extension activates and the canvas opens (checked by hand).

**C3 measured:** same `vnoc/ping` server written twice (`spike-core/` and `spike-core-async/`).

| | `lsp-server` 0.10 | `async-lsp` 0.2.4 (+ tokio) |
|---|---|---|
| Code lines (no blanks or comments) | 40 | 55 |
| Release binary, stripped | 0.87 MB | 1.84 MB (2.1×) |
| Crates in the dependency tree | 41 | 103 |
| Clean release build | 17.5 s | 45.2 s |
| Style | threads + channels, `match` on method string | async, one *type* per method, tower layers |

Gotchas found while porting (each cost a failed run):
1. **Unhandled notification is fatal in `async-lsp`.** The server shut down on `initialized` until we added a handler. `lsp-server` ignores it. Set `unhandled_notification` to log-and-continue, or VS Code will kill the core on something unexpected.
2. **`shutdown` is not automatic.** `lsp-server` has `handle_shutdown`. In `async-lsp` we add `request::Shutdown` ourselves.
3. **`PipeStdout` needs a real pipe.** Redirecting stdout to a file panics. Fine under VS Code, annoying for manual tests.
4. **`async-lsp` pins its own `lsp-types` (0.95).** A separate `lsp-types` 0.97 dependency gives two unrelated `Request` traits. Use `async_lsp::lsp_types`.
5. **Closing stdin ends the server at once** (EOF = client gone), so piped test scripts must keep stdin open.

**C3 decision (yared): async. Use `async-lsp` (tokio) for the core.** The core will juggle drivers, file watching and the canvas at once, so we take the async crate
now instead of a rewrite later. Handle the five gotchas above in the core's skeleton: log-and-continue for unknown notifications, explicit `Shutdown`, use the re-exported `lsp_types`.

**C6 answered: yes, in both editors.** `#:schema ./schema.json` on line 1 of `vnoc.toml` gives completions, hover descriptions, and errors for bad values and
unknown keys, in Even Better TOML and in Tombi. The relative path is resolved from the TOML file's own folder.
- Even Better TOML logs `data did not match any variant of untagged enum SchemaCatalog` on startup (its SchemaStore catalog download fails). It doesn't affect `#:schema`.
  Silence it with `"evenBetterToml.schema.repositoryEnabled": false`.
- Tombi had no such errors, so **recommend Tombi in our docs**.
- Not tested: `#:schema https://…` (a URL instead of a local path).

Still to check by hand (see `spikes/s1-hello-core/README.md`): time to first pong inside VS Code, panel restore after restart, Restricted Mode,
C18 (watching above the folder), C6 (`#:schema`), C3 (`async-lsp` comparison).

Next: [`02-python-driver.md`](02-python-driver.md)
