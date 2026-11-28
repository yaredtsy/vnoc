# 26.03 The core: from the log to a tour you can play

```
core
├── 1. replaying              lines → a tour: the tree, the route, the steps in reading order, the state
├── 2. finding each stop      by ID, by name · a block that moved · gone
├── 3. out of date            lazy, and recorded: when a check runs, what it reads, when a recorded answer still holds
├── 4. the place              where the reader is: one function decides what "next" is, for the canvas and the command
├── 5. as written             the code of a pinned commit, for a stop that changed or went
├── 6. tours on a node        the list's second view, ▶ N
├── 7. the module             files, and what is pure
├── 8. requests               vnoc/tours · vnoc/tour · go · halt · pin · share · delete; the notifications; the watcher
└── 9. what a tour never does
```

Layers, each cheaper than the next, and each done only when something asks for it:

```
   the files in the two folders
        │  replay (§1): read one file, no code, no index                    for every tour, once, and again when its file changes
        ▼
   Tour: title, state, the tree, the texts, the names
        │  tours on a node (§6): a map from names and IDs to tours          from the replayed tours; no code read
        │
        │  the recorded check (§3): one stat per file of this tour          for a tour whose row is drawn, or that is opened
        │       still valid → use it.   not valid ↓
        │  find + check (§2): the index, and the files of this              for ONE tour; then recorded
        ▼                     tour's stops
   TourView: every stop with its node as it is now, and fresh · moved · changed · gone
        │  the place (§4): the tour's state file                            read with the view; written when the reader moves
        │  as written (§5): git show, for one stop                          only for a changed or gone stop of a pinned tour, when it is on the stage
```

## 1. Replaying

`tours::fold(lines) -> Tour` is a pure function: text in, a tour out, no disk, no index.

| Step | Rule |
|---|---|
| the first line | must be a `tour` record with a known `schema`. Otherwise the tour is `Unreadable { why }` |
| each later line | by its `type` ([`01`](01-on-disk.md) §4). Unknown type: skipped. Broken: one `Problem { line, why }`, skipped |
| the route | the stops in the order they first appeared (planned or written), each nested stop moved to after the last stop nested in its `from`. A `stop` with a known id replaces in place; `drop` removes it and what is nested in it |
| nesting | a stop's depth is its `from` stop's depth + 1; no `from`, or a `from` that was dropped: depth 0 |
| the number on screen | the position on the route, from 1 |
| a stop's blocks | a tree by `parent`; children in the order they were added. A block whose parent is missing is at the top ([`01`](01-on-disk.md) §10) |
| **the steps of a stop, in reading order** | the intro; then each top block followed by its details, depth first ([`01`](01-on-disk.md) §3). This list is what `Next` walks and what "step 2 of 5" counts. A planned stop has the intro only. A `same_as` stop and a short stop have the intro only because `add` refuses blocks on them; a file that has some anyway (a hand edit) shows them |
| being written | with a plan: the first planned stop is the one "the agent is on". With no plan: none; the route's foot says the agent is adding |
| the state, pinned | [`01`](01-on-disk.md) §4's table |
| the size | stops written, stops planned, steps. Minutes = steps × 20 s (2 steps guessed for a planned stop), rounded up |

Replaying 40 stops is parsing about 150 short lines. **Measured (TR5, `tours_checks measure-check`, macOS, release build; a tour of 40 stops and
100 blocks over 12 files, an 18 KB file; median of 9): 348 µs to read the file and fold it.**

## 2. Finding each stop, and comparing

`tours::find(index, project, tree, stop) -> Option<&Node>` is the whole of finding (D6), and the one function a later module replaces
(as built in TR4):

```
the stop has `node` (an ID), and the index has it   →  Index::resolve_ref(NodeRef::Id)         the def that carries it, wherever it is now
else                                                →  Index::resolve_ref(NodeRef::Location)   the exact path, qualname and occurrence
neither                                             →  None: GONE
```

