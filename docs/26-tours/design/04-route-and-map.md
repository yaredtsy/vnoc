# 04 The route and the map: the sidebar panel, the canvas, the bar, the status bar

```
route and map
├── 1. the route: the tour's panel in the sidebar
├── 2. the map: the canvas in a tour context
├── 3. the tour card
├── 4. the camera
├── 5. the bar
├── 6. the status bar item and the one notification
├── 7. menus
└── 8. the session, after this
```

Both are for **finding** (T3, T4, T5). Neither shows a step's text (W3).

## 1. The route

The **TOURS** section of the sidebar's webview, while a tour is open ([07](07-tour-list.md) §2: with no tour open the same section is the
list of tours). It sits under the context panels and behaves as one (`../../16-ui-redesign/06-sidebar.md` §4): same header, same splitter, same
"click inside to activate".

```
┌ STRUCTURE ─────────────────────────────── ⊞  ⊟  ⋯ ┐
│ Search nodes…                                     │
│   MAIN                                            │
│   ⌖  minicart › app                         ←  ✕  │
│   ▸ checkout.py                                   │
│   ▸ inventory.py                                  │
│ ═══════════════════ drag ════════════════════════ │
│ ┃ ‹ Tours  ▶ checkout_order           3 of 10   ✕ │   header. ┃ = the active context · ‹ Tours = the list
│ ┃ ✓  1  ƒ checkout_order                          │   seen
│ ┃ ✓  2    ƒ check_stock                           │
│ ┃ ●  3    ƒ reserve_stock                         │   here: the only stop that is unfolded
│ ┃           what it is for                        │     its steps, as titles
│ ┃         ▸ Refuse what is not there              │     ▸ the current step
│ ┃           Take the units, warn when few are le… │
│ ┃ ○  4      ƒ check_stock                     ⇢ 2 │   ahead · explained at stop 2
│ ┃ ○  5    ƒ charge_payment                        │
│ ┃ ◌  6      ƒ retry_charge               writing… │   not written · the one the agent is on
│ ┃ ◌  7    ƒ cancel_checkout                       │
│ ┃ ◌  8      ƒ release_stock                       │
│ ┃ ◌  9    ƒ create_order                          │
│ ┃ ◌ 10      ◆ CompletedOrder                      │
│ ┃ ─────────────────────────────────────────────── │
│ ┃ writing 6 of 10                          ■ Stop │   the foot: one line, only when it has something to say
└───────────────────────────────────────────────────┘
```

### 1.1 A stop's row (22 px, as every sidebar row)

| Left to right | Shows |
|---|---|
| the mark | `✓` seen · `●` here · `○` ahead · `◌` not written. Four **shapes**, so they read without colour. `●` is `--vscode-focusBorder`; `✓` and `◌` are dim |
| the number | the stop's number, right-aligned in two digits |
| the indent | one step (12 px) per level: a stop sits under the stop it was reached from |
| kind icon, name | as a structure row (`../../16-ui-redesign/07-sidebar-rows.md`) |
| the description, at the right | **one** word, the first that applies: `writing…` · `⚠ fallback` · `changed` · `⇢ 2` |
| on hover | `↗` Go to Code, the only hover button |

### 1.2 A step's row (20 px, dim, no icon)

Only under the stop you are in, and only when it has more than one step. The intro's row reads *what it is for*; a block's row is its title,
cut at the panel's edge. The current step has `▸` and the normal text colour. **Leaving the stop folds them away**: the panel never shows the
titles of two stops, so a tour of 40 stops is 40 rows plus at most six.

### 1.3 Gestures

| Gesture | Result |
|---|---|
| click a stop's row | jump to its first step (03 §4). On the stop you are in: nothing |
| click a step's row | jump to that step |
| double-click a stop's row | Go to Code (a row's double-click, kept from the redesign) |
| `↑` `↓` in the panel, `Enter` | move the row focus; jump |
| `‹ Tours` on the header | the section shows the list of tours; the tour stays open ([07](07-tour-list.md) §2) |
| `✕` on the header | close the tour's context ([05](05-states.md) §2). It does not stop the agent |
| click anywhere in the panel when another context is active | activate the tour's context: the canvas draws the tour, on the step you left |
| the search box | unchanged: it searches the structure. Picking a result reveals it in MAIN and activates MAIN; the tour's panel stays |

The panel keeps the current stop's row in view. Its header stays at the top while the rows scroll.

### 1.4 The foot

One line under the rows. No line at all when the tour is complete and nothing is wrong.

| The tour | Foot |
|---|---|
| being written | `writing 6 of 10` · `■ Stop` |
| stopping | `stopping: the agent finishes stop 6` |
| stopped | `stopped: 6 of 10 written` |
| no news from the agent ([05](05-states.md) §1) | `no news from the agent for 5 min · 6 of 10 written` |
| written, and the code changed since | `3 stops changed since this was written` |

## 2. The map

