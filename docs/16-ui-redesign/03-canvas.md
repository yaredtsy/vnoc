# 16.03 The canvas: what is drawn, one level at a time, contexts on one canvas

```
canvas
├── 1. what is drawn
├── 2. open / closed, shared with the sidebar (R8)
├── 3. the focus is a path (R2)
├── 4. contexts: MAIN and EXPLORE (R3, R4)
├── 5. the session, after this change
├── 6. edges and layout
├── 7. keys
├── 8. the four states, controls, minimap, docs panel
└── 9. files
```

## 1. What is drawn

The canvas draws the **active context**. Its root card is the focus's last step, or the project when there is no focus. Under each **open**
card, its children, in this order:

| Card | Its children on the canvas |
|---|---|
| folder | its folders and files (as today) |
| file | its classes and functions (as today) |
| class | its methods and nested classes (as today) |
| **function** | its nested defs, **then its calls** (call cards, [`05-calls-as-nodes.md`](05-calls-as-nodes.md)) |
| **call** | the calls made by the function it calls (the next level of the same call tree) |

**Everything starts closed** except the root card, which is always open. One click opens **one level**. Nothing is ever opened for the user,
except by: a reveal (opens the ancestors of the revealed node), **Show Calls** (opens one function's calls), and **Expand All Below** (structure
only; calls are never opened in bulk, see 05 §4).

So "focus `checkout`" draws `checkout` and its direct calls, closed:

```
[ƒ checkout] ┄┄ [ƒ checkout_order ▸]
```

and opening `checkout_order` draws the next level:

```
[ƒ checkout] ┄┄ [ƒ checkout_order ▾] ┄┄ [ƒ check_stock]
                                     ┄┄ [ƒ reserve_stock ▸]
                                     ┄┄ [ƒ charge_payment ▸]
                                     ┄┄ [ƒ cancel_checkout ▸]
                                     ┄┄ [ƒ create_order ▸]
                                     ┄┄ [◆ CompletedOrder]
```

instead of today's list of all fourteen rows at once.

## 2. Open / closed, shared (R8)

`contexts[c].expanded` is **one set** for the sidebar and the canvas of that context. Opening `api.py` in the sidebar opens its card on the
canvas, and the other way round. Ids in the set are structure keys (`app/api.py::checkout`) and call ids (05 §1). Call ids, and the calls
under an open function, are drawn by the canvas only: the sidebar has no call rows (07 §2, C1).

| Gesture | Where | Intent |
|---|---|---|
| chevron on a card or a row · `Space` · `→` on a closed one | both (calls: the canvas) | `toggle` / `expand` |
| Collapse All | `⋯` on the canvas, `⊟` on the sidebar's title | `collapseAll` (the active context) |
| Expand All Below | menu, folders, files and classes | `expand` with up to 500 ids (kept from 10/04 §5) |

What changes for people used to 0.1: opening many folders in the sidebar makes the canvas bigger. That is the point of sharing: the canvas is the
sidebar drawn as a graph. Fit View (`⋯`) frames it again.

## 3. The focus is a path (R2)

`contexts[c].focus` holds the keys from the project's top level down to the focused node: `["app", "app/api.py"]`. The project root `/`
is never in it.

| Gesture | Intent | New focus | Selection |
|---|---|---|---|
| Focus on `reserve_stock` (any surface, any context) | `focus` | **its lineage**: `["app", "app/inventory.py", "app/inventory.py::reserve_stock"]` | `reserve_stock` |
| Focus on a **call** card | `explore` (§4) — focusing a call means following it | | |
| Focus on what is already the last step | nothing | unchanged | unchanged |
| `←` · `Alt+←` · Back | `focusBack` | the last step dropped: `["app", "app/inventory.py"]` | kept if still under the new last step, else the new last step (kept rule) |
| a crumb, a step in the sidebar's focus bar | `focusJump n` | the first `n` steps | the same rule |
| the project crumb · `✕` on the focus bar · Clear Focus | `focusJump 0` | `[]` | kept |
| reveal a node outside the focus (Open in Canvas, a search pick, Reveal in Sidebar) | `reveal` | steps dropped from the end until the last one contains the node; never added | the node |
| the code changed and a step's node is gone | `treeChanged` | steps dropped from the end until the last one exists | the rule of 10/03 §2, kept |

