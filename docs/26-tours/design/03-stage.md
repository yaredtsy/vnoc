# 03 The stage: one step, its code and its sentence

```
stage
├── 1. anatomy
├── 2. each part: what it shows, what a click does
├── 3. every kind of step
├── 4. moving: Next, Prev, jumps, and ← "where I was"
├── 5. the text budget (W5)
├── 6. sizes: the dock, a narrow canvas, the mini bar
└── 7. look
```

The stage is the only place a step's text is shown (W3). It is a dock inside the canvas webview, under the map, as wide as the canvas (W4).

## 1. Anatomy

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 3  ƒ reserve_stock   inventory.py:19   ↳ called on line 16   stop 3 of 10 · 2 of 3  ↗   ⌄  │  1  header, 30 px
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  19    def reserve_stock(sku: str, quantity: int) -> … │ Refuse what is not there            │  3  title
│  20        """ID: 8a9cc864-…"""                        │                                     │
│  21 ▌      if not check_stock(sku, quantity):          │ It checks the stock again by        │  4  text
│  22 ▌          raise ValueError(f"Not enough stock…")  │ itself. checkout_order already did, │
│  23                                                    │ but a caller that skips the check   │
│  24        _STOCK[sku] -= quantity                     │ must not push the count below zero. │
│  25        remaining = _STOCK[sku]                     │                                     │
│  26                                                    │                                     │
│  27        if remaining <= LOW_STOCK_THRESHOLD:        │ ▰▰▰▱▱▱▱▱▱▱         ‹ Prev   Next ›  │  5  foot
└──────────────────────────────────────────────────────────────────────────────────────────────┘
   2  code pane: the rest of the width                     text pane: 40%, 320 to 460 px
