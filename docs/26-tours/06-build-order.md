# 26.06 Build order

```
build order
├── 1. rules for every step
├── 2. phase 0: checks                    TR0
├── 3. phase 1: the file and the commands TR1 – TR12    stop points: a tour is written and printed (TR4) · every command exists (TR12)
├── 4. phase 2: the skill                 TR13 – TR14   stop point: real agents write good tours, measured
├── 5. phase 3: the screen, playable      TR15 – TR19   stop point: a tour plays on the canvas
├── 6. phase 4: the screen, complete      TR20 – TR24   live writing · out of date and as written · on a node · the user's actions · by hand
└── 7. what each step leaves working
```

On a branch `tours` from `main`. Each step is one commit and leaves the branch green. Phases 1 and 2 touch `crates/` and `skills/`
only; nothing on screen changes until TR15. The order is agreed (README §2, row 13): the files are the part that cannot change later,
so agents use them for real (TR14) before any page depends on them.

## 1. Rules for every step

- **Stage files by path.** Never `git add .` or `git add docs`. Don't touch `docs/futureplan/`.
- **No logic in TypeScript** (P1, [`05`](05-on-screen.md) §6). If a step finds itself replaying a tour, comparing a hash, counting
  stops, sorting the list or working out the next step in the extension or a page, that belongs in `crates/vnoc/src/tours/`.
- **Nothing outside `crates/vnoc/src/tours/` and `cli/tour.rs` learns the files' formats.** `userdata/layout.rs` gets the paths, and
  that is all `userdata/` knows.
- **Finding a node is `tours::find` and nowhere else** (D6). No other file of the module asks the index where a stop's node is.
- **Every call of git is in `tours/pin.rs`**, with its arguments built there and nowhere else.
- **A tour never moves the index's revision** (D12). TR15 has the test; every later step keeps it green.
- **Checks per commit, light:** `cargo test -p vnoc` and `cargo clippy` when `crates/` changed (the Python driver built first, or
  `VNOC_DRIVER`); typecheck and the touched packages' unit tests when TypeScript changed; the window suite `VNOC_SUITE=tour` (added in
  TR17) from TR17 on, with `env -u ELECTRON_RUN_AS_NODE`; full `pnpm -r test` on the last step of phases 3 and 4.
- **A test that needs git** makes its own repository in a temp folder, as `tests/cli.rs` does for `check --changed`.
- **Golden files** (`crates/vnoc/tests/golden/`) are regenerated with `VNOC_BLESS=1 cargo test -p vnoc --test cli` and read before
  they are committed.
- A step that settles a `[check]` or a `[measure]` writes the answer into the file that asked, in the same commit or a `docs:` commit
  after.
- Spec gaps go into these docs, not into silent choices.
- `design/` is not edited. A change to what the screen does is written in [`05-on-screen.md`](05-on-screen.md).

## 2. Phase 0: checks

### TR0 Seven checks (no product change)

One example program, `crates/vnoc/examples/tours_checks.rs`, with a subcommand per check (as A0's `oneshot.rs`: it can use the
library, the driver and the sample, and it is kept for measuring again), and three things looked at by hand. Each answer is written
into the README §7. Nothing under `crates/vnoc/src/`, `extension/`, `webview/` or `shared/` changes.

