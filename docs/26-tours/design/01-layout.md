# 01 Layout: the jobs, the rules against walls of text, the three parts

```
layout
├── 1. the jobs a reader comes with
├── 2. five rules against the wall of text
├── 3. the words
├── 4. a tour is a context (W2)
├── 5. the three parts, and what is never in each
├── 6. who draws what, in which webview
└── 7. not in the basic tour
```

## 1. The jobs

Every control in this folder serves one of these. A control that serves none goes. They continue the list of `../../16-ui-redesign/01-layout.md` §2.

| # | Job | Example | Where it is done |
|---|---|---|---|
| **T1** | **Know what I am about to get** | "how long is this tour, what is on it?" | the agent's one question; the cover |
| **T2** | **Understand one piece** | "what do these four lines do, and why?" | the stage |
| **T3** | **Keep my place** | "which stop is this, how many are left?" | the route's `●`, the stage's "stop 3 of 10 · 2 of 3", the ring on the map |
| **T4** | **See where it sits** | "who calls this, what does it call next?" | the map |
| **T5** | **Skip and return** | "I know `check_stock`; take me to `charge_payment`" | click a row or a card; `↑` `↓` |
| **T6** | **Start reading before it is finished** | the agent is on stop 6, I am on stop 2 | every part shows what is written and what is not |
| **T7** | **Leave, and come back** | "let me look at `api.py` for a minute" | `✕` / the Main chip; the status bar item |
| **T8** | **Make the agent stop** | "this is enough" or "this is the wrong function" | `■` |
| **T9** | **Trust it** | "is this text about the code as it is now? did the agent really write it?" | the *changed* and *fallback* marks |

## 2. Five rules against the wall of text

You asked for this by name. A tour fails when it becomes a long document with a Next button.

| # | Rule | What it forbids |
|---|---|---|
| **1** | **One step is on screen.** Its text is in the stage and nowhere else | an outline with a summary under every row; a "full text" view; the tour pasted into the chat |
| **2** | **A step is a title and one short paragraph**, with a budget the core checks when the agent saves it ([03](03-stage.md) §5) | a scrollbar in the step; a heading or a table inside a step |
| **3** | **The code says most of it.** The step sits beside the lines it explains and says only what the lines do not: the reason | text that retells the code line by line |
| **4** | **Lists show names, not prose.** A route row is a name; a step row is its title; a card is a card | "description" columns; tooltips that hold the paragraph |
| **5** | **The agent answers in one line** in its chat: what it started and how big it is | "Here is a summary of the walkthrough: …" |

Rule 3 is for the skill's writing rules, rule 5 for the skill's reply rule. The others are layout, and are in this folder.

## 3. The words

From the phase document, kept: **stop**, **step**, **block**. New: **tour**, **route**, **map**, **stage**, **cover**, and the four marks.

| Word | Meaning | On screen |
|---|---|---|
| **tour** | one session file: a start node, a depth, a list of stops | chip `▶ checkout_order`, panel `TOUR` |
| **route** | the stops in order, nested by the stop each was reached from | the sidebar panel |
| **stop** | one node on the route. Numbered from 1 on screen (`s0` is stop 1) | a row, a card |
| **step** | one thing the stage shows: a stop's intro, or one of its blocks | the stage; "stop 3 of 10 · 2 of 3" |
| **block** | a range of lines in a stop with a title (the file's `focus`) and a text | the lit lines |
| **stage** | the dock that shows the current step | bottom of the canvas |
| **cover**, **end** | the stage before step 1 and after the last step | [02](02-flow.md) frames 2 and 9 |
| **seen** `✓` | you were on every step of the stop | kept by the extension, per tour |
| **here** `●` | the stop of the current step | the context's selection (W6) |
| **ahead** `○` | written, not seen | |
| **not written** `◌` | on the route, the agent has not saved it yet | from the file: `state: "pending"` |

Two things the phase document calls by one name are apart here: a stop's **state in the file** (pending, ready, degraded: the agent's
progress) and its **mark for you** (seen, here, ahead: your progress). The route shows your progress; the file's state only decides `◌` and the
*fallback* mark.

## 4. A tour is a context (W2)

The redesign has MAIN and EXPLORE contexts: each has a panel in the sidebar, a chip on the bar, and the one canvas draws the active one
(`../../16-ui-redesign/03-canvas.md` §4). A tour is a third kind.

