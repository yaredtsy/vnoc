# 10.02 The sidebar: VS Code's native tree

```
sidebar
├── 1. what it looks like
├── 2. rows: what each part of a TreeItem is used for
├── 3. the focus stack: a focus bar row and the FOCUS view
├── 4. what a click does (the canvas is never covered)
├── 5. selection, both ways
├── 6. search
├── 7. the cursor node
├── 8. open / closed rows
├── 9. Missing
├── 10. the four states
└── 11. what the native tree cannot do (why a webview sidebar may come later)
```

## 1. What it looks like

```
V-NOC
├─ FOCUS                                          ←  ✕      ← a view; shown only while there is a focus. Drag its bottom edge to resize.
│    All code
│    📁 app
│    📄 checkout                                  ●          ← the current step
├───────────────────────── drag ──────────────────────────
├─ STRUCTURE                                  🔍  ⟳  ⊟      ← Search · Refresh · Collapse All
│  ⌖ Focus: app › checkout                       ←  ✕       ← the focus bar row (only while there is a focus)
│  ▾ 📄 checkout                                             ← the tree starts at the focused node (built)
│      ƒ checkout_order           📄 2        ⌖  ↗  ＋        ← hover buttons (＋ on every row: files and folders take docs too)
│      ƒ validate
└─ MISSING (2)                                               ← shown only when something is missing
```

Without a focus there is no FOCUS view and no focus bar row, and the tree starts at the project's top folders (built).

## 2. Rows

The tree has three kinds of element. One provider (`StructureView`) gives all of them, so the focus bar row sits inside the same tree.

```ts
// extension/src/structureView.ts
type Row = { row: "focusBar" } | { row: "node"; node: TreeNode };
```

`getChildren(undefined)` returns `[focusBar, focusedNode]` with a focus, and the top folders without one. Every other call returns the node's
children. `getParent` stays (`reveal` needs it) and returns `undefined` for the focused node.

How a node row uses the parts of a `TreeItem`:

| Part | Used for | State |
|---|---|---|
| `label` | the name; during a search the matching letters are bold (`TreeItemLabel.highlights`) | name ✅ |
| `iconPath` / `resourceUri` | the user's file icon theme for folders and files (`resourceUri`), codicons `symbol-class` / `symbol-function` for defs | ✅ |
| `description` | `📄 2` when there are docs · `syntax error` | ✅ |
| `tooltip` | a `MarkdownString`: kind, path:line, the link state ("found by marker" / "by name only"), the titles of its docs | text ✅, Markdown new |
| `contextValue` | the flags the menus need, as words: `node function docs hasCode` (`when` clauses test them with `=~ /\bfunction\b/`) | kind only ✅ |
| `command` | `vnoc.open` with the node (§4). Today `vscode.open`. | changes |
| `id` | the node key, so VS Code keeps the row's open/closed state and `reveal` finds it | ✅ |
| `accessibilityInformation` | "function checkout_order, 2 docs, selected on canvas" for screen readers | new |

The focus bar row: label `Focus: app › checkout` (the last two names, `…` before them when the stack is longer; the full stack in the tooltip),
icon `target`, `contextValue` `focusBar`, no command, not collapsible. Its inline buttons are Back and Clear (`04-menus.md` §4).

## 3. The focus stack

Two drawings of the same `contexts.main.focus`, for two jobs:

| | Focus bar row (top of STRUCTURE) | FOCUS view (above STRUCTURE) |
|---|---|---|
| job | "where am I, and get me out", where you are already looking | the whole history; jump back to any step |
| shows | one line, the last two names | one row per step, plus "All code" at the top; the current step has a dot |
| click | nothing (it is a label) | jump back to that step (`focusJump`) |
| buttons | inline `←` Back · `✕` Clear | title `←` Back · `✕` Clear; inline `✕` on a row: drop this step and those after it |
| size | one row | **resizable**: VS Code lets you drag the edge between views, collapse the view, or move it |
| when | the stack is not empty | the stack is not empty (`"when": "vnoc.focused"` on the view) |

These replace the title-line text (`sidebarView.description = "Focus: …"`) and the two title buttons on STRUCTURE.

> **🧩 VS Code note: views are the resizable panels.** Every view in a view container (`contributes.views.vnoc`) is a section of the side bar that
> the user can resize, collapse and drag to reorder, and VS Code remembers the sizes. The web version builds this with `ResizablePanelGroup`; here it
> costs one entry in `package.json` and a second small `TreeDataProvider` (`extension/src/focusView.ts`, about 50 lines), which reads the same store.

## 4. What a click does

This is the question "I select another node while the canvas is open: what should happen?". The answer follows one idea: **the canvas is where you
are working while it is open, so a click shows the node there; the code comes along beside it, never on top of it.**

