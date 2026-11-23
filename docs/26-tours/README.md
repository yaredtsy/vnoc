# 26. Tours: an agent explains a piece of the code, step by step (static)

Written 2026-11-23 from the code at `21df5fc` (`main`, release 0.3.1), from the mock design made in `vnoc_playground/walkthrough/`
(2026-11-15, copied here as [`design/`](design/README.md)), and from the conversation of the same day. **Revised the same day after your
answers, twice** (§2). The design says what a tour **looks like**. This folder says what a tour **is**: where it is kept, how it is built,
how an agent writes it, how V-NOC reads it, where the reader's place is kept, how it knows a tour is out of date, what a pin to a commit
gives, and the order to build it in. **Nothing here was run.** Every `[check]` is a claim to verify before depending on it; every
`[measure]` is a number to write here once a test gives it. It is not tied to a release number.

This is the **static** tour: an agent writes it once, people read it, as often as they like. The **interactive** tour (ask a question
inside it, go deeper) is [`../27-tours-interactive.md`](../27-tours-interactive.md); it is built after this, on the same files.

```
tours
├── README (this file)      what ships · what you decided · decisions · what changed from the design · where it stands · asks · open checks
├── 01-on-disk.md           one file per tour, a log · the structure: stops in stops, blocks in blocks · ids · names and IDs · hashes
│                           the pin to a commit · this machine's state file (the place, the last check) · the settings
├── 02-commands.md          vnoc tour start · add · finish · where · go · check · pin · ids · edit · delete · share · export · import · plan · route
├── 03-core.md              log → tour · finding each stop · out of date: lazy, and recorded · the place · as written · tours on a node · the requests
├── 04-skill.md             a second skill, vnoc-tour · the text · vnoc guide tour · install · the trial
├── 05-on-screen.md         how to read design/ now: what stays, what is renamed, what is new
├── 06-build-order.md       steps TR0 – TR24 in five phases, each with files, tests and its commit message
└── design/                 the mock design, as it was made: layout, flow, stage, route and map, states, the list, mockup.html
```

## 1. What it ships

You ask your coding agent "walk me through `checkout_order`". The agent reads the code with `vnoc`, and writes a tour with `vnoc tour`:
a list of **stops** (a function, a class, a file, a folder), each with a sentence on what it is for and **blocks** that light some
lines and say why they are there, the general first and the detail inside it. You read it on the V-NOC canvas, one step at a time, while
the agent is still writing the rest. The agent can ask where you are, and can move the tour for you. The tour stays: you find it in a
list, or on any node it passes through. When the code under it changes, the tour says which steps no longer match; if it is pinned to a
commit, it shows those steps with the code as it was.

| In | Out (later, or [`27`](../27-tours-interactive.md)) |
|---|---|
| a tour written **stop by stop**: the agent starts it, reads and searches in between, adds, finishes. Each stop is on screen the moment it is added | asking a question in a tour, going deeper, side trips (27) |
| a tour as **one JSON document, nested, for a person to read and edit**: `export`, `import` (new, or replacing one) | starting a tour with a click (27) |
| the structure: stops nested in stops (a call, a file in a folder), blocks nested in blocks (general, then detail) | blocks more than two levels deep (the format allows it; the screen is designed for two) |
| playing it: route, map, stage, as [`design/`](design/README.md) draws them | the whole project opened at the pinned commit, in its own worktree |
| **the reader's place in the core**: `vnoc tour where` tells an agent which step you are on; `vnoc tour go` moves it | several readers with separate places on one machine |
| **private** tours (this machine) and **project** tours (committed); share, edit, delete from the screen and the command line | finding a function again after a rename without an ID (your later module for all of V-NOC) |
| the tours **on the selected node**; `▶ N` on a node | a tour as a web page |
| **out of date**: a hash per stop and per block, checked lazily, **recorded** so an unchanged tour is not checked twice | sound, auto-play |
| **pinned**: a commit is written only when every stop is in it; an option commits the tour's files to make that true; a pinned tour plays its changed stops **as written** | |
| IDs written for a tour's stops when you ask (`vnoc tour ids`), so the tour follows renames | |
| stopping the agent from the canvas; continuing a stopped tour | |
| a planned route (`◌` not written yet), and a route proposed from the call tree | |
| a second skill, `vnoc-tour`, and `vnoc guide tour` | |