The canvas, drawing the tour's context. It is the redesign's canvas (`../../16-ui-redesign/03-canvas.md`): the same cards, columns and edges.

```
   ┌──────────────────────┐        ┌──────────────────────┐
   │ ✓1  ƒ checkout_order │━━━┳┄┄┄┄│ ✓2  ƒ check_stock    │
   └──────────────────────┘   ┃    └──────────────────────┘
                              ┃    ┏━━━━━━━━━━━━━━━━━━━━━━┓        ┌──────────────────────┐
                              ┣━━━━┃ ●3  ƒ reserve_stock  ┃┄┄┄┄┄┄┄┄│  4  ƒ check_stock ⇢2 │
                              ┆    ┗━━━━━━━━━━━━━━━━━━━━━━┛        └──────────────────────┘
                              ┆    ┌──────────────────────┐        ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐
                              ├┄┄┄┄│  5  ƒ charge_payment │┄┄┄┄┄┄┄┄┆ ◌6  ƒ retry_charge   ┆
                              ┆    └──────────────────────┘        └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘
                              ┆    ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐        ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐
                              ├┄┄┄┄┆ ◌7  ƒ cancel_checkout┆┄┄┄┄┄┄┄┄┆ ◌8  ƒ release_stock  ┆
                              ┆    └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘        └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘
                              ┆    ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐        ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐
                              └┄┄┄┄┆ ◌9  ƒ create_order   ┆┄┄┄┄┄┄┄┄┆ ◌10 ◆ CompletedOrder ┆
                                   └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘        └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘
```

| Rule | Why |
|---|---|
| **Every stop of the route is a card, from the first second.** Nothing else is drawn | the route is decided by the core before a word is written, so the whole trip is on the map at the cover. 40 stops at most: far under the canvas's budget of 200 cards |
| **The current stop is the context's selection** (W6) | so it has the ring (2 px, `--card-selected`), and the edges from the first stop to it are heavy (2 px, the same colour): the redesign's "on the path to the selection" rule, unchanged. In the picture: `━━━` |
| the other edges keep their meaning | dashed = "calls", solid = "contains". Seen edges get no colour of their own: one colour on the map means "here" |
| a stop that is **not written** has a dotted outline | the card is known, its text is not. It is not dim: 60% already means "unknown callee" |
| **No chevrons, no Focus, no Explore, no Code in Card** | the route is fixed (the core chose it), and the stage shows the code. A tour that could be folded and refocused is a second structure browser |
| what a stop calls **outside** the tour | not drawn. Its count is the card's `+N`, dim, tooltip *"2 calls are not on this tour"*. No click in basic; it is where Deeper will be |

## 3. The tour card

The redesign's card, 300 × 80 px, three rows (`../../16-ui-redesign/04-node-cards.md`). The differences:

```
┌──────────────────────────────────────────────┐      ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
│ ✓1  ƒ  checkout_order                     ↗  │      ┃ ●3  ƒ  reserve_stock                      ↗  ┃
│        checkout.py:11                        │      ┃ ↳ line 16                      inventory.py  ┃
│ ──────────────────────────────────────────── │      ┃ ──────────────────────────────────────────── ┃
│        3 steps                               │      ┃        3 steps   1 doc                       ┃
└──────────────────────────────────────────────┘      ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
  seen                                                  here

┌──────────────────────────────────────────────┐      ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐
│  4  ƒ  check_stock                        ↗  │      ┆ ◌6  ƒ  retry_charge                       ↗  ┆
│ ↳ line 21                                    │      ┆ ↳ line 23                       payments.py  ┆
│ ──────────────────────────────────────────── │      ┆ ──────────────────────────────────────────── ┆
│                           ⇢ explained at 2   │      ┆                                    writing…  ┆
└──────────────────────────────────────────────┘      └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘
  ahead, explained before                               not written, being written
```

| Part | On a structure or call card | On a tour card |
|---|---|---|
| the chevron's place | `▸` `▾` `⌖`, a spinner | **the stop's badge**: `✓1` seen (dim) · `●3` here (filled, `--card-selected`) · ` 4` ahead (outlined) · `◌6` not written (dotted; it turns while the agent is on that stop) |
| the buttons | `⌖` `↗` `</>` `＋` or `◇` `↗` `</>` | `↗` Go to Code |
| the meta line | where it is, or the call sites | **unchanged**. A stop reached by a call is a call card: `↳ line 16` and the callee's file |
| the footer, left | `+N` · docs · link | `3 steps` (written, more than one) · the docs count (kept: `ⓘ` works on a tour) · `+N` calls not on the tour |
| the footer, right | the calls' state | one of: `writing…` · `fallback` · `changed` · `⇢ explained at 2` |
| click | select | **jump to this stop**. That selects it (W6) |
| double-click | Focus, or Explore | the same as a click |

## 4. The camera

The tour moves the map as little as it can. The rule is one sentence: **the current card is in view after every step, and nothing else is
promised.**

