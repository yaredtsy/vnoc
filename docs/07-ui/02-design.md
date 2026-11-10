# 07.02 UI design plan

```
ui
├── 1. the surfaces (where V-NOC appears in VS Code)
├── 2. the sidebar
│   └── the right-click menu in the code editor
├── 3. the canvas
│   ├── layout of the panel
│   ├── what is open at first (one level)
│   ├── focus and the focus stack
│   ├── the node card
│   ├── writing the first doc
│   ├── the doc panel (read here, edit in a tab)
│   └── docs beside the code
├── 4. looking native: theme, icons, spacing
├── 5. code in a card: Shiki for reading, Monaco for editing
└── 6. what is in 0.1, and what waits
```

## 1. The surfaces

```
VS Code window
├── Activity bar          the V-NOC icon                                         ✅ exists
│   └── Side bar
│       ├── STRUCTURE     tree: folders ▸ files ▸ classes ▸ functions            ✅ exists
│       └── MISSING       documented nodes that cannot be found  (hidden when empty)   0.1
├── Editor area
│   ├── V-NOC canvas      a webview tab                                          ✅ exists (basic)
│   │   └── doc panel     on the right of the canvas: the doc, rendered, read-only     0.1
│   ├── V-NOC docs        a webview tab BESIDE a code editor: the same doc panel,      0.1
│   │                     without the canvas; follows the cursor
│   └── a doc (editing)   a normal Markdown editor tab, opened by the panel's Edit     0.1
├── Editor title bar      an ⓘ button on code editors: open V-NOC docs beside          0.1
├── Context menus         a "V-NOC" submenu in the code editor's right-click menu,      0.1
│                         on tree items and on Explorer items
├── Command palette       "V-NOC: …" commands                                    ✅ 3 exist
└── Status bar            (nothing in 0.1)
```

> **🧩 VS Code note: native views vs webviews.** The sidebar tree is a **native** `TreeView`: you give VS Code data (`getChildren`, `getTreeItem`) and it
> draws, themes, virtualises and makes it keyboard-accessible for you. The canvas is a **webview**: you draw everything yourself in HTML. Rule of thumb:
> use a native view whenever the thing is a list or a tree, and a webview only for what VS Code cannot draw (a graph). That is why "Missing" should be a
> second native view and not a panel inside the canvas.

## 2. The sidebar

```
STRUCTURE                                    ⟳
▾ 📁 src
  ▾ 📁 billing                    📄1            ← doc count, shown as the item's description
    ▸ 🐍 invoice.py
  ▾ 🐍 orders.py
    ▾ ◆ OrderService
        ƒ parse_order             📄2
        ƒ validate                               ← hover: [⌖ focus on canvas] [＋ attach doc]

MISSING (2)
  ƒ parse_legacy   src/old.py · 1 doc            ← click: Relink…    context menu: Relink · Remove
  📁 src/reports   · 2 docs
```

| Element | 0.1 behaviour |
|---|---|
| click a file / class / function | opens the code at that line (exists) |
| hover buttons | **Focus on canvas** (exists) · **Attach doc** (new) |
| context menu | Add doc · Focus on canvas · Open docs ▸ (list; opens the canvas with the doc panel) |
| description text | `📄 n` when the node has docs; `syntax error` (exists) |
| MISSING view | appears only when something is missing (`"when"` clause on a context key the extension sets) |
| Explorer context menu | "V-NOC: Attach doc" on files and folders, so documenting does not require opening the V-NOC view |

**A click keeps jumping to the code.** A real VS Code editor always shows the whole file; it cannot hide the lines around a function. So the tree
click stays "open the file at that line", and the place to see *only* the function is its card on the canvas (`</>`).

### The right-click menu in the code editor

VS Code lets an extension add entries to the editor's own right-click menu. Put V-NOC's entries in **one submenu**, so the menu grows by one line:

```
(right-click inside a .py file)
  Go to Definition
  …
  V-NOC                    ▸   Show on canvas        reveal + select the node at the cursor; the focus stack is not changed
                               Focus on canvas       push it on the focus stack (only this node and its children are drawn)
                               Show docs             open V-NOC docs beside this editor
                               Add doc…              the "first doc" flow (§3)
```

"The node at the cursor" is the innermost function or class whose range contains the cursor line, else the file. The extension already holds the tree
with ranges, so this is a lookup, not a request to the core.

**Show on canvas** when the node is hidden by the current focus: jump the stack back to the nearest entry that contains the node (or clear it), expand
the node's ancestors, select it and centre it.

> **🧩 VS Code note: a submenu is three entries in `package.json`.**
> ```jsonc
> "contributes": {
>   "submenus": [ { "id": "vnoc.editorMenu", "label": "V-NOC" } ],
>   "menus": {
>     "editor/context": [                                   // the code editor's right-click menu
>       { "submenu": "vnoc.editorMenu", "group": "navigation@9", "when": "resourceLangId == python" }
>     ],
>     "vnoc.editorMenu": [                                  // what is inside the submenu
>       { "command": "vnoc.showOnCanvas", "group": "1_canvas@1" },
>       { "command": "vnoc.focus",        "group": "1_canvas@2" },
>       { "command": "vnoc.showDocs",     "group": "2_docs@1" },
>       { "command": "vnoc.addDoc",       "group": "2_docs@2" }
>     ],
>     "explorer/context": [                                 // right-click on a file or folder in the Explorer
>       { "submenu": "vnoc.editorMenu", "group": "7_modification@9" }
>     ]
>   }
> }
> ```
> `when` decides if the entry is shown. `resourceLangId == python` is hard-coded for now; when a second driver exists, the extension sets its own
> context key (`vnoc.hasDriver`) from the driver's `files.extensions`. A command run from a menu gets the file's `Uri` as its argument; the cursor
> position comes from `vscode.window.activeTextEditor.selection.active`. The same commands also serve the sidebar tree, which passes a tree node
> instead, so each command starts by turning its argument into a node key.

A node found only "by name" gets a tooltip: *"Attached by name. It will be lost if renamed. Write ID to make it permanent."*

## 3. The canvas

### The panel

```
┌────────────────────────────────────────────────────────────────────────────┐
│ V-NOC   my-project · 12 folders · 87 files · 430 functions          ⟳      │  bar
│ All code › src › billing › invoice.py                                      │  breadcrumbs = the focus stack   ✅
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│   ┌──────────┐      ┌──────────────┐      ┌───────────────────┐            │
│   │ 📁 src   ├──────┤ 📁 billing   ├──────┤ 🐍 invoice.py     │   …        │  cards, left to right by depth    ✅
│   └──────────┘      └──────────────┘      └───────────────────┘            │
│                                                                    ┌─────┐ │
│                                                                    │ map │ │  minimap, zoom controls           ✅
└────────────────────────────────────────────────────────────────────┴─────┘─┘
```

The tree layout you have (depth to the right, one row per node) is the right one for 0.1: it is predictable and it matches the sidebar. A graph layout
library (dagre, elk) becomes necessary in 0.2, when call edges cross the tree.

### What is open at first

Today the first open draws every node expanded. On a real project that is thousands of cards. Start with **one level open**:

```
first open                         after a focus on `billing`
┌─────────┐   ┌──────────┐         ┌──────────┐   ┌──────────────┐
│ project ├─┬─┤ ▸ src    │         │ billing  ├─┬─┤ ▸ invoice.py │
└─────────┘ ├─┤ ▸ tests  │         └──────────┘ └─┤ ▸ tax.py     │
            └─┤ ▸ app.py │
              └──────────┘
```

- The root (or the focused node) is open; its children are drawn **closed**. The user opens what they want.
- Store the set of **expanded** keys, not the set of collapsed ones. Today `App.tsx` keeps `collapsed`, so "everything closed" means a set holding
  every node in the project. With `expanded`, the default is the empty set, a new file that appears is closed without any work, and the saved view
  state stays small.
