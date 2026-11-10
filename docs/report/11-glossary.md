# 11. Glossary

## VS Code terms

| Term | Meaning |
|---|---|
| **Extension** | a folder with `package.json` + JS code that VS Code loads. Ships as a `.vsix` |
| **Extension host** | the Node.js process where all extensions' code runs (shared, so keep it light) |
| **`package.json` / manifest** | declares commands, settings, views, menus, when to activate |
| **`contributes`** | the part of `package.json` that adds things to VS Code's UI |
| **Activation event** | when VS Code starts your code (`onCommand`, `onWebviewPanel:…`, `workspaceContains:…`) |
| **`activate()` / `deactivate()`** | your entry and exit functions |
| **Command Palette** | ⌘⇧P / Ctrl+Shift+P; lists commands like "V-NOC: Open canvas" |
| **Extension Development Host** | the second VS Code window F5 opens, with your extension loaded from source |
| **`@vscode/test-electron`** | runs tests inside a real, downloaded VS Code |
| **Output channel** | a log panel (View → Output → "V-NOC") |
| **Webview** | a sandboxed web page in a tab; no files, no Node; talks only via `postMessage` |
| **`postMessage` / `onDidReceiveMessage`** | the message pipe between webview and extension host |
| **`setState` / `getState`** | small state a webview keeps while hidden or reopened |
| **Webview panel serializer** | lets VS Code restore a webview tab after a restart |
| **CSP / nonce** | Content-Security-Policy: which scripts may run; the nonce marks ours |
| **`asWebviewUri` / `localResourceRoots`** | how the webview loads our bundled files, and from which folders only |
| **TreeView** | the native sidebar tree (planned for M1) |
| **Workspace Trust / Restricted Mode** | VS Code's "do you trust this folder?"; in Restricted Mode extensions limit themselves |
| **`restrictedConfigurations`** | settings whose workspace value VS Code ignores in Restricted Mode |
| **`WorkspaceEdit` / `applyEdit`** | the safe way to change files: undoable, works on unsaved buffers, open editors follow moves |
| **`onWillSaveTextDocument` / `waitUntil`** | add edits inside a save; ~1.5 s budget, dropped after 4 slow saves |
| **`onWillRenameFiles` / `onDidRenameFiles`** | events for renames done inside VS Code |
| **`storageUri`** | VS Code's private per-workspace storage folder |
| **`vscode-languageclient`** | a library for talking to a sidecar over JSON-RPC; we use only its pipe |
| **LSP framing** | `Content-Length: N\r\n\r\n{json}`: how JSON-RPC messages are split on stdin/stdout |
| **VSIX** | the extension package (a zip); one per platform for us |
| **Marketplace / Open VSX** | the two stores; Cursor, Windsurf, VSCodium use Open VSX |
| **Pre-release** | an opt-in channel on the stores; the MVP ships there |
| **Monaco** | the editor component inside VS Code; planned for editing in the canvas |

## V-NOC terms

| Term | Meaning |
|---|---|
| **Core** | the Rust binary `vnoc`; all the rules live here |
| **Sidecar** | a separate process next to the editor (the core) |
| **Driver** | a separate binary for one language (`vnoc-driver-python`) |
| **pylspt** | the Rust Python analyzer (sibling repo) on top of ruff + ty; the driver wraps it |
| **Walking skeleton** | the thinnest version where every part is connected end to end (what exists now) |
| **Spike** | a throwaway experiment that answers a question (S1–S5) |
| **Scope** | which files are indexed (and where IDs may go) |
| **`.vnoc/`** | V-NOC's folder at the git root: `vnoc.toml`, `nodes/`, `local/` |
| **Node** | a folder, file, class or function |
| **`NodeId`** | the permanent 26-char ID (BLAKE3 hash, Crockford base32). Not a ULID |
| **Shard** | the last 2 chars of the ID; the subfolder in `nodes/<shard>/<id>/` |
| **`LocationKey`** | where a node is now: `path::qualname#n`. Changes on rename; not an identity |
| **`ProvisionalId`** | a hash you may compare but never store |
| **`persist()`** | the only way to make a new `NodeId`; writes it into code first |
| **`node.toml`** | per node: its anchor (path + qualname) and status |
| **Anchor** | the path + qualname saved in `node.toml`, used to find the node on a fresh clone |
| **`ID:` line** | the ID written in a function's docstring (MVP: on attach) |
| **Matching ladder** | steps to find a lost node again (1 same path+name, 2 file hash, 3 git rename, 4 body hash, 5 similarity score, 6 no match → Missing). MVP uses step 1 only |
| **Step 0** | "observed moves": renames seen live in the editor. MVP: only VS Code rename events |
| **Missing** | a documented node that's gone in this commit → the user is asked |
| **Not on this branch** | a node whose docs belong to another branch → no prompt (VS-D74) |
| **Archive / Relink / Remove** | soft-delete that waits · attach to another node · move docs to the V-NOC trash |
| **V-NOC trash** | `.vnoc/local/trash/`; never emptied without a click |
| **Silent match / ask / provisional** | found with certainty · ask the user · "unconfirmed link" (built, off) |
| **Twin** | two functions with identical bodies; always asks |
| **Focus stack** | canvas navigation: focus a node, Back, Clear, breadcrumbs |
| **Node card** | the canvas card, ported from old V-NOC (expand, code, zoom) |
| **Doc panel** | the side panel in the canvas for a node's docs |
| **Quick look** | index a folder with nothing written into the repo |
| **VS-D / VS-P / VS-O** | decided / proposed / open rows in the decision log |
| **C1…C23** | facts to check (checklist part C), answered by the spikes |
| **M0…M10** | milestones; M0–M6 = MVP |
| **P1…P4** | the post-MVP list from the draft MVP cut |
| **`plan/` vs `plan_vs/`** | old server plan vs the current VS Code extension plan |
