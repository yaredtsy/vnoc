# 10. UI rework: one state, the native tree with a focus bar, matching menus

Written 2026-11-10 after reading `extension/src/`, `webview/src/`, `shared/` (commit `11b99ce`), the files in [`../07-ui/`](../07-ui/), and the web
version's Dashboard (`ide/v-noc/src/frontend/src/features/Dashboard/`: store slices, Sidebar, `NodeContextMenu`, `useNodeHandlers`).

**Revised the same day.** The first version replaced the sidebar with a webview. You chose to keep VS Code's native tree for now and to move the
webview sidebar to a later release (U1). The fixes from the review of the first version are in too (§7).

```
ui rework
├── README (this file)        what is wrong · what you decided · what is kept from 07-ui
├── 01-state.md               one session in the extension · intents up, state down · typed messages
├── 02-sidebar.md             the native tree: focus bar, FOCUS view, what a click does, search
├── 03-focus-and-contexts.md  focus rules on every surface · contexts (MAIN now, EXPLORE in 0.2)
├── 04-menus.md               the node menu on the row and on the card · the V-NOC submenus
├── 05-build-order.md         where each file goes · the steps, each with a proof · the webview sidebar later
└── 06-how-it-looks.md        selected · highlighted · cursor · revealed · focused, on the card and in the tree
```

## 1. What is wrong today

| Problem | What the code does | Where |
|---|---|---|
| state management | The plan in `07-ui/01-state.md` was never built. `shared/messages.ts` is an empty file. `TreeNode` is typed by hand in three places. Message names are strings checked in a `switch` and an `if` chain. `App.tsx` holds nine `useState` values and nine effects that read and write each other. | `shared/messages.ts` · `extension/src/structureView.ts:10` · `webview/src/types.ts:5` · `extension/src/extension.ts:342` · `extension/src/panels.ts:102` · `webview/src/App.tsx` |
| focus and the focus stack | The stack has **two writers**. The extension owns it (`FocusState`), the canvas keeps its own copy, changes the copy first, then reports it, and both sides `prune` on their own. It works only because an echo is ignored. | `extension/src/focus.ts` · `extension/src/extension.ts:123` · `webview/src/App.tsx:26,95–106,126` |
| the sidebar does not show the focus like the web | The tree does start at the focused node (since `11b99ce`), but the stack is only text in the view's title line, with Back and Clear as two small title icons. | `extension/src/extension.ts:135–140` |
| which node is selected | The selected card looks like every other card: `NodeCard` never reads React Flow's `selected` and `node-card.css` has no rule for it. Selection lives in one `useState` in the canvas; the sidebar never shows it, and it is lost on reload. | `webview/src/node-card/NodeCard.tsx:13` · `webview/src/App.tsx:30` |
| a sidebar click covers the canvas | Every row runs `vscode.open`, so a click with the canvas in front opens a code tab over it. | `extension/src/structureView.ts:90` |
| the menus | Two hand-written lists (`view/item/context` for the row, `webview/context` for the card): different order, different entries, neither has "show me this on the other surface". The **V-NOC** submenu exists for the editor and the Explorer, but only for Python, it has no "Reveal in V-NOC Sidebar", and the Explorer reuses the editor's list (a folder is offered "Add doc" and answers with an error). | `extension/package.json` (`menus`) |
| the submenu "is not there" | `vnoc-darwin-arm64.vsix` in the repo root was built at 16:50; the first commit with these menus is 16:52. An installed copy of it shows none of them. **Not run yet**: step 0 of the build order checks a fresh build. | repo root |

## 2. What you decided