## 2. What you decided (2026-11-23)

| # | Question | Your answer | Where it went |
|---|---|---|---|
| 1 | `vnoc tour` or `vnoc walk`? | **tour** | D1 |
| 2 | one file per tour? | **yes, one tour is one file.** Never all tours in one file | D2 |
| 3 | private first? | **yes** | D3 |
| 4 | the agent builds the route? | **yes** | D4 |
| 5 | where is the reader's place? | **in the Rust core, with a command**, so an agent in a chat can ask where the reader is and step forward or back | D17 (changed), [`02`](02-commands.md) §7, [`03`](03-core.md) §4 |
| 6 | the structure | **every stop highlights a node; a file, function or class stop has code blocks; a block can hold blocks (general, then detail); a stop can hold stops (calls, files of a folder)** | D18, [`01`](01-on-disk.md) §3 |
| 7 | finding a node | **by ID or by its exact name, nothing clever now; an easy way to write IDs.** A module for finding nodes comes later, for all of V-NOC | D6 (changed), [`02`](02-commands.md) §9 |
| 8 | out of date by hash? | **yes; and record the result**: no recomputing when nothing changed; `check` on the command line checks all and reports; the sidebar checks lazily, once | D7, D8 (changed), [`03`](03-core.md) §3 |
| 9 | the commit | **only a commit that really holds every stop; an option to commit the tour's files so it can be pinned; a flag when it is not; a pinned tour is always playable** | D19, [`01`](01-on-disk.md) §8 |
| 10 | the title | **given at start / create** | [`02`](02-commands.md) §3 |
| 11 | who may edit, delete, share | **the user gets everything**: edit, delete, share, and export or import a tour as a JSON file | D14 (changed), D20 |
| 12 | `start` in a project with no `.vnoc/` | **it initializes it** | D16 |
| 13 | the rest (a second skill, no copy of the code in the tour, commands and trial before the screen) | **agreed** | D13, §5 |
| 14 | writing IDs, and committing to pin: who decides, and when? | **a setting in `vnoc.toml`, easy to change:** always, ask, or never, for each. And the explicit commands from a terminal as well | D21, [`01`](01-on-disk.md) §12, [`02`](02-commands.md) §5 |
| 15 | check out the pinned commit, or read it? | **read it (`git show`), for now. What must be captured is the commit's id; how it is shown can change at any time** | D19, [`01`](01-on-disk.md) §8 |
| 16 | *(your question)* is the structure a JSON tree with child arrays, or a flat array with id chains? | **flat, with id chains, as storage** (the file, and what the pages get). The document a person exports and imports is the nested tree (row 18). Answered in D18 and [`01`](01-on-disk.md) §3 | D18 |
| 17 | *(your question)* if the tour's id is the session id, how does the agent edit while you navigate? | they are two files, and every write names what it writes to by id, never "the current step". Answered in [`03`](03-core.md) §4 | D17 |
| 18 | export and import | **the user gets the nested document** (`export`), edits it, and gives it back (`import`); V-NOC translates it to the flat lists, which are what it stores | D20, [`02`](02-commands.md) §10 |

## 3. Decisions

**ask** = still yours to call; §6 has each with my pick.

