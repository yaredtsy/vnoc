# 02. VS Code extension basics (from zero)

Every VS Code idea this project uses, in the order you meet them. Each one says **where it shows up in our code**.

---

## 1. What an extension is

An extension is a folder with a **`package.json`** (a manifest) and JavaScript code. VS Code reads the manifest to learn what the extension offers, and runs the
code only when needed.

- **Ours:** `extension/package.json` and `extension/src/extension.ts` (compiled to `extension/out/extension.js`).
- The **`publisher.name`** is its ID: ours is `vnoc.vnoc` (the test looks it up with `vscode.extensions.getExtension("vnoc.vnoc")`).

## 2. The extension host

Your extension's code runs in a separate **Node.js process** called the **extension host**. All the user's extensions share it. It can read files, start programs
and call the `vscode` API, but anything slow there slows every extension. That's why V-NOC keeps it thin and puts the work in Rust.

## 3. `package.json`: `contributes`, activation, capabilities

| Field | What it means | Ours |
|---|---|---|
| `contributes.commands` | entries in the **Command Palette** (⌘⇧P / Ctrl+Shift+P) | `vnoc.openCanvas` = "V-NOC: Open canvas" |
| `contributes.configuration` | settings the user sees under Settings | `vnoc.server.path`, `vnoc.driver.path` |
| `activationEvents` | **when** VS Code starts your code. Until then it isn't loaded at all | `onWebviewPanel:vnoc.canvas` (commands activate it on their own in modern VS Code) |
| `capabilities.untrustedWorkspaces` | what works in Restricted Mode (see §8) | `"limited"` |
| `engines.vscode` | the oldest VS Code version supported | `^1.90.0` |

Later milestones add `contributes.views` (the sidebar tree), `menus` (right-click entries), `keybindings` (⌘S in the canvas).

## 4. `activate()` and `deactivate()`

When an activation event fires, VS Code calls your `activate(context)` function once. You register commands and other things there and push them into
`context.subscriptions` so they are cleaned up later. `deactivate()` runs when VS Code closes.

**Lesson from spike S1, now in our code:** `activate()` must register everything **before any `await`**. We start the Rust core in the background and every request
waits on a `ready` promise (`extension.ts: ready = startCore(ctx)`).

## 5. Running and debugging: F5 and the Extension Development Host

Press **F5** in this repo: VS Code builds everything (`.vscode/tasks.json` → "build: everything") and opens a **second window**, the **Extension Development Host**,
with our extension loaded from source. You test there. `.vscode/launch.json` has two configurations: one opens the Python fixtures, one opens no folder.

Tests do the same thing automatically with **`@vscode/test-electron`**: it downloads a VS Code, starts it with a throwaway profile, and runs Mocha tests inside it
(`extension/src/test/`).

## 6. Talking to a sidecar: `vscode-languageclient`

VS Code language servers (Python, Rust…) are separate programs that talk **JSON-RPC over stdin/stdout** with `Content-Length:` headers (the **LSP framing**).
The `vscode-languageclient` library does that connection for you. **We use it only as the pipe**: we call `client.sendRequest("vnoc/ping")` with our own method names and
use none of its language features (no document selector).

- **Rule:** the core's **stdout is the protocol only**. Logs go to stderr. A stray `println!` would corrupt the stream; Rust lints (`deny(clippy::print_stdout)`) and tests guard this.
- Logs show up in an **Output channel** (View → Output → "V-NOC").

## 7. Webviews

A **webview** is a web page (HTML/JS/CSS) inside an editor tab. It's a sandboxed iframe: **no Node, no files, no `vscode` API**. It can only:

- `acquireVsCodeApi().postMessage(msg)` → the extension host receives it in `panel.webview.onDidReceiveMessage`;
- receive messages the host sends with `panel.webview.postMessage(msg)`;
- save a little state with `vscode.setState()` / `getState()` (survives hiding and reopening the tab).

