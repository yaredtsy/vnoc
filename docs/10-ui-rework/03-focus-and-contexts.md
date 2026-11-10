# 10.03 Focus, the focus stack, and contexts

```
focus and contexts
├── 1. what focus means (and the three words next to it)
├── 2. the rules of the stack
├── 3. how each surface shows it
├── 4. every way to change it
├── 5. contexts: MAIN now, EXPLORE in 0.2
└── 6. differences from the web version, on purpose
```

## 1. What focus means

**Focus a node, and V-NOC shows only that node and what is under it.** On the canvas *and* in the sidebar, at the same time, because both draw the
same `contexts[id].focus`. Each focus is added to a stack, so you can go back one step or clear it all.

Four other words sit next to it. Three are from `07-ui/03` §2; **highlight** is new (the web version's `highlightedNode`). How each one looks:
[`06-how-it-looks.md`](06-how-it-looks.md).

| Word | Meaning | Changes what is drawn? |
|---|---|---|
| **focus** | "show only this and what is under it". A stack: a history you can walk back. | **yes**, on both surfaces |
| **selection** | "the node I am working with". Menus, the doc panel and keys act on it. One or none. | no; the card gets the ring, the row VS Code's selection |
| **highlight** | "look here" without selecting: the search list's active item, a hovered crumb. Menus and the doc panel do not follow it. | no; the card gets a dotted outline |
| **cursor node** | the function / class / file the text cursor is in, in the code editor | no; it gets a mark |
| **reveal** | a one-time request: "open what hides this node and bring it into view" | it may open ancestors and step the focus back |

1. **Focus sets selection; selection never sets focus.** Clicking around inside a focus does not move the crumbs.
2. **The cursor node never changes the selection.**
3. **A reveal is consumed once**: the canvas reports it done and the store clears it.
4. **Selecting never opens anything.** A click may *also* run a command that opens the canvas or the code (`02-sidebar.md` §4); the selection itself
   is just a value.

## 2. The rules of the stack

All of these are one place in the code: `reduce` in `shared/session.ts`, calling `push` / `pop` / `jump` / `prune` from `shared/focusStack.ts`
(which exist, with tests).

| Rule | Example |
|---|---|
| The stack is a **history, not a path**. | `[billing, Invoice.total]` means "I was on billing, then went to Invoice.total", whether or not one contains the other. |
| A focus **appends**, from wherever it comes. | focused on `billing`, then "Focus on Canvas" from the editor on `parse_order` → `[billing, parse_order]` |
| Focusing what is **already on top** does nothing. | double-click the root card of the view |
| Focusing a node that is **lower in the stack** jumps back to it. | `[a, b, c]`, focus `a` → `[a]`. (The web version would give `[a, b, c, a]`.) |
| **Back** removes the top. **Clear** empties the stack. A **crumb** or a FOCUS-view row keeps everything up to it. | |
| A focus **selects** the focused node. | so an open doc panel shows its docs |
| After Back / a crumb, the selection is kept if it is still under the new top, else the new top is selected. | the ring is never on a node that is not drawn |
| The focused node is drawn **open**, whatever `expanded` says. | otherwise there would be nothing to see |
| After the code changes, entries whose node is gone are dropped **from the top** until one exists. | you deleted the function you were focused on → you are one step back, not looking at nothing |
| **Reveal** steps the focus back to the newest entry that contains the node, or clears it. It never pushes. | "Show on Canvas" for a node outside the current focus |
| The stack is saved per workspace. | reload the window: same focus |

## 3. How each surface shows it

One stack, drawn in three places. Each is a pure drawing of `contexts[active].focus`; none keeps its own.

```
SIDEBAR                                          CANVAS
┌ FOCUS ─────────────────────── ← ✕ ┐            ┌───────────────────────────────────────┐
│   All code                        │            │ ←  All code › app › checkout          │
│   📁 app                          │            ├───────────────────────────────────────┤
│   📄 checkout              ●      │            │  ┌──────────────┐   ┌───────────────┐ │
├ STRUCTURE ────────────── 🔍 ⟳ ⊟ ─┤            │  │ 📄 checkout ⌖├───┤ ƒ checkout_…  │ │
│ ⌖ Focus: app › checkout    ← ✕    │            │  └──────────────┘   └───────────────┘ │
│ ▾ 📄 checkout                     │            └───────────────────────────────────────┘
│     ƒ checkout_order              │
└───────────────────────────────────┘
```

| Element | Sidebar | Canvas |
|---|---|---|
| the stack | the **FOCUS view**, one row per step (resizable) · the **focus bar row** at the top of STRUCTURE, one line | **crumbs** (built): `All code › app › checkout`; the last one is bold and not clickable |
| Back | `←` on the focus bar row and in the FOCUS title | `←` in the crumb bar · `Alt+←` |
| Clear | `✕` on the focus bar row and in the FOCUS title | the `All code` crumb |
| jump back | a row in the FOCUS view | a crumb |
| no focus | no FOCUS view, no focus bar row; the tree starts at the project | no crumb bar |
| the focused node | the first node row of the tree, open | the root card of the view, with a `⌖` chip in its header |

The docs tab does not show the focus. It follows the code (`01-state.md` §2).

## 4. Every way to change it

| Gesture | Where | Intent |
|---|---|---|
| `⌖` · `Ctrl/Cmd+Enter` | a sidebar row | `focus` |
| double-click · `⌖` · Enter | a canvas card | `focus` |
| menu **Focus** | a row, a card ([`04-menus.md`](04-menus.md) §2) | `focus` |
| menu **Focus on Canvas** | the code editor, an editor tab, the Explorer | `focus`, and the canvas is shown |
| `←` · `Alt+←` · palette "Focus Back" | focus bar row, FOCUS view, canvas, anywhere | `focusBack` |
| `✕` · `All code` · palette "Clear Focus" | focus bar row, FOCUS view, canvas, anywhere | `focusJump 0` |
| a FOCUS-view row · a crumb | sidebar, canvas | `focusJump n` |
| **Show on Canvas** / **Reveal in V-NOC Sidebar** / a search pick, for a node outside the focus | any menu, the search | `reveal` (steps back, never pushes) |
| the focused node is deleted or renamed | the core reports a new index | `treeChanged` |

There is no gesture that changes the focus on one surface only. That is the point.

## 5. Contexts

In your third screenshot of the web version the sidebar has two panels: **MAIN**, and **EXPLORE: CHECKOUT_ORDER** under it, each with its own focus
bar and tree. In the web code that is a *tab* (`useTabStore`): selecting a **call node** opens a child tab focused on the function being called, and
every piece of state (`focusStack`, `selectedNode`, `expandedNodeIds`) is a map from tab id to value.

V-NOC for VS Code has no call nodes until 0.2, so there is nothing to explore yet. What U3 buys now is the **shape**:

```ts
session.contexts = { main: { id: "main", title: "Main", parent: null, focus: [...], selected: ..., expanded: [...] } }
session.order    = ["main"]
session.active   = "main"
```

Every intent already names its context, every component already reads `contexts[context]`. In 0.1 that argument is always `"main"`.

### What 0.1 builds

- the `Session` shape above, the `context` argument on `dispatch` and on the `intent` message
- `data-vscode-context` on cards carries `contextId`; tree rows carry it in their element; so a menu command knows which context it was opened in

### What 0.2 adds (with call trees)

| Piece | Addition |
|---|---|
| intents | `openContext { from: ContextId, call: string }` · `closeContext { id }` (closes its children too, like `destroyTabBranch`) · `activate { id }` |
| reducer | opening a context for a call: title `Explore: <callee>`, `parent` = the context you were in, focus = the callee's lineage (folder › file › function), selected = the callee. Selecting a different call in the parent closes the child that belonged to the previous call. |
| sidebar (native) | an **EXPLORE** view under STRUCTURE (`"when": "vnoc.exploring"`): its top rows are the open contexts (`◇ checkout_order  ✕`), each opening into its own tree. Clicking a context's row activates it. |
| sidebar (webview) | if the native EXPLORE view feels cramped in use, this is the moment to build the webview sidebar with the web's stacked, resizable panels ([`05-build-order.md`](05-build-order.md) §6) |
| canvas | shows `session.active` |
| menu | "Explore in New Context" on a call node |

Left open until 0.2, because call trees decide it: **one canvas that switches to the active context, or one canvas tab per context?** My lean is
one canvas that switches. Nothing in 0.1 depends on the answer.

> **🧩 Why key it now.** Adding a `context` argument to forty call sites later is a rewrite of the state layer under working features, and the saved
> session in every user's workspace would need converting. Adding it while there is one context costs one extra argument that is always `"main"`.

## 6. Differences from the web version, on purpose

| Web | Here | Why |
|---|---|---|
| `pushFocus` always appends, so the same node can be in the stack twice | focusing a node lower in the stack jumps back to it | shorter crumbs; already built in `push` |
| the focus bar is plain text with Back and Clear | the focus bar row has Back and Clear; every step is clickable in the FOCUS view | a crumb you cannot click is half a crumb |
| `selectedNode`, `secondarySelectedNode`, `highlightedNode`, `focusedNode`, `focusTargetId`, `activeNodeId` | `selected`, `highlight`, `cursor`, `reveal` (and `focus`) | six fields, and `setSelectedNode` quietly clears two others; four do the job until call nodes need a second selection (0.2) |
| the stack stores whole node objects | it stores node keys | the tree is replaced on every index change; a stored object would be stale |
| open nodes are shared by the sidebar and the canvas | separate in 0.1 | a native tree cannot close a row from code (`02-sidebar.md` §8) |
| a click on a row selects, and the Code tab follows | a click selects; with the canvas open the canvas follows, else the code opens as a preview tab | VS Code's code editor is a separate tab that can cover the canvas |

Next: [`04-menus.md`](04-menus.md)