| # | Decision | Why |
|---|---|---|
| **D1** | **One word everywhere: tour.** On screen (W1), on disk (`tours/`), on the command line (`vnoc tour`). | A person who reads "Tour of checkout_order" and says "continue the tour" is naming the command. |
| **D2** | **One tour is one file, and the file is a log:** one JSON record per line, and the step-by-step commands **only add lines**. Reading a tour is replaying its lines. | It is the thread you described, and what Claude Code and Codex do for a conversation. A viewer follows a file that grows; two writers (the agent, and your Stop button) cannot overwrite each other; a question or a side trip later is one more kind of line. [`01`](01-on-disk.md) §2 |
| **D3** | **Two folders. Every tour starts private** in `.vnoc/local/tours/` (not committed). **Sharing moves the file** to `.vnoc/tours/` (committed). | An agent that explains ten things leaves ten tours; they must not land in a pull request by themselves. |
| **D4** | **The agent builds the route, one stop per command.** It may announce the route first (a *plan*). `vnoc tour route` proposes one from the call tree. | "How does a request reach the database" is not a call tree to a depth. The plan keeps what a fixed route gave the screen: the outline before the first word, "stop 3 of 10". |
| **D5** | **Ids are made by the core and never reused.** A tour: `checkout-order-7k2q`. A stop: `s1`. A block: `s3.b1`. The number on screen is the position, not the id. | The agent types them, so they are short. The reader's place, and later a question, point at an id; it must never come to mean another step. |
| **D6** *(changed)* | **A stop names its node by its name (`path::qualname`) and by its ID when it has one. Finding it again is two direct checks: the ID, then the exact name.** A tour writes no ID by itself; **`vnoc tour ids <tour>` writes one for every stop** when you ask. | Your call: nothing clever now; a module that finds nodes is a later piece of work for all of V-NOC, and it will replace one function here (`tours::find`). With no ID, a renamed function is *gone* from the tour, and the tour says so. With IDs, the tour follows it as docs do. |
| **D7** | **Out of date is decided by a hash, per stop and per block,** taken when each is written. | A hash works with uncommitted edits, on another branch, with no git, and says *which* steps no longer match. |
| **D8** *(changed)* | **The check is lazy, and its answer is recorded on disk,** in this machine's state file for that tour. A recorded answer is used as long as the tour's file and the files of its stops have the size and changed-time they had. `vnoc tour check` checks every tour and reports; the sidebar checks a tour when its row is first drawn. | Your call. Knowing "nothing changed" costs one `stat` per file, not a read and a hash. It is recorded **beside** the tour and not in it, because "is it stale" is true of one working tree, not of the tour: on your branch a shared tour is fresh, on mine it is not. [`03`](03-core.md) §3 |
| **D9** | **A text that breaks a rule is refused when it is written,** with every reason at once. **There is no fallback text.** | The agent is in a loop with a shell: it reads the refusal and sends the stop again. One state less on every surface. |
| **D10** | **The reader's Stop is a line in the same file.** The agent's next write reads it and is refused. Writes take one lock file, `.vnoc/local/tours.lock`, for a few milliseconds. | The design's two beats (*stopping*, *stopped*) are two records. It is also the mechanism a question needs in 27. |
| **D11** | **The core replays the log and answers with the tour, resolved against the code as it is now.** The extension and the pages draw it. | P1: the command line, the extension and a later editor must agree on what stop 3 is and whether it is stale. |
| **D12** | **A tour changing is not the index changing.** A write under the tours folders sends `vnoc/toursChanged`; it starts no index update and moves no revision. | Today every path under `.vnoc/**` can move the revision, which marks call trees stale and makes every page ask for the structure again. An agent adds a stop every few seconds. |
| **D13** | **A second skill, `vnoc-tour`,** with its own description, and one guide topic, `vnoc guide tour`. | A tour is a different job with different trigger words. The trial (TR14) measures it against a section in the first skill. |
| **D14** *(changed)* | **Every command is the user's.** The skill tells an agent which ones it runs by itself (write, read, `where`) and which only when asked (`go`, `ids`, `pin --commit`, `share`, `delete`, `import`). Only the first group is on the agent's allow list. | Your call: the user gets the full option. The guard is no longer "the command does not exist" but "the agent is not allowed to run it unasked", and a delete goes to the trash. |
| **D15** | **No tour is ever deleted by V-NOC itself,** and a deleted tour goes to `.vnoc/local/trash/`. | A private tour is the only copy until it is shared. |
| **D16** | **`vnoc tour start` in a project with no `.vnoc/` creates it** (what `init` creates) and says so. | "Walk me through this" is the first thing someone asks in a project they do not know. |
| **D17** *(changed)* | **The reader's place is kept by the core:** the current step, the seen stops, the way back. One place per tour on this machine, in the tour's state file. `vnoc tour where` reads it; `vnoc tour go` moves it, and the canvas follows. **What "next" is is decided in Rust,** once, for the canvas and the command. | Your call. An agent in a chat can find out what "this" is when you say "explain this more", and can take you somewhere. The design kept the place in the extension's session; there no command could reach it. The price: two windows on one project are on the same step. [`03`](03-core.md) §4 |
| **D18** | **The structure is a tree with two kinds of thing.** A **stop** highlights one node of any kind and may hold stops (a call made from it, a file of its folder, a method of its class). A stop on a file, class or function may hold **blocks** (some of its lines), and a block may hold blocks whose lines are inside its own. Reading order is the tree's: a stop's intro, its blocks with each one's details after it, then the next stop. | Your call: first the general, then the detail, at every level. The canvas has one component for each kind (the node's card, the lit lines), so every tour is drawn by the same two. **V-NOC stores it flat, with id chains, never as nested arrays:** a list of stops in route order, each naming the stop it is nested in (`from`); each stop's blocks in reading order, each naming the block it is a detail of (`parent`). The same shape in the tour's file and in what the pages get. Only the document a person exports is nested (D20). [`01`](01-on-disk.md) §3 |
| **D19** | **A commit is written into a tour only when it is true: every stop's lines are in that commit.** Then the tour is **pinned**. `finish` pins when it can. When files of the tour are not committed, it says which, and `vnoc tour pin --commit` commits exactly those files and pins. A tour that is not pinned shows a flag. **A pinned tour is always playable:** a stop whose code changed or went is shown with the code of the pinned commit, read from git. **Nothing is checked out: the commit's id is the thing kept, and its files are read with `git show`.** A different way to show them (a worktree, a checkout you ask for) can be added later without changing a tour. | Your call, thought through in [`01`](01-on-disk.md) §8. A commit that is only "where HEAD was" promises nothing if the tree had edits. Reading the files of a commit gives "always playable" without touching your working tree, your running program, or the index. |
| **D20** | **A tour can be taken out and put back as one JSON document:** `export` and `import`. **The document is nested** (a stop's `stops`, a block's `blocks`), because that is how a person sees the shape; `import` translates it into the flat lists of D18 with the same checks as `add`, and `export` translates back. `import --replace` is the one command that rewrites a tour's file. | Your call: the user can edit a tour as a file. Agents still write stop by stop, because a tour that arrives whole cannot be read while it is written. |
| **D21** | **Two settings in `vnoc.toml` decide what `finish` does by itself:** `[tours] ids` (write IDs for the tour's stops) and `[tours] commit` (commit the tour's files so it can be pinned), each `"always"`, `"ask"` or `"never"`. **Both start as `"ask"`.** A flag on `finish` overrides the setting for one call, and `vnoc tour ids` and `vnoc tour pin --commit` do the act at any time. | Your call: a team that wants every tour pinned and following renames sets it once; one that wants neither is never asked. `"ask"` means the command prints what could be done and the agent asks you; it never means a prompt in a terminal. [`01`](01-on-disk.md) §12 |

