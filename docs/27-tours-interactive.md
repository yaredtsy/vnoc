# 27. Tours, interactive: ask, go deeper, come back

Written 2026-11-23, with [`26-tours/`](26-tours/README.md), from `26-tours/design/08-interactive.md` (what it looks like) and the
conversation of the same day; revised the same day, after 26 moved the reader's place into the core (26 D17). **This is the second phase. It is built after 26, and it is a first plan:** the questions it has to answer
are named and each has a pick, the file format is settled far enough that 26 leaves the right room, and the steps are listed without
their files and tests. Those are written when 26's trial (TR14) and its by-hand pass (TR24) are done, because both will change this.
**Nothing here was run.**

```
interactive tours
├── 1. what it is                    one thing: a side trip
├── 2. the three questions           how a click reaches the agent · where a side trip is kept · what the agent runs
├── 3. reaching the agent            the chat · the agent listens · the extension starts it · MCP; the request is a record every way
├── 4. where a side trip is kept     the reader's own file, beside the tour
├── 5. the records
├── 6. the commands                  answer (--here) · add --trip · wait · asks
├── 7. what 26 leaves ready
├── 8. order of work                 TI0 – TI10
└── 9. asks, open checks
```

## 1. What it is

`design/08` stands as the picture. While you read a step you can **ask** about it, or **go deeper** into a call the tour left out. Both
give a **side trip**: steps like any other, hanging from the step where you started, with one way back. Its five rules:

| # | Rule | What it asks of the data |
|---|---|---|
| 1 | the main route never changes | a side trip is never a stop of the route. The tour's own file is not written for it (§4) |
| 2 | every side trip has one way back | a side trip knows the step it hangs from |
| 3 | you never wait | asking returns at once; the answer arrives as the file grows, as a stop does in 26 |
| 4 | an answer is a step | the same record, the same rules, the same budget as 26's `step` |
| 5 | what you asked stays with the tour | it is on disk, found again by that step's id, which never moves (26 D5) |

## 2. The three questions

26 is almost a one-way street: the agent writes, the person reads. Two things already go the other way: Stop (read by the agent at
its next write), and the reader's place, which the agent can read and move (`vnoc tour where`, `go`: 26 D17). Interactive needs a
question to reach an agent **that is doing nothing**.

| # | Question | Pick |
|---|---|---|
| 1 | How does a question reach the agent? | **Three ways, one queue, built in this order.** The chat you already have with the agent (it knows where you are); an agent that is *listening* for questions asked on the canvas; the extension *starting* an agent (§3) |
| 2 | Where is a side trip kept? | **In the reader's own file, beside the tour**, never in the tour's file (§4) |
| 3 | What does the agent run? | `vnoc tour answer` or `vnoc tour add --trip` to write the side trip; `vnoc tour wait` to get a request made on the canvas (§6) |

## 3. Reaching the agent

| Way | How | Good | Not good |
|---|---|---|---|
| **0. The chat** | you type the question to your agent, as you do now: "what if two orders reserve the last unit?" The agent runs `vnoc tour where`, knows the step, and instead of answering in the chat runs `vnoc tour answer <tour> --here`: the answer is a side trip at your step, and `go` shows it | nothing to build but `answer` and the side trip on screen. No waiting, no queue: the agent is in its turn. It is the agent that wrote the tour, with everything it read | you leave the canvas to type. The question is asked in one window and answered in another |
| **A. The agent listens** | after `finish` the agent runs `vnoc tour wait <tour>`: it blocks until a request is in the file, prints it with the step it is about, and exits. The agent answers and waits again | no new machinery: one command and a loop in the skill. Works in every agent that has a shell. The agent that wrote the tour answers, with everything it read still in its context | only while that session is alive and waiting. A command that blocks meets each agent's command time limit, so `wait` returns after a while with "nothing yet, run me again", and every return is a turn |
| **B. The extension starts the agent** | on a request with nobody listening, the extension runs the user's agent without a window, with one prompt: answer request `q3` of this tour. A setting holds the command for Claude Code and for Codex | a question next week is answered. Nothing has to stay open | a second way for V-NOC to run a program; the agent's own permissions when it runs unattended; it starts cold, so its context is the tour itself (`vnoc tour show --text`) and the code. **[check]** TI0: each agent's flags for a run without a window, and for continuing a named session |
| **C. MCP** | a server the agent's host keeps open | a warm channel both ways | postponed in 21.08 for reasons that still hold |

**0 first, then A, then B.** 0 is possible because 26 put the place in the core: it gives the answer-as-a-step, the way back, and
"finding it again" with no new channel, and it is how the side trip's look is judged. A adds the ask line on the canvas. B is what
makes it a feature for someone who closed their terminal.

