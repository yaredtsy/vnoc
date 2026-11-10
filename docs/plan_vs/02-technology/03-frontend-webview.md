# 02.03: Frontend: React + xyflow in a webview, plus a native tree (VS-D11)

## Two UIs, each for what it's good at

| | Native **TreeView** (sidebar) | **Webview** canvas (editor tab) |
|---|---|---|
| Built with | the `vscode` API in the extension host (TS) | React app in an iframe |
| Good at | fast, themed, keyboard-friendly lists; thousands of rows | graphs, layout, the focus stack, call trees, rich side panel |
| Shows | Root ▸ folders ▸ files ▸ classes ▸ functions, the Archived list, the Missing list, the "Recently removed" list (Restore, "Empty V-NOC trash") | a node and its neighbourhood, call trees, the doc panel (read, edit, add, remove) |

The tree is the "always there" view; the canvas opens on demand ("Open in canvas").

## The webview stack (port from old v-noc)

| Need | Pick | From old v-noc? |
|---|---|---|
| Build | **Vite** → one JS + one CSS file in `dist/webview/` | ✅ same tool |
| UI | **React 19** | ✅ |
| Graph | **@xyflow/react 12** | ✅ |
| Layout | **dagre** for trees (fast). **elkjs** if we need nicer layered layouts later | ✅ dagre |
| State | **zustand** (+ immer) | ✅ the `focusSlice` pattern |
| Markdown (doc panel) | `react-markdown` to read, Monaco (Markdown mode) to edit | ✅ / ✏️ (BlockNote dropped: lossy Markdown round-trip) |
| Styling | Tailwind, with colours mapped to VS Code theme variables (below) | ✏️ shadcn pieces can be kept, re-themed |
| Code in the node card / dialog / Code tab | **Monaco**, editable as a view of VS Code's document (VS-D57), theme from VS Code **[check]** | ✅ old v-noc used Monaco for code |
| Node card | `EnhancedNode` + `NodeHeader` + `NodeCodeView` + `CodeViewDialog` (below) | ✅ the design, trimmed |

