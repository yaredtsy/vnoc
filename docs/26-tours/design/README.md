# Tours: what a walkthrough looks like (basic)

Written 2026-11-15. A UI and UX design only: no business logic, nothing here is built. The goal is a mockup you can click to see if it is
usable. Read for it: the two phase documents from the web ("Phase 2: walkthrough, basic", "Phase 3: walkthrough, interactive"), the web
prototype's player (`ide/v-noc/src/frontend/src/features/Dashboard/features/Agent/walkthrough/` and `tool/faces/walkthrough/`), its vision
(`ide/v-noc/doc/01-vision.md` §3 "Canvas over chat"), and [`../../16-ui-redesign/`](../../16-ui-redesign/) (the surfaces, contexts, cards and states this
design has to live in).

This folder is the **basic** tour: the agent writes it, you read it, you can stop it. The interactive one (Deeper, Ask, Back) is not designed
here. §6 only shows where it will plug in, so nothing has to move later.

```
tours
├── README (this file)       what the phase documents got right and wrong · the decisions · the one picture · what to ask
├── 01-layout.md             the jobs, the five rules against walls of text, the words, the three parts and what each is for
├── 02-flow.md               the storyboard: ten frames from "walk me through checkout_order" to the end card
├── 03-stage.md              the stage: one step, its code and its text together. Anatomy, every kind of step, sizes, the text budget
├── 04-route-and-map.md      the route panel in the sidebar, the tour on the canvas, the bar, the status bar
├── 05-states.md             writing · waiting · stopping · stopped · stale · fallback · lost agent · keys · what the UI needs from the file
├── 06-mockup-brief.md       for whoever builds mockup.html: the fixture (real Minicart code, all 20 steps), the screens, the fake clock
├── 07-tour-list.md          the TOURS section: every tour, the tours on the selected node, opening one at its root, asking, deleting
└── 08-interactive.md        a first design of the interactive tour: ask, go deeper, come back, find it again. With its mockup fixture
```

## 1. Do the phase documents fit?

The half about **data and commands fits**. The half about **the player went off**, both from the prototype and from the redesign.

| Part of the phase 2 document | Verdict | Why |
|---|---|---|
| the session file, stable IDs (`s3`, `s3.b1`), atomic writes, `code_hash` | **keep** | the UI below needs nothing else, except six small fields and two checks ([05](05-states.md) §7) |
| the route is decided by the core, the agent only writes | **keep** | it is why the outline can be drawn before a word is written |
| Stop through `status`, live growth, "Next waits for the next step" | **keep** | all three are in the design, with a look for each |
| **Player: a native tree view** | **off** | the redesign removed the native trees (R1: the sidebar is a webview). A second, native tree would be the only one |
| **Player: the code in an editor tab, the text in a separate panel** | **off** | three places to look (tree, editor, panel), and the eye travels between the highlighted lines and the sentence about them on every step |
| **Player: the canvas is not used** | **off** | the vision is "canvas over chat": a map you can verify. The prototype played on the canvas. Without it this is CodeTour |
| "the editor returns to the tabs and selection you had" | **not needed** | here a tour never touches your editor tabs (W2), so there is nothing to restore |
| one **Stop** button | **split in two** | leaving the tour and stopping the agent are different wishes (W7) |
| the word "walkthrough" on screen | **renamed** | VS Code has its own "Walkthroughs" (the Get Started pages). On screen it is a **tour**; `vnoc walk` stays the command (W1) |
| phase 3's screen (tree left, code right, a panel with eight buttons) | **redo later** | same three problems; §6 shows the seams |

## 2. The idea

Google Maps, as the vision says. Three parts, each with one job, and **the text of a step exists in exactly one of them**:

| Part | Where | Answers | In Maps |
|---|---|---|---|
| **route** | a panel in the sidebar | "what is on this tour, in what order, how far am I" | the list of turns |
| **map** | the canvas | "where is this in the code, what is around it, what comes next" | the map with the blue line |
| **stage** | docked at the bottom of the canvas | "what does **this** piece do": the code and the sentence about it, side by side | the turn banner |

## 3. The decisions

**ask** = it changes what the prototype or the phase document does; say yes or no before the mockup is built.