**The request is a record in every way.** In way 0 the agent writes the `ask` itself (with your words) and the answer after it, so a
question asked in the chat is found again on the tour like any other. On the canvas, the UI writes `ask` through the core and shows
`answering`. It does not know or care who
answers. A listening agent takes it; else, when B exists, the extension starts one; else it stays open, the row says
`waiting for an agent`, and the next agent that runs `vnoc tour asks` finds it. That is the same shape as 26's Stop: a line in a file,
read by whoever comes next.

**A cold agent can answer** because a tour is its own context: the question, the step it was asked on, the lit lines and the tour's
texts are what the answer needs, and `wait` prints them. So B does not depend on finding the old conversation; continuing it (a
`session` key in the `tour` record, written by `start` when the agent gives one) only makes the answer better. **[check]** TI0.

## 4. Where a side trip is kept

```
.vnoc/tours/checkout-order-7k2q.jsonl               the tour (a project tour here; a private one is in local/tours/)
.vnoc/local/tour-asks/checkout-order-7k2q.jsonl     NEW: this reader's questions, answers and deeper trips for that tour
```

- **Always the second file, for private tours too.** One rule; and rule 1 (§1) becomes a fact about bytes: nothing a reader does
  writes the tour's file. Sharing a tour does not share your questions. A committed tour is not dirtied by reading it.
- The second file is a log of the same kind: one record per line, only added, the same lock. Its records point into the tour by `on`: the id of the step a side trip hangs from. (`parent` is taken: in 26 it is the block a detail is inside.)
- A tour is replayed, then its second file. An `on` that no longer exists (the step was dropped) leaves the side trip listed under
  the tour's cover as *"asked on a step that is no longer in the tour"*.
- **Keep** (`＋ Keep` on an answer) is how one becomes the team's: it is saved as a doc on the node, by the attach flow, as a person's
  act. The record gets `kept`.
- Deleting a tour moves all three files to the trash: the tour, its state (26.01 §9), and this one. Not in `local/tours/`, so that 26's rule "every `.jsonl` there is a tour" stays true
  and a V-NOC with only 26 never reads it.

## 5. The records

In the second file. `step` and `stop` are 26's own records with one more key.

| `type` | Written by | Keys | Means |
|---|---|---|---|
| **`ask`** | the core, for the ask line; `answer --here`, for a question asked in the chat | `id` (`q1`) · `on` (a step id, of the tour or of a side trip) · `lines`? (when the reader selected some) · `text` · `at` | a question, open |
| **`deeper`** | the core, for a `+N`, a chip, a call in the code | `id` (`d1`) · `on` · `line` · the callee's name | "explain this call", open |
| **`taken`** | `wait` | `id` · `by` | an agent has it. Two listeners do not answer twice |
| **`step`** | `answer` | 26's keys, and `trip: "q1"`; id `q1.a1` | the answer: lit lines, a title, one paragraph. A follow-up's answer is `q1.a2` |
| **`stop`**, **`step`** | `add --trip d1` | 26's keys, and `trip: "d1"`; ids `d1.s1`, `d1.s1.b1` | the stops of a deeper trip |
| **`done`** | `answer`, `finish --trip` | `id` | the trip is written |
| **`failed`** | the agent, or the extension when a started agent ended without answering | `id` · `why` | shown on the row, with **Ask Again** |
| **`kept`** | the core, for Keep | `id` · the doc's path | |
| **`drop`** | the core, for a row's Delete | `id` | |

Side trips nest: an `ask` whose `on` is `d1.s2.b1` is a question inside a deeper trip. The header then reads
`from stop 6 › send_charge`, as the design draws it.

A deeper trip that would be more than six stops is offered as **a new tour** (design 08 §7). That tour's `tour` record gets
`from_tour` and `from_step`; the list nests it under the one it came from. It is an ordinary 26 tour in every other way.

## 6. The commands

```
vnoc tour wait   <tour> [--timeout <s>]              the next open request, with what it is about. Marks it taken
vnoc tour asks   <tour>                              the open requests, without waiting
vnoc tour answer <tour> <q> --body-file <file>|-     one step: "## 21-24 Title" and one paragraph. Checked as 26's add
vnoc tour answer <tour> --here --question "…" --body-file <file>|-
                                                     way 0: a question from the chat. Writes the ask at the reader's place, and its answer
vnoc tour add    <tour> <node> --trip <d> …          a stop of a deeper trip; 26's add, written to the reader's file
vnoc tour finish <tour> --trip <d>
```

`wait` prints what an agent with nothing else in its context needs:

```
q3 · a question on stop 3, step 3 (reserve_stock, app/inventory.py:24-28)
  "What if two orders reserve the last unit at the same time?"
the step:   Take the units, warn when few are left
            The count drops at once; there is no separate "reserved" number. …
the lines:  24  _STOCK[sku] -= quantity
            …
answer:     vnoc tour answer checkout-order-7k2q q3 --body-file -      (one step: lines, a title, one paragraph)
```

| Exit | Means |
|---|---|
| `0` | a request was printed |
| `1` | nothing arrived in the time given: run it again, or stop listening |

