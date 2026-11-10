# 07.03 Selection, focus and menus: one state, shown everywhere

> **Replaced by [`../10-ui-rework/`](../10-ui-rework/README.md).** Its core (the four words, the session, the click rules, how selection looks,
> the FOCUS view) is carried over there, with fixes. Kept for history; do not build from this file.

Written 2026-11-10 after reading `extension/src/`, `webview/src/`, `shared/` and the web version's store
(`ide/v-noc/src/frontend/src/features/Dashboard/store/`). It extends [`01-state.md`](01-state.md) and [`02-design.md`](02-design.md); where it
changes one of their rules, §9 lists the change.

```
selection and menus
├── 1. what is wrong today
├── 2. four words: focus · selection · cursor node · reveal
├── 3. the state: one session, owned by the extension
│   ├── the shape and the reducer (shared/session.ts)
│   ├── intents up, state down
│   └── what stays in the webview
├── 4. the click rules (who opens what)
│   └── a sidebar click never covers the canvas
├── 5. showing it: what "selected" looks like, on every surface
├── 6. the focus stack as its own resizable view
├── 7. the menus, entry by entry
│   ├── code editor ▸ V-NOC
│   ├── Explorer ▸ V-NOC
│   ├── sidebar item · sidebar title · focus view
│   ├── canvas card · canvas background · crumb
│   └── context keys and `when` clauses
├── 8. build order
└── 9. what this changes in 01-state and 02-design
```

## 1. What is wrong today

| # | Problem | Where |
|---|---|---|
| P1 | **The selected card looks like every other card.** `NodeCard` does not read React Flow's `selected` prop and `node-card.css` has no rule for it. The state exists; nothing draws it. | `webview/src/node-card/NodeCard.tsx:13`, `node-card.css` |
| P2 | **A sidebar click always opens the code editor**, also when you are looking at the canvas. The editor tab covers the canvas and you lose your place. | `extension/src/structureView.ts:90` (`item.command = vscode.open`) |
| P3 | **Selection lives in one `useState` in the canvas.** The sidebar does not know it, the docs tab does not know it, it is not saved, and it is gone after a window reload. | `webview/src/App.tsx:30` |
| P4 | **The focus stack has two writers.** The extension owns it (`FocusState`), the canvas keeps a copy, changes the copy first and then reports it, and both sides `prune` on their own. It works because an echo is ignored, but every new feature (select on focus, reveal) adds another effect that reads and writes the copy. | `App.tsx:26,95–106,126`, `extension.ts:123` |
| P5 | **"Show on canvas" for a closed canvas is one global slot** (`pendingReveal`). Two quick commands: the first is lost. | `extension.ts:31` |
| P6 | **The canvas does not show where the editor's cursor is.** With code and canvas side by side there is no "you are here". | — |
| P7 | **Menus are incomplete and uneven.** No "Reveal in V-NOC sidebar" anywhere. The sidebar has Focus but no "Show on canvas". The card menu cannot expand, show code or reveal in the sidebar. The canvas background has no menu. The Explorer reuses the editor's submenu, so a folder is offered "Add doc" and gets an error. Entries inside the V-NOC submenu read "V-NOC: Focus on canvas". | `extension/package.json:87–231` |
| P8 | **The messages are still untyped strings** (`shared/messages.ts` is empty; `request(method: "structure" \| …)` and an `if` chain in `panels.ts`). 01-state §3 asked for the typed map; this work touches every message, so it is the moment to do it. | `shared/messages.ts`, `webview/src/host.ts:24`, `panels.ts:102` |

## 2. Four words

The web version has `selectedNode`, `secondarySelectedNode`, `highlightedNode`, `focusedNode`, `focusTargetId` and `activeNodeId`, each a map per tab,
and `setSelectedNode` quietly clears two of the others. Keep the idea (pointing at a node is not the same as scoping to it) and drop the rest:

| Word | Meaning | How many | Who changes it | Changes what is drawn? |
|---|---|---|---|---|
| **focus** | "show only this node and what is under it". A stack: a history you can go back through. | a stack | Focus commands, double-click, Back, Clear, a crumb | **yes** (canvas and sidebar are narrowed) |
| **selection** | "the node I am working with". Menus, the doc panel and the key bindings act on it. | one or none | a click on a card or a sidebar item · any Focus (the focused node becomes selected) · Show on canvas | no |
| **cursor node** | the function / class / file the text cursor is in, in the active code editor | one or none | moving the cursor (nobody "sets" it) | no; it is only **marked** |
| **reveal** | a one-time request: "make this node visible and bring it to the middle" | an event, not state | Show on canvas, a sidebar click while the canvas is visible | it may open ancestors and step the focus back |