- `layout()` then only walks what is open, so its cost follows what is on screen and not the size of the project.
- "Show on canvas" and a sidebar Focus add the node's ancestors to `expanded`.

### Focus and the focus stack

Focus is the same idea as in the web version: **focus a node and the canvas shows only that node and what is under it.** Everything else is hidden.
Each focus is added to a stack, so you can go back one step or clear it all.

```
All code                                   stack: []                         the whole project
  focus  src/billing                       stack: [billing]                  only billing and its children
  focus  invoice.py › Invoice.total        stack: [billing, Invoice.total]   only that method
  ← Back                                   stack: [billing]
  ✕ Clear                                  stack: []
```

Most of this is built: `webview/src/canvas/focusStack.ts` has `push`, `pop`, `jump` and `prune` with tests, `App.tsx` draws the breadcrumbs, a
double-click on a card focuses it, the sidebar's Focus button pushes onto an open canvas, and the stack is saved in `vscode.setState`.

**The rules** (✅ = built, 0.1 = to add):

| Rule | State |
|---|---|
| A **single click selects** a card. It never changes the focus. | ✅ |
| **Focus** = double-click a card · the card's `⌖` · the sidebar's Focus button · "Open docs" on a node that is not on screen | ✅ except `⌖` (0.1) |
| A focus **appends** to the stack, also when it comes from the sidebar while you are already focused somewhere else. | ✅ |
| The stack is a **history, not a path**: `billing › Invoice.total` means "I was on billing, then went to Invoice.total", whether or not one contains the other. | ✅ |
| Focusing the node that is already on top does nothing. | ✅ |
| Focusing a node that is **already lower in the stack** jumps back to it instead of adding it a second time. (The web version adds a duplicate; this keeps the crumbs short.) | 0.1 |
| **Back** (`←` button in the crumb bar, and `Alt+←`) removes the top entry. `pop` exists; the button does not. | 0.1 |
| **Clear** = the "All code" crumb. Clicking any other crumb jumps back to it. | ✅ |
| Focusing a node also **selects** it, so an open doc panel shows that node's docs. | 0.1 |
| The focused node is drawn expanded even if it was collapsed. | ✅ |
| After a refresh, entries whose node is gone are dropped from the top (`prune`). | ✅ |
| The sidebar tree **reveals** the focused node (`treeView.reveal`). It does not hide the rest of the tree, unlike the web version: the sidebar is how you leave a focus. | 0.1, small |

The crumb bar with the two additions:

```
│ ←   All code › billing › Invoice.total                                     │
  │      │         │            └ the node on screen (not clickable)
  │      │         └ click: jump back to it
  │      └ click: clear
  └ back one step (disabled when the stack is empty)
```

Not in 0.1: pushing a whole lineage at once (`pushFocusBulk` in the web version, used by share links and the agent), and one focus stack per canvas
tab (there is one canvas).

### The node card

```
┌────────────────────────────────────────────────────┐
│ ▸  ƒ  parse_order              🔗   📄 2    </>    │   header
├────────────────────────────────────────────────────┤
│   (self, raw)                                      │   body:  signature / children count
│                                                    │          or the code (when </> is on)
├────────────────────────────────────────────────────┤
│ src/orders.py:42                     ⌖    ↗    ＋  │   footer
└────────────────────────────────────────────────────┘
```

| Part | Element | Meaning | State |
|---|---|---|---|
| header | `▸` | expand / collapse children | ✅ |
| | kind icon + name | folder, file, class, function | ✅ |
| | `🔗` link icon | **solid** = found by marker · **outlined** = found by name · absent = no ID | 0.1 |
| | `📄 n` | number of docs; **shown only when n ≥ 1**; click opens the doc panel on this node | 0.1 |
| | `</>` | show the code in the body | ✅ |
| body | one of two contents | summary · code | code ✅ |
| footer | path:line | where it is | ✅ |
| | `⌖` | focus (same as double-click) | 0.1 (double-click exists) |
| | `↗` | go to code in the editor | 0.1 |
| | `＋` | add a doc: **always shown**, also on a node with no docs yet | 0.1 |

