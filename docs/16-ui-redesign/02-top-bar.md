# 16.02 The canvas's top bar: one bar instead of three

```
top bar
├── 1. today
├── 2. the one bar
├── 3. each element: use case, gesture, result
├── 4. how it reads in each state
├── 5. narrow canvas
└── 6. where the old things went
```

## 1. Today (`webview/src/App.tsx`)

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ V-NOC  minicart: 2 folders · 11 files · 2 classes · 16 functions        [⟳ Refresh] [ⓘ]  │  header
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ ←  All code  ›  app  ›  api.py                                                            │  crumbs
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ ƒ checkout()  app/api.py:11                         ◎ Show   ⌖ Focus   ↗ Code   ✕        │  SelectionStrip
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

Three lines, about 100 px. "V-NOC" is already in the tab title. The stats are read once. The strip repeats the card's buttons. A "Refresh"
button is a primary blue button for an action you should almost never need (the index follows the code by itself).

## 2. The one bar

30 px, `--vscode-editor-background`, a 1 px bottom border `--vscode-panel-border`. Left to right:

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ [←] │ [Main] [◇ reserve_stock ✕] │ minicart › app › inventory.py › reserve_stock   ⚠ [⌖] [ⓘ] [⋯] │
└──────────────────────────────────────────────────────────────────────────────────────────┘
   1          2 (only with ≥ 2 contexts)        3                                        4   5   6   7
```

Under it, while something loads, VS Code's thin progress line (2 px, `--vscode-progressBar-background`, indeterminate), not text.

## 3. Each element

| # | Element | Use case (job) | Gesture → result | Shown when |
|---|---|---|---|---|
| 1 | **`←` Back** | J2, J4: "one level up" | click / `Alt+←` → `focusBack` in the active context: the focus path loses its last step (R2). Tooltip names the target: *"Back to inventory.py (Alt+←)"*. | always; disabled with no focus |
| 2 | **context chips** | J4: "which context am I looking at; switch" | click a chip → `activate`. `✕` on an EXPLORE chip → `closeContext` (its children too). Hover → tooltip *"Explore: reserve_stock, from checkout_order (line 16)"*. The active chip is filled (`--vscode-button-secondaryBackground`), the others outlined. More than three EXPLORE chips: the oldest fold into `+2 ▾` (a menu). | `session.order.length ≥ 2` |
| 3 | **crumbs** | J2: "where am I; jump up" | the first crumb is the **project name** (the opened folder; tooltip: the stats line from today's header). Then one crumb per focus step. The last one is bold and does nothing. Click a crumb → `focusJump n` (the project crumb = clear the focus). Hover → `highlight` its card (kept). | always (the project crumb alone without a focus) |
| 4 | **`⚠`** | J8: "something is off" | shown only when the structure is `truncated` or `failed.length > 0`. Click → the NEEDS ATTENTION view (`vnoc.attention.focus`). Tooltip: *"3 files could not be read"*. | only then |
| 5 | **`⌖` Reveal selection** | "where is the thing I selected?" (what the selection strip was for) | click → `reveal` of the selection in the active context: opens what hides it, centres the card, pulses it. | enabled when there is a selection |
| 6 | **`ⓘ` Docs** | J6 | toggles the docs panel on the right (pressed while open). | always |
| 7 | **`⋯`** | rare actions | a menu: **Fit View** · **Collapse All** · **Show Minimap** (a check) · ─ · **Refresh Structure** (`vnoc.refreshStructure`) | always |

The crumbs are the **focus**, not the selection. The web's header shows the selected node's path instead
(`Minicart / app / inventory / reserve_stock (promote)`); here the selection is the ring on the card, the row in the sidebar, and the docs panel's
title, and the bar keeps one meaning. The web's "(promote)" is not needed: a selected call is explored with `◇`, which shows where it lives.

## 4. How it reads in each state

| State | Bar |
|---|---|
| reading the code (first start) | `[←] minicart` + the progress line; the canvas shows *"Reading the code…"* centred |
| ready, no focus | `[←]` disabled · `minicart` |
| focused in MAIN | `[←] minicart › app › api.py` |
| an EXPLORE context exists, MAIN active | `[←] [Main] [◇ reserve_stock ✕] │ minicart › app › api.py` |
| the EXPLORE context active | `[←] [Main] [◇ reserve_stock ✕] │ minicart › app › inventory.py › reserve_stock` (the chip filled) |
| calls loading for an open function | the progress line; the card itself shows its own loading row (05 §4) |
| the core failed | `minicart` · the canvas shows the message with **Try Again** and **Show Log** (as the sidebar's welcome does today) |

## 5. Narrow canvas

Below 520 px of width: the crumbs collapse from the left into `… ›` (a menu of the hidden steps), keeping the last two. Below 360 px the chips
become one chip with the active context's name and a `▾` menu. The four buttons on the right never hide.

## 6. Where the old things went

| Was | Now |
|---|---|
| "V-NOC" title | gone; the tab title says it |
| stats line | the project crumb's tooltip |
| "N hidden (click a node to expand)" | gone; the `+N` badge on each closed card says it where it matters |
| "TRUNCATED", "N files could not be read" | `⚠` (4) |
| `⟳ Refresh` (primary button) | `⋯` → Refresh Structure |
| crumb bar `←` and `All code` | `←` (1); the project crumb is "All code" |
| strip: Show | `⌖` (5) |
| strip: Focus · Code | the card's own `⌖` and `↗`, and the menu |
| strip: `✕` clear selection | `Esc` on the canvas (kept from 10/06 §7), and the pane menu's Clear Selection |

Files: `webview/src/App.tsx` (the bar), a new `webview/src/canvas/TopBar.tsx`, delete `webview/src/canvas/SelectionStrip.tsx`.

Next: [`03-canvas.md`](03-canvas.md)