Three rules keep them apart:

1. **Focus sets selection; selection never sets focus.** Clicking around inside a focus does not move the crumbs.
2. **The cursor node never changes the selection** and never changes what the doc panel shows on the canvas. It is a mark, like the web version's
   `highlightedNode`. (The docs tab beside an editor already follows the cursor; that stays its own rule.)
3. **A reveal is consumed once.** It carries a number, so the same node can be revealed twice.

## 3. The state: one session, owned by the extension

Selection has to be seen by three things that cannot share memory: the sidebar (extension host), the canvas (webview) and the docs tab (another
webview). The focus stack is already owned by the extension for that reason. Selection moves to the same place, and the two become one object.

### The shape and the reducer

```ts
// shared/session.ts — pure, no vscode, no React; tested with `node --test` like focusStack.ts

export interface Session {
  focus: readonly string[];          // node keys; last = the node on screen
  selected: string | null;
  cursor: string | null;             // the node at the editor's cursor; not saved
  reveal: { id: string; n: number } | null;   // the last reveal request; `n` grows by one each time
  seq: number;                       // grows on every change; a webview drops a state older than the one it shows
}

export type Intent =
  | { type: "select"; id: string | null }
  | { type: "focus"; id: string }                 // push (or jump back, as `push` does today) + select
  | { type: "focusBack" }
  | { type: "focusJump"; depth: number }          // a crumb; 0 = clear
  | { type: "reveal"; id: string }                // step the focus back to the nearest entry that contains id, select, bump reveal.n
  | { type: "cursor"; id: string | null }
  | { type: "treeChanged" };                      // prune: drop focus entries, selection and cursor whose node is gone

export function reduce(s: Session, intent: Intent, tree: TreeNode): Session { … }
```

- `reduce` calls the existing `push` / `pop` / `jump` / `prune`; they stay as they are, with their tests.
- The "step the focus back" half of `webview/src/canvas/reveal.ts` moves into `reduce` (it needs only the tree). The "which ancestors to open" half stays
  in the webview, because `expanded` is view state.
- `selected` after Back / a crumb: keep it if it is still inside the new focus, else select the new top. Selection is never left pointing at a node
  that is not drawn.

### Intents up, state down

```
canvas click ──► post { intent: select }  ─┐
sidebar click ─► vnoc.select command ──────┤
editor menu ───► vnoc.focus command ───────┼─► session.dispatch(intent)
cursor moves ──► (debounced) cursor ───────┘        │  reduce → save → set context keys
                                                    ├─► sidebar: re-root, reveal + select the item, update the FOCUS view
                                                    ├─► canvas webview:  { event: "session", session }
                                                    └─► docs webview:    { event: "session", session }   (it uses `cursor` / `selected`)
```

```ts
// extension/src/session.ts
export class SessionStore {
  private state: Session;                               // restored from workspaceState (focus, selected)
  readonly onChange: vscode.Event<Session>;
  dispatch(intent: Intent): void {
    const next = reduce(this.state, intent, this.tree());
    if (next === this.state) return;                    // reduce returns the same object when nothing changed
    this.state = next; this.save(); this.emitter.fire(next);
  }
}
```

**The webview keeps no copy it can change.** It shows the last `session` it was sent and posts intents. This removes `focusLoaded`, the
`focusChanged` echo, the second `prune`, `pendingReveal` (a canvas that starts asks for the session and finds `reveal` in it) and the effect in
`App.tsx` that reads `focus` to compute a reveal and writes `focus` back.

> **🧩 VS Code note: is the round trip fast enough for a click?** A `postMessage` to the extension host and back is well under a frame on a
> warm window **[measure]**. If a selection ring ever feels late, let the webview draw the ring at once from its own click and correct it when
> the state arrives; the extension stays the owner. Do not start with that.

### What stays in the webview

```
webview state
├── DATA      structure snapshot, docs            (a copy; replaced as a whole)             01-state §4
├── SESSION   focus · selected · cursor · reveal  (a copy; replaced as a whole; read-only)  ← new
└── VIEW      expanded · codeOpen · docsOpen · doc panel width · viewport                   (owned here; saved with setState)
```

This is the moment to add the `zustand` stores 01-state §4 recommends (zustand is not a dependency yet): one store per group above. Cards then
subscribe to exactly what they draw:

```tsx
const selected = useSession((s) => s.selected === node.id);     // only two cards re-render on a click: the old and the new
const isCursor = useSession((s) => s.cursor === node.id);
```

Today a click runs `setNodes(current.map(…))` over every card to flip React Flow's `selected` flag (`Canvas.tsx:46`). With the store, React Flow's
own selection is switched off (`elementsSelectable={false}`) and the card draws its ring itself. `CanvasActions` (a context that only passes
callbacks down) goes away: a card calls `dispatch({ type: "select", id })` and the view store's `toggle(id)` directly.

## 4. The click rules

| Where | Gesture | Result |
|---|---|---|
| canvas card | click | **select** (nothing else; the code editor is never opened by a click on the canvas) |
| canvas card | double-click · `⌖` · Enter | **focus** (and select) |
| canvas background | click · Esc | clear the selection |
| canvas card | `↗` in the footer · menu "Go to code" · `Ctrl/Cmd`+click | go to code, in the column **beside** the canvas |
| crumb | click | jump the focus back to it |
| sidebar item | click | **select**, then see below |
| sidebar item | inline `⌖` | focus on canvas (opens the canvas) |
| sidebar item | inline `↗` | go to code, always |

### A sidebar click never covers the canvas

```
click a sidebar item
  1. dispatch select
  2. is the canvas the tab you are looking at?   (panel.visible)
       yes ─► dispatch reveal: the canvas opens its ancestors, centres it, flashes it.
              The code editor is NOT opened in the canvas's column.
              If a code editor is visible in ANOTHER column (canvas and code side by side),
              the file opens there with preserveFocus: both views follow the click.
       no  ─► open the code at the definition, as today.
```

A folder has no code: when the canvas is not visible, a folder click only selects and expands.

```ts
// structureView.ts: one command instead of vscode.open, so the rule lives in one place
item.command = { command: "vnoc.select", title: "Select", arguments: [node] };
```

A setting for people who want one behaviour always:

```jsonc
"vnoc.sidebar.click": { "enum": ["auto", "code", "canvas"], "default": "auto" }
//  auto   = the rule above        code = always open the code        canvas = always show on canvas (opens it if closed)
```

> **🧩 VS Code note: `visible`, not `active`.** `WebviewPanel.visible` is true while the tab is the one shown in its editor group; `active` is true
> only while it has keyboard focus, and clicking the sidebar takes focus away, so `active` is already false when the command runs. To open a file
> without covering the canvas, pass the column of a visible text editor: `showTextDocument(uri, { viewColumn: other.viewColumn, preserveFocus: true })`.
> `onDidChangeViewState` reports both flags; set the context key `vnoc.canvasVisible` there.

Selecting on the canvas goes the other way too: the sidebar item is revealed and selected with
`treeView.reveal(node, { select: true, focus: false, expand: false })`, only while the Structure view is visible. Do not listen to
`treeView.onDidChangeSelection` to produce intents: `reveal` fires it, and the two would chase each other. The command on the item is the only way in.

## 5. Showing it

"Hard to know which is selected" is P1. One visual language, used the same way on every surface, colours only from the theme:

| State | Canvas card | Sidebar | Elsewhere |
|---|---|---|---|
| **selected** | 2 px ring in `--vscode-focusBorder`, header filled with `--vscode-list-activeSelectionBackground` (text in `…activeSelectionForeground`), drawn above its neighbours | the native selected row (via `reveal`) | a **selection strip** in the canvas bar (below); its dot in the minimap is `focusBorder` |
| path to the selection | the edges from the view's root down to the selected card are 2 px and `focusBorder`; the rest stay thin | — | — |
| selection hidden inside a closed card | the nearest drawn ancestor shows a small filled dot beside its `+n` badge ("the selection is in here") | — | the strip still names it; its **Show** button reveals it |
| **focused** (top of the stack) | the root card of the view has a `⌖ focus` chip in its header | the tree starts at it; the view's description says `Focus: a › b` (built) | the last crumb, bold (built) |
| **cursor node** | a 1 px **dashed** outline in `--vscode-editorCursor-foreground` and a small `▌` mark in the header; tooltip "the cursor is here" | — | — |
| **just revealed** | the ring pulses twice (about 1 s), then stays as the normal selected ring | — | — |
| hover | border in `--vscode-list-hoverBackground`; the footer buttons appear | native | — |
| syntax error | the red outline (built) moves **inside** the card so it can sit together with the ring | `syntax error` (built) | — |