## 4. What changed from the design, and from the phase documents it was read against

The design is a UI design, and almost all of it stands. [`05-on-screen.md`](05-on-screen.md) is the full table; these matter most:

| The design says | Now | Why |
|---|---|---|
| `.vnoc/cache/walkthroughs/w_5d20.json`, one JSON object, rewritten on every change | `.vnoc/local/tours/<id>.jsonl` and `.vnoc/tours/<id>.jsonl`, a log | D2, D3 |
| `vnoc walk start / next / write / finish / estimate` | `vnoc tour …` ([`02`](02-commands.md)) | D1, D4 |
| every stop is known at `start` | known when the agent planned them; otherwise the route grows | D4 |
| a stop has an intro and a flat list of blocks | blocks nest: a block and its details | D18 |
| `code_hash` per stop, from when the tour **started** | a hash per stop **and per block**, from when **each was written** | D7 |
| a stop's `state: pending / ready / degraded`; the *fallback* mark | planned / written. No fallback | D9 |
| "where you are is yours, not the file's": the place is in the extension's session, per window | the place is in the core, per tour, on disk; the session holds only which tour is open | D17 |
| *changed*: the code as it is now, the old text, a line under it | the same when the tour is not pinned. Pinned: the code **as written**, with a switch to "now" | D19 |
| "written for the code as it is at `f4d6e15`" on every cover | only on a pinned tour. Otherwise *not pinned*, and why | D19 |
| tours live in a cache; sharing is out | private and project tours | D3 |
| the cache keeps the 50 newest | nothing is removed by V-NOC | D15 |

