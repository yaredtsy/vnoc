# 26.05 On screen: how to read `design/` now

```
on screen
├── 1. what stands                  almost all of it
├── 2. read it with these changes   names · the file · the states · the six fields · the place
├── 3. what is new                  a detail inside a block · pinned and as written · the size with no plan
│                                   private and project · the user's actions · moved
├── 4. what is not built            fallback · the side trips of design/08
├── 5. the session                  only which tour is open; the place is the core's
├── 6. who draws what               the core answers, the pages draw
└── 7. the text renderer
```

[`design/`](design/README.md) is the mock design of 2026-11-15, copied as it was made (only its links to the redesign were changed, to
`../../16-ui-redesign/`). It is the specification of what a tour looks like and how it moves. It was written against the old phase
documents' data and commands; this file is the list of where to read it differently. **Where this file and `design/` differ, this file
wins. Where `design/` and `16-ui-redesign/` differ about a tour, `design/` wins.**

`design/mockup.html` is the thing to hold a built screen against. Open it in a browser.

## 1. What stands

- The three parts (route, map, stage) and that a step's text is in the stage only (W3, W4). The five rules against the wall of text.
- A tour is a context (W2); the current stop is its selection (W6); Exit and Stop are two things (W7); you set the pace (W8); a short
  stop is one step (W9); the cover and the end card (W10); nothing steals the window (W11).
- The stage: anatomy, every kind of step but *fallback*, moving, `←` "where I was", the sizes, the mini bar, the look (03).
- The route panel, the map's cards, the camera, the bar, the status bar item, the one notification, the menus (04).
- The TOURS section, the two views, opening a tour at its root or where you left it, `▶ N`, Ask for a Tour, Delete with Undo (07).
- The keys (05 §6) and the checks (05 §8).

**The two components** (D18). Every tour is drawn by two things the canvas already has, and nothing else:

| The tour's | Is drawn as | Already here as |
|---|---|---|
| **stop** (a node, of any kind) | the node's card on the map, with the ring when it is the current one; a row on the route | the card and the row of `16-ui-redesign/` |
| **block** (some lines of that node) | the lit lines in the stage's code pane, with the title and the text beside them | a card's `</>` code, read-only, with a range lit |

A nested stop is a card joined to its parent's. A nested block is lit lines inside lit lines (§3).

## 2. Read it with these changes

| `design/` says | Read |
|---|---|
| `vnoc walk start`, `write`, `next`, `finish`, `estimate` | `vnoc tour start`, `add`, `finish`, `route` ([`02`](02-commands.md)) |
| "the phase document", "the session file" | [`01-on-disk.md`](01-on-disk.md); the tour's file |
| `.vnoc/cache/walkthroughs/w_5d20.json` · "the cache" | `.vnoc/local/tours/` and `.vnoc/tours/` · "the tours" |
| a tour id `w_5d20` | `checkout-order-7k2q` |
| `status`: `generating` · `complete` · `stopped` | the tour's state: **writing** · **finished** · **stopping** / **stopped** ([`01`](01-on-disk.md) §4) |
| a stop's `state`: `pending` · `ready` · `degraded` | **planned** · **written**. There is no `degraded` |
| `code_hash`; "keeps the hash from when the tour started" | `fresh` on a stop and on a step, from the core ([`03`](03-core.md) §2). The pages compare nothing |
| *contextual* stop, `first_seen` | `same_as` |
| `via_line` | `line` |
| `writing: "s5"`, `updated_at` | `TourView.writing` (the first planned stop; null with no plan) · `TourRow.quiet` for "no news" |
| `stop_seen` | the state **stopped** (the `halted` record) |
| `step_estimate`, "about 7 min" | `TourRow.minutes`, computed by the core |
| `created_at` | `at` |
| the tour's `depth`, "and what it calls, and what those call" | a tour has no depth. The cover shows the title, the question and the size |
| `query` | `question` |
| "written for the code as it is at `f4d6e15`" on the cover; "for the code at f4d6e15" in the tooltip | only on a **pinned** tour: `pinned at f4d6e15`. Otherwise `not pinned` (§3) |
| "block" as a flat list under a stop; "stop 3 of 10 · 2 of 3" | blocks nest one level (§3). "2 of 3" counts the stop's steps in reading order, details included: `stop 3 of 10 · 2 of 5` |
| "where you are is yours, not the file's": the current step and the seen steps are in the extension's session, per window; two windows can be on different steps (01 §6) | the place is the core's, per tour ([`03`](03-core.md) §4). Two windows on one project show the same step. §5 |
| `tourGo`, `tourBack` as intents the reducer answers (04 §8) | requests to the core: `vnoc/tourGo`. `reduce` never computes "next" |
| "the stage's code comes from the index (the same `code` request a card's `</>` uses)" | the same `code` request, which the extension serves from VS Code's document. Its params are the stop's `node.path` and `node.range` from `TourView`. For "as written": `vnoc/tourCode` |
| "the core keeps one active tour per worktree" (01 §4) | the core keeps no "active" tour. Several can be written at once. A window shows one tour context |
| "`[check]` whether the core gives a file stop blocks" (03 §3.1) | a **file** stop may have blocks and is then shown as code, like a class. With no blocks it is *contents*. A **folder** is always *contents* |
| "the cache is small: `start` removes all but the 50 newest" (07 §9) | nothing is removed by V-NOC (D15) |
| "can it be undone: in basic, ask the agent to continue. Resume is phase 3" (05 §2) | it works now: the agent runs `vnoc tour resume` |
| *changed*: "the stage shows the code as it is **now**; the text is the old one" (05 §4) | that is a tour that is **not pinned**. A pinned one shows the code as written (§3) |
| 05 §7, "what the UI needs from the file and the core" | [`03`](03-core.md) §8: `TourRow`, `TourView` |