Not ported: router, react-query (the host relay replaces HTTP), the old code save path (saving now goes through VS Code's document, VS-D57), BlockNote (lossy Markdown round-trip; the doc panel uses Monaco, VS-D61).

> **🧩 VS Code note: the webview rules you'll hit on day one.**
> 1. **Files must be served through `webview.asWebviewUri(...)`.** You can't use `file://` paths; convert the URI of `dist/webview/main.js` and put it in the `<script src>`.
> 2. **Content Security Policy.** The HTML must declare a CSP (`default-src 'none'; script-src 'nonce-…'; style-src ${webview.cspSource} …`). Inline scripts need a random nonce. No CDN loads.
> 3. **Theme for free.** VS Code injects CSS variables like `--vscode-editor-background`, `--vscode-focusBorder`, and a `vscode-dark` / `vscode-light` / `vscode-high-contrast` class on `<body>`. Map Tailwind colours to these and the canvas follows every theme (FR-4A).
> 4. **Hidden = destroyed (by default).** When the user switches tabs, the iframe is thrown away. Either keep it alive with `retainContextWhenHidden: true` (costs memory), or save state with `vscode.setState(obj)` and read it back with `vscode.getState()` when it's rebuilt. **We do the second** (cheap, and needed for restart anyway).
> 5. **Restore after restart.** Register a `WebviewPanelSerializer`; VS Code then gives us back the canvas tabs that were open, with the state we saved (FR-44).
> 6. The old `@vscode/webview-ui-toolkit` is **deprecated** (since early 2025). Don't start with it.

## The message bridge (VS-P8)

```
webview                          extension host                       sidecar
  │ {type:"req", id:7,              │                                    │
  │  method:"vnoc/children",        │ sendRequest("vnoc/children", …) ─► │
  │  params:{node}} ──────────────► │                                    │
  │                                 │ ◄────────────────────── result ─── │
  │ ◄── {type:"res", id:7, result}  │                                    │
  │                                 │ ◄── notification graphChanged ──── │
  │ ◄── {type:"event", name:"graphChanged", ids:[…]}                     │
  │ {type:"cmd", name:"openCode", node} ──► host runs showTextDocument (no Rust involved)
```

- **Requests** that need data are passed through to Rust *unchanged* (same method names, same generated types).
- **Commands** that need VS Code (open an editor, run a terminal, show a QuickPick) are handled by the host. The **"Find it…"** QuickPick (02-05 §6) **preselects the top candidate** (VS-D75).
- The host keeps an allow-list of methods the webview may call. A webview is a web page, so we don't let it call anything it likes.

## The node card and the Canvas | Code tabs (VS-D48, your round-5 note)

The canvas uses the **old v-noc node design**, from
`v-noc/src/frontend/src/features/Dashboard/features/Main/components/Canvas/components/nodes/` (`EnhancedNode`, `NodeHeader`, `NodeCodeView`, `CodeViewDialog`,
`NodeDescription`, `NodeFooter`):

```
┌────────────────────────────────────────────────────────┐
│ [▸]  ƒ parse_order    🔗 📄2  ●            [</>]  [⤢]   │ ← header
├────────────────────────────────────────────────────────┤
│ Parses the raw payload twice because…                  │ ← body: first line of the first doc,
│                                                        │   or "Function parse_order"
├────────────────────────────────────────────────────────┤   when </> is on, the body becomes:
│ src/orders.py:42                                       │ ← footer   ┌ orders.py   ●  [Copy] [Open in editor] ┐
└────────────────────────────────────────────────────────┘           │ 42  def parse_order(self, raw):          │
                                                                     │ 43      …        (300 px, scrolls)       │
                                                                     └──────────────────────────────────────────┘
```

| Control | Does | Old file |
|---|---|---|
| **[▸] expand toggle** | show/hide this node's children (structure) or callees (call tree). On a call-tree frontier it calls `vnoc/callTree.expand` | `NodeHeader` (`expandable` / `onToggle`) |
| **🔗 link icon** | **solid** = found by in-code ID, **outlined** = found by `node.toml` (`id_source`, 02-05 §1). None = no permanent ID | new |
| **●** dirty dot | the document has unsaved changes (from the card or a regular editor) | `CodeViewDialog`'s `hasChanges` |
| **📄 n** | doc count; click toggles the **doc panel** for this node (below) | replaces the old task badge |
| **● status dot** | Missing / not on this branch / archived / pulled-in (dimmed card) | `statusColors` |
| **[</>] code toggle** | shows the node's code **inside the card** (the body is swapped, as in the old `useNodeCode.toggleCode`) | `NodeHeader` + `NodeCodeView` |
| **[⤢] zoom** | opens the **big dialog** (90 % of the panel) with the same code, larger, with a minimap | `CodeViewDialog` |
| double-click | **focus** (pushes onto the focus stack) | as before |

**Two tabs at the top of the panel: Canvas | Code** (as in the old `WorkspaceTabs`, minus Tasks; docs get their own panel, below):

- **Canvas**: the graph above.
- **Code**: the selected node's code full-size (the old `Code/index.tsx`). Selecting another node on the canvas switches it.

### Docs on the canvas (VS-D61)

Docs are **visible on the canvas**, next to the graph, as in the web V-NOC (`Docs/DocSidebar.tsx`, `RightSidebar/…/DocumentsList.tsx`):

```
┌──────────────── canvas tab (one webview) ─────────────────────────────────────────────┐
│ [Canvas | Code]                                     [📄 Docs ▣]  ← toggle the panel   │
│                                          ┃ OrderService.parse_order                   │
│  [▸] ƒ parse_order  🔗 📄2  [</>][⤢]     ┃ ┌ why-we-parse-twice.md          [✎] [🗑]  ┐ │
│        │                                 ┃ │ 🔒 my-todo.md                   [✎] [🗑]  │ │
│   ƒ validate     ƒ load                  ┃ └ [+ Add doc ▾ Shared / Private]           ┘ │
│                                          ┃ ── Why we parse twice ──────────────────── │
│                                          ┃ Because the payload…   (rendered Markdown)  │
└──────────────────────────────────────────┸────────────────────────────────────────────┘
```

| Part | Does |
|---|---|
| **[📄 Docs] toggle** (toolbar) and the card's **📄 n** badge | show / hide the doc panel. The panel follows the **selected** node. Its open/closed state is saved with `vscode.setState`. |
| Doc list | the node's docs from **both sides** (private ones 🔒, 02-04 §3) |
| **[+ Add doc]** | Shared or Private → a new `.md` on that side (02-04 §4; mints the ID if the node has none) |
| **[🗑 Remove]** | confirm, then move to the V-NOC trash (VS-D70) |
| ⋯ menu | rename, Share this doc / Make private (a move, VS-D58), Open beside (a regular VS Code editor in the next column) |
| Viewer | rendered Markdown (`react-markdown`, ported); `vnoc://node/<id>` links focus that node |
| Freshness label | **"Code changed since this doc was last edited"** when the node's body hash no longer matches the doc's; **[Still accurate]** clears it without editing the doc (VS-D77, M9) |
| **[✎] Edit** | switches the doc to an **editable Markdown source view in the panel**, under the same rule as code (below): a view of VS Code's document |

**Why not BlockNote (the old editor) for editing:** it stores blocks and converts to Markdown on save, which is **lossy**. It would rewrite a teammate's hand-written `.md`
(spacing, tables, HTML) every time someone opened it in the canvas. Monaco in Markdown mode edits the text as it is. The rendered view covers reading.

The card body still shows the first line of the first doc, so docs are visible even with the panel closed.

### Code (and docs) are editable everywhere they're shown (VS-D57, VS-D62)

The card, the zoom dialog, the Code tab and the doc panel are all editable. A regular VS Code editor on the same file can be open at the same time.
The rule that keeps this safe: **every webview editor is a view of VS Code's document model, not of the file on disk.**

**Where the text comes from:** always the **live document**, never a copy in the cache. The host reads `workspace.openTextDocument(uri)` (which includes unsaved
edits), the core says which line range the node covers (the whole file for a doc), and the host sends that slice to the webview.

| Step | API | Why |
|---|---|---|
| Read | `workspace.openTextDocument(uri)`, then the node's line range | includes unsaved edits from a regular editor |
| Follow | `workspace.onDidChangeTextDocument` → send the new slice to the webview (debounced) | typing in the regular editor shows up in the card |
| Write | each change in the webview → a `WorkspaceEdit` on that document (offsets = slice start + offset in the slice) | **one undo stack, one dirty marker** (● in the card header), and an open regular editor stays consistent |
| Save | **⌘S / Ctrl+S inside the canvas** (below), the card's Save button, or the user's normal save in a regular editor → `document.save()` | same file, same save participants (save-time IDs, formatters) |

**Listening for ⌘S / Ctrl+S.** When a webview has focus, VS Code still runs its keybindings. The extension contributes one bound only to the canvas:

```jsonc
// package.json → contributes.keybindings
{ "command": "vnoc.canvas.save", "key": "ctrl+s", "mac": "cmd+s",
  "when": "activeWebviewPanelId == 'vnoc.canvas'" }
```

When the binding matches, VS Code runs **only** `vnoc.canvas.save`: it doesn't also run its own save afterwards. So the command itself must save the right document:

1. The webview reports which editor has focus (a card, the dialog, the Code tab or a doc) and that editor's document URI.
2. The host calls `document.save()` on that document. Unsaved edits in *other* cards stay unsaved, as with normal editor tabs.
3. `onWillSaveTextDocument` fires with reason **`Manual`** (API saves count as manual, per the API docs), so save-time IDs run (VS-D31), from precomputed edits (VS-D66).
4. VS Code applies those edits and writes the file once.

Nothing focused (the graph itself) → the command does nothing.

**[check] C23 (half answered):** the save reason is confirmed by the docs. Still to test in the extension: that the binding really wins while the canvas has focus, and that the
webview always reports the right document. Fallback: a `keydown` handler in the webview (`(e.metaKey || e.ctrlKey) && e.key === "s"`, then `preventDefault`) that posts `{cmd: "save"}` to the host.

> **🧩 VS Code note: `when` clauses.**
> A keybinding's `when` is a condition on VS Code's *context keys*: `editorTextFocus`, `resourceLangId == python`, `activeWebviewPanelId == 'vnoc.canvas'`… The same
> key can mean different commands in different places, and the most specific match wins. That's how ⌘S saves a file normally but "saves the focused card" inside the canvas.

- **Version check:** each webview edit carries the document `version` it was based on. If the document moved on in between (typing in both places), the host
  re-reads the slice and re-applies the edit if it lands outside what changed, otherwise drops it and refreshes the card. That's a rare race; losing one keystroke beats corrupting the file.
- **The slice follows the node:** after an edit the core re-reports the node's range (a new line added inside the function grows the slice).
- **Editor component:** Monaco for code and Markdown source, as in the old `NodeCodeView` / `CodeViewDialog` (several MB in the webview bundle, still well inside NFR-32).
  Lazy-loaded the first time an editor opens.
- **Accepted v1 limit:** **no language-server completions, hovers or diagnostics** inside webview editors. **[Open in editor]** is right there for heavier edits.
- Untrusted workspace: all webview editors are read-only (VS-D18).

Not ported from the old card: diff colours, task badges, coverage, walkthrough popovers (history, tasks and the agent are out of v1).

## The focus stack (★ must work, FR-42)

Ported from `v-noc/src/frontend/src/features/Dashboard/store/slices/focusSlice.ts`, with three changes:

| Old | New | Why |
|---|---|---|
| The stack holds whole node objects (`AnyNodeTree[]`) | The stack holds **keys**: the **permanent ID** wherever the node has one, else the location key (plus a display name for the breadcrumb) | Node data changes after a sync; fresh data is fetched for whatever is on top. Permanent IDs never change. Location keys do, and a **hidden webview isn't running**, so it can't rewrite its own keys when a rename happens. Instead **the core keeps a `key_moves(old_key, new_key)` table** from each sync, and when a tab is shown or restored the host sends its saved keys to `vnoc/keys.translate` first (VS-D64). |
| Keyed by `tabId` in one store | **One store per canvas tab** (each webview is its own iframe anyway) | Simpler, no cleanup code. |
| Lost on reload | Saved with `vscode.setState` on every push/pop | Survives hidden tabs and restarts (FR-44). |

Operations stay the same: `push` (double-click = focus), `pushBulk` (Reveal from the editor pushes the path Root → … → function),
`pop` (Back), `clear`, and jump (click a breadcrumb = pop until that ID is on top).

**Edge case:** a sync removes the node on top of the stack (deleted, or Missing). Then we pop until the top is a live node, and show
"`parse_order` is gone, showing `orders.py`". A Missing node with docs stays focusable, shown greyed with its docs.

## Lazy call tree in the canvas (FR-32)

```
open:   vnoc/callTree {entry, depth: 3}           → nodes to depth 3; frontier nodes carry `hasMore`
expand: vnoc/callTree.expand {callNode, depth: 3} → 3 more levels under that node only
```

Frontier nodes show a `+` handle. Collapsing just hides them in the UI (the data stays cached in the webview store).
Markers from pylspt (recursion ↺, cut ✂, unresolved ?, external ↗) are drawn as node badges.
Nodes in files **outside the scope** (pulled in by this tree, VS-D20) are drawn dimmed with an "outside scope" hint; third-party targets are "external" leaves.
The depth comes from the `call_tree.depth` setting (lazy is the only mode, VS-D19).

In **manual sync** mode the webview may show structure that is a little behind (new functions appear after Sync), but call-tree expansions are always
checked against current file hashes by the core before they're returned (VS-D22).

Next: [`04-storage-and-git.md`](04-storage-and-git.md)