Selected and cursor can be on the same card: solid ring outside, dashed line inside. Nothing uses colour alone: selected also has the filled
header, cursor also has the mark, so it holds in high-contrast themes. The pulse is skipped under `prefers-reduced-motion`.

```css
/* node-card.css — the ring is an outline, so the card's size and the layout constants do not change */
.node-card.selected          { outline: 2px solid var(--vscode-focusBorder); outline-offset: 1px; }
.node-card.selected .node-header { background: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }
.node-card.cursor            { box-shadow: inset 0 0 0 1px var(--vscode-editorCursor-foreground); /* dashed via a ::after border */ }
.node-card.revealed          { animation: vnoc-pulse .5s ease-out 2; }
@media (prefers-reduced-motion: reduce) { .node-card.revealed { animation: none; } }
```

The selection strip, so the answer to "what is selected?" is always on screen, also when the card is scrolled away:

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ V-NOC  my-project · 87 files · 430 functions                              ⟳   ⓘ  │
│ ←  All code › billing › Invoice                                                  │  crumbs = focus
│ ƒ Invoice.total · src/billing/invoice.py:42          Show   ⌖ Focus   ↗ Code   ✕ │  strip = selection (hidden when nothing is selected)
├──────────────────────────────────────────────────────────────────────────────────┤
```

`Show` centres the card (a reveal). `✕` clears the selection. The strip has the same right-click menu as the card.

Keys, while the canvas has keyboard focus:

| Key | Does |
|---|---|
| `↑` `↓` | select the previous / next card in the column order |
| `←` `→` | select the parent · open the card and select its first child |
| `Enter` | focus the selection |
| `Space` | open / close the selection's children |
| `Alt+←` | focus back (built) |
| `Esc` | clear the selection |

A selection moved by a key is scrolled into view only if it left the screen (no re-centring on every arrow).

## 6. The focus stack as its own resizable view

The web version stacks one tree per context in resizable panels (`TabContextStack`). In VS Code that already exists: **every view in a view
container is a section the user can resize, collapse and move**. So the focus stack becomes a second native view above Structure, and nothing has
to be drawn or resized by hand.

```
V-NOC
├─ FOCUS                                  ←  ✕      ← title buttons: Back · Clear      (the view is hidden while the stack is empty)
│    All code
│    📁 billing
│  ▸ ◆ Invoice                            ⌖ here    ← the top: what the canvas and the tree below show
├───────────────────────── drag to resize ─────────
├─ STRUCTURE                              ⟳
│  ▾ ◆ Invoice
│      ƒ total            📄2
│      ƒ tax
└─ MISSING (2)
```

| Element | Behaviour |
|---|---|
| a row | click: jump the focus back to it (`focusJump`). The last row is the current one. |
| row inline `✕` | remove from this entry up (the same as clicking the row above it) |
| "All code" | clear |
| view `when` | `vnoc.focused` (the key exists) |
| the canvas crumbs | unchanged; the same stack, drawn in two places from the one session |

Structure keeps starting at the focused node, as built. (02-design said the tree should not hide the rest; the code went the web version's way and
the FOCUS view is now the way out, so 02-design is the one to change: §9.)

Inside the canvas, the doc panel is already resizable by its left edge (`DocPanel.tsx:75`), but its width is a `useState(380)`: move it into the view
store so it is saved with `expanded` and `docsOpen`.

## 7. The menus

Rules for every menu:

- One command per action, the same command in every menu. Each command starts with `nodeFor(arg)` (built), which turns a tree item, a card, a Uri
  or the cursor into a node.
- Commands get `"category": "V-NOC"` and a short `title`. The palette then shows "V-NOC: Focus on Canvas" and the submenu shows "Focus on Canvas".
- An entry that cannot work is **hidden** by its `when`, not shown and answered with an error.
- Groups in the same order everywhere: `1_show` (look at it) · `2_code` · `3_docs` · `4_view` (expand, code in card) · `9_copy`.

### Code editor ▸ V-NOC

```
(right-click in a .py file)
  V-NOC ▸  Show on Canvas          reveal + select the node at the cursor. The focus is not pushed.      vnoc.showOnCanvas
           Focus on Canvas         push it on the focus stack: canvas and sidebar show only it.          vnoc.focus
           Reveal in Sidebar       open the V-NOC view, select the node in Structure. No canvas.         vnoc.revealInSidebar   NEW
           ─────────
           Open Canvas             bring the canvas up as it was. Selection and focus do not change.     vnoc.openCanvas
           Open Canvas to the Side the same, in the column beside this editor                            vnoc.openCanvasBeside  NEW
           ─────────
           Show Docs               the docs tab beside this editor                                       vnoc.showDocs
           Add Doc…                only when the cursor is in a function or class                        vnoc.addDoc