| # | Check | How |
|---|---|---|
| 1 | an older V-NOC and the new folders | **the command:** the released 0.3.1 `vnoc` (the release archive, unpacked into a temp folder) on a copy of the sample with `.vnoc/tours/x.jsonl`, `.vnoc/local/tours/y.jsonl`, `.vnoc/local/tour-state/y.json`: `status`, `tree`, `check`, `doc new`. A listing of the copy before and after differs only by the new doc; `check` passes. **By hand (yours):** the released VSIX on the same copy: open, browse, attach a doc; nothing listed, `git status` shows nothing new |
| 2 | what a change under `.vnoc/local/` does today | read `server.rs` `files_changed` and `index/build.rs` and say, with file and line, whether `.vnoc/` is read again and whether the revision moves; then prove it with a probe in the style of `tests/lifecycle.rs` (the real core, a `vnoc/filesChanged` for `.vnoc/local/tours/y.jsonl`, wait for `vnoc/indexChanged`). The probe is not committed |
| 3 | appends under a lock file | `tours_checks race <dir>`: starts two copies of itself; each takes `tours.lock` (the `try_lock` loop of `userdata/lock.rs`) and appends one line to the same file, 200 times, while a third reads the file in a loop. 400 whole lines after, no read failed. macOS here. **Windows:** TR3's race test runs in CI on Windows and is the answer there |
| 4 | marker lines from `Def.id` | `tours_checks markers <root> <driver>…`: on the sample and on the Bend fixture, for every def with an ID, the lines of the def that contain its ID text. Exactly one line each, and it is the marker; a nested def's marker is found in its parent's lines by the nested def's own ID. The Bend half is skipped, loudly, when its driver is not built |
| 5 | a node folder with no doc | on a copy of the sample: delete a node's doc files, keep `node.toml` and the marker. `vnoc tree`, `show`, `check`, `missing`, and (by reading `index/tree.rs` and `extension/src/attentionView.ts`) what the tree's counts and Needs Attention do with it. Decides whether `vnoc tour ids` can leave exactly that |
| 6 | git from Rust | `tours_checks git`: makes its own repositories in a temp folder. **[measure]** `rev-parse HEAD`; `status --porcelain -z -- <12 paths>`; `show <commit>:<path>` (median of 9). Then what each of these gives: a worktree; no commit yet; no `git` on the `PATH`; a commit that is not in the repository; `commit --only -m x -- a b` with another file staged (does it stay staged and out of the commit), with `b` untracked (is `add` needed first), with a pre-commit hook that fails |
| 7 | size and changed-time | `tours_checks stat <dir>`: write a file, `stat`, write the same number of bytes again at once, `stat`: does the time differ, and what is its resolution. macOS here. **Windows and Linux:** TR5's recorded-check tests run in CI on all three |

Commits: `tours TR0: seven checks (crates/vnoc/examples/tours_checks.rs)`, then `docs: 26: TR0's answers`.

## 3. Phase 1: the file and the commands

### TR1 The records and the replay (no disk)

| Files | What |
|---|---|
| `crates/vnoc/src/tours/mod.rs`, `lib.rs` | the module |
| `tours/record.rs` | one type per record of [`01`](01-on-disk.md) §4, with serde; `parse_line`, `print_line` (keys in a fixed order, no white space, `\n`); an unknown `type` parses to `Record::Other`; `TourId`, `StopId`, `StepId` with their forms |
| `tours/fold.rs` | `fold(text) -> Result<Tour, Unreadable>`: [`03`](03-core.md) §1, with `Problem`s; the blocks as a tree; `steps_in_order(stop)`; pinned or not |

Tests (tables): each record prints and parses back to itself; a line with an unknown key parses; an unknown type is skipped with no
problem; `\r\n` line ends; a last line with no `\n` is not read; a broken middle line is one problem and the rest loads; `schema: 2` is
unreadable; a plan then stops that fill it; a stop with no plan; replace keeps the place and clears the blocks; drop of a stop takes
its nested stops, of a block its details; nesting by `from`, a nested stop added late sits after its siblings; a block with a missing
parent is at the top; the reading order of a stop with two blocks, the first with two details; `meta` replaces the title; each state
of 01 §4's table, with `halt`, `halted`, `resume` and `end` in every order that can happen; a `pin`, then a `step` after it is not
pinned, then a second `pin` is; `ids` sets a stop's `node`; the sizes and the minutes.

Commit: `tours TR1: the records of a tour and the replay`.

### TR2 The text, its rules, the hash (no disk)

| Files | What |
|---|---|
| `tours/body.rs` | the text of [`02`](02-commands.md) §4 → `Body { intro, blocks: tree of { lines, title, text } }` from `##` and `###` |
| `tours/rules.rs` | `check(body, node lines, kind, detail, options) -> Vec<Refusal>`: every row of 02 §4's two tables, each `Refusal` with `where` and its sentence; the inline rule (code spans and emphasis pass; a list, table, fence, image, link, HTML are named) |
| `tours/hash.rs` | `of(lines, marker_ids) -> Hash`: [`01`](01-on-disk.md) §7, with TR0's answer to check 4 |

Tests (tables): an intro only; three blocks; a block with two details; `## 24 Title`; a `###` before any `##`; a `####`; a detail
outside its block; two details that overlap; a blank line inside a block; each refusal with its exact sentence, and several in one
call, in order; the three budgets at their limit and one over, counted in characters (a text with `é` and with an emoji); the hash is
the same for `\n` and `\r\n`, with and without trailing spaces, with and without the marker line, and differs for one changed
character and for a changed indent.

Commit: `tours TR2: the text of a stop, the rules, the hash`.

### TR3 The folders