| Concept | Meaning | Ours |
|---|---|---|
| `createWebviewPanel` | opens the tab | `extension.ts` → "V-NOC canvas" |
| `asWebviewUri` | turns a file path into a URL the webview may load | loads `webview/dist/assets/index.js` |
| `localResourceRoots` | the only folders the webview may load from | `webview/dist` |
| **CSP** (Content-Security-Policy) + **nonce** | only our script may run, nothing remote | set in the HTML in `setupPanel` |
| `retainContextWhenHidden` | keep the page alive when the tab is hidden (costs memory) | `true` for now |
| `registerWebviewPanelSerializer` | lets VS Code **restore** the tab after a restart | registered in `activate()` |

**So the canvas reaches the core in two hops:** webview → (postMessage) → extension host → (JSON-RPC) → core. And back.

## 8. Workspace Trust (Restricted Mode)

When you open a folder you don't trust, VS Code runs in **Restricted Mode**. Extensions declare what they do there. A cloned repo could put a malicious
path in `.vscode/settings.json` to make us run its program, so we list our two binary paths as **`restrictedConfigurations`**: in Restricted Mode VS Code
ignores the workspace's value for them. Plan: in Restricted Mode V-NOC only indexes and browses, it doesn't edit code or run playgrounds.
`vscode.workspace.isTrusted` tells the code which mode it's in.

## 9. Editing files the right way: `WorkspaceEdit`

The user may have a file open with unsaved changes. If the core wrote the file directly, it would conflict with that buffer. So **every change to a user file goes through
VS Code**: the core returns the edits, and the extension applies them with `vscode.workspace.applyEdit(new WorkspaceEdit(...))`. That gives **undo (⌘Z)**, works on dirty
buffers, and open editors follow file moves (`WorkspaceEdit.renameFile`). Spike S3 tested this. **Not used in product code yet** (comes with M3).

## 10. Save hooks: `onWillSaveTextDocument`

An extension can add edits **inside** the user's save with `event.waitUntil(promise)`. Spike S3 learned a hard limit: VS Code gives a hook about **1.5 s**; if a hook is slow
4 times, VS Code **drops it for the rest of the session**. V-NOC's plan: compute ID edits ahead of time and only hand them over at save. **Post-MVP** (save-time IDs are P3).

## 11. Other APIs the plan uses (not built yet)

| API | What for | Milestone |
|---|---|---|
| `TreeDataProvider` / `createTreeView` | the native sidebar tree | M1 |
| `createFileSystemWatcher` | notice file changes (S1: works even for `.vnoc/` above the opened folder) | M1–M4 |
| `onWillRenameFiles` / `onDidRenameFiles` | renames done inside VS Code → update anchors exactly | M4 |
| `showTextDocument` + `Selection` | "Go to code" | M1 |
| `openTextDocument` / `onDidChangeTextDocument` | edit code inside the canvas as a view of VS Code's own document | M6 |
| keybinding with `when: activeWebviewPanelId == 'vnoc.canvas'` | ⌘S inside the canvas | M6 |
| `storageUri` | VS Code's private storage per workspace (Quick look's cache, nothing in the repo) | M1 |
| `StatusBarItem`, `showInformationMessage`, `QuickPick` | progress, "Missing" notices, setup questions | M1–M4 |

## 12. Packaging and stores

An extension ships as a **`.vsix`** file (a zip). `vsce package --target darwin-arm64` builds one per platform (we ship native binaries, so one VSIX per OS/CPU).
`vsce publish` puts it on the **VS Code Marketplace**; `ovsx publish` on **Open VSX** (what Cursor, Windsurf and VSCodium use). `--pre-release` marks a version as
opt-in. **Not built yet** (M0's CI). macOS binaries must be at least ad-hoc signed or the OS kills them (S2 finding).

Next: [`03-progress-overview.md`](03-progress-overview.md)