## 5. Where it stands

Read from the code on 2026-11-23. Nothing about tours exists in `crates/`, `extension/`, `webview/` or `shared/`.

- **The command line is ready for it.** Clap subcommands in `cli/args.rs`, one thin handler per command (C2), `--json` with `"schema": 1`,
  exit codes in `cli/mod.rs`. A one-shot is about 30 ms on httpx (21.05 §3), so a command per stop, and a `where` in the middle of a
  chat, cost nothing a person would notice. `Lookup::resolve` (`cli/nodes.rs`) turns any `<node>` into a `Node`.
- **No node has a content hash today.** D7 adds one, in the tours module only; BLAKE3 is already a dependency.
- **A node is found by ID or by name already:** `Index::resolve_ref` with `NodeRef::Id` and `NodeRef::Location`. D6 is those two calls.
- **Writing an ID without a doc is not a flow today.** `identity::attach` ends in `create_doc`. `vnoc tour ids` needs "make sure this
  node has an ID" split out of it: the same split 22's PG2 planned (`ensure_node`). **[check]** TR0: what a node folder with a
  `node.toml` and no doc looks like in the tree, in the counts and to `vnoc check`.
- **`Def.id` carries the ID text found in the code.** That is how the hash leaves marker lines out, so that writing an ID into a
  function on a tour does not make the tour stale ([`01`](01-on-disk.md) §7).
- **`.vnoc/local/` holds `write.lock` and `trash/` today.** `userdata/layout.rs` is the only file that knows the layout;
  `userdata/atomic.rs` has `write_atomic` and `create_new`; `userdata/lock.rs` has the wait loop to share.
- **The extension already watches `.vnoc/**`** (`extension/src/core.ts`) and sends `vnoc/filesChanged`. A tour's file and its state
  file changing arrive there with no new watcher: that is how `vnoc tour go` in a terminal moves the canvas. The known hole stays: with
  `.vnoc/` above the opened folder, VS Code's watcher does not see it ([`03`](03-core.md) §7).