Visual states of the whole card: normal · selected (focus ring) · syntax error (exists: `broken`) · Missing cards are **not** on the canvas (they have no
place in the tree); they live in the MISSING view.

Card height is a constant today (`ROW_H`, `CODE_H`). The docs moved to the panel, so the card keeps two body modes and the constants still work.

Later (not 0.1): the web version shows the first lines of the doc in the card body. That is a good summary mode once docs exist; it needs measured card
heights (React Flow reports each node's rendered size; feed those into `layout()` instead of constants).

### Writing the first doc

A node with no docs has no `📄` badge, so the way in is the `＋`. There are five places to start, and all of them run the same command
(`vnoc.addDoc`, with the node as its argument):

```
card footer            ＋
doc panel              "＋ New doc…" in the dropdown, and the button in the empty state
sidebar tree           hover button ＋ and the context menu "Add doc"
Explorer               context menu "V-NOC: Add doc" on a file or folder
Command palette        "V-NOC: Add doc to the function at the cursor"
```

What happens after the click:

```
1. title?            VS Code input box, pre-filled with "Overview" for a node's first doc
                     (Esc cancels; nothing has been written yet)
2. first time in     one question, once per project:
   this project        "V-NOC will add an ID line to parse_order's docstring and create a .vnoc/ folder in this repository."
                       [Add doc]   [Add without ID]   [Cancel]
                     "Add without ID" sets `[ids] default = "none"` in vnoc.toml. Later docs do not ask again.
3. the core attaches   ID → marker in the code → node.toml → overview.md with frontmatter and "# Overview"
                       (04-storage/03-writing-safely.md §3)
4. the UI shows it     · the card gets 🔗 and 📄 1
                       · the doc panel opens on the new doc
                       · the .md opens in an editor tab with the cursor under the heading, ready to type
```

The second doc on the same node is the same flow without step 2, and the title box starts empty.

Step 2 exists because the first attach is the only moment V-NOC edits the user's source code without being asked in so many words. One clear question,
once, is cheaper than a surprised user looking at `git diff`.

The title input uses `vscode.window.showInputBox` (native) rather than a text field inside the webview: less code, and it behaves like every other VS Code
prompt.

### The doc panel

```
┌ V-NOC canvas ─────────────────────────────┬──────────────────────────────────┐
│ All code › src › orders.py                │ Overview ▾          ✎ Edit    ✕  │  ▾ = this node's docs + "＋ New doc…"
├───────────────────────────────────────────┤ ƒ parse_order · src/orders.py:42 │  which node the doc belongs to
│                                           ├──────────────────────────────────┤
│   ┌────────┐      ┌──────────────┐        ║                                  │
│   │ 📁 src ├──────┤ 🐍 orders.py │   …    ║  # Why we parse twice            │  the Markdown, rendered
│   └────────┘      └──────────────┘        ║                                  │  read-only
│                                           ║  The upstream API sends…         │
│                                           ║                                  │
└───────────────────────────────────────────┴──────────────────────────────────┘
                                            ║ = drag to resize
```

| Part | Behaviour |
|---|---|
| open | the **ⓘ** button at the top right of the canvas (as in the web version; click again to close) · `📄 n` on a card · "Show docs" in a menu · after adding a doc |
| which node | the selected card, else the focused node. While the panel is open, selecting another card switches the panel to that card's docs. Nothing selected and no focus: "Select a node to see its docs." (Docs on the project itself are open question Q1; when that is settled, this state shows the project's Overview, like the web version.) |
| `▾` dropdown | the docs of this node by title · last entry "＋ New doc…" |
| `✎ Edit` | opens the `.md` in a normal editor tab (`showTextDocument(uri, { viewColumn: ViewColumn.Beside })`) |
| `✕` | closes the panel; the canvas takes the full width again |
| empty state | a node with no docs: "No docs on `parse_order` yet." and a **Write the first doc** button (runs the flow above) |
| width, open/closed, chosen doc | webview view state (`vscode.setState`), so it survives hiding and showing the tab |