| Files | What |
|---|---|
| `userdata/layout.rs` | `tours_dir`, `local_tours_dir`, `tour_state_dir`, `tours_lock_file` |
| `userdata/lock.rs` | the wait loop taken out as a function both locks use, with the time between tries as a parameter. `WriteLock` unchanged in behaviour (50 ms, 5 s). The tours lock tries every **5 ms**, 5 s at most: it is held for microseconds, and a reader's Next waits on it (TR0, check 3: a 50 ms sleep was every wait) |
| `tours/fold.rs` | `Tour` keeps the highest stop number and each stop's highest block number **ever used** (in any `plan`, `stop`, `step`, `drop`, and the `tour` record's `next`): what new ids count from (01 §5) |
| `tours/time.rs` | `now()`: UTC as `2026-11-23T18:40:02Z`, from `SystemTime`, no new crate |
| `tours/store.rs` | `new_id(title)`; `start` (`create_new`; D16's creation of `.vnoc/` by the function `init` uses); `append` (the lock; a file that does not end in `\n` gets one first; one `write_all`); `read`; `list` (both folders, the rules of [`01`](01-on-disk.md) §10); `next_stop_id`, `next_step_id`; the state file: `read_state`, `write_part` (read under the lock, replace one part, `write_atomic`) |
| `crates/vnoc/tests/cli.rs` | the "opens unchanged" fixture test of `04-storage/04-growth-path.md` §5 is designed, not built, so instead: the sample with one project tour, one private tour and its state file: `vnoc status`, `tree`, `check` print the same and exit the same as without them, and the three files are byte for byte as they were (TR0 check 1, as a test) |

Tests: ids for five titles (a long one, one that is all punctuation, two equal titles give two ids); `start` twice; `append` to a torn
file; the race of TR0 as a test (two threads, 100 appends each); `list` with a file of a wrong name, an empty file, an unreadable
first line, the same id in both folders; `start` in a project with no `.vnoc/` creates exactly `vnoc.toml` and `.gitignore`; writing
the place keeps the check, and the other way round; a state file that does not parse is replaced.

Commit: `tours TR3: tours live in .vnoc/tours and .vnoc/local/tours; start, append, list, the state file`.

### TR4 `vnoc tour start`, `add`, `finish`, `list`, `show`

| Files | What |
|---|---|
| `cli/args.rs`, `cli/mod.rs` | `Command::Tour(TourCommand)` |
| `cli/tour.rs` | the five handlers: parse, call, print ([`02`](02-commands.md) §3 – §6, §12); `--json`. `finish` does not pin yet |
| `tours/add.rs` | the records keep `hash` as the `Hash` type of TR2, not a string (`record.rs`); `plan_stop` ([`03`](03-core.md) §7): the node from `Lookup::resolve`, its lines from `source.rs`, the marker IDs, the rules, the hashes, the records |
| `tours/view.rs` | the outline `show` prints, without a check |
| `crates/vnoc/tests/cli.rs`, `tests/golden/tour-*.{txt,json}` | below |

Tests (the process, on a copy of the sample): the loop of 02 §1 end to end, with a nested block, then `show` and `show --text`
(golden); `add` on a function with an ID writes `node`, without one does not; `add` changes no file outside the tours folders (a
listing before and after); each refusal exits `2` and writes nothing; `--from` and `--line`; `--from` alone; `--same-as`; a short
function with a block; a folder; a file with blocks; a wrong tour id gets "did you mean"; `start` with no `.vnoc/`; `start` with no
`--title` is clap's error; `finish` twice; `list` with none; `--json` of each (golden).

**Stop point:** a tour can be written and printed.

Commit: `tours TR4: vnoc tour start, add, finish, list, show`.

### TR5 Out of date: found, checked, recorded

| Files | What |
|---|---|
| `tours/find.rs` | `find` ([`03`](03-core.md) §2): by ID, by name |
| `tours/check.rs` | compare; each block in place, else slid inside its parent's lines, else changed; `fresh(…, mode)`: the recorded check when it is valid (the tour's length, each file's size and time, TR0's answer to check 7), else a check, then `write_part` |
| `tours/on.rs` | the map of 03 §6 |
| `cli/tour.rs` | `tour check [<tour>…] [--again]`; `show --check`; `list --check`; `list` shows a valid recorded answer; `list --on <node>` |

Tests: a tour, then each edit and the answer: a line changed inside a detail (that detail changed, its block changed, the other block
fresh, the stop changed); a line added above a block (moved, fresh); the function moved down the file (fresh); the function renamed,
with an ID (fresh, the new name) and without (gone); the file deleted (gone), then put back (fresh again: the record was not valid);
two equal runs of lines in one function (a block that cannot be placed is changed, not guessed). The record: `check` twice reads the
source files once (a counter in the reader); touching a file of the tour reads again; touching another file does not; `--again` reads;
a new stop added makes the record invalid. `--on` a function, a class, a file, a folder. **[measure]** replay, check, and the recorded
path, for a generated tour of 40 stops and 100 blocks over 12 files, median of 9: into 03 §1 and §3.

