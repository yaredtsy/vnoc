# 02.01: Architecture at a glance

Four kinds of program, three message channels, one folder on disk (two sides), one global drivers folder.

```
┌──────────────────────────────── VS Code / Cursor (one window) ───────────────────────────────┐
│  ┌──────────────────────────────┐        postMessage        ┌──────────────────────────────┐ │
│  │ WEBVIEW (React + xyflow)     │ ◄───────────────────────► │ EXTENSION HOST (TS)          │ │
│  │  canvas · focus stack ·      │   typed messages (JSON)   │  "the relay"                 │ │
│  │  call tree · doc preview     │                           │  commands · menus · TreeView │ │
│  │  (iframe: no Node, no fs)    │                           │  file + git(HEAD) events ·   │ │
│  └──────────────────────────────┘                           │  WorkspaceEdit · save hook · │ │
│                                                             │  trust · terminals · proxy   │ │
│                                                             └──────────────┬───────────────┘ │
└────────────────────────────────────────────────────────────────────────────┼─────────────────┘
                                                                             │ stdio, JSON-RPC 2.0 (LSP framing)
                                                                             ▼
┌────────────────────────── vnoc  (Rust core, one process per window) ──────────────────────────┐
│  vnoc (bin)            lsp | mcp | query | driver install            ← CLI/MCP use the same core │
│  vnoc-app              use cases: open, tree, call tree, attach doc, relink, set ID, share…    │
│  vnoc-sync             scan · snapshot · diff · reconcile (HEAD-aware) · call layers (plan/05) │
│  vnoc-userdata         .vnoc/ shared + local text files, placement rules (VS-D24)              │
│  vnoc-store(-sqlite)   the cache                                                               │
│  vnoc-driver-host      find · install · verify · spawn drivers                                 │
└──────────────┬──────────────────────────────────────────────────┬─────────────────────────────┘
               │ files                                            │ stdio, JSON-RPC (driver protocol, versioned)
               ▼                                                  ▼
 <git root>/.vnoc/                                   ┌────────────────────────────────────┐
   vnoc.toml  nodes/7K/<id>/   shared                │ vnoc-driver-python (pylspt)        │
   local.toml local/nodes/…  local                   │  separate executable, run from:    │
   local/cache/index.sqlite  local (machine data)    │  <data dir>/vnoc/drivers/python/   │
   .gitignore                written by V-NOC        │                      0.3.0/        │
                                                     └────────────────────────────────────┘
```

## Who does what

| Piece | Language | Knows about | Must not know about |
|---|---|---|---|
| Webview | TS/React | drawing; its own UI state (focus stack, viewport) | the file system, Rust, VS Code APIs (other than `postMessage`) |
| Extension host | TS | VS Code APIs: spawning the core, events (files, **git HEAD**, saves), `WorkspaceEdit`, Workspace Trust, proxy settings | parsing, matching, storage rules, where files go |
| Core | Rust | code graph, IDs, matching, storage, `.vnoc/` placement, drivers | VS Code (NFR-40) |
| Driver | Rust (any language later) | one language: outline, call trees, ID text edits | storage, `.vnoc/`, the editor |

> **🧩 VS Code note: the extension host.**
> Your TypeScript runs in a separate Node.js process called the **extension host**, shared with all the user's other extensions. Heavy work there slows
> *every* extension, so we keep it thin. A **webview** is a sandboxed iframe with no Node and no `vscode` API, only `postMessage`. So webview → Rust is
> always webview → host → Rust.

## The main flows

**1. Open project**
```
host: activation ─► spawn core ─► initialize{root, storageUri, trusted, gitHead, proxy}
core: read vnoc.toml + local.toml ─► find/copy driver (02-07) ─► open cache ─► scan scope ─► diff
      ─► reconcile (absent nodes → Missing / "not on this branch", VS-D74) ─► apply ─► notify graphChanged
host: refresh TreeView; open canvases re-request what they show
```

**2. File saved (auto sync)**
```
host: onDidSave / FileSystemWatcher / onDidRenameFiles ─► vnoc/filesChanged (batched)
core: scheduler coalesces ─► small sync ─► graphChanged (+ missingChanged)
(manual sync: the core only counts → host shows "12 changed · Sync")
```

**3. HEAD changes (checkout, pull, rebase…)**
```
host: vscode.git repository.state.onDidChange ─► vnoc/headChanged   (just a nudge)
core: sync ─► absent documented nodes → MISSING (prompt) or "not on this branch" (no prompt),
        decided by where each node's node.toml is, not by how HEAD moved (VS-D74, 02-05 §5)
```

**4. Add a permanent ID to one function**
```
host: context menu ─► vnoc/ids.add{node}
core: derive the ID (hash); driver places it (Python: docstring `ID:`) and returns the edit + span; userdata writes node.toml ─► reply{edits}
host: workspace.applyEdit (undoable) ─► save if the file wasn't dirty ─► the event is recognised as ours ─► no loop
```

**5. Save under an `all` rule**
```
host: onWillSaveTextDocument (reason = Manual or FocusOut; not AfterDelay) ─► e.waitUntil( vnoc/ids.forSave{uri, text} )
core → driver: parse this text only ─► TextEdit[] for clean defs ─► VS Code applies them inside the same save
```

## Why this shape

- **Crash isolation:** a panic in the core or a driver never takes VS Code down; each is restarted (NFR-14).
- **Reuse:** CLI and MCP modes run the same core and the same drivers, with no VS Code (VS-O1).
- **One brain:** matching, IDs, archive and file-placement rules live only in Rust, so the UI can't drift from them.
- **Safe edits:** the only process that touches user-visible files is the one that owns the editor buffers (VS-D16).

Next: [`02-running-rust-sidecar.md`](02-running-rust-sidecar.md)