**Read here, edit in a tab.** The panel never writes. Editing happens in a VS Code Markdown tab, which gives you undo, save, search and spell-check
extensions without writing editor code. The panel is a live preview of that tab:

```
editor tab (the .md)                    extension                              webview panel
  typing ──► onDidChangeTextDocument ──► debounce ~150 ms ──► docText { uri, text } ──► render
  (no tab open: the extension reads the file, and sends it again when the file watcher reports a change)
```

> **🧩 VS Code note: why the text comes from the extension.** A webview cannot read files. The extension reads the doc and posts the text in. If the doc
> is open in an editor, read it with `vscode.workspace.openTextDocument(uri)`: that returns VS Code's in-memory copy, **including unsaved typing**, so the
> panel updates while you type and not only when you save.

Rendering rules:

- Strip the frontmatter (`---` … `---`) before rendering; show the title from the first `# heading`.
- Use a small Markdown renderer (`markdown-it`, or `react-markdown`) with **raw HTML turned off**. Docs arrive by `git pull` from other people, so treat
  them as untrusted input: no `<script>`, no inline HTML at all.
- Code blocks are coloured by Shiki, which is already in the bundle.
- A link is not followed inside the webview. The click goes to the extension, which opens `http(s)` links in the browser and relative links as files.
- Images with relative paths do not load in 0.1 (they need `webview.asWebviewUri` and a wider `localResourceRoots`). Show the alt text.

### Docs beside the code

The web version has a Code tab with the docs on its right. In VS Code the "Code tab" is a normal editor, and an extension **cannot draw over or
inside** it. What it can do is open a webview tab in the column next to it, the same way *Markdown: Open Preview to the Side* works. The result looks
like the web version:

```
┌ api.py ──────────────────────────────┬ V-NOC docs ──────────────────────────┐
│                                 ⓘ    │ Overview ▾            ✎ Edit      ✕  │
│  11  @app.post("/checkout")          │ ƒ checkout · app/api.py:13           │
│  12  def checkout(order):            ├──────────────────────────────────────┤
│  13      """ID: 0Q8V…"""             │  # API module                        │
│  14      try:▌                       │  The api.py module provides the…     │
│  15          completed = …           │                                      │
└──────────────────────────────────────┴──────────────────────────────────────┘
   a normal editor (you type here)        a webview tab: the doc panel alone
```

| Part | Behaviour |
|---|---|
| open | the **ⓘ** button in the editor's title bar · right-click ▸ V-NOC ▸ Show docs |
| which node | the function or class the cursor is in; if it has no docs, its class, then its file. The header line says which one is shown. |
| follows the cursor | when the cursor moves to another documented node, the tab switches to it (debounced). A **pin** button stops that. |
| no docs anywhere on the way up | the empty state with **Write the first doc** for the node at the cursor |
| one tab | there is one V-NOC docs tab; pressing ⓘ in another editor reuses it |

It is the **same React component** as the canvas's doc panel and the same messages (`docText`, the doc list). Only the host differs:

```
webview bundle (one build)
├── mode "canvas"   App = canvas + doc panel on the right
└── mode "docs"     App = doc panel alone                    ← the extension passes the mode when it creates the webview
```

> **🧩 VS Code note: the title-bar button and following the cursor.**
> ```jsonc
> "menus": { "editor/title": [
>   { "command": "vnoc.showDocs", "group": "navigation", "when": "resourceLangId == python" }
> ] }
> ```
> A command with an `icon` (`"$(info)"`) placed in `editor/title` → `navigation` is drawn as a button at the top right of the editor.
> To follow the cursor, listen to `vscode.window.onDidChangeTextEditorSelection` and `onDidChangeActiveTextEditor`, wait ~200 ms, find the node at the
> cursor, and post it to the docs webview only if it changed. Open the tab with
> `createWebviewPanel(…, { viewColumn: ViewColumn.Beside, preserveFocus: true })` so the cursor stays in the code.

Build order: the canvas's panel first (Step 3), then this host. It adds a second place to show the component, not a second component.

