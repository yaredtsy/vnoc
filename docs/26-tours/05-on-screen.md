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

**As built (TR19, `4b89724`, `8b1897e`, `e83a8fc`): phase 3's stop point reached.** A finished tour plays: list, route, map,
stage, bar. The core gives a row's `when` (relative under a day, then `yesterday` or `12 Nov`, by **UTC** day: the core has no time
zone; accepted) and `place.first_unseen`. The map is its own `TourCanvas`/`TourCard` on the cards' CSS (not draggable: the route is
fixed); a call in the code pane is a `⇢ stop N` button at the end of its line (the view gives lines, not names); the camera moves for
a new stop, not for a step inside one, and fits the route at the cover and the end. The bar's `←` is the core's `back`; `Esc` closes
the context (an open menu first). "Moved by the agent" when `moved` or `at` changed from a command. Full `pnpm -r test`: every window
suite, 148 passing; it found and fixed a render loop (React #185) in the bar, duplicate route row keys, the `tour` suite's setup
leaking into other suites, and an old `cli` suite count of permissions. Looked at side by side with `design/mockup.html`.

**Left out at TR19, and where each goes:** the writing dot, the bar's agent text (`writing 6/10 ■`, `stopping`, `stopped`, `no
news`), the status bar item and the one notification, the route's foot line, cards being written → TR20; a card's `changed`, `+N
calls not on this tour` → TR21; the section's `＋`/`⋯`, Show Docs in the card menu → TR22; the narrow bar → TR23. **Open Tour…** in
the bar's `⋯` (the webview cannot run a palette command): the webview asks the host, which runs it — TR22.

**As built (TR19b, `0a6619f`, `2139694`, `10a39a5`).** A step's text keeps its lines; the Markdown rules and their seven refusal
sentences are in `rules.rs` (HTML, an image, a heading, a second Mermaid diagram, a fence over 30 lines, a table over 12 rows, a fence
not closed); import uses them. The renderer is markdown-it (`html: false`) with fences coloured by the cards' Shiki highlighter (one
shared `lib/highlighter.ts`) and Mermaid loaded lazily (57 chunks, 5.2 MB raw, not in the main chunk, which grew 6.5 kB). Vite's
`modulePreload` is off for the canvas build: the CSP refused its preload tags (no nonce); a plain `import()` carries the nonce. Only
`http(s)` links go to the host. Agents, given the choice (Claude t4 ×2, t5): one Mermaid flowchart in 22 steps, no list or table, no
fence retelling code; none of the new refusals fired. Mermaid was installed through a mirror (npmjs stalled); the lockfile names no
mirror and its integrity hash is npmjs's.

**Decided at TR19b's review:** a diagram is drawn at a readable size: no smaller than 80% of its natural size, and a wide one
scrolls sideways in the pane (a wide flowchart shrunk to 320 px was unreadable in the screenshots); the click still enlarges it.
The scorer gets TR14d's softened chat rule, which was decided but never built (a progress line of 100 characters or fewer beside an
`add` is counted, not failed; an end of up to four lines). The 5.2 MB of Mermaid in the package is weighed at the pull request
(fewer diagram types if it matters). The overlay's duplicated SVG ids → TR23.

**As built (TR20, `317aef8`, `d6d8a6f`).** A tour is read while it is written: the bar's agent text (`writing 6/10 ■`,
`stopping`, `stopped`, `no news`), the pulsing dot, planned steps with grey bars and the fade-in, `Next ◌` dimmed before a planned
stop (disabled on the last written step of a tour with no plan), `■` Stop on the bar and the route's foot, the status bar item
(`▶ Tour 3/10 · writing 6/10`), the one notification with **Open Tour** (only for a tour that appears while the window is open and is
being written), the section unfolding for a tour from outside, a draggable splitter (112 px minimum, double-click resets). Rules and
words are in `shared/tourWriting.ts`. A diagram is drawn at no less than 80% and scrolls sideways. The scorer counts progress lines.

**Decided at TR20's review:**
- **The freeze is of the files on disk** (records, the state file, the export document: 01 §11), **not of a command's or a request's
  answer.** The fixtures' goldens have gained keys at every step since TR14d, and may. So the view gains `quiet` and `of` (the plan's
  size, `written + planned`): TypeScript stops summing and stops reading the open tour's row for `quiet`. The holder re-asks
  `vnoc/tour` every 30 s while the open tour is writing (a quiet tour changes no file; rung 1 answers it from memory).
- **W11 as designed:** with the canvas visible, a tour that starts being written adds its tour context and activates it (design 02
  frame 2, README W11); the notification is for a hidden canvas only.
- **The window suites flake** (TR20 saw three different single failures across runs: `tour` "end card … not seen", "Next walks to the
  end card", `edit` "card to say gone"). TR21 starts by making `tour` pass five runs in a row (wait for the core's answer, never a
  timer) and finds whether the `edit` flake is older than tours (run it on `main`'s code in a worktree only if disk allows).
- **Six `cli` tests need the Bend driver** (`bend_driver_path`): `drivers/bend/target/debug/vnoc-driver-bend` is not built, so they
  fail on this machine. Not a tours bug. The full Rust run is green only with it built (1.1 GB); build it once before the pull
  request, not per step.

**Decided before TR21's prompt (2026-11-25).** TR21 is cut in two, to keep a prompt to two commits: **TR21a** (TR20's review
decisions above; out of date and as written) and **TR21b** (the tours of the selected node, `▶ N`, **Show Tours**, `+N calls not on
this tour`). The rest:
- `of` on the view is `null` while the size is not known (being written, no plan), else `written + planned`. `sizeKnown` and
  `sizeOf` in `shared/tourWriting.ts` go; `quietOf` goes (the view's own `quiet`).
- The 30 s re-ask is of `vnoc/tour` for the open tour, and it replaces TR20's 30 s re-ask of the list: while a tour is open the
  section draws its route, not the list.
- W11 adds and activates a tour context only when the window has none open. With one open, TR20's rule stands (the open tour is not
  replaced; the notification).
- `vnoc/tourCode`'s answer gains `steps`: `{ <step id>: [first, last] }`, each block's lines at the commit and the intro's the stop's
  lines. The stage lights the current step's lines and draws its parent's thin, as it does now. (An answer, not a file: allowed.)
- **Now / As Written** is offered only on a changed or gone step of a pinned tour whose commit this repository has
  (`pin_in_repository`, and `tourCode` not `null`). **As Written** is where such a step opens; the choice is not kept (leave the step
  and come back: As Written again). Otherwise the not-pinned look (dashed; no code pane for a gone stop).
- The cover in TR21a: `pinned at f4d6e15` (tooltip: the commit as the core gives it), `pinned at f4d6e15, a commit this repository
  does not have`, or `not pinned`. **The reason after `not pinned`, and the offers, are TR22's** (they need `tourPin`'s reasons). The
  first stop gone: design 05 §4's sentence. **Most of it changed** = `check.changed + check.gone` more than half of `written`: the
  cover's sentence and, on a finished tour, the route's foot `7 of 10 stops changed since this was written` (one rule in `shared/`,
  tested).
- The route's and card's right-hand word: `writing…` > `gone` > `changed` > `⇢ 2`. A gone stop's row and card are struck through.
  There is no `⚠`: the only `⚠` the design has is *fallback*, which is not built (§4).
- The list asks a check for the rows it holds (`check: [ids]`); the first list in a window asks twice (plain, then with the ids).
  **Check All Tours** goes to TR22 with the section's `⋯`.
- The window suite says "no source file was read" with the view's `check.from` (`memory` or `record`, not `check`), reported in the
  stage's test event.
- The `edit` flake: no worktree of `main` (3.6 GiB free). TR21a runs `edit` five times here; a failure is read against
  `git diff main..tours` of the files on its path, and fixed only if tours caused it.

**As built (TR21a, `6337ec3`, `e61dc06`).** The view has `quiet` and `of`; `writing.ts`, `sizeKnown`, `sizeOf`, `quietOf` are gone.
W11 is `appearance(visible, tourOpen, showsTour)`. `tourCode` answers `steps` (a stop with no blocks: its intro only); the page keeps
the answer per (tour, stop) and asks again on each `tour` event, showing the old one meanwhile; a pending answer draws nothing, an
error or `null` falls back to the dashed look (`webview/src/tour/written.ts`). A gone stop shown as written has no switch and no `↗`.
The cover's pin line is not drawn while the tour is written. The list asks plain, then with the readable ids, and again when a new tour
appears. **The core:** a file change that is both a tour's and the index's tells the tours after the index update, and `answers_for`
waits for a running update (a deleted function had been recorded `changed` from the old index). The page sends moves one after the
other (bursts overtook each other); `stageDrawn` waits for the code pane. `tour` 45 tests, five runs green twice; `edit` five green
(the flake did not come back). No screenshots: the terminal has no Screen Recording permission.

**Decided at TR21a's review:**
- **`answers_for` waits too often.** As built, every tour answer, a memory or a recorded one too, waits for a running update, the first
  full build included: opening a tour or the list on a big project waits for the whole build, which 03 §3 says only a real check may
  do. Fix (TR21b's first commit): ask first with no index (rungs 1 and 2 need none); only on `NeedsIndex`, wait for the first build,
  then for a running update, then ask with the index. A recorded check whose file changed is not valid, so it reaches the wait: the
  stale-record bug stays fixed.
- **A second tour appearing while one is open** (design 05 §5): the notification of W11, once, also when the canvas shows a tour (as
  built it says nothing then). The status bar's `▶ New tour: …` is left out: the notification and the list are enough.
- **The `sidebar` failure** ("first read has the progress line") depends on the order of suites (`session` before it), and fails with
  tours switched off: not tours. The end-of-step window run uses the default order of `pnpm -r test`'s suites, where it passes (TR19's
  full run). Fixing the order dependence is not tours' work.
- The three rare flakes TR21a could not catch (cover not leaving at `s1.b2`, the end card's unseen count, the Open Tour… QuickPick not
  taking the keyboard) wait for TR24's full runs; one seen again in a step's end run is fixed in that step.
- A gone file or folder stop keeps the contents look with its sentence: no as written for rows.
- Screenshots (the user): Playwright drives VS Code, as `tools/media` does (`lib/vscode.mjs` `launch({ dev: true })`, then
  `page.screenshot`): it needs no Screen Recording permission. Never `screencapture`. The script stays out of the commits.

**Decided for TR21b:**
- **On the selected node** is the list's second view. It follows the selection of the **active context** (a selected call counts as
  its callee, as the docs panel does), so it shows in MAIN and in an explore context; in a tour context the section shows the route,
  as built. It asks `vnoc/tours { on: <selection's tree key>, check }` when the selection changes, and on `toursChanged`. Line 2 is
  `on.words`; a click opens at the cover for `how: "starts"`, else at `on.first`. The buttons: `All <n>` · `On <name> <n>` (the lengths
  of the two lists the core gave); with nothing selected `On selection`, disabled, tooltip *"Select a node to see its tours"*. The
  choice is `tourList.view`, already saved. `←` `→` on the two buttons switch.
- Its empty state: *"No tour goes through `<name>`."* and **Show All <n>**. **Ask for One** comes with TR22's **Ask for a Tour…**.
- **`▶ N`** on a card's footer and on a structure row's right side, from `vnoc/tourCounts` by the node's tree key. The host holds the
  counts and asks again on `toursChanged` and on `indexChanged` (the keys are the structure's). Tooltip *"on 3 tours: click to see
  them"*; a click selects the node, unfolds TOURS and switches to **On the selected node**. **Show Tours** in the node menu (card and
  row), only when the count is above 0, does the same.
- **`+N calls not on this tour`** comes from the core: a new request `vnoc/tourCalls { id }` → `{ off: { <stop id>: n } }`, for the
  written function and class stops: the callees `vnoc tour route` would see one level down (`route::from_calls` with `levels = 1`),
  counted once each, that are not a stop of this tour (by tree key). Asked when a tour's map is drawn, kept in the core per tour until
  the tour's or a stop file's change, as the answers are. A stop with none, or not a function or class, has no key. The card's footer
  draws `+N` dim, tooltip *"2 calls are not on this tour"*, no click.

**As built (TR21b, `2a17677`, `0379361`, `ccd0fc6`).** `answers_for` asks with no index first. `appearance(visible, tourOpen)`
announces a second tour also while one is shown. `vnoc/tourCalls` counts function and class stops in a background task (each stop's
driver ask bounded at 4 s; kept only when whole and the index revision did not move) and answers within 1.5 s or says "ask again";
the page asks when the stops change and retries up to 8 times. The second view (intent `tourView`; nothing selected, a tour context or
the root → All, the saved choice kept), its buttons and empty state; a click from it is `tourGo` then open. `▶ N` on cards and rows
(`showTours` request, `vnoc.showTours`, context key `tours`, menu group `6_tours`); `+N` on tour cards. Test event `tourChips`.
`tour` 50 tests; full `pnpm -r test` 162 passing, 3 failing (`skeleton` ×2, `bend`'s setup): the Bend driver is not built, as the 13
Rust failures. **Found:** the core answers at most one request per CPU at once (`ConcurrencyLayer`); a page that re-asked `vnoc/tours
{on, check}` on every list change filled the slots and `ping` hung. A handler must answer in about a second or say "ask again"; a page
asks only when what it reads really changed. Screenshots by Playwright: `tools/media/out/tr21/`.

**Decided at TR21b's review (TR22a's first commit):**
- **`tourCalls` always ends.** A stop whose driver ask fails or times out is left out of `off`, and the count is still kept (as the
  answers are, until the tour's or a stop's file changes): as built, a slow driver or an editing session (the revision moves) made it
  count again on every ask and give up after 8. The page asks again every 2 s, with no cap, only while the tour's map is shown and the
  answer says "ask again".
- **The On view keeps the state word.** The row gains `state_word` from the core (the one word of design 07 §3: `writing 6/10`,
  `changed`, `stop 4/9`, `finished`, `new`, …, the word `words` starts with). The On view draws it at the right of line 1, line 2 is
  `on.words`. The All view stays as built (TR18).
- **Seen in the screenshots** (`tools/media/out/tr21/06`): the `as written · <commit>` label covers the first code line in view (it
  must sit on the pane's top edge, in its own strip, and push the code down); the **Now** / **As Written** switch does not show which
  side is on (the chosen one is drawn pressed, `aria-pressed`, as VS Code's toggles are). Both in TR22a's first commit.
- The action strip covering `▶ N` on a hovered row → TR23. **Open Tour…** with the selected node's tours first → TR22b.
- The Bend failures: build the Bend driver once before the pull request (as decided at TR20); not per step.

**TR22 is cut in two**, as TR21 was:
- **TR22a, pin and IDs:** TR21b's fixes above; the cover's `not pinned` and its reason; the offers; **Pin**, **Commit and Pin…** and
  its dialog with *Always do this in this project*; **Write IDs for Its Stops** and its message; the section's `⋯` with **Tour
  Settings…** only.
- **TR22b, the rest:** **Rename…**; **Share with the Project** / **Make Private Again**, the end card's **Share…**; **Export as
  JSON…**, **Open as JSON**, **Import into This Tour**, **Import a Tour from JSON…**; Delete with Undo (hover, menu, `Delete`/`⌘⌫`,
  an unreadable row), **Delete Finished Tours**; **Check All Tours**; **Ask for a Tour…** everywhere it is (`＋`, the node menu, a
  row's **Ask for a New Tour**, the empty states' **Ask for One** and **Copy a Request**); **Open Tour…** from the bar's `⋯` through
  the host, with the selected node's tours first; **Show Docs** in the tour card's menu.

**Decided for TR22a:**
- **Why a tour is not pinned, without pinning it:** `vnoc/tourPin { id, dry: true }` answers as `tourPin` does and never writes a
  record, a file or a commit. Each reason in an answer gains `words`, the core's sentence (`Reason::words`): the pages make no
  sentence from a `code`.
- **The cover** of a finished tour that is not pinned asks the dry pin when it is shown, and on each `tour` event; it says `not pinned`
  and the first reason's `words` under it (05 §3). Then the offers (`offers`, by the settings): *"N stops have no ID: the tour will
  not follow a rename."* **Write IDs** · *"N files have changes that are not committed."* **Commit and Pin…**. `"never"`: no offer.
- **The row's menu** of a finished tour: **Pin** and **Commit and Pin…** when it is not pinned; **Write IDs for Its Stops** always.
  Each runs, and a refusal is shown in the core's words in one message. **Pin** on a tour that only has uncommitted files answers
  `ask`: its message offers **Commit and Pin…**.
- **Commit and Pin…** is the one dialog (05 §3), drawn in the canvas page: the files from the dry pin's `uncommitted` reason and its
  `node_files`, each a link that opens its diff against HEAD (`git.openChange`), the message (`Code for the tour "<title>"`,
  editable), the check box *Always do this in this project* (`tourSettings { commit: "always" }`, sent only when the commit
  succeeded), **Commit and Pin** / **Cancel**. A failed commit shows git's own message in the dialog.
- **Write IDs:** before `tourIds`, if a file of the tour's stops is dirty, one message: *"Save N files first? The IDs are written into
  them."* **Save and Write IDs** · **Cancel**. After: *"IDs written into N files."* with **Show Changes** (the source control view)
  and **Always in This Project** (`tourSettings { ids: "always" }`). A project with `[ids] default = "none"`: the core's refusal.
- **Tour Settings…** (the section's `⋯`, its only entry for now): a QuickPick of the two settings, then of the three values, written
  through `tourSettings`.

**As built (TR22a, `27b9471`, `07ddea0`).** `tourCalls` always ends; the row's `state_word` (empty for an unreadable row); the label
strip and the pressed switch. `vnoc/tourPin { dry }` (a new `Ask::Look`; with `commit` too: `INVALID_PARAMS`), reasons with `words`
in the request's answers (the CLI already prints the words); **Pin** on uncommitted files answers `not_pinned`, and both answers offer
**Commit and Pin…**. The cover's reason names the count; the files are in the dialog. The dialog, Write IDs with save-first ("Every
stop already has an ID." when none was written; a failed save stops it), the section's `⋯` with **Tour Settings…**, and the palette
command `vnoc.tourSettings`. The suite resets its workspace with `git reset --hard` to the head at its start and restores
`vnoc.toml`. Rust 556 lib, 72 lifecycle; TS 865 / 295 / 417. No screenshots (the script `tools/media/out/tr22a-shots.mjs` was never
run). The row's menu is right-click only (its hover `⋯` → TR23).

**The window suites are not reliable** (TR22a's end): two full runs failed 15 and 5 (3 of them Bend), both at *"a function deleted…"*
(it passed in TR21b's full run), and in the second the core stopped answering `vnoc/tours` (the hook's request never returned). Alone,
`VNOC_SUITE=tour` failed about one run in three, before TR22a as after, under load (another session's job), each time as one test
followed by a chain of failures. The core's index update has no time limit on its driver calls and holds `one_at_a_time`; a real check
waits for it, and waiting requests fill the core's slots: one hung driver call can stop every request. Not proven.

**As built (TR22r, `909c771`, `ae61677`; reviewed).** Only moved. `server.rs` → `server/` (`mod` 444, `index` 236, `docs` 273,
`calls` 59, `tours` 873 lines); `cli/tour.rs` → `cli/tour/` (`mod` 534, `write` 660, `read` 758, `act` 1,285). Review check: the sorted
non-blank lines before and after, `use` and `mod` lines left out, `pub(super)` stripped. `server`: only the `impl Core {` and `}`
of `index.rs`. `cli/tour`: only `super::args::ChoiceArg` → `crate::cli::args::ChoiceArg` (`super` now means `tour`). Rust counts equal
TR22a's (lib 556, lifecycle 72, cli 274 + 6 Bend, the same 13 Bend failures); fmt and clippy clean; no TypeScript run.
Choices: a child module `act` would clash with `crate::tours::act::{self, …}` in `mod.rs`, so `mod.rs` imports it without `self` and
the helpers that call it (`read_tour`, `pin_tour`, `write_ids`, `picks_of`, `placed_check`, `project_writes_ids`, `can_pin_words`,
`ids_after_end`) live in `act.rs`; `plan` too. Git finds no rename (each new file is under half the old one): to follow a line, use
`git blame -C -C`. `act.rs` is now the biggest file (1,285 lines); leave it until it hurts.

**As built (TR22s, `baa1a98`, `4fd9eee`, `c0829bc`; reviewed).** The plan above was wrong on one point: each driver call already had its
limit (10 s, 30 s for a call tree, `driver_host::timeout_of`); only their sum and the waiters were unbounded. The core hang was never
reproduced (6 runs of `VNOC_SUITE=tour`; the one failure was the stage not drawn after a typed QuickPick, `sample` showed a core that
answered), so **its cause is unproven**. Two bounds anyway: one update has at most 60 s (`UPDATE_LIMIT`), then it is dropped, the lock
is free and the next one reads everything (`needs_everything`); the index is written in one place at the update's end, so a dropped
update leaves it whole. A request that waits for the first build or a running update (`ready_project`, `wait_for_index`) waits at most
8 s (`WAIT_LIMIT`), then answers `REQUEST_FAILED` *"the index is being read and is not ready yet: ask again"*. After a good first build a
timed-out update keeps `Ready` and the old index (better than the brief's "failed" status: kept). Not bounded: the first-build waits of
`structure`, `languages`, search, and `attaching`. Window tests: one guarded helper (`util.ts`), every core request ≤ 10 s, a 3 s `ping`
before each test, and after one stall the suite's other tests fail at once (a frozen core: the whole `tour` suite ends in 22 s).
`tour` tests start clean (git reset to the suite's start, `shop.py`, `vnoc.toml`, tour state) and open their own tour; **Open Tour…**
takes a tour id (`vnoc.openTour(id)`, no pick). Rust lib 560, lifecycle 72; `tour` 58/58 three times. Full window run: 3 pin tests (fixed
in `c0829bc`) and the 3 Bend ones; a second run had 9 failures (`session`, `sidebar` among them), not repeated. `sidebar` *"a click
selects and reveals on the canvas"* fails 3 of 3 on the old code too; `session` *"canvas reveal"* 2 of 4. TR22a's screenshots ran:
`tools/media/out/tr22a/`.

**Decided at TR22s's review:**
- **The 8 s wait broke what used to wait.** A request asked during a first build longer than 8 s (any real project) now fails, and
  only the map's `tourCalls` is asked again. `tours.ts` `start()` asks the list once and only logs a failure; `askCounts` failing while
  `counts` is null is never asked again (`indexChanged` asks only when counts are held); the open tour's answers, `tourCode`, call
  trees and `hasCalls` show the refusal. Fix (TR22t's first commit):
  - the core marks "ask again" in data, not only in words: `ResponseError.data = { "askAgain": true }` on the wait-limit refusal and
    on `tourCalls`' "being counted" one. The host and pages test the field; `saysAskAgain` reads it (the text test goes);
  - one host helper asks again every 2 s, no cap, while what asked is still wanted (the list held, the tour open, the tree shown),
    as the map already does. The list, the counts (also when none were held), the open tour, `tourCode`, `tourPin { dry }`, call
    trees and `hasCalls` use it. Meanwhile a page draws the core's words as a quiet line, not an error;
  - a window test: a core whose first build is held past the wait limit (a slow-driver switch for tests, or `update_limit`/`wait_limit`
    from env in debug builds only) still shows the TOURS list and a tour's cover once the build ends.
- **The suites are not yet trusted.** TR22t makes the full window run pass apart from Bend: find and fix `sidebar`'s *"a click selects
  and reveals on the canvas"* (3 of 3: a broken test or a broken product; say which) and `session`'s *"canvas reveal"*. Give the other
  suites the `soft` hooks `tour` has. Proof: **two** full `pnpm -r test` runs, each failing only the 3 Bend tests.
- **Needs Attention said "ID lost (2)" for checkout and has_stock** in shots 05 and 06, after Write IDs had written both IDs into
  `shop.py` (`04-note.txt`). It may be the script (it resets files between shots), or Write IDs leaving records the index does not
  match. TR22t checks it in a window test: Write IDs on a clean tour, then nothing in Needs Attention. If it fails, it is a product bug
  and is fixed there.
- **Seen in the screenshots, for TR22b's first commit:**
  - the cover says *"1 file has changes that are not committed"* twice (the reason and its offer). A reason that has an offer is shown
    once, as the offer; under `not pinned` only the reasons with no offer;
  - with two offers, **Start** falls below the stage's fold: the cover draws title, line, meta, **Start**, then the pin state and
    offers;
  - Write IDs: *"IDs written into 1 file."* hides that 2 stops got IDs, and the host made that sentence. `tourIds`' answer gains
    `words` (*"IDs written for 2 stops in 1 file."*), shown as is; the button becomes **Always Write IDs in This Project**;
  - the dialog's check box: *Always commit and pin without asking in this project*. The dimmed cover behind the dialog is a normal
    backdrop, not a fault;
  - the script leaves the Write IDs toast over shots 05 and 06: the script closes notifications before each shot.
- **For TR23** (the list): in the TOURS section, the first stop row is drawn under the sticky header (`Tours ▶ Pinned shop 1 of 3`) in
  shots 05 and 06.

**As built (TR22t, `96d0f49`, `7a6a3e7`; reviewed, the two full runs still to come).** **The core hang is found**, and it was
never slow slots: in `async-lsp` 0.2.4 `MainLoop::dispatch_message` awaits `ConcurrencyLayer::poll_ready` while `self.tasks` (the
requests already running) is not polled and stdin is not read. With every slot taken (one per CPU by default) the slots never free:
a permanent deadlock, no `ping`, and the core never sees its editor go (the orphaned `vnoc --stdio` cores with parent 1). Fix:
`ConcurrencyLayer::new(4096)` (`main.rs` `REQUESTS_AT_ONCE`); lifecycle test `more_requests_waiting_than_there_are_request_slots_…`
(0 of 33 answered before, 2.3 s after). A watchdog thread logs requests running ≥ 10 s. **The released 0.3.1 has the same layer**
(`main` `21df5fc`, `ConcurrencyLayer::default()`). The `sidebar`/`session` failures: a test window covered by another app is hidden to
Chromium (no animation frames, React Flow's fit never ends): `runTest.ts` starts it with `--disable-backgrounding-occluded-windows`,
`--disable-renderer-backgrounding`, `--disable-features=CalculateNativeWinOcclusion`. Asking again: `data.askAgain` on both refusals,
the host's `askUntilAnswered(ask, stillWanted, …)` (a closure), `toursWaiting`, `indexChanged` always asks the counts; the map keeps its
loop. Debug-only `VNOC_WAIT_LIMIT_MS` and `VNOC_HOLD_FIRST_BUILD_MS` (`cfg!(debug_assertions)`), a `waits` suite in its own window,
`VNOC_GREP`. Write IDs on stops that never had an ID: both IDs, Needs Attention empty. TR22a's "ID lost (2)" was the shot script (IDs
written earlier, markers reset away). Rust lib 563, lifecycle 74; TS 305 / 865.

**Decided at TR22t's review:** the fix is right (read in `async-lsp` 0.2.4 `lib.rs` and `concurrency.rs`) and the 8 s waits stay (a page
should still not sit on a long wait). The TOURS section does not exist while the structure loads, so its quiet line is seen only on a
slow update: kept, unit-tested. For TR22b's first commit: Write IDs on stops whose IDs are recorded but whose markers are gone says so
in the core's words (not *"Every stop already has an ID."*), and the shot script uses a fresh file. The 0.3.1 hotfix is the user's call.

**The user, 2026-11-26:** TR22t's two full runs are not run; go on. They are owed at TR24's by-hand pass at the latest.

**TR22b is cut in two**, to keep two commits a prompt:
- **TR22b:** the review fixes (TR22s's screenshot list, TR22t's Write IDs on lost markers, the shot script on a fresh file); then
  **Rename…**, **Share with the Project** / **Make Private Again** and the end card's **Share…**, Delete with Undo (hover `🗑`, the
  row's menu, `Delete`/`⌘⌫`, an unreadable row with only Delete; a tour being written shows `■` Stop instead), **Delete Finished
  Tours**. Every message after an act is the core's `words` in its answer (`tourShare`, `tourDelete`, `tourEdit`), as `tourIds`'.
- **TR22c:** **Export as JSON…**, **Open as JSON**, **Import into This Tour**, **Import a Tour from JSON…**; **Check All Tours**;
  **Ask for a Tour…** everywhere (`＋`, the node menu, a row's **Ask for a New Tour**, the empty states' **Ask for One** and **Copy a
  Request**); **Open Tour…** from the bar's `⋯` through the host (`vnoc.openTour(id)`), the selected node's tours first; **Show Docs**.

**As built (TR22b, `3eb6623`, `c3a469a`, `8263143`, `848dff5`; reviewed).** The cover: title, line, meta, **Start**, then the pin state,
the offers (a reason with an offer said once, as it), the notes; reasons with no offer joined by ` · `. `tourIds`, `tourEdit`,
`tourShare`, `tourDelete` answer `words`; Write IDs on lost markers says *"1 stop's ID is lost from the code: see Needs Attention."*.
**Rename…**, **Share with the Project** / **Make Private Again** (not while written, not on the private copy of a shared tour), the end
card's **Share…**, Delete with Undo (hover `🗑`, menu, `Delete`/`⌘⌫`; `■` Stop on a `writing` tour; the core refuses deleting one being
written), a new `vnoc/tourDeleteFinished` (one token per tour, one Undo row). The Undo row is the core's words and an **Undo** button,
six seconds, page-only (a reload loses it; the trash keeps the tour). Fixed on the way: a dry-pin test that compared seconds, a
window test whose names matched another suite's node. Rust lib 563, lifecycle 75; TS 417 / 873 / 310; one full window run: Bend's 3
and the Write IDs test (fixed after, not re-run in full). Screenshots `tools/media/out/tr22b/`: Start in view, no ID lost, no toast.

**Decided at TR22b's review:**
- The Undo row says *"Deleted the tour of checkout."* while the row it replaces said *How the shop checks out*: the words name the
  tour by its **title** (*"Deleted “How the shop checks out”."*), the root only for a tour with no title. TR22c's first commit.
- The row's menu lacks **Open**, **Start Again**, **Reveal Root in Main** (05 §3): TR22c adds them with **Open as JSON** and **Ask for
  a New Tour**, so the menu is whole.
- **Delete All Tours…** (design/07) is not built: 05 §3's `⋯` has **Delete Finished Tours** only (my pick, kept).
- With two offers the second (**Commit and Pin…**) is below the stage's fold at the shot's size: TR23 (the narrow stage).
- A coding session kills a stray core by pid, only a `target/debug/vnoc --stdio` with parent 1 (TR22b's `pkill` also killed the
  installed extension's cores).

**As built (TR22c, `fe25c46`, `3ae704f`, `7832225`; reviewed).** `tours/document.rs` holds export, the import write, the line finder
and the ask sentence; the CLI calls it. Requests: `vnoc/tourExport { id }` → `{ text }` (as `vnoc tour export` prints it);
`vnoc/tourImport { text, replace? }` → `{ id, words }` or `{ refused: [{ place, line, words }], words }` (data, nothing written); the
import does what `finish` does after the end (IDs by the setting, the pin), new or `--replace`, as the CLI always did;
`vnoc/tours { check_all }` (a real check of every tour, the same rungs and wait), `{ first_on: key }` (the node's tours first, each with
`on`); `vnoc/tourAsk { key? | tour? }` → `{ text, words }` (for a row: its question and depth too). The screen: the row's menu whole
(05 §3's order, with **Export as JSON…** beside **Open as JSON**); the JSON tab's **Import into This Tour** with refusals as
diagnostics by line (cleared on edit); the section's `⋯`; **Ask for a Tour…** in every place; **Open Tour…** in the bar's `⋯` (tour and
MAIN), a picker with `on <name>` then `other tours`, `ignoreFocusOut`; **Show Docs** as the bar's `ⓘ`. Rust lib 567, lifecycle 82; TS
874 / 317. **The full window run was not clean:** Bend's 3; the import test (words, fixed in `7832225`); **Open Tour…** timed out at 120 s
(passes alone, twice); the `waits` window timed out (passes alone); a later `tour` run's *"the tour's bar"* hook timed out at s2. The disk
fell to 544 MB during that run (the session removed `target/debug/incremental`, 2.1 GB, to go on): a likely cause, not proven.

**Decided at TR22c's review:**
- The window suites are owed one full clean run on a disk with room (≥ 5 GB free), before anything is given to a person to try.
- **Show Docs** from a card's menu opens the docs of that card's stop, not the current one; the picker's rows show `on.words`; Check
  All Tours ends with the core's one sentence (how many changed). TR23.
- Everything else as built.

**The user, 2026-11-26** asked how far a prototype is. My proposal (awaiting their yes): **TR22p** next, a pre-release VSIX to try by
hand (TR24's by-hand pass moved ahead), then TR23 shaped by what that pass finds, then the rest of TR24.

**As built (TR22p, `1311ded`).** The owed full run is clean (only Bend's failures); 0.4.0 pre-release VSIX (19.7 MB, Bend inside) and
CLI archive; notices regenerated (`elkjs`, through mermaid: EPL-2.0 chosen of its two, fine beside Apache-2.0). `vnoc --version` still
says 0.3.1. `tours` is now rebased on `origin/main` (`a9dfd32`, with Bend's canvas PR #25) and has an upstream `origin/tours`.

**The user's by-hand pass on the 0.4.0 VSIX (2026-11-26)**, and what is decided for each:
- **Tour Settings… closes itself at once.** The same fault as TR22c's Open Tour… picker: a QuickPick or InputBox opened from a page's
  menu loses focus to the page. Every QuickPick and InputBox the extension opens gets `ignoreFocusOut` (Tour Settings…, Rename…,
  Open Tour…, any other), and a window test opens each from its menu and finds it still open after 1 s.
- **Nothing shows that an agent is writing a tour, and Stop is hard to find.** Stop was only `■` on a row's hover. Now:
  - a row being written: a live mark (a pulsing dot, still under reduced motion) and `writing 6/10`, and a **Stop** button always
    shown on the row, not only on hover;
  - the TOURS section's header, when any tour is being written: `● writing` (with the count when more than one) and, for one,
    **Stop**; the open tour's route header the same;
  - the status bar item (`▶ Tour 3/10 · writing 6/10`): its click offers **Stop Writing**;
  - after Stop the row says `stopping` until the agent's next command sees it, then `stopped` (the core's states, as now).
- **Nothing in the sidebar says which tour is playing, nor how to stop it** (the user, the same day). Only the route view's header
  and the status bar say it. Now:
  - in the list (All, On the selected node), the open tour's row is marked `▶ playing · stop 2/5` (the core's place) and has a
    **Stop Playing** button, always shown;
  - the TOURS header, while the list is shown and a tour is open: `▶ <title>` (click: back to its route) and **Stop Playing**;
  - the route header's `×` becomes a labelled **Stop Playing** button (tooltip: *the place is kept*);
  - the status bar item's click offers **Stop Playing** too;
  - **Stop Playing** closes the tour as `×` does now: the stage closes, the place is kept, the row says `stop 2/5` after. Different
    from **Stop** on a tour being written, which stops the agent: the two words stay apart.
- **The lit lines are not noticeable.** design/03 §2 asks for a 3 px gutter bar, a background, and **every other line at 55%**. Make the
  contrast plain in both themes: other lines at 40%, the lit lines' background from the theme's
  `editor.rangeHighlightBackground` (or `editor.findMatchHighlightBackground`), at full strength, and the gutter bar in the focus
  colour. Checked in a light and a dark screenshot.
- **`＋` should open a prompt builder.** design/07's W18 (a click copies a sentence) stays, but the sentence is built in a small form
  first. `＋`, the node menu's **Ask for a Tour…**, a row's **Ask for a New Tour**, and the empty states' **Ask for One** / **Copy a
  Request** all open it, filled in; it sits at the top of the TOURS section, above the list, and Esc or **Cancel** closes it:
  - **About:** the selected node (a chip; empty: "anything: say it in the question");
  - **Question:** *What do you want to understand?* (optional, one or two lines);
  - **How far:** *Let the agent decide* (default) · *Only this* · *And what it calls* · *Two levels of calls*;
  - **Detail:** *Let the agent decide* (default) · *Quick* · *Normal* · *Detailed*;
  - the prompt as it will be copied (the core's text, live), and **Copy Prompt**, then *"Copied. Paste it to your agent."*.
  The core builds the text: `vnoc/tourAsk { key?, question?, reach?, detail? }`, e.g. *"Walk me through
  orders/service.py::OrderService.parse_order: why does it retry twice? Cover it and what it calls. Make it a quick tour."*
- **How the depth is controlled now** (the user did not know): the agent decides; above ~12 stops it asks a sizing question. `detail`
  exists (`start --detail quick|normal|detailed`, the text budgets) but the skill never says to use it. The skill gains: *when the
  request names a reach or a detail, use them (`--detail`, the route depth); otherwise choose the detail from the question: a quick
  look → quick, learning or an onboarding → detailed, else normal.*
- **Tour titles look like file paths.** The skill says `--title "<what it is of>"`, which an agent reads as the node's path. The skill
  says instead: *a phrase that says what the tour explains, as a person would name it ("How checkout survives a payment timeout"),
  never a path or a function's name*. The core refuses a title that is only a node address or a path (`title: say what the tour
  explains, not where it is`) with its own example. Old tours keep theirs (Rename…).
- **The steps read as one-liners** (the user's tour of `base_resource.py`: intros and blocks of 60–250 characters). Not a limit: the
  budgets are 420 (normal) and 720/900 (detailed), and no text came near them. It is the skill: *"a step is short Markdown that says
  why"*, *"never retell it"*, and every tour `normal` because the skill never sets `--detail`. The skill now asks for a full
  paragraph: *2–4 sentences: why this part exists, what depends on it or would break without it, and anything surprising (a bug,
  a trap, a default)*; quick tours keep one sentence; detailed tours use their larger budget for the reasoning. Still never a
  retelling of the code. The same tour also shows two other skill faults, both fixed with the title rule: its title was the path
  (`knowledge_base/base_resource.py`) and its `question` was the copied request (`Walk me through app/api/…`). The skill: *the
  question is what the user wants to understand; a request that only names code has none: leave `--question` out*.
- **The version:** the Cargo version goes to 0.4.0 with the extension, and the skill's `vnoc-version`.

**TR23 is re-cut:** **TR23a** = the by-hand pass above (two commits: the core and skill, then the screen). **TR23b** = the old TR23 list:
narrow stage, mini bar, keys (↑↓, Alt+←), labels, the contents' fold, the cover title said twice, duplicate SVG ids in the Mermaid
overlay, the row's hover `⋯`, the action strip over `▶ N`, the sticky header over the first stop row, the second offer below the fold,
the code pane's long lines (no wrap), **Show Docs** from a right-clicked card, the picker's `on.words`, Check All Tours' sentence.

**As built (TR23a, `4660181`, `029018e`; reviewed).** `vnoc/tourAsk { key?, question?, reach?, detail?, tour? }` (with `tour`, `filled`
prefills the form); the title check (a title with no whitespace that is an address, a path, a folder or the root's name is refused,
in `start`, `edit`, `tourEdit` and import); the skill's title, question, reach/detail (`start --route <node> --depth N`, `--detail`) and
full-paragraph rules; 0.4.0 in every crate, both drivers and the skills; guide examples renamed (*How an order is parsed*).
`ignoreFocusOut` on every picker and input box. Writing: `● writing` + **Stop** on the row and in a strip under the TOURS header;
playing: `▶ playing` on the row, `▶ <title>` in the strip, **Stop Playing** in the route header; the status bar item's pick (Show Tour ·
Stop Writing · Stop Playing). The lit block: others at 40%, the theme's range highlight, a `focusBorder` bar. The prompt builder at the top
of TOURS (native selects). Rust lib 574, lifecycle 82, cli 275; TS 417 / 896 / 318; window 189 + Bend's 3 + one not reproduced (*"Esc closes
the tour's context and MAIN is as it was"*, MAIN's viewport). The disk fell to 843 MB again (debug info): prompts set
`CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=line-tables-only` for every cargo call from now on.

**Decided at TR23a's review** (for TR23b):
- **Dark themes:** Dark Modern's range highlight is ~4% white, so the lit block shows mostly by the dimming. The lit lines' background
  gets a tint of the bar's colour on top of the theme's (`color-mix(in srgb, var(--vscode-focusBorder) 14%, transparent)`), in every
  theme.
- **`writing` is said twice on a writing row** (line 1 `● writing`, line 2 `writing · 1 of 3 stops`): the core's line 2 leaves the state
  out when line 1 says it, and the title gets the room.
- **The route header is too narrow** for `How t…` beside `1 of 2` and **Stop Playing**: at a narrow width **Stop Playing** is an icon
  button (`■`, the label in its tooltip). Part of TR23b's narrow-width work.
- The playing row was not photographed: TR23b's shots include it.
- The unreproduced window failure: watched in TR23b's full run; if it recurs it is fixed there.

**As built (TR23p, `42cc486`, `bbcdf49`; reviewed).** The lit block is tinted with `focusBorder` at 14% over the theme's highlight: plain
in Dark Modern (screenshot `tools/media/out/tr23p/04`). Notices regenerated. A 0.4.0 VSIX (19.8 MB; `vnoc 0.4.0`; Bend inside) and CLI
archive for the user. **`vsce` refused to package:** Dependabot raised `@types/vscode` to `^1.146.0` (`d8bb328`, on `origin/main` too)
above `engines.vscode` `^1.90.0`; the session set it back only while packaging.

**Decided at TR23p's review:** `@types/vscode` is pinned back to `^1.90.0` (lockfile too) and Dependabot ignores it; `engines.vscode`
stays `^1.90.0`. Raising the engine would refuse installs in Cursor and other editors built on an older VS Code. `origin/main` has the
same fault: it goes with the 0.3.1 deadlock fix if the user wants one on `main`. TR23b's first commit.

**Next, in this order:**
- **The user tries the 0.4.0 VSIX**, then **TR23b** (the pin above first), then TR24.
- Later, when they hurt: `tests/cli.rs` (13,476) as one test binary with a `tour` module; `tours/view.rs` types and text apart;
  `cli/tour/act.rs` by its parts (pin, IDs, settings).

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

**Changed by the user (2026-11-25): Markdown everywhere, with Mermaid.** A step's text (intro, block, detail) is Markdown, drawn by
a full renderer, and a ` ```mermaid ` fence is drawn as a diagram. Built in **TR19b** (after TR19, before TR20):
- **What may be written** (02 §4 changes; the refusal sentences say each): paragraphs, `code`, *emphasis*, **strong**, lists,
  tables, block quotes, code fences, a ` ```mermaid ` fence, links (a click goes to the host, as in the doc panel). Still refused: raw
  HTML, images, and a Markdown heading inside a text (`#`…`######` at a line's start: the `##`/`###` lines are the stop's own block
  structure, 02 §4). At most **one** Mermaid fence per step; a fence (code or Mermaid) is 30 lines or fewer.
- **Budgets, loosened:** the prose (everything outside fences and tables) counts, at about one and a half times today's: intro
  (240, 420, 720), block (240, 420, 900) for quick, normal, detailed; a `same_as` intro (180, 180, 240). Fences and tables do not count,
  but are held by their own limits (30 lines; a table 12 rows or fewer). Titles stay at 60.
- **The text keeps its lines.** Today `body.rs` joins a paragraph's lines; from TR19b the step's text is stored as written (its
  newlines kept), because a list, a table and a fence need them. **No format change**: `text` is still a string (schema 1, frozen, 01
  §11). A tour written before reads the same: one line of prose is Markdown too. An older V-NOC shows the Markdown source as text.
- **The renderer** (`webview/src/tour/text.ts` becomes a Markdown renderer): markdown-it with `html: false`, as `docs/markdown.ts`
  (links to the host, images as their alt text); code fences highlighted by the same Shiki highlighter the cards use; Mermaid loaded
  only when a step has a diagram, `securityLevel: "strict"`, its theme from VS Code's light or dark; a diagram fits the text pane's
  width and a click opens it large. Still never `innerHTML` from anything but the sanitised renderer's output.
- **The skill** (04 §3): `say why, not what` stays; lists and tables when the point is a set of things; a Mermaid flowchart when the
  point is a flow across stops or files (best in the first stop's intro of a cross-file tour); a code fence only for what is not on
  screen (an example call, a payload), never the stop's own code.
- **The stage's code pane gets syntax colours** in the same step (the cards' Shiki highlighter and themes), moved up from TR23.

Next: [`06-build-order.md`](06-build-order.md)