| Moment | The map |
|---|---|
| the cover, the end card, `⋯ → Fit Route` | fits the whole route. **The only times the tour changes the zoom** |
| a step inside the same stop | does not move |
| a new stop, its card fully in view | does not move |
| a new stop, its card cut or out of view | pans by the least that shows the whole card with a 24 px margin. If that is more than one screen, it centres the card instead. 200 ms; a jump with *Reduce motion* |
| you pan or zoom | it is yours. Nothing snaps back until the next stop that is out of view |
| `⌖` on the bar | centres the current card |
| the stage is dragged taller, folded or unfolded | the map's view gets shorter or taller. It pans only if the current card is now covered |
| you go to MAIN and come back | the tour's own pan and zoom, as it was (each context keeps its viewport, redesign 03 §4) |

## 5. The bar

The redesign's one bar (`../../16-ui-redesign/02-top-bar.md`), in a tour context:

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ [←] │ [Main] [▶ checkout_order ✕] │ checkout_order › reserve_stock      writing 6/10 ■   [⌖] [ⓘ] [⋯] │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
   1               2                              3                            4            5   6   7
```

| # | Element | In a tour context |
|---|---|---|
| 1 | **`←`** | **back to where I was before the last jump** (03 §4), `Alt+←`. Tooltip *"Back to stop 4"*. Disabled when there is nothing to go back to |
| 2 | **the tour's chip** | `▶ checkout_order ✕`. Click: activate. `✕`: close the tour's context. Tooltip: *"Tour of checkout_order: stop 3 of 10"*. While the agent writes, a dot on the chip pulses (still with *Reduce motion*), so you see it from MAIN too |
| 3 | **crumbs** | **the stops from the first one down to the current one**, by the stop each was reached from. A click on a crumb jumps to that stop; hover highlights its card. The project is not a crumb here: a tour has no focus to clear |
| 4 | **the agent** | only when there is something to say: `writing 6/10 ■` · `stopping: the agent finishes stop 6` · `stopped at 6 of 10` · `no news from the agent`. Empty when the tour is complete. `■` is the only button here |
| 5 | **`⌖`** | centre the current stop's card |
| 6 | **`ⓘ`** | the docs panel, unchanged: it follows the selection, which is the current stop. The docs a node already has and the tour's sentence about it can be read side by side |
| 7 | **`⋯`** | **Fit Route** · **Show Minimap** · ─ · **Start Again** · **Open Tour…** · **Close Tour** |

Narrow (under 640 px): the crumbs keep only the last stop; the agent's text becomes `6/10 ■`.

## 6. The status bar item and the one notification

| When | Status bar | Click |
|---|---|---|
| a tour's context exists | `▶ Tour 3/10` | show the canvas with the tour active |
| … and the agent writes | `▶ Tour 3/10 · writing 6/10` | the same |
| the context was closed, the agent still writes | `▶ Tour: checkout_order · writing 7/10` | open it again, on the step you left |
| the context was closed, the tour is complete or stopped | nothing | **Open Tour…** is in the palette |

**One notification**, and only in one case: a tour started while the canvas was hidden ([02](02-flow.md) frame 2). Nothing is announced when
the agent finishes: the status bar loses its `writing` part, and that is enough.

## 7. Menus

A tour row and a tour card share one list, as cards and rows do in the redesign (R12):

| Entry | Use case |
|---|---|
| **Go to Code** | read the stop in the real file |
| **Reveal in Main** | leave the tour at this node: MAIN becomes active and reveals it, so you can focus it, open its calls, add a doc. The tour stays where it was |
| **Show Docs** | as `ⓘ` |

The chip and the panel's header: **Start Again** · **Close Tour**.

## 8. The session, after this

```ts
// shared/session.ts
export interface ContextState {
  // …
  kind: "main" | "explore" | "tour";                         // "tour" is NEW
  /** the tour this context plays, and where you are in it; null for the other kinds */
  tour: TourPlace | null;                                    // NEW
  // focus: the lineage of the tour's start node, fixed.  selected: the current stop's node (W6).  open: not used
}

export interface TourPlace {
  id: TourId;                                  // "w_5d20": the session file in .vnoc/cache/walkthroughs/
  at: StepId | "cover" | "end";                // "s2.b1"; a stop id alone ("s1") is its intro, or its only step
  seen: readonly StopId[];
  back: readonly StepId[];                     // the jump stack of 03 §4, 20 at most
}

export type Intent =
  | …
  | { type: "tourOpen"; id: TourId; activate: boolean }
  | { type: "tourGo"; to: StepId | "cover" | "end" | "next" | "prev" | "nextStop" | "prevStop"; jump: boolean }
  | { type: "tourBack" }
  | { type: "tourStop" };                      // the only one that reaches the file: the host asks the core to set "stopped"
// closing is the existing closeContext
```

`reduce` gets the tour's stops beside the tree, the way it gets `CallLookup` for calls: it needs them to know what "next" is.

Next: [`05-states.md`](05-states.md)