| # | Decision | Why | Where |
|---|---|---|---|
| **W1** **ask** | On screen it is a **tour**. Stops and steps keep their names | VS Code's own "Walkthroughs"; the prototype already says "Exit tour" | everywhere |
| **W2** **ask** | **A tour is a context**, like EXPLORE (R3): a chip on the bar, a panel stacked in the sidebar, drawn by the one canvas. Closing it gives MAIN back exactly as it was. A tour never opens, moves or closes an editor tab by itself | contexts already solve "keep what I was doing"; no restore code, no second mechanism | [01](01-layout.md) §4 · [04](04-route-and-map.md) |
| **W3** | **Route, map, stage** (§2). A step's text is in the stage only: the route shows names and one-line titles, the map shows cards | one place to read; the other two are for finding | [01](01-layout.md) §2 |
| **W4** **ask** | **The stage is a dock at the bottom of the canvas: code left, text right.** Not a popover on the card (the prototype), not an editor tab plus a panel (the phase document) | the highlighted lines and their sentence are one glance apart; nothing is covered and nothing is placed by guesswork; the map never reflows; there is room for Ask later | [03](03-stage.md) |
| **W5** | **One step on screen, with a text budget.** Title ≤ 60 characters, body ≤ 280 at normal verbosity, checked by `vnoc walk write` | the wall of text is stopped where it is written, not hidden by a scrollbar where it is read | [03](03-stage.md) §5 |
| **W6** | **The current stop is the tour context's selection** | the ring on the card, the heavy edges from the root, the docs panel following it: all exist already (09) | [04](04-route-and-map.md) §2 |
| **W7** | **Exit and Stop are two things.** `✕` / `Esc` leaves the tour; the agent keeps writing. `■` tells the agent to stop; what is written stays playable | "I want to look at something else" must not throw away a tour that is half written | [05](05-states.md) §2 |
| **W8** | **You set the pace.** No timer moves to the next step | people read at different speeds; so phase 3's "pause playback" has nothing to pause | [03](03-stage.md) §4 |
| **W9** **ask** | **A short stop is one step.** A stop under 8 lines shows its intro with the whole node lit, instead of an intro step and then a one-block step | two clicks for a three-line function is how a tour starts to feel long | [03](03-stage.md) §3 |
| **W10** | **A cover before step 1 and an end card after the last.** The cover shows the route fitted on the map and its size; you can start while the agent writes | the cover is where you wait for the first stop, and where you see the whole trip once | [02](02-flow.md) frames 2 and 9 |
| **W11** | **Nothing steals the window.** Canvas visible: the tour's context is added and activated. Canvas hidden: a notification with **Open Tour**, and a status bar item | R10's spirit: one verb, one effect. The agent may finish `start` while you are typing somewhere else | [02](02-flow.md) frame 2 |
| **W12** | **The agent's chat answer is one line**, never the tour as prose. A rule in the skill | the tour is the answer; a summary in the terminal is the wall this design removes | [02](02-flow.md) frame 1 |
| **W13** | **Depth is offered as routes with a size**: "6 stops · about 4 min", not "depth 1" | a number of levels says nothing; minutes do | [02](02-flow.md) frame 1 |
| **W14** | **One TOURS section in the sidebar: the list of tours, or the route of the open one.** `‹ Tours` goes from the route to the list without closing the tour | a list of tours and a tour's stops are two levels of one thing | [07](07-tour-list.md) §2 |
| **W15** | **The list has two views: All, and On the selected node.** The second follows the selection and says why each tour is there (`starts here` · `stop 3 of 10` · `4 stops inside`) | "is there a tour about this?" is the usual question | [07](07-tour-list.md) §4 |
| **W16** | **A tour opens at its root** (the cover, the map fitted from its first stop); one you left, where you left it; from "On a node", at that node's stop | each is the reason you clicked | [07](07-tour-list.md) §5 |
| **W17** **ask** | **A node on tours shows `▶ N`** on its card and row, like the docs count | the answer is on the node before you ask | [07](07-tour-list.md) §6 |
| **W18** | **`＋` Ask for a Tour… copies a sentence for the agent** | a click cannot reach the agent yet; the button is where the real one will be | [07](07-tour-list.md) §7 |

## 4. The one picture

You asked for two levels of `checkout_order`. You are in the third stop of ten, on the second of its three steps. The agent is writing stop 6.