So J4 works like this, in the EXPLORE context of `reserve_stock`: `←` shows `inventory.py` (with `reserve_stock` and its siblings), `←` again shows
`app`, `←` again the whole project.

`shared/focusStack.ts`'s `push` (with its "jump back if it is lower in the stack" rule) is no longer used by `reduce`; `pop`, `jump` and `prune`
stay. A new `lineage(tree, id)` in `shared/tree.ts` gives the path (it is `pathTo` without the root).

**Saved sessions from 0.1** hold a history (`[billing, Invoice.total]`). The first `treeChanged` after start replaces each context's focus with the
lineage of its last step. A test covers it.

## 4. Contexts: MAIN and EXPLORE (R3, R4)

A context is MAIN or an EXPLORE context opened from a **call** in another context (its *parent*). They form a tree; the sidebar and the bar show
them in `session.order` (depth first: a child right after its parent).

| Gesture | In context | Result |
|---|---|---|
| **select a call** whose callee the project knows | P | P's selection = the call. P's EXPLORE children for **other** calls close (with their own children). An EXPLORE child for **this** call opens if there is none: title = the callee's name, `focus` = the callee's lineage, `selected` = the callee. **It is not activated**: the canvas stays on P. |
| select anything that is not that call | P | P's EXPLORE children close (the web's `handleNodeSelection`, R4) |
| **double-click a call · `◇` Explore · menu Explore · Enter on a call** | P | the same as selecting it, **and the child is activated**: the canvas switches to it |
| click inside a context's sidebar panel, or its chip on the bar | — | `activate` that context |
| `✕` on a panel header or a chip | — | `closeContext`: it and its children close; if the active one closed, its parent becomes active |
| select a call whose callee the project does not know (`key: null`, 05 §6) | P | selected; no EXPLORE child |

**Why closing on select is right here** (and the other choice): an EXPLORE panel answers "where does the selected call live?". When the selection
moves on, the question is gone. The other choice, panels that stay until closed, needs a close-all, a limit, and a way to tell which panel is
for which call. If testing shows people lose panels they wanted, the change is one rule in `reduce`: don't close children on `select`.