## 3. What is new

**A detail inside a block** (D18). A stop's steps are its intro, then each block followed by its details.

```
├ STAGE ───────────────────────────────────────────────────────────────────────────────────────┤
│ ● 1  ƒ checkout_order   checkout.py:11                    stop 1 of 10 · 3 of 5      ↗   ⌄   │
│ ───────────────────────────────────────────────────────┬──────────────────────────────────── │
│  11 ▌  def checkout_order(order: OrderRequest) -> …    │ Hold the stock, then charge  ›      │   the block this is a detail of: one dim line, a click goes to it
│  12 ▌      """ID: 0Q8V5W…"""                           │ Check the stock, then hold it       │   the detail's title
│  13 ▌      if not check_stock(order.sku, order.quan…   │                                     │
│  14 ▌          raise ValueError(f"Out of stock: {or…   │ check_stock only looks;             │
│  15 ▌                                                  │ reserve_stock takes the units out   │
│  16 ▌      reserve_stock(order.sku, order.quantity)    │ of the count, so a second buyer     │
│  17 ▏                                                  │ cannot get them while this payment  │
│  18 ▏      try:                                        │ runs.                               │
│  19 ▏          transaction_id = charge_payment(order)  │ ▰▱▱▱▱▱▱▱▱▱         ‹ Prev   Next ›  │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | A block (the general) | A detail |
|---|---|---|
| the code pane | its lines lit (`▌` and the background); the rest dim | **its** lines lit; the rest of its parent's lines keep the thin mark `▏` and are not dimmed; everything outside the parent is dim |
| the text pane | title, text | one dim line above the title: the parent's title and `›`. Then title and text |
| the route's step rows | a row | a row, indented one level under its block's |
| scrolling | the design's rule (03 §2.2) | the same, for the detail's lines |

Going from a block to its first detail never scrolls the code when the detail is in view: the light narrows from the part to the piece.

**Pinned, and as written** (D19).

| Where | Shows |
|---|---|
| the cover | `pinned at f4d6e15` (the tooltip: the full commit and when). Or `not pinned`, and the reason in a line: *"2 files have changes that are not committed"* · *"1 stop changed since it was written"* · *"this project is not a git repository"* |
| a tour's row, line 2 | nothing for a pinned tour. `not pinned` only on a **project** tour: a shared tour should be |
| the stage, a **changed** stop of a pinned tour | the code pane shows the code **as written** (`vnoc/tourCode`), the lit lines exactly as the text says, on a slightly different background, with a label on the pane's top edge: `as written · f4d6e15`. Under the text: *"The code changed since then."* and a switch: **Now** / **As Written**. `↗` Go to Code opens the file as it is now |
| the stage, a **gone** stop of a pinned tour | the same, with *"This is no longer in the code."* and no **Now** |
| the stage, a changed or gone stop of a tour that is **not** pinned, or whose commit is not in this repository | the design's marks (05 §4): the code as it is now, the lit lines dashed; no code pane for a gone stop |
| the row and the card of a changed stop | `changed`, as designed, pinned or not: it is a fact about the code, and the stage is where it is handled |
| the row's menu, always | **Pin** when nothing is in the way · **Commit and Pin…** when files are not committed · **Write IDs for Its Stops** when some have none |
| the cover of a finished tour | **offers**, by the project's settings ([`01`](01-on-disk.md) §12, `TourRow.offers`). `"ask"`: a line each, with its button: *"4 stops have no ID: the tour will not follow a rename."* **Write IDs** · *"2 files have changes that are not committed."* **Commit and Pin…**. `"always"`: nothing to offer, it was done at `finish`. `"never"`: no offer; `not pinned` is still said |

**Commit and Pin…** opens one dialog, the only one a tour has: the files that will be committed (each a link to its diff), the message
(`Code for the tour "checkout_order"`, editable), a check box *Always do this in this project* (it sets `[tours] commit = "always"`),
**Commit and Pin** / **Cancel**. It commits those files and nothing else
([`02`](02-commands.md) §8). A failed commit shows git's own message.

**The size, with and without a plan** (D4). The design always knows the number of stops. Now:

| The tour | "Stop 3 of 10" reads | The cover | The segments | The route's foot |
|---|---|---|---|---|
| planned, being written | `stop 3 of 10` | `10 stops · about 7 min` | 10, the unwritten ones dotted | `writing 6 of 10` `■ Stop` |
| not planned, being written | `stop 3 · 6 so far` | `6 stops so far · the agent is adding more` | as many as there are stops, and one dotted segment after them | `writing · 6 stops so far` `■ Stop`, and one row under the last stop: `◌ …` |
| finished | `stop 3 of 10` | `10 stops · about 7 min` | 10 | none |

On the last written step of a tour that is still being written and has no plan, `Next` is dim with `◌`, as on a stop that is not
written (design 02 frame 6), and the text pane's place is taken by: *"The agent is adding the next stop."* When it arrives, `Next`
lights. Nothing moves by itself.

**The place moved from outside** (D17). When `vnoc/tourPlaceChanged` arrives for the tour a window shows, the stage, the route and the
map follow, with the design's own moves (the ring fades, the map pans only if the card is out of view). It counts as a **jump**: `←`
goes back to where you were. The bar shows, for three seconds, `moved by the agent`. A tour that is not on screen is not opened.

**Private and project** (D3), **and the user's actions** (D14).

| Where | Shows |
|---|---|
| a tour's row, line 2 | a project tour starts with `shared ·`: `shared · 9 stops · 12 Nov`. A private one says nothing: it is the usual case |
| the row's menu | **Open** · **Start Again** · **Reveal Root in Main** · ─ · **Rename…** (the title; an input box) · **Pin** / **Commit and Pin…** · **Write IDs for Its Stops** · **Share with the Project** / **Make Private Again** · **Open as JSON** · ─ · **Ask for a New Tour** · **Delete** |
| **Write IDs for Its Stops** | `vnoc/tourIds`. One message after: *"IDs written into 4 files."* with **Show Changes** (the source control view) and **Always in This Project** (it sets `[tours] ids = "always"`). Not offered in a project with `[ids] default = "none"` |
| **Share with the Project** | after it, one message: *"Moved to `.vnoc/tours/`. Commit it to share it."* If the tour is not pinned: *"… It is not pinned: teammates will see changed stops without the old code."* with **Pin** |
| **Export as JSON…** · **Open as JSON** | the tour's nested document ([`02`](02-commands.md) §10): saved to a file you choose, or opened in an untitled editor tab. The tab's title bar has **Import into This Tour**, which is `import --replace` with that text; refusals are listed in the Problems style, by their place in the document |
| the end card | a fourth button on a private, finished tour: **Share…** (the same act) |
| Delete | the tour and its state go to the trash; Undo for six seconds, as designed. Private or project |
| the TOURS section's `⋯` | **Tour Settings…** (a pick of three values for each of the two settings; it writes `vnoc.toml` through `vnoc/tourSettings`) · **Import a Tour from JSON…** (`import`: a new tour from a file) · **Check All Tours** (`vnoc tour check`'s work, with the result as each row's state) · **Delete Finished Tours** (private ones only) |

**Moved** ([`03`](03-core.md) §2). A block whose lines are found elsewhere in its node is lit at its new place. It has no mark. Only
*changed* and *gone* are said.

**Changed, per step.** The design marks a stop. The mark on the stage is now on the step that changed: a stop with three blocks, one of
them changed, handles that block only (as written, or dashed). The row and the card still say `changed` for the stop.

**Problems.** A tour with broken lines (`TourView.problems`) plays without them; the cover adds one dim line:
*"2 lines of this tour could not be read."* with the file as a link.

## 4. What is not built

| From `design/` | Why |
|---|---|
| the *fallback* mark and state (03 §3, 05 §3) | D9: an invalid text is never stored |
| everything in `08-interactive.md` | [`../27-tours-interactive.md`](../27-tours-interactive.md). The seams of the design's README §6 stay: the `+N` dim with its tooltip, the empty space under the text |
| "Editor follows" (A6) | as the design says: later |
| `＋` sending the sentence to the agent | 27. It copies, as designed (W18) |
| two windows on different steps of one tour | D17 |

## 5. The session

Smaller than `design/04-route-and-map.md` §8 and `design/07-tour-list.md` §9 planned, because the place moved to the core:

```ts
// shared/session.ts
kind: "main" | "explore" | "tour";
// ContextState.tour: TourId | null            which tour this context shows. Nothing else of the tour is in the session
// Session.tourList: { view: "all" | "onSelection" }
// Intent: tourOpen { id, at? } · closeContext (existing)
// NOT in the session: TourPlace, tourPlaces, tourGo, tourBack. They are the core's (03 §4)
```

| Thing | Where it is | Why |
|---|---|---|
| which tour is open, and that its context is active | the session | it is about the window, like every context |
| the current step, the seen stops, the way back | the core, the tour's state file | D17: a command can read and move it |
| the tour context's `selected` (W6: the current stop is the selection) | the session, **set by the host** whenever the place it holds changes | the ring, the heavy edges, the docs panel following the stop: all read the selection, as today |
| the tour context's viewport | the session, as every context's | |

The host (`extension/src/tours.ts`) holds the open tour's `TourView` with its place. A click on `Next` → `vnoc/tourGo` → the new
place → the host updates what it holds, sets the context's selection, and the pages draw. `vnoc/tourPlaceChanged` from outside takes
the same path from its second arrow.

**As built (TR16, `3dae1c6`; TR14e `282bb8b`).** `shared/tours.ts` holds the answer types, checked against the Rust goldens by
`shared/tours.test.ts` (declared keys = actual keys). The session has `kind: "tour"`, `ContextState.tour`, `tourList`, the intents
`tourOpen { id, title?, at? }` and `tourSelect { key }` (the host's selection), one tour context at a time (`tour:<id>`, parent
MAIN). The host's holder is `extension/src/tours.ts` (no `vscode` import, unit tested); it re-reads every 2 s only when the git root
(`core.gitRoot`) is a strict ancestor of the opened folder, and skips a redraw when the answer is the same text. Each stop carries
`key` (its `Node.id`) for the selection.

**Decided at TR16's review (built first in TR17):** P1 holds for the three lookups TR16 had to do or could not do:
- the row gains `words` from the core: the second line design 07 §3 gives a row (its one state word, size, where you are), built by
  `view.rs` as `list` builds its row text. The QuickPick's detail and TR18's row draw it; TR18's "state word and its order" test is a
  Rust test, not a TypeScript one;
- the place gains `stop` (the current stop's id, `null` at the cover and the end) and `position` (`stop`, `of`, `step`, `steps`: the
  "stop 3 of 10 · 2 of 5" numbers, the same code as `where`); the selection reads `place.stop`, never a step id;
- each reading step gains `kind`, decided by the core from design 03 §3: `intro`, `block`, `detail`, `short`, `contents`,
  `same_as`, `planned` (the cover and the end are places, not steps); the stage picks its layout from it;
- `key` follows a check: a stop the check found at another name or path is keyed by where it is now (`now`), so the selection
  finds it;
- the holder sends `tourOpen {open: true}` again after a core restart.

**As built (TR17, `1d9b864`, `ccf1b94`, `0a0275d`).** The stage is `webview/src/tour/Stage.tsx` + `stage/*`, the renderer
`tour/text.ts`; the window suite `tour` (9 tests, 13 s) makes its tours with the built commands. The core gives `words`
(`[shared ·] <state> · <size>`, the state word the first that applies of: no news, stopping, stopped, writing, changed, `stop N`,
finished, new; else `seen N of M`), the view's `place` as `PlaceView` (`stop`, `position`; the state file keeps the plain place), each
reading step's `kind`, and `key` for planned stops too (the stage reads their code) and after a check (`now` or `displaced`). The
code pane finds the node by the stop's `key` in the structure (a file: the whole file). Stage height, text width and fold are in the
window's view state. Two TR16 bugs found by the suite and fixed: a request with no params went out as `[null]`; **Open Tour…** did
nothing with the canvas closed. Screenshots of every kind were looked at (cover, intro, block, detail, short, same_as, contents, end).

**Left out of design 03 at TR17, and where each goes:** the line the stop was called from → the step holding it, and the end
card's "all seen / N not seen" → **TR18's first commit** (the core gives them: `line_step` on a stop reached by a call, `unseen` on
the place; the pages count nothing); `Next` dimmed with `◌` before a planned stop → TR20; a call in the code as a jump (design 03
§2.2) → TR19; `↑ ↓` (next / previous stop), `Alt+←` (back), the contents' `+ N not on this tour` fold, the code pane's syntax colours
(as a card's code view has them, if it does), the cover's title said twice → TR23.

**As built (TR18, `25261a6`, `8b1a621`).** The core gives `line_step` (a call's step in its `from` stop) and `place.unseen`. The
TOURS section is `webview/src/sidebar/tours/*`: folded by default (fold saved per window; it unfolds when a tour is opened), the
**All** view only, rows drawn from `words` with the question after it in quotes, a click opens at the cover or where it was left. With a
tour open it shows the route (`‹ Tours`, stops nested by depth, ✓ seen, ● current, ◌ planned, `⇢ N`; the current stop's steps
under it when it has more than one, as design 04 §1.2). The list is asked for again each time the section shows it (a move sends no
`toursChanged`, so a row's `words` would go stale). The section is 45% of the view when open; a shared tour and its private copy are
two rows. The window suite `tour` has 17 tests.

**Left out at TR18, and where each goes:** the "when" word of a row's line 2 (design 07 §3) and the end card's **Go to the first one
I skipped** (`place.first_unseen`) → **TR19's first commit**, from the core; hover `↗` and double-click for Go to Code on the route,
**Reveal Root in Main**, **Start Again** → TR19; the route's foot line (`writing 6 of 10 ■ Stop`), the live writing dot, unfolding
when a tour appears from outside, the splitter → TR20; `changed` and `⚠` on the route, the second view and its switch, the empty
state "on node" → TR21; the section's `＋` / `⋯`, the row menu, Delete (also on an unreadable row), `Delete`/`⌘⌫` → TR22.

## 6. Who draws what

| | Does | Does not |
|---|---|---|
| **the core** | replays, finds, checks, records; keeps the place and decides what "next" is; says which tours are on a node, a tour's state, its size, its minutes, each step's lines now; reads a pinned commit's code; runs git | know which window shows what, beyond `open` |
| **the extension** | holds the `TourView` of the open tour and asks again on `vnoc/toursChanged`; passes clicks on as requests; owns the session; the status bar item, the notification, the palette's **Open Tour…**, the one dialog | compute a state, a count, a next step, or whether something changed |
| **the pages** | draw `TourRow` and `TourView`; send intents | read a tour file, compare a hash, sort the list, walk the steps |

The test of P1 for every step: a list and a text box in a second editor could be built from the requests of [`03`](03-core.md) §8
alone, and a terminal already is (`vnoc tour where`, `go`).

## 7. The text renderer

A step's `text` and `title` are a model's output, or came by `git pull`. They are drawn by one small function in the webview that knows
two things: `` `code` `` becomes a code span, `*emphasis*` becomes emphasis. Everything else is text. It is **not** the doc panel's
Markdown pipeline and it never sets HTML. `vnoc tour add` already refuses anything more ([`02`](02-commands.md) §4), but a file from a
pull was not written by this `add`.

Next: [`06-build-order.md`](06-build-order.md)
