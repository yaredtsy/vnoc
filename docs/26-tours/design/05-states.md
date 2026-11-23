# 05 States: what the agent is doing, what you have read, what went wrong

```
states
├── 1. the tour's states, on every surface
├── 2. Exit and Stop are two things (W7)
├── 3. a stop's states, on every surface
├── 4. the code changed
├── 5. the canvas hidden, two windows, a second tour
├── 6. keys
├── 7. what the UI needs from the file and the core
└── 8. checks
```

## 1. The tour's states

From the file's `status`, plus one the file cannot say by itself (no news).

| State | It means | Route foot | Bar | Stage | Status bar |
|---|---|---|---|---|---|
| **writing** | `generating`: the agent is in its loop | `writing 6 of 10` `■ Stop` | `writing 6/10 ■` | an unwritten stop shows grey bars | `· writing 6/10` |
| **complete** | `complete` | no foot | nothing | — | `▶ Tour 3/10` |
| **stopping** | you pressed `■`; the file says `stopped`; the agent has not noticed yet | `stopping: the agent finishes stop 6` | the same | unchanged | `· stopping` |
| **stopped** | the agent's next command was answered with "stopped" (§7) | `stopped: 6 of 10 written` | `stopped at 6 of 10` | an unwritten stop: *"Not written: the tour was stopped here. Ask the agent to continue it."* | `▶ Tour 3/10` |
| **no news** | `generating`, and the file was not touched for 5 minutes: the agent's session ended, crashed, or waits for a permission in its terminal | `no news from the agent for 5 min · 6 of 10 written` | `no news from the agent` | an unwritten stop: *"Not written. The agent may have stopped: look at its terminal, or ask it to continue the tour."* | `· no news` |
| **cannot be read** | the file is gone, or its `schema` is newer than this extension | — | — | the context closes with one message: *"The tour of checkout_order is no longer in the cache."* / *"This tour was made by a newer V-NOC."* | — |

- **No news is not an error.** The UI does not know the agent died; it knows the file is quiet. It says only that, and the written stops play
  as before. If the agent writes again, the state is *writing* again without a word.
- No state has a spinner that turns forever in the middle of the canvas. Writing is one dim line in two places and a dot on the chip.
- No state opens a dialog.

## 2. Exit and Stop are two things (W7)

| | **Exit** | **Stop** |
|---|---|---|
| the wish | "I want to look at something else" | "the agent should stop writing" |
| the control | `✕` on the tour's chip or panel · `Esc` on the canvas · **Close Tour** | `■` on the bar or the route's foot |
| shown | always | only while the agent writes |
| what happens | the tour's context closes; the canvas draws the context it came from | the file's `status` becomes `stopped` (through the core); the UI says *stopping*, then *stopped* |
| the agent | **keeps writing** (README A5) | ends at its next command, at most one stop later |
| what is kept | the file, your step, your seen marks. One click on the status bar or **Open Tour…** brings it back | everything written |
| can it be undone | yes: open it again | in basic: ask the agent to continue. Resume is phase 3 |

Neither asks "are you sure": Exit loses nothing, and Stop loses only what was not written yet.

## 3. A stop's states

Your progress (seen, here, ahead) and the agent's (not written, being written) and three facts about the text (explained before, fallback,
changed). Each has its own channel, so they can meet on one stop.

| State | Route row | Map card | Stage | Segment |
|---|---|---|---|---|
| **seen** | `✓`, dim | badge `✓1`, dim | — | filled |
| **here** | `●` in the focus colour; its step rows under it | badge `●3` filled; the ring; heavy edges from the first stop | it is what the stage shows | filled |
| **ahead** | `○` | badge ` 4`, outlined | — | hollow |
| **not written** | `◌`, dim | badge `◌6`; a **dotted** outline | grey bars, the code readable | dotted |
| **being written** | `◌` and `writing…` | the badge turns; footer `writing…` | *"The agent is writing this stop."* | dotted, pulsing |
| **explained before** | `⇢ 2` at the right | footer `⇢ explained at 2` | one sentence, **Go there** | as its mark |
| **fallback** | `⚠ fallback` at the right | footer `fallback` | the title is the line range; `⚠ No explanation was written for these lines.` | as its mark |
| **changed** | `changed` at the right | footer `changed` | the lit lines dashed; *"The code changed after this was written."* | as its mark |

| Channel | Row | Card |
|---|---|---|
| the mark / the badge | seen · here · ahead · not written | the same |
| the outline | — | here (solid ring) · not written (dotted) |
| the right-hand word | one of: `writing…` > `⚠ fallback` > `changed` > `⇢ 2` | the footer's right side, the same order |
| colour | only "here" has one | only "here" has one |

Not written and here can meet (you went ahead of the agent): the ring is drawn over the dotted outline, the badge is `◌` in the focus colour.

## 4. The code changed

A tour is about one state of the code. Each stop keeps the `code_hash` of its node from when the tour started.

| What changed | Shown |
|---|---|
| a stop's code | the stop is **changed** (§3). The stage shows the code as it is **now**; the text is the old one, with the line under it |
| a stop's node is gone (deleted, or renamed without an ID) | the row and the card stay, struck through, `gone`. The stage has no code pane; the text stays, with *"This is no longer in the code."* |
| the tour's first node is gone | the tour still opens; its cover says *"checkout_order is no longer in the code. The tour shows what was written about it."* |
| many stops (a branch switch, a big edit) | the route's foot: `7 of 10 stops changed since this was written`. The cover: *"Most of this tour is about older code. Ask the agent for a new one."* |
| a stop that was not written yet | nothing: when the agent gets to it, it reads the code as it is then |

