# 07. Code: the extension (`extension/`)

The TypeScript part that VS Code loads. It's **"the relay"**: it starts the core, relays the canvas's requests to it, and (later) applies edits and listens to VS Code events.
It must stay thin: no parsing, no rules about where files go. If a VS Code term is new to you, see [`02-vscode-extension-basics.md`](02-vscode-extension-basics.md).

## Files

| File | What it is |
|---|---|
| `package.json` | the manifest: one command (`vnoc.openCanvas`), two settings (`vnoc.server.path`, `vnoc.driver.path`), activation on `onWebviewPanel:vnoc.canvas`, Restricted Mode = `limited` |
| `src/extension.ts` | everything (138 lines) |
| `src/test/runTest.ts` | starts a real VS Code with `@vscode/test-electron` and a throwaway profile |
| `src/test/suite/skeleton.test.ts` | 3 tests: the core answers ping; the core reads the fixture folder through the driver; "Open canvas" opens a tab and the webview reports what it drew |
| `out/` | compiled JS (`npm run build` = `tsc`) |

## What `extension.ts` does, step by step

1. **`activate()`** creates the "V-NOC" Output channel and starts the core **in the background** (`ready = startCore(ctx)`). It doesn't `await` it, so the commands and the
   panel serializer are registered right away (S1 lesson: a restored tab waits on the serializer).
2. **`startCore()`** finds the two binaries: the setting if set, else the repo's build folders (`target/debug/vnoc`, `drivers/python/target/debug/vnoc-driver-python`).
   It starts the core with `vscode-languageclient` (`TransportKind.stdio`, no document selector = not a real language server) and sends the **driver path** in `initializationOptions`.
   **The extension never starts the driver itself**; the core does.
3. **`request()`** waits for `ready`, then `client.sendRequest("vnoc/…")`.
4. **"V-NOC: Open canvas"** → `createWebviewPanel` → `setupPanel()`:
   - writes the HTML with a strict **CSP** and a random **nonce** (only our bundled script runs, nothing remote);
   - loads `webview/dist/assets/index.js` and `index.css` through `asWebviewUri`;
   - listens for messages. `{type:"req", method:"structure"}` → asks the core for `vnoc/structure` with the **first workspace folder** → posts `{type:"res", id, result}` back.
     Only `structure` and `ping` are allowed (an allow-list). `{type:"rendered"}` is recorded for the tests.
5. **Restoring after a restart:** `registerWebviewPanelSerializer` re-runs `setupPanel` on the restored tab.
6. **Returns a small API** (`ping`, `structure`, `waitRendered`) that only the tests use.

> **🧩 VS Code: why paths are "ignored in Restricted Mode".** `restrictedConfigurations` in `package.json` tells VS Code that in an untrusted folder it must ignore
> the *workspace's* value of `vnoc.server.path` / `vnoc.driver.path`. Otherwise a cloned repo's `.vscode/settings.json` could make V-NOC start any program.

## Built vs left

| Built ✅ | Left ❌ (milestone) |
|---|---|
| start the core, Output channel, error message if the core binary is missing | find the binaries **inside the VSIX** (`bin/`), restart the core if it crashes (M0/M1) |
| one command, canvas tab, CSP, message relay with an allow-list | sidebar **TreeView**, context menus, Quick look, Set up here, Go to code, Reveal in V-NOC (M1–M2) |
| panel restore after restart | status bar (indexing, "12 changed · Sync"), notifications (Missing prompt) (M1–M4) |
| Restricted-Mode-safe binary paths | read-only behaviour in Restricted Mode, file watchers, `onWillRenameFiles`/`onDidRenameFiles` (M1–M4) |
| one end-to-end test in a real VS Code | applying `WorkspaceEdit`s from the core (the ID on attach, trash moves) (M3–M4) |
| | document views for canvas editing, ⌘S keybinding (M6, after S5) |
| | packaging as a VSIX (`vsce package --target`), CI (M0) |

**Known shortcuts in the skeleton:** uses only the first workspace folder; `retainContextWhenHidden: true` (keeps the canvas alive in memory when hidden; the plan wants state
restore instead); the webview's dist folder is found relative to the repo (`../webview/dist`), which won't work inside a packaged VSIX.

Design docs: `plan_vs/08-extension/01…03` (all three are outlines; 08-01 should be written next, S1 is done).

Next: [`08-code-webview.md`](08-code-webview.md)