```
┌ side bar: V-NOC ───────────────┐┌ V-NOC canvas ────────────────────────────────────────────────────────────────────────────────┐
│ STRUCTURE              ⊞  ⊟  ⋯ ││ [←] [Main] [▶ checkout_order ✕] │ checkout_order › reserve_stock       writing 6/10 ■  ⌖ ⓘ ⋯ │
│ Search nodes…                  ││                                                                                              │
│   MAIN                         ││   ┌────────────────────┐      ┌────────────────────┐                                         │
│   ⌖ minicart › app             ││   │ ✓1 ƒ checkout_order│━━┳┄┄┄│ ✓2 ƒ check_stock   │                                         │
│   ▸ checkout.py                ││   └────────────────────┘  ┃   └────────────────────┘                                         │
│   ▸ inventory.py               ││                           ┃   ┏━━━━━━━━━━━━━━━━━━━━┓      ┌────────────────────┐             │
│ ══════════ drag ══════════════ ││                           ┣━━━┃ ●3 ƒ reserve_stock ┃┄┄┄┄┄┄│  4 ƒ check_stock ⇢2│             │
│ ┃ ‹ Tours ▶ checkout_order   ✕ ││                           ┆   ┗━━━━━━━━━━━━━━━━━━━━┛      └────────────────────┘             │
│ ┃ ✓  1  ƒ checkout_order       ││                           ┆   ┌────────────────────┐      ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐             │
│ ┃ ✓  2    ƒ check_stock        ││                           ├┄┄┄│  5 ƒ charge_payment│┄┄┄┄┄┄┆ ◌6 ƒ retry_charge  ┆             │
│ ┃ ●  3    ƒ reserve_stock      ││                           ┆   └────────────────────┘      └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘             │
│ ┃           what it is for     ││                           ⋮   stops 7 to 10 below, not written yet                           │
│ ┃         ▸ Refuse what is n…  │├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ┃           Take the units, …  ││ ● 3  ƒ reserve_stock   inventory.py:19                      stop 3 of 10 · 2 of 3    ↗   ⌄   │
│ ┃ ○  4      ƒ check_stock  ⇢ 2 ││ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│ ┃ ○  5    ƒ charge_payment     ││  19    def reserve_stock(sku: str, quantity: int) -> … │ Refuse what is not there            │
│ ┃ ◌  6      ƒ retry_charge     ││  20        """ID: 8a9cc864-…"""                        │                                     │
│ ┃ ◌  7    ƒ cancel_checkout    ││  21 ▌      if not check_stock(sku, quantity):          │ It checks the stock again by        │
│ ┃ ◌  8      ƒ release_stock    ││  22 ▌          raise ValueError(f"Not enough stock…")  │ itself. checkout_order already did, │
│ ┃ ◌  9    ƒ create_order       ││  23                                                    │ but a caller that skips the check   │
│ ┃ ◌ 10      ◆ CompletedOrder   ││  24        _STOCK[sku] -= quantity                     │ must not push the count below zero. │
│ ┃ writing 6 of 10       ■ Stop ││  25        remaining = _STOCK[sku]                     │                                     │
│                                ││                                                        │ ▰▰▰▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└────────────────────────────────┘└──────────────────────────────────────────────────────────────────────────────────────────────┘
   route: the order                  map: where it is (top)  ·  stage: this one piece, code and sentence together (bottom)
```

What to notice:

- **The text is one short paragraph**, next to the two lines it is about. The other 19 steps are not on screen as text anywhere.
- **The place is said in stops**: "stop 3 of 10 · 2 of 3". The stops are known from the first second; the number of steps is not, until the
  agent has written the last one.
- The **route** opens only the stop you are in: its steps are three one-line titles. Every other stop is one row.
- The **map** is the redesign's canvas with one change: a stop number where the chevron was. Heavy edges run from the start to where you are.
- Stops 4 and 5 are written (`○`), 6 to 10 are not (`◌`). You can already click any of them: the **code** of a stop is known before its text.
- Your MAIN context is still there, above the tour's panel and as a chip on the bar.

## 5. What you still have to decide

| # | Question | My default | The other choice |
|---|---|---|---|
| **A1** | W4: where is the stage? | the bottom dock, code and text together | the prototype's popover on the card's lines: livelier, but it covers the next cards, needs placement rules, and has no room for a question box in phase 3 |
| **A2** | W2: is a tour a context? | yes | its own view in the sidebar and its own canvas mode: a second way to "be somewhere else" beside EXPLORE |
| **A3** | W9: a short stop is one step? | yes | the phase document's intro step plus a block step for every stop |
| **A4** | W1: "tour" on screen? | yes | keep "walkthrough" and live with VS Code's meaning of the word |
| **A5** | Does `✕` on a tour that is still being written also stop the agent? | no: it keeps writing, the status bar says so, `■` is one click away | yes: simpler to explain, but a tour you left for a minute is cut short |
| **A7** | W17: `▶ N` on every card and row that is on a tour? | yes, in the footer, only when N > 0 | no mark on the node: the list's "On the selected node" view and the menu's Show Tours are the only ways |
| **A6** | The code pane of the stage, or the real editor? | the stage's own code pane; `↗` opens the real file at the lines | an "Editor follows" switch that also moves and marks the editor on every step. Possible later without changing anything here |

## 6. Where the interactive tour will plug in

A first design is in [`08-interactive.md`](08-interactive.md). Each phase 3 verb already has one obvious home, so the basic layout does not have to move:

| Phase 3 | Its home in this layout | Already here in basic as |
|---|---|---|
| **Deeper** | a card's `+N` on the map ("2 calls not on this tour"), and the call's name in the stage's code | the `+N`, shown dim, with that tooltip and no click |
| the stops a detour adds | cards joined to the map where the call is; rows nested under the stop in the route | nesting and numbering by parent stop, not by position |
| **Ask** | one input line under the step's text | the empty space under the text |
| an answer | a step of its own in the stage, a row under the stop it was asked on | the route's "steps of the current stop" rows |
| **Back** from a detour | the bar's `←` and the crumbs (`checkout_order › reserve_stock › line_tax`) | `←` is already "where I was before I jumped"; the crumbs are already the path of stops |
| "detour 1 · depth 2" | the stage's header, beside "stop 3 of 10 · 2 of 3" | the header |
| **Pause** writing | the `■` slot | the slot |
| **Save as doc** | `＋` in the stage's header, the icon a card uses for Add Doc | the docs panel `ⓘ` already follows the current stop |

Next: [`01-layout.md`](01-layout.md)