Commit: `tours TR5: a tour knows which steps no longer match, and remembers; tours on a node`.

### TR6 Fixing a tour, Stop, and the user's commands

| Files | What |
|---|---|
| `tours/add.rs`, `cli/tour.rs` | `--replace <stop>`, `--to <stop> [--under <block>]` (`body::read` gets an option for a text with no intro: TR2 requires one); `tour drop`; `tour resume`; `tour edit` |
| `tours/store.rs`, `cli/tour.rs` | `share` both ways; `delete` (the tour and its state to `local/trash/`), `restore`; `halt(id)` (for the core, TR15; a hidden `vnoc tour halt` for tests and the trial); a write on a halted tour writes `halted` once and is refused, exit `1`, with 02 §5's text |

Tests: replace keeps the number and the id, and block ids go on from the highest used; `--to` adds blocks and checks overlap against
the stop's blocks; `--under` adds details; drop then add does not reuse an id; `edit` changes the title, not the id; halt → `add`
refused, `halted` written once, `show` says stopped; `resume` → `add` works; `finish` on a halted tour is refused the same way; share
moves the file byte for byte and the state file still applies; share of a tour being written is refused; delete then restore; the
same id in both folders is listed as [`01`](01-on-disk.md) §1 says.

Commit: `tours TR6: replace, drop, edit, share, delete; a stopped tour refuses the next write`.

### TR7 IDs for a tour