**Inside an EXPLORE context** everything works as in MAIN: focus, Back (up the callee's parents), expand, select. Selecting a call there opens a
grandchild panel under it.

**Switching** changes the canvas only: its cards, its crumbs, and the docs panel (which follows the active context's selection). Each context's
pan and zoom are remembered in the page (`Map<ContextId, Viewport>`, not saved); a context shown for the first time is fitted.

**Saved**: every context, as today's `toSaved`. A saved selection or open set that names a call is kept until the call tree of its function is
known again (05 §5).

## 5. The session, after this change

```ts
// shared/session.ts
export interface ContextState {
  id: ContextId;
  title: string;
  kind: "main" | "explore";                                // NEW
  parent: ContextId | null;
  /** the call in `parent` this context explores; null for MAIN */
  call: CallId | null;                                     // NEW
  /** node keys from the top level down to the focused node: a path (R2) */
  focus: readonly string[];
  /** a structure key or a call id */
  selected: string | null;
  /** structure keys and call ids, open on BOTH surfaces (R8) */
  expanded: readonly string[];
}

export type Intent =
  | …the ones of today, with `focus` now meaning "the lineage of"…
  | { type: "explore"; call: CallId; activate: boolean }
  | { type: "activate"; id: ContextId }
  | { type: "closeContext"; id: ContextId };

/** what reduce needs to know about calls: the callee of a call id, when its tree is known */
export type CallLookup = (call: CallId) => { key: string | null; name: string } | undefined;

export function reduce(s: Session, context: ContextId, intent: Intent, tree: TreeNode, calls: CallLookup): Session;
```

`reveal` loses its `canvas: boolean` meaning "also the sidebar": the sidebar always scrolls its selected row into view (06 §6), so `reveal` is
only for the canvas. `reveal.context` says which context; a canvas showing another context ignores it until that context is active.

## 6. Edges and layout

| Edge | Look | Why |
|---|---|---|
| structure: folder → file → def | solid, 1 px, `--vscode-editorWidget-border` | "contains" |
| **call**: function → call, call → call | **dashed** `4 3`, 1.5 px, `--vscode-charts-blue` at 70% | "calls"; the web draws them dashed too |
| on the path from the root card to the selection | either style, 2 px, `--card-selected` (kept from 10/06 §3) | where the selection sits |

The layout stays `layout.ts`'s columns (one column per depth, rows top to bottom). It gains call cards as children (§1) and loses the "Calls"
section heights (`callsHeight`, `CALLS_*`, `Placed.callRows`). Every card is 72 px high (04 §1) except with the code shown.

## 7. Keys

From 10/06 §7, kept: `↑` `↓` previous / next card, `←` the parent, `→` open or the first child, `Space` open / close, `Esc` clear the selection,
`Alt+←` Back. Changed:

| Key | On a structure card | On a call card |
|---|---|---|
| `Enter` | Focus | **Explore** (activates it) |
| `Ctrl/Cmd+Enter` | Go to Code | Go to Call Site: one site → there; several → the QuickPick of 04 §2.1 |

## 8. The four states, controls, minimap, docs panel

| State | Canvas |
|---|---|
| loading (first structure) | a centred *"Reading the code…"* and the bar's progress line |
| empty (no supported file) | *"No Python files here yet."* centred |
| ready | the cards |
| error | the message, **Try Again** (`vnoc.refreshStructure`), **Show Log** |

- **Controls** (zoom in / out / fit) stay bottom-left, VS Code button colours.
- **Minimap**: off by default, `⋯ → Show Minimap` turns it on (remembered in the page's `vscode.setState`). Today it is always on and draws a big
  grey block on small graphs (your first screenshot).
- **Docs panel** (`ⓘ`): unchanged inside; it follows the **active context's** selection, and for a selected call shows the callee's docs with a
  line on top: *"called by checkout_order · line 16"*. "Unchanged" is today's `webview/src/docs/DocPanel.tsx` (07-ui/02 §3), all of it:

  | Part | Keeps |
  |---|---|
  | header | the node's name and `path:line` · the **doc picker** (this node's docs by title, last entry **New Doc…**) · **✎ Edit** (the `.md` in a tab beside the canvas) · **✕** |
  | body | **one** doc, the picked one, **rendered** Markdown (raw HTML off, links through the host), live while it is typed in its tab |
  | empty | "No docs on `<name>` yet." and **Write the First Doc** · nothing selected: "Select a node to see its docs." |
  | width | the left edge drags, 280–800 px, kept in `setState` |

  Not a list of every doc as boxes with a title and a summary: that was the mockup's stand-in before 2026-11-12, not a design.
  0.3 adds editing inside the panel ([`../17-edit-in-canvas.md`](../17-edit-in-canvas.md) §8).

## 9. Files

| File | Change |
|---|---|
| `shared/session.ts`, `shared/session.test.ts` | §3, §4, §5 |
| `shared/tree.ts` | `lineage` |
| `shared/calls.ts` (new) | call ids and lookups, 05 §1 |
| `webview/src/canvas/layout.ts`, `layout.test.ts` | call cards, no Calls section, 72 px cards |
| `webview/src/canvas/Canvas.tsx` | dashed call edges, per-context viewport, minimap toggle |
| `webview/src/canvas/keys.ts` | `Enter` / `Ctrl+Enter` on a call |
| `webview/src/App.tsx` | the top bar (02), no `SelectionStrip`, the docs panel follows the active context |

Next: [`04-node-cards.md`](04-node-cards.md)