| # | Question | Answer | Details |
|---|---|---|---|
| **U1** | How is the sidebar built? | **VS Code's native tree in 0.1.** The webview sidebar (the web version's look: boxed panels, a search box inside the view) is **postponed** to a later release. | [`02-sidebar.md`](02-sidebar.md) · [`05-build-order.md`](05-build-order.md) §6 |
| **U2** | Where does the one true copy of focus and selection live? | **In the extension host.** The tree reads it directly; the canvas and the docs tab are sent copies. | [`01-state.md`](01-state.md) |
| **U3** | Stacked contexts (MAIN + one EXPLORE per call)? | **Design now, build in 0.2.** State is keyed by context id from the first day; 0.1 only has `main`. | [`03-focus-and-contexts.md`](03-focus-and-contexts.md) §5 |
| **U4** | "V-NOC in the VS Code context list" means… | **The right-click menus**: a V-NOC submenu in the code editor, the editor tab and the Explorer, to jump to the node on the canvas or in the sidebar. | [`04-menus.md`](04-menus.md) §3 |
| **U7** | What does a sidebar click do while the canvas is open? | **It shows the node on the canvas and never opens a code tab over it.** With the canvas closed it opens the code, as a preview tab. A setting forces one behaviour. | [`02-sidebar.md`](02-sidebar.md) §4 |

## 3. Proposed, waiting for your yes or no

| # | Proposal | Why | Where |
|---|---|---|---|
| **U5** | Open / closed nodes are **not** shared between the tree and the canvas in 0.1. "Reveal" opens the ancestors on both. | A native tree cannot be told to close one item from code, so sharing would only work in one direction. Comes back with the webview sidebar. | [`02-sidebar.md`](02-sidebar.md) §8 |
| **U6** | The node menu is VS Code's own menu on both surfaces: two lists in `package.json` (`view/item/context`, `webview/context`) that run **the same commands**, and a test that fails when they drift. | The native tree and a webview use different contribution points; one list is only possible when both are webviews. | [`04-menus.md`](04-menus.md) §1 |
| **U8** | ~~Three webview entry files~~ **dropped**: one bundle stays for the canvas and the docs tab. | There is no sidebar webview to keep light. Comes back with it. | |
| **U9** | Missing is a second native view under Structure. | A flat list with two actions. | [`02-sidebar.md`](02-sidebar.md) §9 |
| **U10** | The focus stack is shown twice in the sidebar: a **focus bar row** at the top of Structure (one line, Back and Clear), and a **FOCUS view** above Structure with every step, which you can resize. | The row is where you are already looking, like the web's bar. The view is the resizable stack you asked for, and VS Code resizes views for free. | [`02-sidebar.md`](02-sidebar.md) §3 |

## 4. What is kept from `07-ui/`

`07-ui/03-selection-and-menus.md` is **replaced** by this folder (a note at its top says so). Its core is kept here word for word.

| `07-ui` section | Status |
|---|---|
| `01-state` §2 "the webview never changes its copy of the data", revisions | **kept** |
| `01-state` §3 typed message map | **kept**, built in step 2; the map is in [`01-state.md`](01-state.md) §4 |
| `01-state` §1, §4, §6 "the webview owns view state" | **replaced** by U2: the extension owns what more than one surface shows |
| `02-design` §2 the sidebar | **kept as a native tree**, extended by [`02-sidebar.md`](02-sidebar.md); its menu table is replaced by [`04-menus.md`](04-menus.md) |
| `02-design` §3 canvas, node card, doc panel, docs beside the code; §4 looking native; §5 Shiki / Monaco | **kept** |
| `03` §2 the four words · §3 session · §4 click rules | **kept**, with the fixes in §7, plus a fifth word, `highlight` |
| `03` §5 how selected / cursor / revealed look · the keys | **moved** to [`06-how-it-looks.md`](06-how-it-looks.md), extended for the tree and `highlight` |
| `03` §6 the FOCUS view | **kept** (U10), plus the focus bar row |
| `03` §7 menus · §8 build order | **replaced** by [`04-menus.md`](04-menus.md) and [`05-build-order.md`](05-build-order.md) |

## 5. The idea in one picture

