# 16.06 The sidebar: a webview view with context panels

```
sidebar
├── 1. why a webview now, and what it costs
├── 2. what it looks like
├── 3. the focus bar (one per panel)
├── 4. context panels
├── 5. search
├── 6. selection and scrolling
├── 7. keyboard
├── 8. the four states
├── 9. the view's title buttons, and NEEDS ATTENTION
└── 10. how it is built
```

Replaces 10-ui-rework/02 (the native tree, the FOCUS view, the focus bar row, the QuickPick as the only search).

## 1. Why a webview now (R1), and what it costs

What the native tree could not do (10/02 §11), and this redesign needs: stacked context panels with their own focus bars; open / closed shared with the canvas (a native tree cannot close a row from code); one menu list with the card;
a search box in the view.

What has to be rebuilt, and is part of the steps:

| The native tree gave it free | Here |
|---|---|
| keyboard (arrows, type-ahead, Home / End) | §7, `role="tree"` with one roving `tabindex` |
| screen reader | `role="treeitem"`, `aria-level`, `aria-expanded`, `aria-selected`, `aria-setsize`, `aria-posinset` |
| virtual scrolling | `@tanstack/react-virtual` over the flattened visible rows (10,000 rows is fine) |
| the user's **file icon theme** | **lost**: a webview cannot read it. Codicons (`folder`, `file-code`) instead. The one visible loss. |
| VS Code's list colours | the same, from `--vscode-list-*` variables (09 §3) |
| sticky scroll, indent guides | indent guides drawn (1 px, `--vscode-tree-indentGuidesStroke`, on hover of the tree as VS Code does); no sticky scroll |

## 2. What it looks like

With one context (most of the time):

```
V-NOC
┌ STRUCTURE ─────────────────────────────── ⊞  ⊟  ⋯ ┐   view title: Open Canvas · Collapse All · more
│ 🔍 Search nodes…                                  │
│ ⌖  minicart › app › api.py                 ←  ✕  │   focus bar (only while focused)
│ ▾ 📄 api.py                                       │   the focused node, always open
│   ƒ checkout                            📄 1      │   no call rows: calls are on the canvas (07 §2)
│   ƒ health                                        │
└───────────────────────────────────────────────────┘
┌ NEEDS ATTENTION (2) ──────────────────────────────┐   native, only when needed
```

With an EXPLORE context open:

```
┌ STRUCTURE ─────────────────────────────── ⊞  ⊟  ⋯ ┐
│ 🔍 Search nodes…                                  │
│ ┃ MAIN                                            │   panel header (only with ≥ 2 contexts); ┃ = the active one
│ ┃ ⌖  minicart › app › api.py               ←  ✕  │
│ ┃ ▾ 📄 api.py                                     │
│ ┃   ƒ checkout                                    │   no row selected: the selection is the reserve_stock CALL card
│ ═══════════════════ drag ════════════════════════ │
│   ◇ EXPLORE: reserve_stock · from checkout_order ✕│
│   ⌖  minicart › app › inventory.py › reserve_stock │
│   ƒ reserve_stock                     (selected)  │   the callee at home; ← shows inventory.py with its siblings
└───────────────────────────────────────────────────┘
```

The panel opens with one row, the callee, as the web's does (your third screenshot). `←` in its focus bar goes up to its file, then its folder:
that is J4 in the sidebar.

The web's boxed rows (each row a rounded card, your second screenshot) are not copied: in a 250 px side bar they cost a third of the width, and
every other view in VS Code is a flat list. The behaviour is copied; the look is VS Code's.

## 3. The focus bar

One line at the top of each panel, while its context has a focus. It is the same path as the canvas's crumbs (02 §3), for the same job (J2).

| Part | Click |
|---|---|
| `⌖` | nothing (a label) |
| `minicart` | `focusJump 0` (clear) |
| each step | `focusJump n`; the last one is bold and does nothing |
| `←` | `focusBack` (one level up) |
| `✕` | `focusJump 0` |

Long paths collapse from the left into `…` (a menu of the hidden steps), keeping the last two. Tooltip: the whole path.

This replaces **both** the FOCUS view and the focus bar row of 0.1: one drawing of the focus per panel.

## 4. Context panels

One panel per context, in `session.order` (a child under its parent), splitters between them.