- **The code a card shows comes from the extension** (`code` in `shared/messages.ts`, from VS Code's document). The stage uses it for
  "now". "As written" is new and comes from the core, which asks git.
- **Git:** the core runs `git` once today (`cli/check.rs` `changed_files`: `git status --porcelain`). The pin adds `rev-parse`,
  `status` for some paths, `show <commit>:<path>`, and, only for `--commit`, `add` and `commit`.
- **The session** has two kinds of context (`"main" | "explore"`, `shared/session.ts`). The tour is the third, and it holds only the
  tour's id.
- **The skill** is one folder, compiled in (`cli/guide.rs`), installed by `agents.rs`. Claude Code's allow list has read commands only.
- **The fixture of the design (Minicart) is not in this repository.** The command tests run on `crates/vnoc/tests/sample/`.

## 6. Asks

Yours of the first round are answered (§2). These came out of the answers. Ask 1 is answered too (§2, row 15) and is kept here for its
number.

| # | Question | My pick |
|---|---|---|
| 1 | ~~D19: should V-NOC ever switch your working tree to the pinned commit?~~ | **Answered: no checkout now.** The commit's files are read with `git show`; the id is what is kept, the view can change later. |
| 2 | D19: `pin --commit` commits **the whole of each file** the tour touches, not only the lines the tour lights, and nothing else. On screen it asks first, with the list of files and the message. From the command line it does it and prints what it committed. OK? | **Yes.** A commit of some lines of a file would pin a state of the code that never ran. |
| 3 | D17: one place per tour on this machine, so two windows on the same project follow each other. OK? | **Yes.** It is what makes `vnoc tour where` have one answer. Separate readers would need a session id on every command; the state file has room for it (`places`), and nothing needs it yet. |
| 4 | D14: on the agent's allow list (no permission question): `start`, `add`, `finish`, `plan`, `drop`, `resume`, `list`, `show`, `check`, `where`, `route`, `export`. **Not** on it: `go`, `ids`, `pin`, `share`, `delete`, `edit`, `import`, `settings`. OK? | **Yes.** The first group writes only inside the tours folders and can lose nothing. `ids` and `pin --commit` touch your source or your history; `delete`, `share`, `import` change what you keep; `go` moves your screen. |
| 5 | D20: `import --replace` replaces the tour's file (a new file, renamed over the old one). Ids in the document are kept, so your place survives. The old file goes to the trash. OK? | **Yes.** Turning a document back into "the lines to add" is possible and buys nothing: a person who edits a tour as a file wants the file to be what they wrote. |
| 6 | D21: both settings start as `"ask"`. With `ids = "always"` the IDs are written **at `finish`**, in one go, not at every `add`. OK? | **Yes.** `"ask"` costs one question per tour and never surprises anyone. Writing a marker at every `add` would move the lines of a file the agent is still reading; at `finish` every stop is written, and the hashes leave marker lines out. |

## 7. Open checks

| # | Check | Step | Decides |
|---|---|---|---|
| 1 | The released 0.3.1 opens a project that has `.vnoc/tours/x.jsonl`, `.vnoc/local/tours/y.jsonl` and `.vnoc/local/tour-state/y.json`: nothing listed, nothing changed, `git status` clean, `vnoc check` passes | TR0 | D2, D3 are additions to the format |
| 2 | What the running core does today when a file under `.vnoc/local/` changes: is `.vnoc/` read again, does the revision move? | TR0 | how much D12 has to filter |
| 3 | Two processes appending to one file under the lock file, 200 lines each: every line whole, none lost. macOS here, Windows in CI | TR0 | D10. The lock is **not** on the tour file itself: on Windows a lock on a file stops other processes from reading it |
| 4 | Which lines of a def hold an ID marker, from `Def.id` alone: "a line that contains the ID text of this def or of a def inside it". Python and Bend | TR0 | D7: the hash rule of [`01`](01-on-disk.md) §7 |
| 5 | A node folder with `node.toml` and no doc: the tree, the doc counts, `vnoc check`, Needs Attention | TR0 | D6: whether `vnoc tour ids` can use `ensure_node` as it is |
| 6 | Git, from Rust: **[measure]** `rev-parse HEAD`, `status --porcelain -z -- <12 paths>`, `show <commit>:<path>`; in a worktree; with no commit; with no `git`; a commit that is not in the repository; `commit --only -- <paths>` with other files staged, with an untracked file among the paths, with a failing pre-commit hook | TR0 | D19 |
| 7 | A file's size and changed-time as "it did not change": the time's resolution on macOS, Linux, Windows; a file changed twice within one tick | TR0 | D8: whether the size and time are enough, or a hash of the file is recorded too |
| 8 | **[measure]** replaying and checking a tour of 40 stops and 100 blocks over 12 files; the same with a recorded answer (12 `stat`s) | TR5 | D8 |
| 9 | An agent's permission prompts: with the allow rules of ask 4, does Claude Code run a heredoc `vnoc tour add … <<'EOF'` without asking? Codex? | TR13 | whether the text goes through standard input or a temp file |
| 10 | Does a second skill trigger on "walk me through …" more often than a section in the `vnoc` skill? | TR14 | D13 |
| 11 | `.vnoc/` above the opened folder: a tour written, or a place moved, by the command is not seen by the watcher | TR15 | a 2-second re-read of the open tour |

## 8. What later work stands on

| Later | Uses from here | So this folder must leave |
|---|---|---|
| the interactive tour (27) | the log, the ids, the lock, `where`, `vnoc/toursChanged`, the skill | record types a reader skips when it does not know them; ids that never move; the Stop button's path (a record written by the core for the UI) as the model for Ask; the place readable by a command, so "this" in a chat has a meaning |
| your module for finding nodes | `tours::find` | one function, with the stop's name, ID and hash as its input; nothing else in the module asks where a node is |
| the project at a pinned commit, in a worktree | the `pin` record | the commit and each stop's path and lines in it |
| a tour as a page, an export | `tours::fold`, `tours::view`, the JSON document | the replay and the resolving in the library, nothing in TypeScript |
| `vnoc mcp` | every library function behind a command | C2 |
| search (`has:tour`), a count in `vnoc tree` | the tours-on-a-node map ([`03`](03-core.md) §6) | the map in the core, keyed as the tree is |