| Situation | Click a row (or Enter) |
|---|---|
| **canvas open** (in front, or behind another tab) | select · reveal the card (open its ancestors, centre it, flash it) · bring the canvas to the front of its group without taking the keyboard away from the sidebar. **No code tab is opened in the canvas's column.** |
| canvas open **and a code editor visible in another column** (canvas and code side by side) | the same, and that code editor also goes to the definition, as a preview tab, keyboard staying in the sidebar. Both sides follow the click. |
| **canvas closed** | select · open the code at the definition as a **preview** tab (italic title; the next click replaces it, so tabs do not pile up), the same as a click in the Explorer |
| a folder | select (the tree opens it, as always) · with the canvas open, reveal its card |

The other gestures, the same whatever is open:

| Gesture | Result |
|---|---|
| inline `↗` · menu "Go to Code" | open the code, always. With the canvas in front, in the column **beside** it (created the first time, reused after). Keyboard goes to the code. |
| inline `⌖` · menu "Focus" · `Ctrl/Cmd+Enter` | focus: the tree and the canvas both start at this node. Does not open a closed canvas (the editor menu's "Focus on Canvas" does). |
| `↑` `↓` | move VS Code's focus (the highlighted row); nothing opens. **Checked in step R6** (real window, `list.focusDown` and the like): the row's command does not run on an arrow key, **and the selection does not move either**. `Enter`, `Space` or a click select the row (and run `vnoc.open`), and then the canvas ring follows. The plan assumed the arrows select; they do not in this VS Code. |
| right-click | the row is selected by VS Code, then the menu |

**Recommended layout**, offered by "V-NOC: Open Canvas to the Side": code on the left, canvas on the right. Then every click in the sidebar moves
both, and nothing ever covers anything.

A setting for people who want one behaviour always:

```jsonc
"vnoc.sidebar.click": {
  "enum": ["auto", "code", "canvas"], "default": "auto",
  "enumDescriptions": [
    "Show the node on the canvas while the canvas is open; otherwise open the code.",
    "Always open the code (the behaviour before 0.1).",
    "Always show the node on the canvas, opening the canvas if it is closed."
  ]
}
```

Why not "canvas visible" instead of "canvas open": the click comes from the sidebar, and the user may have just switched to a code tab in the same
group to read something. Keying on *visible* would make the same click do two different things depending on which tab was last in front, which is
hard to predict. *Open* is a choice the user made and can see in the tab bar. If testing shows people want *visible*, it is one line.

```ts
// extension/src/commands.ts
async function open(node: TreeNode) {
  store.dispatch({ type: "select", id: node.id });
  const mode = config("sidebar.click");
  const canvas = panels.canvas;
  if (mode === "canvas" || (mode === "auto" && canvas)) {
    store.dispatch({ type: "reveal", id: node.id, canvas: true });
    panels.showCanvas({ preserveFocus: true });                          // reveal(column, preserveFocus = true)
    const code = vscode.window.visibleTextEditors.find((e) => e.viewColumn !== panels.canvas?.viewColumn);
    if (code && node.kind !== "folder") await showCode(node, { viewColumn: code.viewColumn, preview: true, preserveFocus: true });
    return;
  }
  if (node.kind !== "folder") await showCode(node, { preview: true, preserveFocus: true });
}
```

A click inside the sidebar never needs a focus step back: every row drawn is inside the focus, because the tree starts at the focused node.

> **🧩 VS Code note: no double-click.** A native tree has no double-click event: the item's `command` runs on a click and on Enter. That is why focus
> is the `⌖` button and `Ctrl/Cmd+Enter` (a key binding with `"when": "focusedView == vnoc.structure"`, running `vnoc.focusSelection`) and not a
> double-click as on a card. **[check: `Ctrl+Enter` does not clash with a built-in list binding]**

## 5. Selection, both ways

| From | To | How |
|---|---|---|
| a row (click, arrows) | the session | `treeView.onDidChangeSelection` → `dispatch({ type: "select" })`. The click's `vnoc.open` dispatches the same `select` first. |
| the session (a card was clicked, a menu, a reveal) | the row | the store's `onChange`: if `selected` moved and the tree is visible, `treeView.reveal(row, { select: true, focus: false })` |

The two do not chase each other: `reveal` fires `onDidChangeSelection`, that dispatches `select` with the id already selected, and `reduce`
returns the same object, so nothing is broadcast. A test checks it (`05-build-order.md` §4).

`reveal` opens the row's ancestors in the tree. It is only called while the tree is visible (`treeView.visible`); when the sidebar is shown again,
the current selection is revealed once (`onDidChangeVisibility`).

On the canvas the selected card has the ring from [`06-how-it-looks.md`](06-how-it-looks.md) §2; in the tree the selected row is VS Code's own selection colour. Different looks,
the same node, and both always agree.

## 6. Search

The native tree has no box of its own, so two ways in:

| Way | Good for | How |
|---|---|---|
| **🔍 in the STRUCTURE title** (`vnoc.search`, also `Ctrl/Cmd+Alt+N` **[check the key]**) | finding any node in the project | a `QuickPick` over **every** node: name, then `path:line` as description, kind icon. `matchOnDescription: true`, so typing `billing/` finds by path. Moving through the list **highlights** the active item on the canvas without
selecting it (`onDidChangeActive` → `highlight`, [`06-how-it-looks.md`](06-how-it-looks.md) §6). Picking one: `reveal` with `canvas` per the click rule, then the tree reveals and selects it. If the focus hides it, the focus steps back (the `reveal` rule). |
| **type in the tree** (VS Code's find box, `Ctrl/Cmd+Alt+F`) | filtering what is open | free; it only searches rows that are loaded (open), so it is not a project search |

The QuickPick is built from the structure the extension already holds: no request to the core, and it works the same on a big project. 10,000 items
are fine for a QuickPick **[measure on a large repo]**.

## 7. The cursor node

The node the text cursor is in gets a mark in the tree, not the selection (rule 2 in `03-focus-and-contexts.md` §1):

- Def rows get a `resourceUri` in V-NOC's own scheme (`vnoc-node:/src/a.py::parse#0`); their icon stays the codicon (`iconPath` wins over the
  `resourceUri` for the icon).
- A `FileDecorationProvider` for that scheme returns `{ badge: "●", tooltip: "the cursor is here", color: new ThemeColor("editorCursor.foreground") }`
  for the cursor node, and fires a change for the old and the new one when `cursor` moves.

**[check]** that a custom tree honours decorations for a custom scheme. If it does not, fall back to the description: `📄 2 · ●`. This is polish
(step R8); nothing depends on it.

VS Code's Outline view has "Follow Cursor", which *selects* the row. That is deliberately not done here: in V-NOC the selection drives the doc panel
and the menus, and it must not jump while you type.

## 8. Open / closed rows

The tree's rows and the canvas's cards open and close **separately** in 0.1 (U5).

> **🧩 VS Code note: why not shared.** An extension can open a tree row from code (`reveal(row, { expand: true })`), but it cannot close one: the only
> close is "Collapse All" (`workbench.actions.treeView.vnoc.structure.collapseAll`). Sharing would work in one direction only, which is worse than not
> sharing.

What is shared: a **reveal** opens what hides the node on both surfaces. VS Code keeps the tree's open rows by item `id`; the canvas's are
`contexts.main.expanded` in the session.

## 9. Missing

A second native view, `vnoc.missing`, under STRUCTURE (`"when": "vnoc.hasMissing"`): documented nodes that cannot be found, each with Relink and
Remove. That is Step 5 of the MVP; nothing in this folder changes it. `treeView.badge` on STRUCTURE shows the count, so it is seen while the view is
collapsed.

## 10. The four states

| State | What the sidebar shows |
|---|---|
| loading | `treeView.message = "Reading the code…"` |
| empty | `viewsWelcome` (Markdown with command links, shown while the tree is empty): no folder open: *"Open a folder to see its code."* [Open Folder] · no supported files: *"No Python files here yet."* |
| ready | §1 |
| error | `viewsWelcome` with the message and [Try Again] (`vnoc.refreshStructure`) and [Show Log] (*Output ▸ V-NOC*). The core not starting is the common one. |

## 11. What the native tree cannot do

Kept here so the choice can be made again with the facts:

| Wanted | Native tree | Webview sidebar (later) |
|---|---|---|
| focus bar with buttons | a row with two icon buttons | any layout |
| a search box in the view | QuickPick + VS Code's find box | a box that filters as you type, and opens matches |
| one menu list for the row and the card | two lists, same commands, a test keeps them equal | one list |
| open / closed shared with the canvas | no (§8) | yes |
| stacked EXPLORE contexts (0.2) | one EXPLORE view whose top rows are the contexts | resizable panels, like the web |
| a ring on the selected row, a cursor mark | VS Code's selection colour; a decoration (§7) | any style |
| keyboard, screen reader, virtual scrolling, sticky scroll, file icon theme | **free** | all to rebuild, except the icon theme, which a webview cannot have |

The session design does not depend on the choice: a webview sidebar later reads the same session and sends the same intents. What would change is
listed in [`05-build-order.md`](05-build-order.md) §6.

Next: [`03-focus-and-contexts.md`](03-focus-and-contexts.md)