| Part | Does |
|---|---|
| header `MAIN` / `◇ EXPLORE: reserve_stock · from checkout_order` | shown with ≥ 2 contexts. Click → `activate`. Tooltip: *"checkout_order calls reserve_stock at line 16"*. |
| `✕` on an EXPLORE header | `closeContext` |
| the active panel | a 2 px bar on its left (`--vscode-focusBorder`), its header in `--vscode-sideBarSectionHeader-foreground` bold; the others normal (not faded: the web's 70% opacity makes them hard to read) |
| a click anywhere in a non-active panel | **activates it** and does what the click does (select a row, …): one click, not two |
| sizes | a new EXPLORE panel takes 45% of the height, at least 6 rows; the splitter above it moves it (§4.1) |
| more panels than fit | each keeps at least its header + focus bar + 3 rows; the view scrolls |

### 4.1 The splitter

Every panel after the first has a splitter on its top edge. It is VS Code's sash, drawn the same way, so it feels like the line between two
views of the side bar.

| Thing | Rule |
|---|---|
| where | the top border of each panel after the first; a 4 px hit area centred on the border, over the content (it takes no height) |
| look | invisible at rest (the 1 px border is there); after 300 ms of hover, or while dragging, a 4 px line in `--vscode-sash-hoverBorder`. Cursor `row-resize` |
| drag | changes the height of the panel **below** it; the panels above give or take the room (MAIN is `flex: 1`, it absorbs the rest). Live while dragging, no re-render of the rows |
| limits | every panel keeps its header + focus bar + 3 rows (≈ 112 px); a drag stops at the limit, it does not push other panels |
| double-click | back to 45% |
| kept | the size is a share of the view's height, saved per **depth** (1 = an EXPLORE of MAIN, 2 = an EXPLORE of an EXPLORE) in the page's `setState`. A new panel at that depth opens at that size; resizing the side bar keeps the shares |
| one panel | no splitter |
| keyboard | none, as VS Code's own sashes. The sizes are not needed to use anything |

The splitter only moves the line. It does not activate the panel; a click on it does nothing else.

When an EXPLORE panel opens because a call was **selected** (not explored), it opens **without** becoming active (R4): you see where the callee
lives, the canvas stays where it was. Clicking into it (or the bar's chip) makes it active.

## 5. Search

A box at the top of the view, above every panel. It searches **the whole project**, not the focus.

| Gesture | Result |
|---|---|
| type | the panels are replaced by a **flat result list**: icon, name with the matching letters bold, dim path. Up to 200, best first (name before path, prefix before substring). `billing/` matches by path. |
| `↓` from the box, `↑` `↓` in the list | moves the active result, and **highlights** its card on the canvas (`highlight`, does not select) |
| `Enter` / click a result | select it in the **active** context and `reveal` it there (the focus shortens if it hides it, 03 §3); the box clears; the panels come back with the row in view |
| `Esc` | clear the box, back to the panels |
| `Ctrl/Cmd+F` or `/` while the sidebar has focus | put the cursor in the box |

The palette's **V-NOC: Search…** (`Ctrl/Cmd+Alt+N`, the QuickPick of 0.1) stays for people who do not have the sidebar open; it does the same
`Enter`.

Calls are not searched: they are not in the structure, and "where is `reserve_stock` called" is the callers' feature (later).

## 6. Selection and scrolling

| Rule | |
|---|---|
| the selected row of each panel has the list's selection colour | `--vscode-list-activeSelectionBackground` while the view has keyboard focus, `--vscode-list-inactiveSelectionBackground` otherwise, as every VS Code list |
| **the selected row is always scrolled into view** when the selection changes from elsewhere (a card, a reveal, a search pick) | no flash, no `reveal` request: the sidebar does it by itself. Its ancestors are already open: a reveal opens them in the shared set (R8), and a card can only be clicked if its row exists. |
| the cursor node | a dot `●` in `--vscode-editorCursor-foreground` at the end of its row (10/02 §7, now possible on every kind) |
| hover | `--vscode-list-hoverBackground`, the action strip appears (07 §2), and the card on the canvas gets the dotted **highlight** (as hovering a crumb does) |

`vnoc.revealInSidebar` ("Reveal in Sidebar"): `reveal` in the active context (opens ancestors, shortens the focus if needed, selects), then
`vnoc.structure.focus` to bring the view up. The row comes into view by the rule above.

## 7. Keyboard

While a row has focus:

| Key | Does |
|---|---|
| `↑` `↓` · `Home` `End` · `PageUp` `PageDown` | move **and select** (the canvas ring and the docs panel follow; nothing opens, no reveal on the canvas) |
| `→` | open; if open, the first child |
| `←` | close; if closed, the parent |
| `Space` | open / close |
| `Enter` | the **click rule** (canvas open → reveal there; closed → preview the code, 07 §3) |
| `Ctrl/Cmd+Enter` | Focus |
| `Alt+←` | Back, in this panel's context |
| `Shift+F10` · the context-menu key | the row's menu |
| a letter | type-ahead: the next visible row whose name starts with what was typed in the last 500 ms |
| `Ctrl/Cmd+F` · `/` | the search box |
| `Tab` | from a panel's tree to the next panel's tree |

Arrows select here, unlike 0.1's native tree (10/02 §4 found that its arrows only moved VS Code's focus): in a webview we choose, and the
Explorer's habit is that the highlighted row is the selected one.

## 8. The four states

| State | View |
|---|---|
| no folder open | *"Open a folder to see its code."* and an **Open Folder** button |
| reading the code | the search box, then five skeleton rows (the web's `SidebarProjectSkeleton`), and VS Code's progress line on the view |
| no supported file | *"No Python files here yet."* |
| error | the message, **Try Again**, **Show Log** |
| ready | §2 |

These replace the native view's `viewsWelcome` entries.

## 9. The view's title buttons, and NEEDS ATTENTION

| Button | Command | Job |
|---|---|---|
| `⊞` Open Canvas | `vnoc.openCanvas` (no node: just open; beside the code editor if one is visible) | get the graph |
| `⊟` Collapse All | `vnoc.collapseAll` (the active context) | start over without losing the focus |
| `⋯` Refresh Structure | `vnoc.refreshStructure` | the way out if the tree ever does not follow the code |
| `⋯` Search… | `vnoc.search` (the QuickPick) | |
| `⋯` Restore Removed Doc… · Empty Trash | unchanged (11) | |

Gone from the title: `🔍` (the box is in the view), Back and Clear (in each panel's focus bar).

**NEEDS ATTENTION** stays the native view of `11-missing-relink-remove.md` §6, under STRUCTURE, with its badge. Nothing in it changes.

**It stays at the bottom; STRUCTURE scrolls.** Because it is a second view, VS Code gives it its own pane, so a page cannot push it away:

| Rule | How |
|---|---|
| NEEDS ATTENTION sits under STRUCTURE, never scrolled away | it is its own native view in the same container (default order: STRUCTURE, then NEEDS ATTENTION). VS Code draws the line between the two panes; the user can drag it or collapse the view, and VS Code remembers that |
| STRUCTURE's search box stays at the top; only the panels scroll | in the page: the search box is fixed, the panels' box is `flex: 1; min-height: 0; overflow-y: auto`, so it never grows past the pane |
| nothing needs attention | the view is hidden (`when: vnoc.hasAttention`, as in `extension/package.json`) and STRUCTURE takes the whole height |

The mockup draws both views in one column, so it reproduces this by hand: the panels' box scrolls and NEEDS ATTENTION is `flex: none`
under it. Before 2026-11-12 the mockup let a tall sidebar stretch the window and push NEEDS ATTENTION off the screen. That was the mockup,
not the design.

## 10. How it is built

| Piece | |
|---|---|
| view | `contributes.views.vnoc`: `{ "type": "webview", "id": "vnoc.structure", "name": "Structure" }`. **The same id** as the native view, so `vnoc.structure.focus`, the view's position and the `focusedView == vnoc.structure` key bindings keep working **[check `focusedView` for a webview view in 8.6]**. The `vnoc.focusStack` view goes. |
| provider | `extension/src/sidebarView.ts`: a `WebviewViewProvider`, registered with `webviewOptions: { retainContextWhenHidden: true }` (the page is small; keeping it saves a reload each time the side bar is hidden) |
| relay | the same `serve` as the canvas (`relay.ts`); `Panels.emit` sends to the sidebar too (rename `Panels` → `Surfaces`) |
| bundle | **a second entry**, `webview/src/sidebar/main.tsx` → `assets/sidebar.js`: no React Flow, no Monaco, no Shiki. The canvas's bundle with Monaco is several MB; the side bar should open instantly. (Vite: `rollupOptions.input` with two entries.) |
| page | `webview/src/sidebar/`: `Sidebar.tsx` (search + panels), `ContextPanel.tsx`, `FocusBar.tsx`, `Tree.tsx` (flatten + virtualise + keys), `Row.tsx` (07), `SearchResults.tsx`; the stores in `webview/src/state/` are shared with the canvas |
| removed | `extension/src/structureView.ts`, `focusView.ts`, `cursorMark.ts`, `treeMirror` logic (12), the `focusBar` row, `vnoc.focusDrop`, `vnoc.open`'s tree-row form (the page sends an intent and a `click` request instead) |
| tests | the window suite's `sidebar.test.ts` drives the page through notes (`rendered` from the sidebar: `{ rows, selected, contexts }`) instead of `TreeView`; unit tests for `flatten`, type-ahead and the key map in `webview/src/sidebar/*.test.ts` |

Next: [`07-sidebar-rows.md`](07-sidebar-rows.md)
