# 01. The big picture

## What V-NOC does for a user

1. You open a Python project in VS Code (or Cursor, Windsurf, VSCodium: they are all built on VS Code).
2. V-NOC reads the code and builds a tree: **folders → files → classes → functions**.
3. You open the **canvas**, a graph view in an editor tab. Each node is a card. You can expand it, read its code, and open its **call tree** (what this function calls,
   level by level).
4. You **attach docs** (Markdown files) to any node. They are saved in `.vnoc/nodes/<shard>/<id>/` and you commit them, so your team sees them too.
5. You keep coding: you rename, move and edit. V-NOC **finds the node again** and your docs stay attached. If it can't find it, it says so ("Missing") and asks you.
   It never guesses silently.

The hard part of the whole project is step 5. Most of the planning and the spikes are about it.

## History in one paragraph

V-NOC started as a Python backend with a web app (`~/Documents/Programing/ide/v-noc`). On 2026-11-04 the plan was to rewrite the backend in Rust as a server
(TerminusDB + gRPC); that plan is `plan/`. On **2026-11-05** the product **pivoted to a VS Code extension**; that plan is `plan_vs/`. `plan/` is still used where
the Rust logic is the same (scanning, diffing, call-tree layers), but storage, API, packaging and identity are now defined in `plan_vs/`.

## The four programs

V-NOC is four separate programs that pass messages to each other:

```
┌──────────────────────────── VS Code window ─────────────────────────────┐
│                                                                          │
│   ① WEBVIEW (React canvas)  ◄── postMessage ──►  ② EXTENSION HOST (TS)  │
│      draws the graph                              commands, menus,      │
│      no file access                               applies edits, saves  │
│                                                                          │
└──────────────────────────────────────────────────────┬───────────────────┘
                                                       │ stdin/stdout, JSON-RPC
                                                       ▼
                                           ③ CORE  (Rust binary `vnoc`)
                                              the brain: graph, IDs,
                                              matching, .vnoc/ files, cache
                                                       │ stdin/stdout, JSON-RPC
                                                       ▼
                                           ④ DRIVER (Rust binary `vnoc-driver-python`)
                                              knows one language: parses Python
                                              using pylspt (ruff + ty underneath)
```

| Program | Folder | Language | Its job | It must not |
|---|---|---|---|---|
| ① Webview | `webview/` | TypeScript + React + xyflow | draw the canvas | touch files or talk to the core directly |
| ② Extension host | `extension/` | TypeScript | the bridge to VS Code: commands, menus, file events, applying edits | parse code or decide where files go |
| ③ Core | `crates/` | Rust (async, tokio) | everything with rules: the graph, IDs, matching, `.vnoc/`, the cache | know VS Code exists |
| ④ Driver | `drivers/python/` | Rust (threads) | read one language: outline, call trees, where an ID line goes | know about `.vnoc/` or storage |

> **🧩 VS Code: why four and not one?**
> A VS Code extension is TypeScript code that VS Code loads into a shared Node.js process (the **extension host**). Heavy work there slows every other extension.
> So the heavy work runs in a separate Rust process (a **sidecar**). The canvas is a **webview**, a sandboxed web page inside a tab, which can only send messages
> to the extension host. The driver is separate again so other languages can be added later as separate downloads, and so it can be used without VS Code (CLI, MCP).
> More in [`02-vscode-extension-basics.md`](02-vscode-extension-basics.md).

## Where the data lives

```
<git root>/.vnoc/
├── vnoc.toml                    settings (MVP: only the scope = which files to index)
├── .gitignore                   written by V-NOC: "local/"
├── nodes/7Y/3RGJ87C…977Y/       one folder per documented node (shard = last 2 chars of the ID)
│   ├── node.toml                where the node is (path + qualname) and its status
│   └── why-we-parse-twice.md    your docs, any number
└── local/                       never committed
    ├── cache/index.sqlite       the cache: rebuilt any time, never shared
    └── trash/                   removed docs go here (V-NOC never deletes your writing without a click)
```

**Rule:** the cache can always be thrown away and rebuilt. Your docs and `node.toml` files are the only things that matter, and they are plain text in git.

## How a node is identified

Each documented node gets a **`NodeId`**: 26 characters, like `3RGJ87CMS6TK775XRJV8BW977Y`. It is a hash (BLAKE3) of the file path, the qualified name and
which occurrence it is (`src/orders.py`, `OrderService.parse_order`, `1`), written in Crockford base32. Same input = same ID on every machine, so two people
documenting the same function get the same folder and git merges cleanly.

In the **new MVP cut** (draft VS-D78), when you attach a doc, V-NOC writes a line `ID: 3RGJ…` into that function's docstring (through VS Code, undoable). Then a rename or
a move carries the ID with the function, and V-NOC finds it again without guessing. The full rule is in `crates/vnoc-domain/src/id.rs` (doc comments).

## The MVP in one sentence

> "Browse → canvas → attach docs → refactor → docs are still there. V-NOC edits your code only to write one ID line when you attach a doc. The edit goes through VS Code's editor and can be undone."

That is the draft VS-D78 promise. What ships in the MVP vs later: [`10-whats-left.md`](10-whats-left.md).

Next: [`02-vscode-extension-basics.md`](02-vscode-extension-basics.md)