The skill's additions. **Way 0:** when the user asks about the tour in the chat, run `where`, then answer with `answer --here` and the
user's own words as `--question`, not in the chat; say one line ("Answered on the tour, at stop 3."). **Way A:** after *finish*, ask
the user once whether to stay for questions asked on the canvas; if yes, loop `wait` → answer. An answer
follows every rule of a step (why, not what; one paragraph; the budget). **"I do not know" is an answer:** one sentence saying what
was looked at.

## 7. What 26 leaves ready

Each of these is in 26 because of this plan. If 26 changes one, this file changes with it.

| 26 | Used here as |
|---|---|
| the log, and readers that skip a `type` they do not know (26.01 §2, §8) | the new records; a V-NOC with only 26 plays a tour that has side trips, without them |
| ids that never move or come back (D5) | a side trip's `on`; the place |
| the place in the core, `where` and `go` (D17) | way 0: the chat is the ask line. `answer --here` asks the core where the reader is; `go` shows the answer |
| the place's `At` as an enum | one more case: a step of a side trip. `Back` out of it is the `back` list, as for any jump |
| the lock and `store::append` | the second file's writes |
| the core writing a record for the UI (`vnoc/tourHalt`) | `vnoc/tourAsk`, `vnoc/tourDeeper`: the same path |
| `vnoc/toursChanged` and the extension asking again | an answer arriving |
| the watcher's split (D12) | `.vnoc/local/tour-asks/` joins the two folders |
| `add`'s checks as a function with no disk (`plan_stop`) | `answer` and `add --trip` are that function with another target |
| the allow rules per command (26.04 §5) | `wait`, `asks` and `answer` join the first group: they write only the reader's own file |
| the design's seams: the space under the text, the dim `+N`, `←` as "where I was", the crumbs | the ask line, the deeper button, Back from a side trip |
| the stage's step as one component for intro, block and short stop | an answer is one more kind of step |

## 8. Order of work

| Step | What |
|---|---|
| **TI0** | checks: whether an agent in a chat reliably answers on the tour instead of in the chat (way 0, with a hand-made `answer`); each agent's command time limit against `wait`; running Claude Code and Codex without a window, with a prompt, with and without a session to continue; what it costs in time and tokens to answer one question cold |
| **TI1** | the records of §5; replaying a tour with its second file; nesting; the orphan rule; the place inside a side trip |
| **TI2** | `answer` (with `--here`), `add --trip`, `finish --trip`; `TourView` carries the side trips of each step |
| **TI3** | on screen: the answer as a step, **Back to stop 3** (`design/08` §4); finding it again: the route's rows, the `?` in the gutter, the count on the card and in the list (`design/08` §6) |
| **TI4** | the skill's way 0 and the guide; a trial: ten questions asked in the chat, the answers read as a person would. **Stop point: ask in the chat, read the answer on the tour** |
| **TI5** | `wait`, `asks`; `vnoc/tourAsk`, `vnoc/tourDeeper`; the ask line and `answering` (`design/08` §3); the skill's listening loop |
| **TI6** | a trial of way A: questions asked on the canvas, a listening agent |
| **TI7** | go deeper: the call in the code, the chip, the `+N`, the side trip's cards (§5) |
| **TI8** | Keep as a doc; Delete; **Ask Again** |
| **TI9** | reach B: the extension starts the agent; the setting; `failed` |
| **TI10** | a side trip too big: a new tour, nested in the list (§7) |

Stop points: after TI4, a question asked in the chat is answered on the tour. After TI6, a question asked on the canvas is answered
on the canvas by a listening agent.

## 9. Asks, open checks

| # | Question | My pick |
|---|---|---|
| 1 | Reach: the chat (0), then a listening agent (A), then the extension starting one (B)? | **Yes, in that order.** 0 costs almost nothing after 26 and gives the side trip its screen. B alone would leave the agent that wrote the tour, the one that knows most, unable to answer |
| 2 | Side trips always in the reader's own file? | **Yes.** The other choice, in the tour's file for a private tour, saves one file and gives two behaviours |
| 3 | May any agent answer, or only the one that wrote the tour? | **Any.** A tour is its own context (§3) |
| 4 | Starting a tour with a click ("Walk Me Through…" on a node) | **With B (TI9)**: it is the same act, starting an agent with one sentence. Until then `＋` copies the sentence (26, W18) |

| # | Check | Step |
|---|---|---|
| 1 | `wait` under each agent's limit for one command; whether a run in the background is told when it ends | TI0 |
| 2 | the flags to run each agent once with a prompt, and to continue a session by its id; whether the agent can learn its own session id to give to `start` | TI0 |
| 3 | an unattended agent and permissions: which rules it needs to read code and run `vnoc tour`, and nothing else | TI0 |
| 4 | **[measure]** one cold answer: seconds, tokens | TI0 |