```
                              extension host (TypeScript)
               ┌───────────────────────────────────────────────────────────┐
   core ─────► │  structure (a copy, replaced as a whole)                  │
   (Rust)      │  SESSION  ← the only copy anyone changes                  │
               │    contexts.main.focus / selected / expanded              │
               │    cursor · reveal                                        │
               │        │ reads directly                                   │
               │        ▼                                                  │
               │  native sidebar: focus bar row · FOCUS view · STRUCTURE   │
               └──────▲──────────────────────────────┬─────────────────────┘
        intents (up)  │                              │  session (down, whole, numbered)
              ┌───────┴──────────────┬───────────────┴──────┐
              │  canvas webview      │   docs webview       │
              │  cards · crumbs      │   follows the cursor │
              │  doc panel           │                      │
              └──────────────────────┴──────────────────────┘
```

Nothing in this plan changes the Rust core or the driver. `vnoc/structure`, `vnoc/docs` and `vnoc/attach` stay as they are.

## 6. Draft rows for your decision log

Paste what you accept into `08-decisions-to-log.md`; I did not edit it.

| Draft row | Decision | Supersedes |
|---|---|---|
| **U1** | The sidebar stays a native `TreeView` in 0.1. A webview sidebar is a later release, decided when EXPLORE contexts are built. | — |
| **U2** | The extension host owns the session (focus, selection, the canvas's open nodes, cursor node, reveal). Webviews send intents and draw the session they are sent. | `07-ui/01` §1 and §6 rule 1 |
| **U3** | Session state is keyed by context id. 0.1 has one context, `main`. | `07-ui/02` §3 "one focus stack per canvas tab: not in 0.1" |
| **U4** | A V-NOC submenu in the editor, editor-tab and Explorer right-click menus, with "Reveal in V-NOC Sidebar". | extends F9 |
| **U7** | A sidebar click selects. With the canvas open it reveals the node there and never opens a code tab in the canvas's column; with it closed it opens the code as a preview tab. Setting `vnoc.sidebar.click`. | `07-ui/02` §2 "a click keeps jumping to the code" |
| **U5, U6, U8–U10** | see §3 | |

## 7. Changed after the review of the first version

| Was | Now | Where |
|---|---|---|
| a whole session arrives with new arrays every time, so the canvas re-lays out (and dragged cards snap back) on every click or cursor move | the canvas keeps the old arrays when their contents are equal | [`01-state.md`](01-state.md) §5 |
| a sidebar `select` had side effects a canvas `select` did not | the row click is its own command (`vnoc.open`); `select` never opens anything | [`02-sidebar.md`](02-sidebar.md) §4 |
| a canvas opened an hour later replayed the last reveal | the canvas reports `revealed`, and the store clears it | [`01-state.md`](01-state.md) §2 |
| every cursor move was written to `workspaceState` | only the saved part is compared and written | [`01-state.md`](01-state.md) §3 |
| the docs tab jumped to the selection when you clicked into it | it keeps the last cursor node until another code editor is active | [`01-state.md`](01-state.md) §2 |
| `revealInExplorer` was wired straight into a webview menu | a `vnoc.revealInExplorer` wrapper turns the node into a `Uri` | [`04-menus.md`](04-menus.md) §2 |
| one command for "Show / Hide Code in Card" | two commands (a menu label is the command's title) | [`04-menus.md`](04-menus.md) §2 |
| `Alt+←` bound everywhere | not while a text field has focus (it is word-left on macOS) | [`04-menus.md`](04-menus.md) §5 |
| the editor-tab menu used the active editor's cursor | it uses the tab's `Uri` | [`04-menus.md`](04-menus.md) §3 |
| the selection ring came in step 3 | it is step 1 | [`05-build-order.md`](05-build-order.md) §3 |
| how each state looks was only in `07-ui/03`, which is replaced | its own file, with `highlight` (pointing without selecting) added | [`06-how-it-looks.md`](06-how-it-looks.md) |

Next: [`01-state.md`](01-state.md)