| | EXPLORE | TOUR |
|---|---|---|
| opened by | selecting a call | a tour file appearing (the agent ran `vnoc walk start`), or a click on a tour in the TOURS list ([07](07-tour-list.md)) |
| panel in the sidebar | the callee's structure | the route |
| chip on the bar | `◇ reserve_stock ✕` | `▶ checkout_order ✕` |
| the canvas draws | the callee's lineage | the route as cards, and the stage under it |
| focus | a path you change | fixed: the tour's start node. There is no Focus in a tour |
| selection | any card | **the current stop** (W6). Selecting a card or a row **is** going to that stop |
| open / closed | yours | fixed: every stop on the route is drawn |
| closes when | the selection in its parent moves on (R4) | **only** by `✕`, `Esc` on the canvas, or the end card's **Close**. Clicking in MAIN does not close it |
| saved with the session | yes | yes: the tour's id, the current step, the seen steps |

What this buys:

- **Leaving costs nothing** (T7). Click the Main chip: the canvas draws MAIN, pan and zoom as you left them. Click the tour's chip: you are on the
  same step.
- **No restore.** The tour context owns its own selection and its own viewport. It never writes into MAIN and never touches an editor tab.
- **One tour at a time is shown.** The core keeps one active tour per worktree. Starting a second one replaces the tour context (the old tour
  stays in the cache and in the TOURS list).

## 5. The three parts

| Part | Surface | Shows | Never shows |
|---|---|---|---|
| **route** | webview sidebar, a panel like a context panel | the stops in order with their marks; **the steps of the current stop only**, as titles; what the agent is doing, in one line | a step's text; code |
| **map** | the canvas, tour context active | the stops as cards with numbers, the edges between them, the ring on the current one | text about a stop; code (the `</>` Code in Card button is not on tour cards) |
| **stage** | a dock inside the canvas webview, under the map | **one** step: the stop's code with the block lit, the title, the paragraph, Prev / Next, how far | another step; a list |

The three stay in step with each other through one value: the current step. Change it anywhere (Next, a row, a card, a key) and all three follow.

Why the stage is inside the canvas and not a panel of its own (W4):

| Choice | Problem |
|---|---|
| the prototype: a popover tied to the lit lines inside the card on the canvas | the card has to grow to show its code, so the map reflows on every step. The popover covers the cards to the right, which are the stops that come next. It needs rules for which side has room. A question box and an answer do not fit in it |
| the phase document: the code in an editor tab, the text in a webview panel | the eye crosses the window between the lines and their sentence. With the sidebar and the canvas that is four columns. The editor's tabs are changed and have to be put back |
| **here: a dock under the map** | costs map height. Answered by the dock's two sizes and the mini bar ([03](03-stage.md) §6) |

## 6. Who draws what

```
   ┌ sidebar webview ───────┐      ┌ canvas webview ──────────────────────────────┐      ┌ VS Code ────────────┐
   │ context panels         │      │ the bar (one more chip kind)                 │      │ status bar item     │
   │ + the TOUR panel       │      │ the map (cards with stop numbers)            │      │ one notification    │
   │   (the route)          │      │ the stage (new)                              │      │ Open Tour… (palette)│
   └───────────┬────────────┘      └──────────────────────┬───────────────────────┘      └──────────┬──────────┘
               │ intents ▲ session, tour                  │ intents ▲ session, tour                 │
               ▼         │                                ▼         │                               ▼
   ┌ extension host ─────────────────────────────────────────────────────────────────────────────────────────────┐
   │ SESSION: contexts (one of kind "tour": tour id · current step · seen steps)                                 │
   │ TOUR: the session file, read again each time the watcher fires. Never written here except "stop" (via core) │
   └──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
               ▲ file watcher
   .vnoc/cache/walkthroughs/w_5d20.json   ◀── written by the core for the agent (vnoc walk start / write / finish)
```

- **Where you are is yours, not the file's.** The current step and the seen steps are in the extension's session, per window. The agent's file
  holds only what was written. Two windows on one worktree can be on different steps of the same tour.
- The stage's code comes from the index (the same `code` request a card's `</>` uses), not from the tour file. That is why a stop that is not
  written yet can already show its code.

## 7. Not in the basic tour

- Deeper, Ask, Back, Pause writing, Save as doc (the interactive tour; README §6 lists the seams).
- Starting a tour from the UI. In basic you ask the agent in its chat (the phase document's Q2.4). A **Walk Me Through…** entry on a node's
  menu comes with phase 1's requests.
- "Editor follows": moving and marking the real editor on every step (README A6).
- Playing with the canvas closed. A tour plays on the canvas; every tour gesture opens it ([05](05-states.md) §5).
- Keeping a tour in the project, sharing it, exporting it (the phase document's Q2.1).
- Sound, auto-play, speed.

Next: [`02-flow.md`](02-flow.md)