**What 0.3 changes:** the panel gets an editor of its own, so typing happens in place like the web version. The layout, the dropdown and the messages
stay; only the read-only body is replaced.

## 4. Looking native

Three habits make a webview feel like part of VS Code instead of a web page inside it:

1. **Colours only from VS Code's theme variables.** VS Code injects hundreds of CSS variables into every webview (`--vscode-editor-background`,
   `--vscode-foreground`, `--vscode-focusBorder`, `--vscode-badge-background`…). Use them and the canvas follows any theme, including high contrast, with no
   work. Collect the ones you use in one place:
   ```css
   /* webview/src/tokens.css — the only file that names a --vscode-* variable */
   :root {
     --card-bg:      var(--vscode-editorWidget-background);
     --card-border:  var(--vscode-editorWidget-border, var(--vscode-contrastBorder));
     --card-focus:   var(--vscode-focusBorder);
     --text:         var(--vscode-foreground);
     --text-muted:   var(--vscode-descriptionForeground);
     --badge-bg:     var(--vscode-badge-background);
     --badge-fg:     var(--vscode-badge-foreground);
     --error:        var(--vscode-errorForeground);
     --font:         var(--vscode-font-family);
     --font-code:    var(--vscode-editor-font-family);
   }
   ```
2. **Icons from codicons,** VS Code's own icon font (`@vscode/codicons`: `symbol-function`, `symbol-class`, `folder`, `link`, `target`, `go-to-file`…).
   The sidebar already uses them through `ThemeIcon`; the same icons on the cards make the two views read as one product. Your CSP already allows fonts
   from the extension.
3. **VS Code's sizes.** 13 px UI text, 22 px rows, 4 px grid. No custom shadows or gradients.

Accessibility comes mostly for free if you keep doing what the code already does: real `<button>` elements with `aria-label` / `aria-expanded`.
Add a visible focus ring (`outline: 1px solid var(--card-focus)`) and check the canvas once with a high-contrast theme.

## 5. Code in a card: Shiki for reading, Monaco for editing

You just added Monaco (with Shiki for colours) for the read-only code view. It works, but look at what each one costs:

| | Monaco editor | Shiki `codeToHtml` |
|---|---|---|
| What it is | a full editor: cursor, selection model, scrolling, input | a function: text in, coloured HTML out |
| Per open card | a whole editor instance (memory, layout work on every pan/zoom) | a few DOM nodes |
| 30 cards open | will be felt **[measure]** | no problem |
| Needed for | **editing** (0.3) | **reading** (0.1, 0.2) |

Shiki is already in your bundle and already doing the colouring. For read-only cards, call it directly and render the HTML; keep line numbers with a CSS
counter. Then in 0.3, mount Monaco **only in the one card the user is editing** and swap back to static HTML when they leave it.

This is a suggestion to measure, not a demand: open 30 code cards on a real project, pan around, and look at the frame rate and memory in the webview
developer tools (*Developer: Open Webview Developer Tools*). If it is smooth, keep what you have until 0.3.

One more gap for later: the Shiki themes are fixed (`github-light` / `github-dark`), so the card's colours differ from the user's editor theme. Acceptable
for 0.x.

## 6. What is in 0.1, and what waits

| | 0.1 | 0.2 | 0.3 | later |
|---|---|---|---|---|
| Sidebar | Structure · Missing · attach from menus | | | search, CodeLens, hover |
| Canvas | tree layout · focus · breadcrumbs · minimap | call edges, graph layout, lazy expand | | groups, saved views |
| Card | link icon · doc count · add doc · code (read) · go to code | call markers, external leaves | **edit code** | zoom dialog, doc summary in the body |
| Docs | **doc panel** (rendered, read-only, live preview) · edit in a VS Code tab | | **edit in the panel** | freshness, links between docs |
| Feedback | Missing view · Problems lines | | | notifications, status bar |

Next: [`03-selection-and-menus.md`](03-selection-and-menus.md) (selection, the click rules and every menu; it replaces the menu in §2 and two focus rules in §3)