An ID the index does not have falls back to the name (decided at TR4's review): a hand-deleted marker line on a function that is still
there finds it. If something else has taken that name since, its hash says so: the stop is *changed*, never silently fresh.

Then, for a stop that was found, `tours::check`:

```
1. its lines     the def's lines now, from the file on disk (source.rs), marker lines left out
2. the stop      hash(lines now) == the stop's hash?            → FRESH   else → CHANGED (the intro's mark)
3. each block    are the block's lines where they were, counted from the node's first line, with the same hash?   → FRESH, same place
                 else slide: is there exactly one run of lines, of the same length, with the block's hash,
                             inside its parent block's lines now (or the node's, for a top block)?
                                                                → MOVED: fresh, at its new lines
                 else                                           → CHANGED: shown at its old offset, cut to the node's end
```

| Result | Means | On screen |
|---|---|---|
| **fresh** | the lines are the lines the text was written about | nothing |
| **moved** | the same lines, somewhere else in the node (code was added above; an `ID:` line was added) | nothing. The block is lit at its new place. It is fresh |
| **changed** | the lines are not the same | pinned: the code as written (§5). Not pinned: the design's *changed* mark (05 §3, §4) |
| **gone** | the node is not in the code | pinned: the code as written. Not pinned: the row struck through; the text stays (05 §4) |

- A stop is *changed* when its intro or any of its blocks is. The stage marks the step that is.
- A changed general block does not make its details changed: each block has its own hash. A detail can be fresh inside a changed parent.
- A file's intro and a folder's intro are never *changed* (no hash); they can be *gone*.
- A planned stop has no hash and no check. Its node is found the same way, to show its code.
- The check reads **the file on disk**. The stage's code pane reads VS Code's document, as a card does, so with unsaved edits the pane
  can be one save ahead of the mark. The mark follows at the save.
- For a node found by its ID at a new path or under a new name, the view carries the node's **current** path, name and lines; the
  tour's own record is not rewritten.

## 3. Out of date: lazy, and recorded (D8)

**What never happens by itself:** a check of every tour. (`vnoc tour check` with no tour named does exactly that, because you asked.)

| A tour's freshness is asked for when | By |
|---|---|
| the tour is opened | `vnoc/tour` |
| its row is drawn in the list | `vnoc/tours` with the ids the list is drawing (`check: [ids]`) |
| `vnoc tour check`, `show --check`, `list --check` | the command |

**Asked is not checked.** The answer comes from the first of these that can give it:

```
1. the running core's memory     the answer it gave last time, if no `vnoc/filesChanged` named a file of this tour since,
                                 and the tour's own file did not change                              (no disk at all)
2. the recorded check            .vnoc/local/tour-state/<id>.json: valid when the tour's file has the recorded length and
                                 every recorded file has the recorded size and changed-time           (one stat per file)
3. a check                       §2: read each file of the tour once, hash each stop and block.
                                 Then record it: the state file is rewritten                          (N file reads)
```

| | |
|---|---|
| **what is recorded** | [`01`](01-on-disk.md) §9: the tour file's length, each file's size and time (a file that was not there is recorded as not there), and only what was **not** fresh: changed stops and blocks, moved blocks with their new lines, gone stops |
| **which files** | every path a stop names, and every path a stop found by its ID is at now. A stop that is *gone* by name records its named path, so the file appearing again makes the record invalid |
| **a one-shot command** | has no memory: it starts at 2, **before it builds an index or starts a driver**: a tour whose record is valid costs `stat`s only, and the index is built only when some tour needs rung 3. So `vnoc tour check` twice in a row reads the source files once, and the second run starts no driver |
| **what a record does not see** | a stop that is *gone* because its ID is Missing, whose code comes back in a file the tour never named (a restored branch): the record stays valid until one of its files changes. `--again` (and the next edit of a recorded file) checks anew. Accepted: it is a rare case, and it errs on the side of *gone* |
| **the open tour is live** | when a file of the open tour changes, the core drops its answer and sends `vnoc/toursChanged`; the window asks again and gets a new check. Edit `reserve_stock`, save, and stop 3 says so |
| **a file of no open or drawn tour changes** | nothing is computed. Its tours' recorded checks are simply not valid any more, and are found so when someone next looks |
| **the list after a restart** | the rows drawn ask; each is answered by `stat`s if nothing changed while VS Code was closed |

**Measured (TR5, the same tour and machine, median of 9; the index is already built, which is the cost the record avoids):** a real check, which finds
all 40 stops, reads each of the 12 files once and hashes every stop and block, **626 µs** (`Index::tree`, which the command builds first, 60 µs more); a
valid recorded check, the state file and 13 `stat`s, **45 µs**. The record is about 14 times cheaper than a check, and much more than that
with the driver: starting it and building the index is what a valid record leaves out. Replay (§1) is the same for both.

**[check]** TR0 (open check 7): that a size and a changed-time are enough to say "this file did not change". If a platform's clock is
too coarse for that, the record also keeps a hash of each file, compared only when the size and time are equal and the time is within
one tick of the check. **Answered for macOS (README §7, TR0 check 7): enough there (the time changed in 100 of 100 back-to-back rewrites of the same size, 44 µs apart or more). Linux and Windows: TR5's tests in CI.**

## 4. The place (D17)

Where the reader is in a tour is kept by the core, in the tour's state file, so that the canvas and a command have the same answer.

```rust
pub struct Place { pub at: At, pub seen: Vec<StopId>, pub back: Vec<At>, pub moved: Time, pub by: By, pub open: bool }
pub enum At { Cover, Step(StepId), End }
pub enum Move { Next, Prev, NextStop, PrevStop, Back, To(At), Jump(At) }

/// Pure: the tour's steps in reading order, the place, a move → the new place. No disk.
pub fn go(tour: &Tour, place: &Place, m: Move) -> Place
```

| Rule (design 03 §4) | In `go` |
|---|---|
| `Next` from a stop's last step is the first step of the next stop; from the last step, the end | the reading order of §1, across stops |
| `NextStop` / `PrevStop` | the next / previous stop's intro |
| a `Jump` (a row, a card, a segment, a call in the code, **Go there**, `vnoc tour go s4`) pushes where you were onto `back`, 20 at most; `Next` and `Prev` never do | |
| `Back` pops it | |
| a stop is **seen** when its last step has been the place | |
| a place whose step no longer exists (dropped, replaced with fewer blocks) | moved to the nearest that does: the stop's intro, else the next stop, else the cover. Ids are never reused (D5), so it can never land on a **different** step |
| a stop that is planned and not written | can be the place: its step is its intro, with no text yet (design 02 frame 6) |

- **The canvas does not compute "next".** A click on `Next` is `vnoc/tourGo { id, move: "next" }`; the core answers with the new
  place and writes the state file. The core is a local process: the round trip is not something a reader sees.
- **`vnoc tour go` writes the state file** from another process. The watcher tells the core; the core reads the place and sends
  `vnoc/tourPlaceChanged`; a canvas showing that tour moves. A canvas that is not showing it does nothing (W11).
- **`open`** is set when a window makes the tour its tour context and cleared when it closes it. It is what lets `where` say "open on
  the canvas".
- **One place per tour on this machine** (ask 3): two windows on one project are on the same step and move together.
- Writes of the state file take the tours lock and replace the file whole. The place and the check are two parts of one file; a
  writer of one reads the file under the lock and keeps the other part.

**The agent edits while the reader moves.** "The tour's id is the session id" means only this: `where <tour>` has one answer. It does
not mean the agent and the reader share a cursor. They write two different files, and **every write names what it writes to by id,
never "the current step"**:

```
the agent    →  the tour's file     .vnoc/local/tours/<id>.jsonl          add <tour> <node> · add --replace s3 · drop s3.b2
the reader   →  the state file      .vnoc/local/tour-state/<id>.json      the place: at s3.b2
```

| The agent | The reader, meanwhile | What happens |
|---|---|---|
| adds stop after stop | is reading stop 2, pressing Next | the tour's file grows; the place is not touched. New stops appear on the route and the map; the stage does not move |
| adds blocks to the stop the reader is on (`--to s3`) | on `s3.b1` | the stop gets more steps: "2 of 3" becomes "2 of 5". The step on the stage is the same step |
| replaces the stop the reader is on (`--replace s3`) | on `s3.b2` | `s3` keeps its id and its place on the route; its blocks are new ones with new ids. `s3.b2` is no more, so the place moves to `s3`'s intro (the rule above), and the stage shows the new text. Nothing else moves |
| drops the stop the reader is on | on `s3` | the place moves to the next stop |
| a person runs `import --replace` | anywhere | ids in the document are kept, so the place holds where its step is kept |
| runs `where` | moves a second later | the answer is a snapshot, and says when the reader last moved |
| runs `go s7` | presses Next at the same moment | two writes of one small file, each under the lock, each applied to the place as it is then. The later one wins. The agent's is a jump, so `←` takes the reader back |

**As built (TR10).** `back` holds `At`, not a step: a jump from the cover or the end has one to push. Jumps (`s4`, `s3.b2`, `cover`,
`end`) push, `next`/`prev`/`next-stop`/`prev-stop` never do, a jump to where you are pushes nothing; `back` with nothing to pop stays
and says `nothing to go back to`. `prev-stop` from the end is the last stop's intro. A stop of one step (short, `same_as`, planned) is
seen on arrival. `settle` applies the nearest-step rule on read (for `where` and before a `go`, not written back by `where`) and drops
`back` and `seen` entries the tour no longer has; "the next stop" of a dropped stop is the lowest stop id above it (it has no place on
the route any more). `go` holds the tours lock from reading the tour and the place to writing the place. A halted tour reads
`stopping` until the agent's next command. `WhereView` and its text are `tours/view.rs`; the builder that reads the files and the pin
is `cli/tour.rs`.

Nothing here needs the two to wait for each other: the lock is held for one small write, and the only thing that can be "lost" is
which of two moves made in the same instant came last.

## 5. As written (D19)

For one stop of a pinned tour, when the stage needs it:

```
tours::pin::as_written(root, tour, stop) -> Option<Excerpt>
    the pin's `where` for the stop:  [path, first, last]
    git show <commit>:<path>         → its lines first..=last, as source.rs would read them
    None when git has no such commit or no such file in it
```

| | |
|---|---|
| **when** | only when a changed or gone stop of a pinned tour is on the stage, or the reader pressed **As Written** on any stop of a pinned tour |
| **the lit lines** | the pin's `where` for each block: the lines of that commit, exact. No sliding, no hash: the pin was made when the files on disk were that commit ([`01`](01-on-disk.md) §8) |
| **kept** | in memory by commit and path. A commit's file never changes |
| **`None`** | the stop falls back to how a tour that is not pinned shows it, and the row says the commit is not in this repository |
| **making a pin** (`tours::pin::make`) | the check of §2 (all fresh, none gone), then `git status --porcelain -z -- <paths>` (says nothing) and `git ls-tree --name-only -z HEAD -- <paths>` (names every path), then `git rev-parse --verify -q HEAD` (exit 1 and nothing when there is no commit yet: TR0); [`01`](01-on-disk.md) §8 step 2; the `pin` record carries where every stop and block is now. `can_pin` answers the same questions without writing, for `check` and for the row's menu |
| **`--commit`** (`tours::pin::commit_files`) | `git add -- <untracked among them>`, `git commit --only -m … -- <paths>`. From `vnoc tour pin --commit`, `vnoc/tourPin { commit: true }`, and `finish` only when `[tours] commit = "always"` or `--commit` says so (02 §5). After a failed commit, `git reset -q --` on the files `add` took in: nothing stays staged |

**As built (TR9).** `pin::head(project, tour) -> Result<Commit, Vec<Reason>>` answers what needs no code (finished, a written stop,
git runs, a repository with a commit, no merge or rebase in the middle), so a project with no git is told so without a driver.
`check::check_placed` returns `(Recorded, Placed)` (the check, and where each stop and block is now) and `check` calls it.
`pin::can_pin(project, tour, head, &recorded, &placed) -> Pinnable`, `pin::make(head, &placed) -> PinRecord`. `vnoc tour show --text
--as-written` implies `--check`, marks lit lines `>` at the line's start (`vnoc code` has no lit mark), lights all of a stop with no
blocks, and adds `as_written` to `--json`. A folder stop has no `where` entry: it has no lines.

**Git, as TR9 found it.** Reads run with `--literal-pathspecs` and `GIT_OPTIONAL_LOCKS=0`; the commit names its paths
`:(literal)<path>` instead, so hooks do not inherit `GIT_LITERAL_PATHSPECS`. `check-ignore` refuses literal pathspecs and runs without
(only a reason's wording). `status` needs `--untracked-files=all`, else a new folder is one `?? dir/` line. `show` gets
`--no-textconv`. `rev-parse --git-path` finds `MERGE_HEAD`, `rebase-merge`, `rebase-apply` (and from TR9b `CHERRY_PICK_HEAD`,
`REVERT_HEAD`) in a worktree too: git 2.5 or newer.

## 6. Tours on a node

The list's second view (*On the selected node*, design 07 §4), `vnoc tour list --on`, and the `▶ N` on a node (W17) need "which tours
pass through this node". That needs no code and no check.

`tours::on::Map`, built from the replayed tours and rebuilt for one tour when its file changes:

```
by ID        node ID            → [(tour, stop)]         for stops that have a node ID
by name      path::qualname#n   → [(tour, stop)]         for every stop
by path      path               → [(tour, stop)]         for "inside this file / folder" (a prefix walk)
```

| The selected node is | Found by | Line 2 of the tour's row |
|---|---|---|
| a function or a class | its ID, if it has one; else its name | the tour's first stop: `starts here` · else `stop 3 of 10` (`stops 2 and 4 of 10`) |
| a class | also the names inside it (`Order.` + anything) | `2 stops inside` |
| a file, a folder | the paths at or under it | `starts here` · `4 stops inside` |

- **By what the tour wrote, as D6 finds it.** A function with no ID that was renamed is not found under its new name: its stop is
  *gone*, and the tour is not "on" the new name. A function with an ID is found under any name.
- The count for `▶ N` is the number of tours, not of stops. A tour being added or removed changes counts: the core sends
  `vnoc/toursChanged`, and the pages that draw counts ask for them again. It does **not** move the index's revision (D12): the counts
  travel in their own small answer (`vnoc/tourCounts`), not in the structure. **[check]** TR21: whether that second request is simpler
  than a field on `Node`; the rule that decides is D12. **Answered (TR15b, `1a897c3`): a request.** A field on `Node` would put tour data in the structure, so a tour write would need a
new revision or go stale. `vnoc/tourCounts` answers `{counts: {<Node.id>: n}}`, keyed by the `id` `vnoc/structure` gives a node (the
path for a file or a folder, `path::qualname#n` for a function or a class), for every node `list --on` would list a tour for: a class,
a file and a folder with stops inside count too; a tour with two stops on one function counts once.

## 7. The module

```
crates/vnoc/src/tours/
├── mod.rs
├── record.rs     the lines: one serde type per record; parse a line, print a line                         pure
├── fold.rs       lines → Tour: the tree, the route, the steps in reading order, state, problems           pure
├── body.rs       the text an agent sends → an intro and a tree of blocks                                  pure
├── doc.rs        the nested JSON document of 02 §10 ⇄ the flat lists: nest, flatten                       pure
├── rules.rs      budgets, the inline text rule, lines, nesting, overlaps; every refusal and its sentence  pure
├── hash.rs       the hash of some lines, with what is left out                                            pure
├── place.rs      Place, Move, go                                                                          pure
├── find.rs       a stop → its node now: by ID, by name. The one function a later module replaces
├── store.rs      the folders (through userdata/layout.rs), ids, start, append under the lock, replace,
│                 list, read, share, delete (to the trash), restore; the state file: read, write a part
├── check.rs      compare, slide; the recorded check: valid?, read, write; the answers kept in memory
├── pin.rs        can_pin, make, commit_files, as_written; every call of git
├── settings.rs   [tours] in vnoc.toml: read (with the defaults), change one key (toml_edit)                 
├── finish.rs     end, then by the settings and the flags: the IDs, the pin (02 §5)
├── rebase.rs     after V-NOC's own edit of a file: new lines and hashes for the stops and blocks that were fresh before (01 §7).
│                 As built (TR7a): rebase(vnoc_root, project, index, host, &Edited { path, before, after, edits }); stop ranges from
│                 the driver's outline of `before` and `after` (the index is a moment old), block ranges mapped through the edits
├── on.rs         the tours-on-a-node map
├── route.rs      a route from the call tree
└── view.rs       Tour + check + place → what a command prints and a request answers
```

| Library function | Used by |
|---|---|
| `store::start(root, StartParams) -> TourId` | `tour start` |
| `store::append(root, id, &[Record])` (the lock, the newline rule, one `write_all`); `store::append_locked(&ToursLock, …)` for a caller that already holds the lock: read the file, make the ids, append, all under one lock (01 §5) | every step-by-step write; the core for `halt` |
| `add::plan_stop(tour, index, node, body, options) -> Result<Vec<Record>, Vec<Refusal>>` | `tour add`: everything of 02 §4, with no disk write |
| `doc::nest(&Tour) -> Document`, `doc::flatten(Document) -> Result<Flat, Vec<Refusal>>` | `export`; `import`: the two translations of 02 §10, pure |
| `doc::plan_tour(index, flat, old: Option<&Tour>) -> Result<Vec<Record>, Vec<Refusal>>` | `import`, `import --replace`: `plan_stop`'s checks for every stop; a refusal carries its place in the document as the person wrote it |
| `store::list(root) -> Vec<(Where, Tour \| Unreadable)>` | `tour list`, `vnoc/tours` |
| `check::fresh(root, index, tour, mode) -> Checked` (memory, record, or check: §3) | `tour show --check`, `tour check`, `vnoc/tour`, `vnoc/tours { check }` |
| `place::go`, `store::read_place`, `store::write_place` | `tour where`, `tour go`, `vnoc/tourGo` |
| `pin::make`, `pin::can_pin`, `pin::commit_files`, `pin::as_written` | `tour finish`, `tour pin`, `vnoc/tourPin`, `vnoc/tourCode` |
| `finish.rs`: the decisions and their words; the order (IDs, then the pin) is `cli/tour.rs` `ids_after_end` then `pin_tour`, one path for both (as built, TR7b and TR9; no single `finish::finish`) | `tour finish`, `tour import`; `offers` is what an `"ask"` setting leaves for the user |
| `settings::read(root)`, `settings::set(root, key, value)` | `finish`, `tour settings`, `vnoc/tourSettings` |
| `ids::give(env, tour, stops)` (the attach flow's `ensure_node`, then one `ids` record) | `tour ids`, `vnoc/tourIds` |
| `on::Map::of(tours)`, `.on(node) -> Vec<On>` | `tour list --on`, `vnoc/tours { on }` |
| `route::propose(index, calls, node, depth) -> Route` | `tour route`, `start --route` |

`plan_stop` is separate from `append` for the reason 22 keeps `plan` apart from `run`: the checks are a table of tests with no disk.

## 8. Requests

The wire is snake_case, as the rest of the protocol. `shared/tours.ts` has the TypeScript side.

| Request | Params | Answer |
|---|---|---|
| `vnoc/tours` | `{ on?: node id, check?: [tour id] }` | `{ tours: [TourRow] }`, newest first |
| `vnoc/tour` | `{ id }` | `TourView` (with the place), or `{ unreadable: why }` |
| `vnoc/tourGo` | `{ id, move }` | the new `Place` |
| `vnoc/tourOpen` | `{ id, open: bool }` | sets `place.open` |
| `vnoc/tourCode` | `{ id, stop }` | the stop's code as written: `{ first, lines }`, or `null` |
| `vnoc/tourHalt` | `{ id }` | writes `halt`. The answer is the new state |
| `vnoc/tourPin` | `{ id, commit?: { message } }` | pinned, or why not: stale stops, the files not committed |
| `vnoc/tourIds` | `{ id }` | the files that were changed |
| `vnoc/tourEdit` | `{ id, title?, question? }` | |
| `vnoc/tourShare` | `{ id, share: bool }` | moves the file between the folders |
| `vnoc/tourDelete` | `{ id }` | the tour's file and state go to the trash. Answers a token for Undo |
| `vnoc/tourRestore` | `{ token }` | Undo |
| `vnoc/tourSettings` | `{ ids?, commit? }` | the two settings of [`01`](01-on-disk.md) §12; with a param, that key is changed first |
| `vnoc/tourCounts` | — | `{ counts: { node id: n } }` for `▶ N` (§6) |

| Notification | When |
|---|---|
| `vnoc/toursChanged { ids }` | a tour's file changed, appeared or went; or the answer the core gave about its freshness no longer holds |
| `vnoc/tourPlaceChanged { id, place }` | the place was moved by another process (`vnoc tour go`), or by another window |

```ts
// shared/tours.ts (what the pages get; nothing here is computed in TypeScript)
export interface TourRow {
  id: TourId; title: string; question: string | null; author: string | null;
  where: "private" | "project";
  state: "writing" | "stopping" | "stopped" | "finished" | "unreadable";
  quiet: boolean;                                  // writing or stopping, and no change for 5 minutes: "no news"
  pin: { commit: string; readable: boolean } | null;   // null = not pinned
  offers: { ids: number; commit: string[] } | null;    // what the cover may offer: stops with no ID, files not committed. By the settings: null when "never"
  root: { kind: NodeKind; name: string } | null;   // the first stop: the row's icon
  written: number; planned: number; steps: number; minutes: number;
  at: string;
  place: { stop: number | null; at: "cover" | "step" | "end"; seen: number } | null;   // null = never opened: "new"
  on: { how: "starts" | "stops" | "inside"; stops: number[]; first: StopId } | null;   // only with `on`
  fresh: "fresh" | "changed" | null;               // null = not asked for (the row is not drawn)
}

export interface TourView extends TourRow {
  stops: TourStop[];                               // the route, in order
  writing: StopId | null;                          // the planned stop the agent is on
  at: Place;                                       // where the reader is (§4)
  problems: { line: number; why: string }[];
}

export interface TourStop {
  id: StopId; number: number; depth: number;
  state: "planned" | "written";
  node: { tree: string; kind: NodeKind; name: string; path: string; range: Range } | null;   // null = gone
  was: { kind: NodeKind; name: string; path: string };                                      // what the tour wrote: for a gone stop
  from: StopId | null; line: number | null; same_as: StopId | null;
  fresh: "fresh" | "changed" | "gone";
  steps: TourStep[];                               // in reading order; [0] is the intro
}

export interface TourStep {
  id: StepId;                                      // "s3" (the intro) or "s3.b1"
  depth: 0 | 1 | 2;                                // 0 the intro · 1 a block · 2 a detail
  parent: StepId | null;                           // the block a detail is in: the stage lights it thin
  title: string | null; text: string;
  lines: [number, number] | null;                  // file lines NOW (moved blocks already moved); null for an intro
  fresh: "fresh" | "changed";
}
```

**As built (TR15a, `22f5e88`).** The answers are the commands' `--json` types (`tours/view.rs`), built by `tours/serve.rs` for both
the CLI and `server.rs`; `shared/tours.ts` is written from them in TR16, not from the sketch above. Names that differ from it: a row's
place summary is `place` (`{stop, at, seen}` or `null`; `at` on a row stays the tour's time), the view's full place is `place`
(`at`, `seen`, `back`, `moved`, `by`, `open`), a stop's steps in reading order are `reading` (`steps` is a count), and freshness stays
`check` (with `from`: `memory` is new, the rung-1 answer; the commands never print it). New keys: `minutes` (`tour.size()`, 20 s a
step), `quiet` (writing or stopping, the tour's file untouched for 5 minutes), `root`, `writing` (the first planned stop while the
tour is writing). A gone stop's steps are all `changed`; a moved block is `fresh` with its lines moved. `place`, `root`, `writing`
are explicit `null`. `vnoc/tours` takes `{check?}`; `tourOpen` writes only when `open` changes and never touches `moved`; `tourHalt`
answers `{halt: written | already | finished, state}`. A tour request takes the tours lock only, and waits for the first index
build only when some tour needs a real check (rung 3). Paths are classified against the git root; a tour-folder path not named like
a tour rescans both folders. **The echo:** the core keeps the place it last wrote or announced per tour, and a state-file event is a
move only when the file's place differs; a check that records itself notes the place already there. A watcher event that lands
between `go`'s write and its note is an echo of the same place: harmless, accepted. Measured: a tour-only batch leaves a new `.py`
file unseen by `vnoc/structure` (no walk); a mixed batch moves the revision by 1. The test temp folders are removed by a thread-local
guard (`tests/common/testdir.rs`; `VNOC_KEEP_TEMP=1` keeps them).

**As built (TR15b – c, `1a897c3`, `1524165`).** `vnoc/tours { on }` takes the tree key first, then any reference `node_ref` reads;
`on::Map` is owned by the server's `Tours`, keyed by (tour, folder), rebuilt for one tour when it is replayed. `vnoc/tourCode` answers
the `AsWritten` of `show --as-written` (commit, path, lines, code, lit) for any stop of a pinned tour whose commit is there, else
`null`. The actions are `tours/act.rs`, shared with the commands (`cli/tour.rs` lost about 630 lines). Answers: `tourPin`
`{tour, state, commit?, already?, reasons?}` (a reason has a `code`, e.g. `uncommitted` with its files and their status);
`tourShare` `{tour, folder, path}`; `tourEdit` `{tour, title, question}`; `tourDelete` `{token}`; `tourRestore` `{tour, folder}`;
`tourIds` and `tourSettings` their `--json` types. Locks: `tourPin {commit}` and `tourIds` take `attaching`, then the project write
lock, then the tours lock, as `write_id`; `tourSettings` with a param takes the project write lock, as the command. **`tourIds` writes
to disk, as the command does** (the editor sink needs an open document, and a stop's file usually is not open): **TR22** must save a
dirty buffer of those files first, or ask, before it sends `tourIds`. `offers` on rows and the view: `{ids, commit}` or `null`;
`ids` counts written stops with no node ID (only under `ids = "ask"` and a project that writes ID lines); `commit` is git status of
the written stops' own paths (only under `commit = "ask"`, and not for a tour pinned at a commit HEAD has behind it), with no check:
a stale stop is not weighed (the pin's answer says it), a stop found at another path is not looked at. **Cost:** as built, one
`git status` per finished unpinned row per list call: TR14d makes it one per list call.

**The watcher.** No new one. `extension/src/core.ts` already watches `.vnoc/**` and sends `vnoc/filesChanged { paths }`. In the core's
`files_changed` (`server.rs`):

| A path | Goes to |
|---|---|
| under `.vnoc/tours/` or `.vnoc/local/tours/` | the tours store: replay that file again, send `vnoc/toursChanged`. **No index update** (D12) |
| under `.vnoc/local/tour-state/` | if the place in it is not the one the core last wrote: `vnoc/tourPlaceChanged`. No index update |
| `.vnoc/local/tours.lock` | ignored |
| anything else | as today; and the answers in memory (§3, rung 1) of the tours that name that path are dropped |

A batch with only tour paths starts no update and moves no revision. **[check]** TR0 says what such a path does today. **Answered (README §7, TR0 check 2): today it starts an update (a walk, a read of `.vnoc/nodes`) that changes nothing, moves no revision and sends no `indexChanged`.**

**`.vnoc/` above the opened folder** (open check 11): VS Code's watcher does not see it. While a tour is open, the extension asks
`vnoc/tour` again every 2 seconds in that setup only; the core answers from the two files' lengths and times, so a tour that did not
grow and a place that did not move cost two `stat`s.

## 9. What a tour never does

- It never starts an index update, and never moves the revision.
- It never writes into the source or into `nodes/`, and never commits, except when you run `vnoc tour ids` or `vnoc tour pin --commit`,
  or the project's `vnoc.toml` says `"always"` for it ([`01`](01-on-disk.md) §12). It never checks anything out.
- It never repairs itself: a stale text stays as written, marked. Fixing it is the agent's `add --replace`, asked for by a person.
- It never hides a problem: a broken line, a gone stop and a changed step are each shown where they are (the design's T9).

Next: [`04-skill.md`](04-skill.md)