```

Left is **what**, right is **why**. The lit lines and the title start on the same row when the block is at the top of the pane; otherwise
the bar `▌` in the gutter and the same colour on the title's left edge tie them together.

## 2. Each part

### 2.1 Header

| Left to right | Shows | Click |
|---|---|---|
| the mark | `●` written · `◌` not written yet | — |
| the stop's number, kind icon, name | `3  ƒ reserve_stock` | — |
| `inventory.py:19` | where the stop lives, dim. The folders are in the tooltip | Go to Code (as `↗`) |
| `↳ called on line 16` | how the tour reached this stop: the line in the stop it hangs from. Not on the first stop, not on a stop reached by containment (a file in a folder) | jump to that stop, on the step whose block holds line 16 |
| `stop 3 of 10 · 2 of 3` | the place (T3). The second part only when the stop has more than one step | — |
| `↗` Go to Code | | the real file in an editor tab beside the canvas, the cursor on the block's first line. The only way a tour opens an editor tab |
| `⌄` | | fold the stage to the mini bar (§6) |

Not in the header: Exit and Stop. Exit is the `✕` of the tour's chip and panel; Stop is `■` on the bar and the route
([04](04-route-and-map.md)). The stage is for reading.

### 2.2 Code pane

| Thing | Rule |
|---|---|
| what code | the stop's node, whole, **as it is now** in the index (the same `code` request a card's `</>` uses), with the file's real line numbers and the editor's font and colours |
| the block | its lines get a 3 px bar in the gutter and a background; **every other line is at 55%**. An intro has nothing lit and nothing dim, only a thin mark `▏` on the first line |
| scrolling by itself | on a new stop: the top. On a new step in the same stop: **only if** the block is not fully in view, and then its first line is put two lines under the top. Never while the pointer is over the pane |
| a block taller than the pane | its start is shown; a chip on the bottom edge says `▾ 9 more lit lines` and scrolls one pane down |
| long lines | not wrapped; the pane scrolls sideways |
| a call that is a stop on this tour (here `check_stock` on line 21) | underlined on hover; a click jumps to that stop (T5). A call that is not on the tour is plain text in basic; it is where **Deeper** goes in phase 3 |
| selecting text | works, for copying. It does nothing else in basic |
| editing | no. `↗` opens the real file |

### 2.3 Text pane

| Thing | Rule |
|---|---|
| **title** | the block's `focus`: one line, 13 px, weight 600, at most 60 characters (§5). An intro has no title |
| **text** | the paragraph. 13 px, line height 1.55, at most 46 characters wide so a line is never long. An intro's paragraph is 14 px: it is the lead |
| what the text may hold | `code`, *emphasis*. No headings, lists, tables, code blocks, images or links: `vnoc walk write` refuses them |
| the marks | under the text, dim, one line each: *fallback* and *changed* (§3) |
| the space under the text | empty in basic. Phase 3's question line goes here |
| scrolling | none at quick and normal verbosity: the budget fits the pane. At detailed the text scrolls inside the pane; the foot stays |

### 2.4 Foot

| Thing | Rule |
|---|---|
| the segments `▰▰▰▱▱▱▱▱▱▱` | **one per stop** (40 at most, so they always fit). Filled: seen or here. Hollow: ahead. Dotted: not written. Hover: the stop's name. Click: jump to it |
| `‹ Prev` | secondary button. Disabled on the cover |
| `Next ›` | the primary button, always in the same place. On the cover it reads `Start ›`; on the last step `Finish ›`; on a stop that is not written it is dim with `◌` |

## 3. Every kind of step

| Kind | When | Code pane | Text pane | Frame |
|---|---|---|---|---|
| **cover** | before step 1 | none: the stage is one column | the tour's name, its question if it has one, its size, what the agent is doing, `Start ›` | [02](02-flow.md) 2 |
| **intro** | the first step of a stop of 8 lines or more | from the first line, nothing lit | the paragraph, lead size, no title | 02 · 3 |
| **block** | each block of that stop | the block lit, the rest dim | title and text | 02 · 4 |
| **short stop** (W9) | a stop under 8 lines: its only step | the whole node lit, nothing dim | the intro paragraph, lead size | 02 · 5 |
| **contents** | a folder or a file: there is no code to light | its children as rows (§3.1) | the intro paragraph | below |
| **explained before** | a *contextual* stop | whole, dim, nothing lit | one sentence, then `⇢ Explained at stop 2  [Go there]` | 02 · 7 |
| **not written** | the agent has not saved the stop | whole, normal: you can read it | three grey bars and *"The agent is writing this stop. You can read the code meanwhile."* If it is not the stop being written: *"Not written yet: the agent is on stop 6."* | 02 · 6 |
| **not written, stopped** | the tour was stopped before this stop | whole, normal | *"Not written: the tour was stopped here. Ask the agent to continue it."* | 02 · 8 |
| **end** | after the last step | none | the counts and three buttons | 02 · 9 |

Two marks can sit on any written step:

| Mark | When | Looks like | Why it is never hidden |
|---|---|---|---|
| **fallback** | the file says `degraded`: the agent's text failed the checks twice and the core cut the stop into even blocks | the title is the line range (`Lines 12 to 15`); under it, in the warning colour: `⚠ No explanation was written for these lines.` | the prototype's rule: the UI never hides a fallback (T9) |
| **changed** | the stop's `code_hash` is not the hash of the code now | the code pane shows the code **as it is now**; the lit lines get a **dashed** bar and no background; under the text: `The code changed after this was written. The lines may not match.` | the text may be about lines that moved or went (T9) |

### 3.1 Contents: a stop with no code

A tour of `app/`, stop 1. The left pane lists what is inside, the way the map draws it, as rows:

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 1  folder app/                                                   stop 1 of 9       ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│   2   api.py           1 function                      │ app/ is the whole shop. A request   │
│   3   checkout.py      2 functions                     │ comes in through api.py; checkout.py│
│   4   gateway.py       1 function                      │ runs the sale using the three files │
│   5   inventory.py     3 functions                     │ under it: stock, payment, orders.   │
│   6   models.py        1 class                         │                                     │
│   7   orders.py        1 class · 2 functions           │                                     │
│   8   payments.py      2 functions                     │                                     │
│   9   __init__.py      empty                           │ ▰▱▱▱▱▱▱▱▱          ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The number in front is the child's **stop number**; a click jumps to it. A child that is not on the tour has no number and is dim; more
  than eight of those fold into `+ 12 not on this tour`.
- The second column is the card's meta line (`../../16-ui-redesign/04-node-cards.md` §1): no new wording.
- `[check]` whether the core gives a **file** stop blocks. If it does, a file is shown as code like a class. If it does not, as contents.

## 4. Moving

| Gesture | Result |
|---|---|
| `Next ›` · `→` · `Space` | the next step in the route's order. From a stop's last step: the first step of the next stop. From the last step: the end card |
| `‹ Prev` · `←` | the step before, in the route's order |
| `↓` · `↑` | the next / previous **stop**, on its first step. This is how you skip (T5) |
| a route row · a card on the map · a segment · a call in the code · `↳ called on line 16` · **Go there** | a **jump** to that stop or step |
| the bar's `←` · `Alt+←` | **back to where I was before the last jump**. Jumps stack (20 at most); Next and Prev never add to the stack. Tooltip: *"Back to stop 4 (Alt+←)"*. Disabled when the stack is empty |
| `Home` · `End` | the cover · the end card |
| `Ctrl/Cmd+Enter` | Go to Code |

- **No timer** (W8). Nothing advances by itself, so there is no play and no pause.
- A stop is **seen** when you have been on its last step. Jumping over a stop leaves it `○`; the end card counts those.
- Order and history are two different things, and each has one control: `Prev` is order, `←` is history. In phase 3 a detour is a jump, so `←`
  is already its Back.

## 5. The text budget (W5)

The wall of text is stopped where it is written. These are checks in `vnoc walk write`, beside the ones the phase document lists, and they
are counted in characters:

| Field | quick | normal | detailed |
|---|---|---|---|
| a block's title (`focus`) | 60 | 60 | 60 |
| a stop's intro | 160 | 280 | 480 |
| a block's text | 160 | 280 | 600 |
| the sentence of a stop explained before | 120 | 120 | 160 |

| Rule | Why |
|---|---|
| 280 characters is about 45 words: six or seven lines in the text pane | it fits beside nine lines of code without a scrollbar |
| the title is 60, not the phase document's 100 | it has to fit one line in the stage, and most of it in a route row |
| over the budget, `write` answers with the count (`block 2 text: 412 of 280`) and the agent shortens it | the same loop as an overlapping block |
| a **short stop has an intro and no blocks**, like a stop explained before (W9) | one step; its budget is the intro's |
| a title says what the block **does**, as a phrase, with no full stop; the text says **why the code needs it** | the phase document's writing rule, kept; the code already says how |
| the UI never cuts a text. An older or longer text scrolls in its pane | cutting hides what the agent said; the budget is the fix |

## 6. Sizes

| Thing | Rule |
|---|---|
| height | **fixed while you read**: nine code lines and the header by default, about 215 px. It does not change from step to step, so `Next ›` never moves |
| changing it | drag the dock's top edge: five lines at least, 45% of the canvas at most. Kept per window |
| the text pane | 40% of the width, 320 px at least, 460 px at most; drag the line between the panes. The code gets the rest |
| the cover and the end card | the same height; one column |

**A narrow canvas** (under 640 px wide, common when the canvas shares the window with a code tab): the panes stack. The code keeps six lines,
the foot stays at the bottom.

```
├ STAGE ───────────────────────────────────────┤
│ ● 3  ƒ reserve_stock      3 of 10 · 2/3  ↗ ⌄ │
│  20        """ID: 8a9cc864-…"""              │
│  21 ▌      if not check_stock(sku, quantit…  │
│  22 ▌          raise ValueError(f"Not eno…   │
│  23                                          │
│ ──────────────────────────────────────────── │
│ Refuse what is not there                     │
│ It checks the stock again by itself.         │
│ checkout_order already did, but a caller     │
│ that skips the check must not push the       │
│ count below zero.                            │
│ ▰▰▰▱▱▱▱▱▱▱                 ‹ Prev   Next ›   │
└──────────────────────────────────────────────┘
```

**The mini bar** (`⌄`): the stage folds to one line of 30 px, and the map gets the room. For looking around (T4) without leaving the tour.

```
├──────────────────────────────────────────────────────────────────────────────────────────────┤
│ ● 3  ƒ reserve_stock  ·  Refuse what is not there             stop 3 of 10 · 2 of 3  ‹  ›  ⌃ │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

`‹` `›` and the keys still move; the title changes; the map follows. `⌃` unfolds. The stage stays folded until you unfold it.

## 7. Look

| Part | Token |
|---|---|
| the dock | `--vscode-panel-background`, a 1 px top border `--vscode-panel-border`; no shadow, no rounded corners: it is part of the canvas, like VS Code's own panel |
| code pane | `--vscode-editor-background`, `--vscode-editor-font-family` and `-size`, line numbers `--vscode-editorLineNumber-foreground` |
| lit lines | background `--vscode-editor-rangeHighlightBackground`; the bar `--vscode-focusBorder`, **the colour of the ring on the current card** (`--card-selected`). One colour means "here" on the map and in the code |
| the title's left edge | the same colour, 2 px |
| dim lines | opacity 0.55 |
| text | `--vscode-foreground`; marks `--vscode-descriptionForeground`; the fallback mark `--vscode-editorWarning-foreground` |
| `Next ›` / `‹ Prev` | `--vscode-button-*` / `--vscode-button-secondary*`, 26 px high |
| segments | 3 px high, 2 px apart; filled `--vscode-focusBorder`, hollow `--vscode-editorWidget-border` |
| grey bars (not written) | `--vscode-editorWidget-border`, a slow shimmer; still with *Reduce motion* |
| icons | codicons, as the redesign: `play` `▶`, `link-external` `↗`, `chevron-down` `⌄`, `call-outgoing` `↳`, `warning` `⚠`, `debug-stop` `■` |

Next: [`04-route-and-map.md`](04-route-and-map.md)