The UI never repairs a tour, and never hides one. It says what no longer matches (T9).

## 5. The canvas hidden, two windows, a second tour

| Case | Rule |
|---|---|
| a tour starts, the canvas is hidden | the notification and the status bar item ([02](02-flow.md) frame 2). Nothing opens |
| you close the canvas tab during a tour | the tour's context stays (its panel, the status bar item). **A tour plays on the canvas**: a click on a route row, the status bar item or **Open Tour…** opens the canvas again, beside the editor, on your step. There is no tour without the canvas in basic |
| the sidebar is closed | the tour plays without the route: the stage and the map are enough. The crumbs and the segments still say where you are |
| two windows on one worktree | each has its own place in the tour (01 §6). `■` in one stops it for both, since there is one file |
| the agent starts a **second** tour while one is open | the open one is not replaced under you. The status bar says `▶ New tour: api.py`, and the notification of frame 2 appears. Opening it replaces the tour's context; the first tour stays in **Open Tour…** |
| a tour is opened from **Open Tour…** while another is open | it replaces the tour's context. There is one tour context per window |

## 6. Keys

When the canvas has the keyboard and a tour context is active. The canvas's own keys (`../../16-ui-redesign/03-canvas.md` §7) move between cards;
here the cards are the stops, so the arrows keep their direction and change their unit.

| Key | In a tour |
|---|---|
| `→` · `Space` | Next step |
| `←` | Previous step |
| `↓` · `↑` | next / previous **stop** |
| `Alt+←` | back to where I was before the last jump |
| `Home` · `End` | the cover · the end card |
| `Enter` | on the cover: Start. On the end card: Close Tour |
| `Ctrl/Cmd+Enter` | Go to Code |
| `Esc` | close the tour's context (Exit). With the stage folded: unfold it first |
| `.` | fold / unfold the stage |

The route panel has the sidebar's keys (`↑` `↓` move the row focus, `Enter` jumps). Nothing in a tour needs the pointer.

## 7. What the UI needs from the file and the core

The phase document's session file is enough for almost everything. The design needs **six small fields** and **two checks** more.

| Field | On | For | Who knows it |
|---|---|---|---|
| `first_seen: "s1"` | a contextual stop | `⇢ 2` and **Go there** | the visit list (the prototype's `first_seen_order`); the phase document returns it from `vnoc walk next` but does not store it |
| `via_line: 16` | a stop reached by a call | `↳ called on line 16`, the call's link in the stage's code, the call card's meta line | the visit list: it walked that call |
| `writing: "s5"` and `updated_at` | the session | "the agent is on stop 6"; the turning badge; **no news** after 5 minutes | `vnoc walk next` sets `writing` to the stop it hands out; every command of the agent bumps `updated_at` |
| `stop_seen: true` | the session | *stopping* becomes *stopped* | the core sets it when it answers a command of the agent with "stopped" |
| `step_estimate: 20` | the session | the cover's "about 7 min" | `vnoc walk estimate` already computes it; `start` copies it in |
| `created_at` | the session | the tour list's "today 18:40" and its order ([07](07-tour-list.md) §3) | `vnoc walk start` |

| Check in `vnoc walk write` | For |
|---|---|
| the text budget, per verbosity (03 §5) | W5 |
| a stop under 8 lines has an intro and no blocks (03 §5) | W9 |

Nothing else changes in the phase document's commands. The route, the IDs, the checks and the stop mechanism are used as they are.

## 8. Checks

Against the mockup first, then against the real thing.

- [ ] At no moment is the text of more than one step on screen.
- [ ] `Next ›` is at the same place on screen from the cover to the end card.
- [ ] From any step, one click or key gives MAIN back as it was, and one more gives the tour back on the same step.
- [ ] You can tell seen, here, ahead and not written apart in a greyscale screenshot.
- [ ] You can read the code of a stop the agent has not written.
- [ ] After `■` the UI says *stopping* until the agent has noticed, never *stopped* before.
- [ ] A tour of 40 stops: the route is 40 rows and a few step rows, the segments fit, the cover fits the map.
- [ ] A canvas 520 px wide: the stage stacks, nothing is cut, `Next ›` is reachable.
- [ ] Dark+, Light+, High Contrast: the lit lines, the ring and the dotted outline are visible in each.
- [ ] *Reduce motion*: nothing fades, pulses or pans.
- [ ] VoiceOver reads a step as *"stop 3 of 10, function reserve_stock, step 2 of 3. Refuse what is not there. It checks the stock again by
      itself…"*, and a route row as *"stop 6, function retry_charge, not written, the agent is writing it, level 2"*.
- [ ] With `reserve_stock` selected, the list answers "is there a tour about this?" without a click, and one click opens a tour **on** it.
- [ ] Leaving a tour halfway and opening it again from the list lands on the same step.
- [ ] **Watch one person** who has not seen Minicart take the tour in the mockup, in silence. Write down every place they hesitate. This is
      the phase 3 document's step 3.10, moved to before anything is built: a mockup is the cheapest place to find out.

Next: [`06-mockup-brief.md`](06-mockup-brief.md)