```

"The node at the cursor" is unchanged (`chainAt`). Because the session now holds `cursor`, the extension knows its kind before the menu opens and
sets `vnoc.cursorKind` (`function` · `class` · `file` · none); "Add Doc…" uses it in its `when`.

"Reveal in Sidebar" when the focus hides the node: the same rule as Show on canvas (step the focus back to the nearest entry that contains it).

### Explorer ▸ V-NOC  (its own submenu, `vnoc.explorerMenu`)

```
(right-click a .py file or a folder)
  V-NOC ▸  Show on Canvas · Focus on Canvas · Reveal in Sidebar
           Show Docs                                                    (files and folders can take docs since Step 4; their Add Doc… entry here comes with the menu rework)
```

### Sidebar: Structure item

| Entry | Group | `when` (besides `view == vnoc.structure`) |
|---|---|---|
| inline `⌖` Focus on Canvas | `inline@1` | — |
| inline `↗` Go to Code | `inline@2` | `viewItem != folder` |
| inline `＋` Add Doc… | `inline@3` | `viewItem =~ /function\|class/` |
| Show on Canvas | `1_show@1` | — |
| Focus on Canvas | `1_show@2` | — |
| Go to Code | `2_code@1` | `viewItem != folder` |
| Reveal in Explorer | `2_code@2` | `viewItem =~ /folder\|file/` |
| Show Docs | `3_docs@1` | — |
| Add Doc… | `3_docs@2` | `viewItem =~ /function\|class/` |
| Copy Path | `9_copy@1` | — |

`contextValue` stays the kind. Title bar: Refresh · Collapse All (`"showCollapseAll": true` on the view, free from VS Code).

### Sidebar: Focus view

Title: Back · Clear. Item: inline `✕`; menu "Jump Back to Here", "Show on Canvas", "Go to Code".

### Canvas card

VS Code draws this menu (`webview/context`); the card describes itself in `data-vscode-context` and `when` clauses read it. Today it sends
`webviewSection`, `nodeKey`, `kind`. Add what the entries need:

```tsx
data-vscode-context={JSON.stringify({
  webviewSection: "node", nodeKey: data.id, kind: data.kind, preventDefaultContextMenuItems: true,
  hasChildren: data.childCount > 0, collapsed: data.collapsed,
  hasCode: data.range !== null, codeOpen: data.codeOpen,
  docs: data.docs, isFocusRoot: data.depth === 0 && focusDepth > 0,
})}
```

| Entry | Group | `when` (besides `webviewId == 'vnoc.canvas' && webviewSection == 'node'`) |
|---|---|---|
| Focus | `1_show@1` | `!isFocusRoot` |
| Focus Back | `1_show@2` | `isFocusRoot` |
| Reveal in Sidebar | `1_show@3` | — |
| Go to Code | `2_code@1` | `kind != folder` |
| Show Code in Card / Hide Code in Card | `2_code@2` | `hasCode && !codeOpen` / `hasCode && codeOpen` |
| Show Docs | `3_docs@1` | — |
| Add Doc… | `3_docs@2` | `kind == function \|\| kind == class` |
| Expand / Collapse | `4_view@1` | `hasChildren && collapsed` / `hasChildren && !collapsed` |
| Expand All Below | `4_view@2` | `hasChildren` |
| Copy Path | `9_copy@1` | — |

A right-click **selects the card first** (a `contextmenu` listener posts `select`), so the ring shows what the menu is about, as in the Explorer.
Entries that change view state (expand, code in card) are commands in the extension that post an event to the canvas; the canvas changes its own
view store. That is the one place the extension asks the webview to change view state, and it only forwards the user's click.

### Canvas background and crumb

| Section | Entries |
|---|---|
| `webviewSection == 'pane'` | Fit View · Collapse All · Clear Selection (`vnoc.hasSelection`) · Focus Back / Clear Focus (`vnoc.focused`) · Refresh |
| `webviewSection == 'crumb'` | Jump Back to Here · Show on Canvas · Go to Code |

### Context keys

| Key | Set by | Used for |
|---|---|---|
| `vnoc.focused` | session (built) | Back / Clear, the Focus view |
| `vnoc.hasSelection` | session | Clear Selection, key bindings |
| `vnoc.canvasOpen` · `vnoc.canvasVisible` | `Panels` (`onDidChangeViewState`, dispose) | the sidebar click rule, "Open Canvas" vs nothing to do |
| `vnoc.cursorKind` | session, from `cursor` | "Add Doc…" in the editor menu |

Key bindings (`contributes.keybindings`, `when: activeWebviewPanelId == 'vnoc.canvas'`): `Alt+←` Focus Back, so it works even when the webview
does not have DOM focus, and the listener in `App.tsx` can go.

## 8. Build order

Each step leaves a working extension. 1–3 are refactors with no visible change except the ring; 4–7 are the features.

| Step | What | Proof |
|---|---|---|
| **1. See it** | `.node-card.selected` ring + header fill; pass `selected` into `NodeCard`. Half an hour, and it fixes the loudest complaint (P1) before anything else moves. | click a card: it is obvious which one |
| **2. Typed messages** | `shared/messages.ts`: `TreeNode`, `Requests`, `Events`, `Intent`. `host.ts` and `panels.ts` use the map; the `if` chain becomes a handler table (01-state §3). | `tsc` fails on a misspelt method |
| **3. The session** | `shared/session.ts` (`reduce` + tests: focus selects, select does not focus, reveal steps back, prune, Back keeps or moves the selection). `extension/src/session.ts` replaces `FocusState`, `setFocus`, `pendingReveal`. The webview gets `session` events and posts intents; delete its focus copy and the echo. | unit tests; reload the window with a focus and a selection: both come back |
| **4. Webview stores** | add `zustand`; `state/data.ts`, `state/session.ts`, `state/view.ts` (expanded, codeOpen, docsOpen, panel width, saved in one subscriber). Cards subscribe; remove `CanvasActions`, `elementsSelectable={false}`. | React DevTools: a click re-renders two cards |
| **5. Click rules** | `vnoc.select` on sidebar items with the "never cover the canvas" rule and the `vnoc.sidebar.click` setting; canvas → sidebar `reveal`; `vnoc.canvasVisible`. | with the canvas up, click through the sidebar: the canvas stays, the card is centred and flashes |
| **6. The rest of the look** | cursor node (reuse `followCursor`'s debounce to dispatch `cursor`), reveal pulse, path edges, "selection is in here" dot, selection strip, arrow keys. | code and canvas side by side: move the cursor, the mark follows |
| **7. Menus** | `category` + short titles; `vnoc.revealInSidebar`, `vnoc.openCanvasBeside`; `vnoc.explorerMenu`; the tables of §7; richer `data-vscode-context`; pane and crumb menus; right-click selects. | walk every table in §7 once and tick each row |
| **8. Focus view** | `vnoc.focusStack` tree view fed by the session. | focus three deep, drag the divider, click the middle row |

Tests to add to the extension suite (it already drives the real window): sidebar `vnoc.select` with the canvas visible leaves
`window.activeTextEditor` unchanged; `vnoc.showOnCanvas` twice in a row before the canvas is up reveals the second node (P5).

## 9. What this changes in 01-state and 02-design

| Document | Was | Becomes |
|---|---|---|
| 01-state §1, §6 rule 1 | "The webview is the only truth for view state. The extension holds neither." | The extension owns the **session** (focus, selection, cursor node): the state more than one surface shows. The webview owns the rest of the view state. The code already does this for focus. |
| 01-state §4 | `focus`, `selected` in the VIEW slice | they move to a read-only SESSION slice |
| 01-state §3 | `Events: indexChanged, focus` · `Requests: reveal, pendingFocus` | `Events: indexChanged, session, viewCommand` · the webview posts `Intent`s; `pendingFocus` / `pending` go away |
| 02-design §2 | "A click keeps jumping to the code." | A click selects; it opens the code only when that does not cover the canvas (§4 here). |
| 02-design §3 focus rules | "The sidebar tree does not hide the rest of the tree." | It starts at the focused node (as built); the FOCUS view is the way out. |
| 02-design §2 menu | four entries, shared with the Explorer | the tables in §7 |

Not in this plan: several canvases with a session each (the web version's tabs), multi-select, and a secondary selection for call nodes. The
`Session` shape can grow a key per canvas when a second canvas exists.

Next: [`../08-decisions-to-log.md`](../08-decisions-to-log.md)