| Files | What |
|---|---|
| `identity/attach.rs` | `ensure_node` split out of `attach` (the ID, the marker, `node.toml`); `attach` = `ensure_node` + `create_doc`. No behaviour change: the existing attach tests pass untouched. (22's PG2 plans the same split; whichever lands second builds on the first) |
| `extension/src/attentionView.ts` | a Missing node with no docs: the tooltip *"It has no docs. Remove cleans it up."* instead of *"Its docs are kept."* (README §6 ask 7; only if the user keeps the pick) |
| `tours/ids.rs`, `cli/tour.rs` | `tour ids <tour> [<stop>…]`: under the project's write lock, `ensure_node` for each stop's node with `EditSink::Disk`; one `ids` record; prints each changed file. Refused with `[ids] default = "none"` |
| `tours/settings.rs`, `cli/tour.rs` | `[tours]` in `vnoc.toml` ([`01`](01-on-disk.md) §12): `read` with the defaults, an unknown value read as `"ask"` and reported once; `set` one key with `toml_edit`; `tour settings [--ids …] [--commit …]` |
| `tours/finish.rs`, `cli/tour.rs` | `finish` after `end`, step 1 of [`02`](02-commands.md) §5: `ids` by the setting, `--ids`, `--no-ids`; the `ask the user` line |

Tests: a tour of four stops, two with IDs already: two markers written, two files named, one `ids` record with four entries; the tour
is still fresh after it (the marker line is left out of the hash, the blocks moved by one); rename one of the functions → found, fresh;
`ids` twice writes nothing the second time; `[ids] default = "none"` → refused, nothing written; a folder stop gets a `.vnoc-id`; what
TR0's check 5 found holds (`vnoc check` prints what it printed before, with the same exit code: the sample has two problems on purpose). The setting: `finish` with `ids` absent or `"ask"` writes nothing and prints the
line (golden); `"always"` writes them; `"never"` prints nothing; `--ids` and `--no-ids` win over each; `ids = "sometimes"` is read as
`"ask"` with one problem line; `tour settings --ids always` changes that key and keeps a comment and an unknown table in the file.

Commit: `tours TR7: vnoc tour ids gives a tour's stops IDs; [tours] ids says when`.

### TR8 A tour as one document: export, import

| Files | What |
|---|---|
| `tours/doc.rs` | the **nested** document of [`02`](02-commands.md) §10; `nest` (the flat lists → the tree) and `flatten` (the tree → the flat lists; the flat shape of `export --flat` is taken too; a document that gives `from` or `parent` against its own nesting is a refusal); `plan_tour`: every stop through `plan_stop`'s checks, each refusal with its place in the document as written (`stops[0].stops[1].blocks[0].text`) |
| `tours/store.rs` | `replace` (the old file to the trash, the new one by `write_atomic`, `next` in its first line) |
| `cli/tour.rs` | `tour export [--flat]`, `tour import <file>\|- [--replace <tour>]` |

Tests: `nest` then `flatten` is the identity on a tour with stops three deep and details (table); `export` (golden, nested) and
`export --flat` (golden); `export` then `import` gives a tour that `show --text` prints the same but for the id; `export` then
`import --replace` with nothing edited: the same ids, the same hashes; `--replace` with one text changed, one stop removed, one added
without an id: the kept ids hold, the new one is above every id ever used; a document with three bad stops lists all three with their
places and writes nothing; a new tour with labels of the author's own as ids (`"a"`, `same_as: "a"`) gets real ids in order; a
`same_as` that names a later stop is refused; a nested stop that also says `from` another stop is refused; a flat document imports to
the same tour as its nested twin; the place set before a `--replace` is still on its step after it.

Commit: `tours TR8: a tour as one JSON document, nested for a person: export, import`.

### TR9 Pinned

| Files | What |
|---|---|
| `tours/pin.rs` | every call of git, with TR0's answers to check 6: `can_pin`, `make` (fresh, then `status` for the paths, then `rev-parse`, then the `pin` record with `where`), `commit_files`, `as_written` (the commit and path checked as [`01`](01-on-disk.md) §10 says before git sees them) |
| `tours/settings.rs`, `tours/finish.rs` | `[tours] commit`; `finish` step 2 of [`02`](02-commands.md) §5: the pin, and by the setting, `--commit`, `--no-commit`: the commit, or the `ask the user` line with the files, or the flag alone |
| `cli/tour.rs` | `tour pin [<tour>…] [--commit [-m]]`; `finish` and `import` run `finish::finish` and print its result; `tour settings --commit`; `check` says `can be pinned`, `can be pinned again`; `list` and `show` say pinned or not; `show --text --as-written` prints a pinned tour's code from its commit |

Tests (each in its own repository): all committed → pinned at HEAD; one file modified, one untracked → not pinned, both named;
`--commit` commits exactly those two, another staged file is still staged and not in the commit, then pinned; `--commit` with a stale
stop is refused; a failing hook; an ignored file; no git; not a repository. After a pin: change a function → `show --as-written`
prints the old lines, lit where the pin says; `add --replace` → not pinned; `pin` again → pinned at the new commit. A `pin` record
with a commit that is not there → `as_written` is `None`, `list` says so. A pin with `commit: "--help"` or a path with `..` is refused
on read. The setting: `finish` on a tour of uncommitted work with `commit` at `"ask"` commits nothing and prints the files (golden);
`"always"` commits exactly them and pins; `"never"` prints `not pinned` and no command; with `ids = "always"` and `commit = "always"`
the commit holds the marker lines too, and the tour is pinned and fresh.

Commit: `tours TR9: a tour is pinned to a commit that holds it; pin --commit; the code as written`.

### TR10 The reader's place

| Files | What |
|---|---|
| `tours/place.rs` | `Place`, `Move`, `go` ([`03`](03-core.md) §4): pure |
| `tours/store.rs`, `cli/tour.rs` | `read_place`, `write_place`; `tour where [<tour>]` (the last moved, with none named); `tour go <tour> <move>` |

Tests (table, `go`): next and prev through a stop with a block and two details, across stops, a short stop, a `same_as` stop, a
planned stop; next stop, prev stop; a jump pushes `back`, next does not; 20 at most; back; seen when the last step was the place;
cover and end; a place whose step is gone moves to the nearest that exists. The commands: `where` on a tour never opened is the cover;
`go next` three times then `where` (golden); `go s4`; `go` to an id that is not there, with "did you mean"; `where` with two tours
picks the one moved last; `where --json`.

Commit: `tours TR10: the reader's place is the core's: vnoc tour where, go`.

### TR11 A planned route

| Files | What |
|---|---|
| `cli/tour.rs`, `tours/add.rs` | `start --plan`, `tour plan`; `add` fills the first planned stop on its node ([`02`](02-commands.md) §4's table) |

Tests: a plan of four, filled out of order; a node planned twice, filled in route order; an unplanned `add` joins the route; `finish`
removes what is left and says how many; `show` marks `planned`; the 40-stop limit counts planned stops; `go` onto a planned stop.

Commit: `tours TR11: a route can be planned before it is written`.

### TR12 A route from the call tree

| Files | What |
|---|---|
| `tours/route.rs` | `propose` ([`02`](02-commands.md) §11): from `calls::merge`, in reading order, `from` and `line` from the call sites, `same_as` on a second visit, cut at 40; containment for a folder and a file; the sizes |
| `cli/tour.rs` | `tour route`; `start --route <node> [--depth n]` |

Tests: the sample's deepest call chain at depth 0, 1, 2 (golden); a function that calls another twice (one stop, the first line); a
call the index does not know; a recursive call; a folder at depth 1 and 2; `start --route` then `show` equals `route`'s list.

May move after phase 3 if the trial (TR14) shows agents do well without it; nothing on screen needs it.

**Stop point:** every command of 02 exists.

Commit: `tours TR12: vnoc tour route proposes a route from the call tree`.

## 4. Phase 2: the skill

### TR13 The guide, the skill, the install

| Files | What |
|---|---|
| `crates/vnoc/guide/tour.md`, `cli/guide.rs` | the sixth topic ([`04`](04-skill.md) §4); `vnoc skill tour` |
| `skills/vnoc-tour/SKILL.md` | [`04`](04-skill.md) §3 |
| `skills/vnoc/SKILL.md` | the one line under *More* |
| `agents.rs` | two skill folders per agent; `status` and `remove` know both; the allow rules of 04 §5 in `CLAUDE_ALLOW`, and Codex's |
| `tests/guide.rs`, `tests/agent.rs` | every example in the topic runs; install writes both, an install of before reports the second as not installed; the allow list has `where` and has no `pin`, `go`, `ids`, `delete`, `settings` |

By hand, open check 9: with the allow rules installed, Claude Code and Codex each run a heredoc `vnoc tour add` without a permission
question, and each **does** ask before `vnoc tour pin --commit`. The answer goes into 04 §5.

Commit: `tours TR13: the vnoc-tour skill, vnoc guide tour, installed beside the first`.

### TR14 The trial

The scenarios of [`04`](04-skill.md) §6, on the sample and on one real project. No product code unless a scenario fails; then
the fix is the wording of a refusal, of the skill, or of the guide, in this step. The results table goes into 04 §6.

**Stop point:** the file formats, the commands and the skill are tried. Anything the trial shows wrong with a **format** is fixed now,
before a page reads it.

Commit: `tours TR14: the trial's results; wording from it` (and `docs:` for the table).

## 5. Phase 3: the screen, playable

### TR15 The core answers the extension

| Files | What |
|---|---|
| `protocol.rs`, `server.rs` | `vnoc/tours`, `vnoc/tour`, `vnoc/tourGo`, `vnoc/tourOpen`, `vnoc/tourHalt`; `vnoc/toursChanged`, `vnoc/tourPlaceChanged`; in `files_changed`, the split of [`03`](03-core.md) §8 (TR0's answer to check 2 says how much there is to filter); the tours held in the state, replayed when their file changes; the answers in memory and when they are dropped (03 §3, rung 1) |
| `shared/tours.ts`, `shared/messages.ts` | the types of 03 §8; the page requests `tours`, `tour`, `tourGo` |
| `extension/src/tours.ts`, `relay.ts`, `extension.ts` | asks the core, holds the open tour's `TourView` with its place, asks again on `toursChanged`, follows `tourPlaceChanged`, passes both to the pages; open check 11's re-read |

Tests (`lifecycle.rs`, the real driver): `vnoc/tours` after `vnoc tour start` in another process; an `add` sends `toursChanged` and
**no** `indexChanged`, and the revision is the same; `vnoc/tour` twice reads the stops' files once; a change to a stop's file sends
`toursChanged` and the next `vnoc/tour` reads again; a change to another file does neither; `vnoc/tourGo` writes the state file;
`vnoc tour go` from another process sends `tourPlaceChanged`, and the core's own `tourGo` does not echo one; `tourHalt` then an `add`
from the command is refused.

Commit: `tours TR15: the core serves tours and the place; a tour changing is not the index changing`.

### TR16 The tour context

| Files | What |
|---|---|
| `shared/session.ts` | `kind: "tour"`; `ContextState.tour: TourId`; `tourList`; `tourOpen`; `SavedSession` ([`05`](05-on-screen.md) §5). No place, no next |
| `extension/src/session.ts`, `tours.ts`, `commands.ts` | the host sets the tour context's selection from the place it holds; `vnoc/tourOpen` on open and close; `vnoc.openTour` (the QuickPick of design 02 frame 10) |

Tests (unit, `shared/`): opening a tour adds and activates the context; a second tour replaces it; closing gives MAIN back unchanged;
save and restore keep which tour was open; the selection follows a place handed in. The window suite later (TR17) covers the rest.

Commit: `tours TR16: a tour is a context; its place comes from the core`.

### TR17 The stage

| Files | What |
|---|---|
| `webview/src/tour/Stage.tsx`, `stage/*` | the dock of `design/03-stage.md`: header, code pane (the `code` request, lit and dim lines, the thin mark for a detail's parent, the scroll rule), text pane (the parent's line above a detail's title), foot with the segments; the kinds of step: cover, intro, block, detail, short stop, contents, explained before, end; the fixed height, the drag |
| `webview/src/tour/text.ts` | the renderer of [`05`](05-on-screen.md) §7 |
| the canvas | draws the stage when the active context is a tour. The map is the plain canvas for now |
| `extension/src/test/suite/tour.ts` | the suite `tour`. Its setup makes its tours by running the built `vnoc tour` commands on the test workspace, so their hashes are real |

Tests: unit for the scroll rule and the renderer (a `<b>` stays text); the window suite: open a tour from the palette, Next to the end
card through a block and its details, Prev, the keys `→ ← Space Home End`, the code of a stop matches the file, Go to Code opens the
file at the line; **`vnoc tour go` run by the test moves the stage**, and `vnoc tour where` run by the test says the step the stage
shows after three clicks on Next.

**Stop point: a finished tour plays, and a terminal can see and move it.**

Commit: `tours TR17: the stage: one step, its code and its sentence`.

### TR18 The TOURS section: the list and the route

| Files | What |
|---|---|
| `webview/src/sidebar/tours/*`, `Sidebar.tsx` | the section of `design/07-tour-list.md` §2: folded, the list (the **All** view), a row (§3, with `shared ·`), the route panel of `design/04-route-and-map.md` §1 with `‹ Tours` and details indented under their block; opening at the cover or where you left it (§5); the keys of §8 |

Tests: unit for the row's one state word and its order; the window suite: the list shows the suite's tours newest first, a click opens
at the cover, leave on stop 3 and the row says `stop 3/…` and a click returns there, `‹ Tours` keeps the tour open.

Commit: `tours TR18: the TOURS section: the list, and the route of the open tour`.

### TR19 The map and the bar

| Files | What |
|---|---|
| `webview/src/canvas/*`, `node-card/*` | a tour context draws its stops as cards: the badge, the ring on the current one, heavy edges from the first stop, `⇢ 2`, a stop reached by a call as a call card, a nested stop joined to its parent; a click is a jump (`vnoc/tourGo` with a jump); the camera of `design/04` §4 |
| `webview/src/canvas/top-bar/*` | the tour's chip, the crumbs, `⌖`, the `⋯` entries (`design/04` §5); `moved by the agent` for three seconds ([`05`](05-on-screen.md) §3) |
| menus | the tour row's and card's list (`design/04` §7) |

Tests: unit for the camera rule (in view: no move; cut: the least pan; more than a screen: centre) and the crumbs; the window suite:
Main ⇄ tour keeps the step and both viewports; `Esc` closes the context and MAIN is as it was; full `pnpm -r test`.

**Stop point: route, map and stage, for a finished tour.** A first by-hand look against `design/mockup.html`.

Commit: `tours TR19: the tour on the canvas: cards, the ring, the bar`.

## 6. Phase 4: the screen, complete

### TR20 While it is written

| Files | What |
|---|---|
| the stage, the route, the map, the bar | *not written* and *being written* (`design/05` §3); the sizes with and without a plan ([`05`](05-on-screen.md) §3); the text fading in where the grey bars were; `■` and the two beats *stopping*, *stopped* (`design/02` frame 8); *no news* |
| `extension/src/tours.ts` | the status bar item and the one notification (`design/04` §6, W11); a second tour appearing while one is open (`design/05` §5) |

Tests: the window suite drives a real writer: it runs `vnoc tour start --plan …`, then `add` by `add`, and checks after each that the
stop is on screen without a reload; reading ahead of the agent; a tour with no plan; `■` then one more `add` (refused) → *stopped*;
the canvas hidden → nothing opens, the notification's **Open Tour** does.

Commit: `tours TR20: a tour is read while it is written; Stop`.

### TR21 Out of date, as written, and tours on a node

| Files | What |
|---|---|
| `protocol.rs`, `server.rs` | `vnoc/tourCode`; `vnoc/tourCounts` (or `Node.tours`, if [`03`](03-core.md) §6's check says so) |
| the stage, rows, cards, the cover | *changed* per step, *gone*, the cover's lines; **as written** for a pinned tour: the pane's label, the switch **Now** / **As Written** ([`05`](05-on-screen.md) §3); the design's marks for one that is not (`design/05` §4); the list asks a check for the rows it draws (`check: [ids]`); the section's **Check All Tours** |
| `webview/src/sidebar/tours/*` | the view **On the selected node** (`design/07` §4) through `vnoc/tours { on }` |
| `node-card/footerChips.ts`, the sidebar row, menus | `▶ N` (W17); **Show Tours** on a node |

Tests: the window suite: edit a function on an open, **pinned** tour and save → its step shows the old lines with the label, **Now**
shows the new ones, the row says `changed`, no other tour's files were read (the counter of TR15); the same on a tour that is not
pinned → the dashed lines; delete a function → gone, as written; restart the window → the list's `changed` is there with no source
file read (the recorded check); select a node → the second view lists its tours with the right line 2, a click opens at that stop;
`▶ N` appears and goes with a tour.

Commit: `tours TR21: a tour says what no longer matches, and shows it as written; the tours of the selected node`.

### TR22 The user's actions

| Files | What |
|---|---|
| `protocol.rs`, `server.rs` | `vnoc/tourPin`, `vnoc/tourIds`, `vnoc/tourEdit`, `vnoc/tourShare`, `vnoc/tourDelete`, `vnoc/tourRestore`, `vnoc/tourSettings`; `TourRow.offers` |
| the list, the cover, the end card | [`05`](05-on-screen.md) §3's table: pinned / not pinned and why; the cover's offers by the settings; **Pin**; **Commit and Pin…** with its dialog and its check box *Always do this in this project*; **Write IDs for Its Stops**, the same check box in its message; **Tour Settings…**; **Rename…**; **Share with the Project** / **Make Private Again**, **Share…**; **Export as JSON…**, **Open as JSON** and **Import into This Tour**; **Import a Tour from JSON…**; Delete with Undo, **Delete Finished Tours**; **Ask for a Tour…** copies its sentence (`design/07` §7) |

Tests: the window suite: a tour of uncommitted work → the cover says not pinned and names the files; **Commit and Pin…** lists exactly
them, Cancel commits nothing, the button commits them and the cover says pinned; the check box writes `commit = "always"` into `vnoc.toml` and nothing else in it changes; with `"never"` the cover offers nothing and the menu entry still works; Write IDs then a rename in the source → the stop is
still found; Open as JSON (it is the nested tree), change one text, Import into This Tour → the stage shows it, and the place is the same step; a bad edit lists its
refusals; share, then Make Private Again; delete, then Undo.

Commit: `tours TR22: pin, commit and pin, IDs, share, edit as JSON, delete with undo`.

### TR23 Small canvases, keys, access

| Files | What |
|---|---|
| the stage, the bar | the narrow layout and the mini bar (`design/03` §6, `design/04` §5); every key of `design/05` §6; *Reduce motion*; the labels a screen reader gets (`design/05` §8), with "detail of …" for a nested block |

Tests: unit for the labels; the window suite: every key; a 520 px canvas; full `pnpm -r test`.

Commit: `tours TR23: the narrow stage, the mini bar, the keys, the labels`.

### TR24 By hand

The checks of `design/05-states.md` §8, on a real project with a real agent, in Dark+, Light+ and High Contrast. Added to them:
say "explain this more" to the agent while on a step, and see it answer about that step; ask it to take you to a stop; take a tour of
uncommitted work to pinned. Then the last one: **watch one person take a tour, in silence, and write down where they hesitate.** What
was found goes into these files as built; fixes are their own commits. Then the pull request.

Commit: `docs: 26 as built`.

## 7. What each step leaves working

| After | A person can | An agent can |
|---|---|---|
| TR4 | read a tour in a terminal (`vnoc tour show --text`) | write one: start, add, finish |
| TR5 | see what went stale, without it being worked out twice (`vnoc tour check`) | find the tours on a node |
| TR6 | rename, share, delete a tour; stop one | fix a tour, be stopped, continue |
| TR7 | give a tour IDs so it follows renames | — |
| TR8 | export a tour as a nested file, edit it, import it; make a tour from a file | — |
| TR9 | pin a tour, commit its files to pin it, read a stale stop as it was | say why a tour is not pinned, and ask |
| TR10 | move a tour from a terminal | know which step the reader is on |
| TR11 – TR12 | — | plan a route, take one from the call tree |
| TR14 | trust that agents write tours worth reading | — |
| TR17 | play a finished tour on the canvas, opened from the palette | see and move what the canvas shows |
| TR19 | find tours in the sidebar; see the route on the map | — |
| TR20 | read while the agent writes; stop it | — |
| TR21 | see what no longer matches, as it was; find the tours of a node | — |
| TR22 | do all of TR6 – TR9 with a click | — |

Back: [`README.md`](README.md)
