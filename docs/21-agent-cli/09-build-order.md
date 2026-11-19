# 21.09 Build order

```
build order
├── 1. rules for every step
├── 2. phase 1: read             A0 – A6     stop point: a read-only command, and `vnoc check` for CI
├── 3. phase 2: write            A7 – A9     stop point: agents can attach notes
├── 4. phase 3: skill, install   A10 – A14   stop point: release 0.5
├── 5. phase 4: after            A15 – A17   optional · conditional · if there is time
├── 6. what each step leaves working
└── 7. tests, by kind
```

On a branch `agent-cli` from `main`, after search (20) is merged: all of it is built (S1 – S8), so `find` has names, filters, references and
docs from its first commit. Each step is one commit and leaves the branch green. The steps below were read against the built search on
2026-11-17; what that changed is marked *(from the built code)* and listed in the README §6.

## 1. Rules for every step

- **Stage files by path.** Never `git add .`, `git add docs` or `git add .github`. Don't touch `docs/futureplan/`.
- **Checks per commit, light:** `cargo test -p vnoc` and `cargo clippy` when `crates/` changed; typecheck and the touched packages' unit tests
  when the extension changed; the window suite on A1, A7 and A12 (the server's flows are moved in the first two); full `pnpm -r test` and
  `cargo test` on the last step of each phase.
- **A handler is three things: parse, call, print** (C2). Logic found in `cli/` moves to the library before the commit.
- **Nothing prints but `cli/out.rs`** (05 §4).
- **`--json` shapes are golden files** (`crates/vnoc/tests/golden/<command>.json`). A step that changes one says so in its commit message.
- A step that adds a crate runs `node scripts/notices.mjs` and commits `extension/THIRD_PARTY_NOTICES.md` (05 §6).
- A step that settles a `[check]` or a `[measure]` writes the answer into the file that asked.
- C16 (`author:`, `tags:`) is yes: A8 builds it. C17 (docs on the root) is its own step, A17, after everything else: A8 does not wait for it.

## 2. Phase 1: read

### A0 Measure and check (no product change)

| What | How | Write the answer into |
|---|---|---|
| the one-shot build | `crates/vnoc/examples/oneshot.rs`: `Project::open`, `DriverHost`, `refresh(All)`, print the times of the walk, the outlines, `.vnoc/`, **the doc texts** (`refresh` logs how many it looked at and read), in all. Release build, on httpx and on 1,000 generated files **with 1,000 generated docs** *(from the built code)* | 05 §3: inside the budget, or A16 is scheduled after A6; and whether the doc texts get a flag |
| one search in a one-shot | the same example: `search::prepare` with two empty slots, then `run`, for one word | 05 §3 (expected: the corpus about 6 ms on 10,000 names and the doc index about 16 ms on 1,000 docs, as 20 measured) |
| one call tree | the same example, `call_tree` depth 2 on one httpx function | 05 §3 |
| `File::lock` on this toolchain, and what a second locker sees | a ten-line test | 04 §7 |
| is `regex` already built? | `cargo tree -i regex-automata` | 05 §6 |

Commit: `cli A0: the one-shot build, measured`.

### A1 The server's bodies become library functions

| Files | What |
|---|---|
| `crates/vnoc/src/index/mod.rs` (or `structure.rs`) | `index::structure(&Index, &Project, capabilities) -> StructureResult`: the body of `server.rs` `structure`, moved |
| `crates/vnoc/src/calls.rs` | `calls::tree_for(project, index, host, key, depth) -> Result<CallTreeResult, CallsError>`: `driver_tree` and the key mapping of `call_tree`, moved; `CallsError` says invalid, missing capability, or internal, and the server maps it to its response codes as now |
| `crates/vnoc/src/server.rs` | the handlers call them |

No behaviour change. Proof: `cargo test -p vnoc` and the window suite pass with no edits; the diff of `server.rs` only removes bodies and adds
calls. Commit: `cli A1: structure and the call tree are library functions`.

### A2 The command line, the root, `status`

| Files | What |
|---|---|
| `crates/vnoc/Cargo.toml` | `clap` (derive) |
| `crates/vnoc/src/main.rs` | parse; none or `serve` → the main loop, as it is; else `cli::run` |
| `crates/vnoc/src/cli/{mod.rs, args.rs, out.rs}` | the types; `--root`, `--driver` (C8), `--json`, `--format`, `--quiet`; text and JSON writers; the closed-pipe rule |
| `crates/vnoc/src/cli/project.rs` | the root (05 §2); `open()`: project, driver, `refresh(All)`; `open_without_driver()` |
| `crates/vnoc/src/cli/status.rs` | `status` (03 §2), over `index::structure` |
| `crates/vnoc/src/lib.rs` | `pub mod cli`; the `print_stdout` / `print_stderr` deny (05 §4) |
| `crates/vnoc/tests/sample/` | **the sample project**: about ten Python files in `billing/`, `orders/`, `api/` (classes, methods, a nested def, two defs of one name), a `.vnoc/` with five docs: on a function, a method, a class, a file, a folder; one record whose code is gone (Missing); one function whose `ID:` line was removed (a lost marker); one doc holding an instruction-like sentence (06 §8 scenario 5). **The `.vnoc/` is made by the library, not typed:** an `#[ignore]` helper in `tests/cli.rs` builds it with `first_free_id`, `UserData::create_node` and `create_doc`, is run once, and its output is committed. *(As built: the plain sources are `crates/vnoc/tests/sample_src/`, so the helper can be run again; its output is `tests/sample/`. Re-run: `cargo test -p vnoc --test cli -- --ignored make_sample`.)* |
| `crates/vnoc/tests/cli.rs` | the harness: the binary by `env!("CARGO_BIN_EXE_vnoc")`, a temp copy of the sample, the driver as `lifecycle.rs` finds it |

Tests: `status` on the sample, text and `--json` (the first golden file); from a subfolder the root is the same; `--root`; a folder with no
`.vnoc/` answers and exits `0`; no driver → `3` with the three places it looked; an unknown flag → `2`; **`vnoc` with no arguments, with `--stdio`, and with `serve` still answers
`initialize`** (`lifecycle.rs`, unchanged); `vnoc status | head -1` exits quietly; the exit-code table (`cli::exit_code`).

Commit: `cli A2: vnoc is a command too; status`.

### A3 `show`, `code`

| Files | What |
|---|---|
| `crates/vnoc/src/node_ref.rs` *(from the built code)* | `NodeRef::Location { path, rest }` in place of the whole text (the split moves from `resolve_ref` into `parse`; the box's tests pass unchanged); `NodeRef::{path, with_path}`; `Why` and `Index::why_not` (02 §2, §4) |
| `crates/vnoc/src/index/tree.rs` *(from the built code)* | `Node::find(&self, id) -> Option<&Node>`: a tree id to its node |
| `crates/vnoc/src/cli/nodes.rs` | arguments → references: relative to the current folder, then to the root (02 §2), through `with_path`; `-` for standard input; `node_ref::parse` and `Index::resolve_ref` from doc 20 |
| `crates/vnoc-search/src/` *(from the built code)* | `nearest(corpus, name, limit)`: names within 2 edits (02 §4). The crate gets no dependency for it |
| `crates/vnoc/src/suggest.rs` (library) | "did you mean": `run` on the last part of the name, the file's own nodes first, then `nearest` (02 §4). The `Query` is built as a value, never formatted |
| `crates/vnoc/src/cli/show.rs` | `show` (header, docs, code with the 120-line cut, `--full`, `--no-code`; a file or folder shows one level of tree), `code` (`--context`) |
| library | reading a def's lines from the file by its range, as one function (the extension does this in VS Code today) |

Tests: `show` by each of the four forms gives the same node; a line inside a method gives the method; three nodes in one call, one of them
wrong → two answers, one error in place, exit `2`; every row of 02 §4's table, each from its `Why`; `code --context 2`; a 200-line function is
cut with the count; a Missing node by ID: `show` answers, `code` is `2`; from a subfolder, a path relative to it and one relative to the root
both work, **for each form that has a path** (`a.py`, `a.py:12`, `a.py::f`); a file name alone with a line finds the one file of that name,
and with two such files is `2` with both paths (`Why::Ambiguous`). `nearest` (a table: one letter dropped, added, changed, two
swapped; a name of 3 letters is not a suggestion for another of 3; the file's own first). `suggest`: a name without its class finds the
method; `parse_ordr` finds `parse_order`; a path with a space in it. `node_ref`'s parse table, with the location's two parts. `lifecycle.rs`
passes with no edits (the box resolves references through the same functions).

Commit: `cli A3: show and code, by any name for a node`.

### A4 `doc`, `tree`

| Files | What |
|---|---|
| `crates/vnoc/src/cli/doc.rs` | `doc list` (with no node: every doc of the project, grouped by node), `doc read`, `doc path`; by ID without the driver |
| `crates/vnoc/src/cli/tree.rs` | printing only: indent, counts, `--flat` |
| `crates/vnoc/src/index/view.rs` (new, library) | `tree_view(&Node, &TreeOptions) -> Vec<Row>`: depth, kinds with ancestors kept, the `--where` set, the limit and what was cut, the counts of a cut folder. Pure, over the tree `Index::tree` gives |

`--where` is `vnoc_search::parse` and **`matches`** over every id of the tree *(from the built code: `matches` is what S8 built for "which of
these does the query find", by name or by a doc of the node, and it ranks nothing)*; the rows kept are the ids it says yes to, and their
ancestors. The corpus comes from `search::prepare`, as for `find`. `--sig` reads `Def.params`: the tree's `Node` does **not** carry them
(checked 2026-11-17: `index/tree.rs` has `range`, `qualname`, `node_id`, `docs`, `link`), so `tree_view` reads them with `Index::def_at`
for the rows it prints; the `Node` on the wire does not grow.

`doc read` prints the file without its frontmatter: `userdata/read.rs` `after_frontmatter` (private today, CRLF handled since S7) becomes
`pub(crate)` and is the one function that cuts it.

Tests (`tree_view`, a table): each option alone and two together (`--kind class --depth all`, `--where 'no:doc kind:fn' --flat`); the limit says
how many it cut; a cut folder shows counts. Integration: `doc` list, read, path on the sample; by ID with `--driver /nonexistent` still works;
`tree billing --sig --docs` golden text.

Commit: `cli A4: doc and tree`.

### A5 `find`, `grep`

| Files | What |
|---|---|
| `crates/vnoc/src/cli/find.rs` | `find`: `search::prepare` (two empty slots), `run`, each node row joined to the tree by its id for both lines and the ID, printed (03 §5) *(from the built code)* |
| `crates/vnoc/src/search.rs` *(from the built code)* | `Prepared` gains `reference: Option<Found>`: what the reference resolved to, so `find <ID>` can say a node is Missing. The server ignores it |
| `crates/vnoc/src/grep.rs` (new, library) | `grep(project, &Index, &Regex, paths, options) -> Vec<NodeHits>`: the index's files, line by line, each hit to its innermost def (`Index::innermost_def`, built in S5) or the file; `--docs` over `Index::doc_texts()` |
| `crates/vnoc/src/cli/grep.rs` | printing |
| `crates/vnoc/Cargo.toml` | `regex` |

Tests: `find` gives the order `vnoc/search` gives for the same query (one test asks both); `--nodes`, `--docs`; nothing found is `0`; a wrong
filter prints its note; `find <file:line>` puts that node first; `find <the Missing node's ID>` prints its `MISSING` line and exits `0`;
a node row has both lines and, when it has one, its ID. `grep`: a hit in a nested def goes to the inner one; a hit outside any def goes to the file; `--names`; `-i`; a pattern
that does not compile is `2` with the regex's message; `--docs`.

Commit: `cli A5: find and grep`.

### A6 `calls`, `missing`, `check`

| Files | What |
|---|---|
| `crates/vnoc/src/cli/calls.rs` | over `calls::tree_for` (A1) |
| `crates/vnoc/src/index/mod.rs` | `Index::lost_markers()` (03 §9) |
| `crates/vnoc/src/check.rs` (new, library) | `check(&Index, project, only: Option<&[RepoPath]>) -> Vec<Finding>`: the five kinds, each with its fix line, which of them fail |
| `crates/vnoc/src/cli/check.rs` | `missing`, `check`, `--changed` (the changed files from `git status --porcelain`; no git → the full check) |

Tests: `calls` on a sample function, depth 1 and 2; on the sample `missing` lists the one, `check` exits `1` and names the Missing node **and the
lost marker**, each with its fix; a clean copy exits `0`; a file with a syntax error is a warning and still `0`; `--changed` with only an
unrelated file changed is `0`. NEEDS ATTENTION lists lost markers too (decided 2026-11-19, 03 §9): **A6b**, its own commit, `structure` carries them
(`lost_markers`, a protocol addition), the extension shows a fourth group with *Write ID* on each row; a window test.

Commit: `cli A6: calls, missing, check`; then `cli A6b: a lost ID: line shows in NEEDS ATTENTION, with Write ID`. As built: [`03`](03-read-commands.md) §7 – §9. **Stop point:** a read-only command; `vnoc check` works in CI. Full test run.

### A6d Style for people (added 2026-11-19, after the first by-hand look)

03 §11 "Style for people" and 03 §2's new layout. `cli/style.rs`: the palette as named styles (`style::location`, `style::dim`, …) over
`anstyle`; `out.rs` writes through `anstream` (codes stripped for a pipe). Every renderer marks its parts with the palette, never with raw
codes. `--color auto|always|never`. Thousands separators in counts. Tests: every golden is unchanged except `status`'s text (the new layout;
JSON unchanged); a run through a pipe has no `\x1b`; `--color always` has them, `NO_COLOR=1` with `auto` has none; one golden of
`status --color always` so the palette is pinned. Commit: `cli A6d: style for people in a terminal; status in labelled lines`.

## 3. Phase 2: write

### A7 The seam, and the lock

| Files | What |
|---|---|
| `crates/vnoc/src/identity/edits.rs` (new) | `EditSink { Editor, Disk }`, `apply` (04 §2); `apply_text_edits(text, edits) -> String` with the driver's position rule |
| `crates/vnoc/src/identity/{attach.rs, relink.rs, write_id.rs}` | `Env.edits` in place of `Env.client`; `apply_edits` goes through it; `document_for` lets the caller supply a document read from disk |
| `crates/vnoc/src/userdata/lock.rs` (new) | `WriteLock::take(root, wait) -> Result<WriteLock, …>` on `.vnoc/local/write.lock`; only in an initialized project (04 §7) |
| `crates/vnoc/src/server.rs` | builds `Env` with `EditSink::Editor(client)`; takes the write lock inside `attaching` |

No behaviour change in the extension. Tests: `apply_text_edits` (an insert after a non-ASCII line, CRLF kept, a byte-order mark kept); `Disk`
on a temp file: the edit lands; the file changed between plan and apply → `Err`, file untouched; the lock: a second taker waits, then fails
with the message; `lifecycle.rs` and the window suite pass with no edits (attach, relink, write ID are all exercised there).

Commit: `cli A7: who applies an ID edit is a parameter; one write lock`.

### A8 `init`, `doc new` *(split 2026-11-20: **A8a** init, doc new, author and tags in the frontmatter, the race; **A8b** `tag:` / `author:` in
search, the wire, the sidebar, `--where` on `doc list` / `doc read`. A8a's first commit also replaces A7's BOM shift by 04 §2's rule.)*

| Files | What |
|---|---|
| `crates/vnoc/src/cli/write.rs` | `init`; `doc new`: the document from disk, `identity::attach` with `EditSink::Disk`, the lock, the output of 04 §8 |
| `crates/vnoc/src/identity/attach.rs` | the refusals that are rules, not printing: a title the node has (unless `again`), the body's size, a body with its own frontmatter; `body` written after the heading |
| `crates/vnoc/src/userdata/mod.rs` | `create_doc` takes the body, the author and the tags |
| `crates/vnoc/src/userdata/{read.rs, doc_text.rs}` | `frontmatter(text) -> { title, author, tags }` beside `title_of`; the doc texts keep the two values |
| `crates/vnoc-search/src/{query.rs, run.rs}`, `lib.rs` | `tag:` `author:` (docs that carry it; nodes that have such a doc); `DocEntry` gains the two fields; **a query of only filters lists docs when one of them is `tag:` or `author:`** (04 §4) *(from the built code: today filters alone find no doc)* |
| `crates/vnoc/src/search.rs` | `corpus_of` hands the two values from the doc texts to each `DocEntry`; `answer` puts them in the doc rows |
| `crates/vnoc/src/protocol.rs`, `shared/messages.ts` | `DocInfo` gains `tags`, `author` (`vnoc/docs`); the doc rows of `vnoc/search` too |
| `webview/src/sidebar/SearchResults.tsx` | a doc row shows its tags after the title |
| `docs/20-search/01-query.md` §2, §3 | the two filters as built, and the row for a query of filters alone, in the `docs:` commit |
| `crates/vnoc/src/cli/doc.rs` | `--where` on `doc list` and `doc read` (03 §3); tags shown on the rows (`doc list` with no node was built in A4) |
| `docs/04-storage/01-vnoc-folder.md` §"Docs" | the two keys, in a `docs:` commit |

Tests: `doc new` with no `.vnoc/` → `2` and the sentence; `init`, then `doc new` on a function: the `ID:` line is in the file, the node folder
and the doc exist, the three output lines; `--no-marker`; `[ids] default = "none"`; `--body-file -`; the same title twice → `2` with the path,
`--again` → `-2.md`; on a file (`# FileID:`), on a folder (`.vnoc-id`); a function with a syntax error → attached by name with the notice, `0`;
`--author x --tag plan --tag billing` → the two frontmatter lines; a doc with no such keys reads as none; `find 'tag:plan'`, `find 'kind:fn tag:plan'`,
`doc list --where 'tag:plan'` (no node), `doc read <node> --where 'author:x'`; `vnoc/docs` answers with the tags (`lifecycle.rs`);
in the crate (tables): `tag:plan` alone lists the docs that carry it and the nodes that have one, `kind:fn` alone still lists no doc,
`tag:a tag:b` needs one doc with both, `-tag:draft`, `matches` agrees with `run` on each; a doc whose tags were edited by hand is found by
the new tag on the next search with no `indexChanged` (`lifecycle.rs`);
**the race**: 20 rounds of two `doc new` on one function at once → one node folder, one `ID:` line, two docs; then `vnoc check` is clean.
A lifecycle test: a doc made by the command shows in a running core's `vnoc/structure` after `filesChanged`.

Commit: `cli A8: init and doc new`.

### A9 `relink`, `repair`

| Files | What |
|---|---|
| `crates/vnoc/src/cli/write.rs` | `relink` over `identity::relink`; `repair` over `identity::write_id` for `Index::lost_markers()`, given nodes, or `--all` |

Tests: on the sample, `relink` the Missing node to a function: `missing` is empty after, the ID is at the new place; each of the library's
refusals is `2` with its words; `repair` puts the lost `ID:` line back and `check` passes; `repair` on a clean project says nothing to do, `0`;
a node set to never carry an ID is left.

Commit: `cli A9: relink and repair`.

### A9c One fact per line (added 2026-11-20, after the second by-hand pass)

03 §11 "Layout: one fact per line": every text renderer; kind words in English; no leading blank line. Text goldens change, JSON goldens
do not; the plain/colour equality test and the no-ESC-in-a-pipe check stay. Commit: `cli A9c: one fact per line; no row wraps for a long
path`.

### A9b Tags: list, edit, describe *(added 2026-11-20; 04 §4 "Listing, editing and describing tags")*

| Files | What |
|---|---|
| `crates/vnoc/src/userdata/tags.rs` (new, library) | read `.vnoc/tags.toml` (bad keys/values → Problems, the rest read); `tag_counts(&DocTexts, &Descriptions)`; `rewrite_tags(text, add, remove) -> Result<(String, Change), …>` (pure: only the `tags:` line, block form folded, rules of 04 §4) |
| `crates/vnoc/src/cli/doc.rs` | `tags [--authors]` (no driver); `doc tag <node> [file] --add … --remove …` (the write lock, atomic, re-read before write) |
| `crates/vnoc/src/cli/write.rs` | the new-tag notice in `doc new` (and `doc tag`) |

Tests: `rewrite_tags` as a table (add, remove, both, the last removed, no line and an add, the block form, already has / already without,
a bad tag, every other line byte-identical); `tags.toml` with a bad key and a good one; `tags` on a sample copy with three tagged docs
and a described unused tag, with `--driver /nonexistent`; `doc tag` on a node with two docs and no file → `2` with the names; the file
changed between read and write → refused, untouched; the new-tag notice with a near tag (`planning` → `plan`) and with none; twice the
same `doc tag` command → the second says `already…`, exit `0`.

Commit: `cli A9b: tags lists them, doc tag edits one doc's, tags.toml describes them`. **Stop point:** agents can write. Full test run.

## 4. Phase 3: skill, install

### A10 `guide`, the skill

| Files | What |
|---|---|
| `skills/vnoc/SKILL.md` | the text (06 §5), the description on one line |
| `skills/vnoc/rules.md` | the rules block (06 §4), compiled in for A11 |
| `crates/vnoc/guide/{read,tree,find,write,check}.md` | the topics (06 §6), examples run on the sample |
| `crates/vnoc/src/cli/guide.rs` | `guide`, `guide <topic>`, `skill`; all `include_str!` |
| `crates/vnoc/tests/guide.rs` | every line in a guide or the skill that starts with `vnoc ` is run on the sample: none may exit `2` for an unknown flag or command |

Also: the `about` text of every command, read once as an agent would, cold. **[check]** `skills-ref validate skills/vnoc`.

*(Added 2026-11-20.)* First commit of the step: `vnoc doc tags` → **`vnoc tags`** (04 §4: the read must not start with the write's name);
the old name is not kept as an alias (nothing has shipped). The guide test's rules are 06 §6's (placeholders parsed only; `# exit 2` marks
a refusal shown on purpose; pasted outputs compared with a run). The rules block of 06 §4 goes into `skills/vnoc/rules.md` now (compiled
in, not written anywhere yet: A11 writes it), so the guide test checks its one `vnoc` line too.

Commits: `cli A10a: the tag list is vnoc tags`, then `cli A10: the skill and vnoc guide`.

### A11 `vnoc agent`

First: read 07 §1's table again from each agent's documentation, and correct it (`docs:` commit).

| Files | What |
|---|---|
| `crates/vnoc/src/agents.rs` (new, library) | the table of names → skill folder and rules file (07 §2); `detect(root) -> Vec<Agent>`; `plan(root, picked, options) -> Vec<Action>` (pure: which file, write / replace block / leave, why), `apply`, `status`, `remove`; the block's markers; "is this file ours" |
| `crates/vnoc/src/cli/agent.rs` | `agent install / list / status / remove`; `--agent`, `--all`, `--global`, `--dry-run`, `--force`, `--skill-only`, `--rules-only`; the numbered list in a terminal; the permission lines (07 §4) for the agents picked, only those checked |

Tests (`detect` and `plan`, tables over temp folders): each row of 07 §2's table; `claude,cursor` → both folders; `codex,pi` → one folder, written
once; `--all`; `gemini` with no `GEMINI.md` → created; `detect` on each marker; no terminal, nothing detected, no flag → `2` with the names; no
terminal, `.claude/` there → `claude`; an `AGENTS.md` with other text → the block added, the rest byte-for-byte; installed twice → one block;
`claude` with a `CLAUDE.md` → it gets the block; with a `CLAUDE.md` importing `@AGENTS.md` → `AGENTS.md` does;
a hand-written `SKILL.md` → left, exit `2`; `--force`; `status` on current, older, hand-changed, missing; `remove` leaves a file that had other
text and deletes nothing it did not write; `--global` writes no rules. By hand: 07 §2's `[check]` on an agent that reads both folders.

Commit: `cli A11: vnoc agent install writes the skill and the rules`.

### A12 The extension's two commands

| Files | What |
|---|---|
| `extension/src/cliInstall.ts` (pure part) + its test | which link or wrapper to make, when to move it, per platform (07 §3) |
| `extension/src/commands.ts`, `package.json` | `vnoc.installCli`, `vnoc.uninstallCli`, `vnoc.setUpAgents`; the move on activation; the context keys `vnoc.cliInstalled`, `vnoc.agentsSetUp`; the one-time offer after a first attach |

`setUpAgents` runs the bundled binary (`agent list --json` for the multi-select with the detected names ticked, then `agent install --agent …
--dry-run --json`, then the install): no list of agents and no file logic in TypeScript (P1).

Tests: unit, the pure part on each platform's paths; window, `setUpAgents` with one agent picked on a temp workspace writes the files the dry run listed. By hand on
macOS: install, `vnoc status` in a terminal, update the extension, the link follows. Windows **[check]** by hand once.

Commit: `cli A12: Install vnoc in PATH; Set Up Coding Agents`.

### A13 Releases, and the README

| Files | What |
|---|---|
| `.github/workflows/publish.yml` | per target, the archive of `extension/bin/` 's two programs; their sums into `SHA256SUMS`; `install.sh` attached |
| `scripts/install.sh` | 07 §6 |
| `README.md`, the extension's README | "Using V-NOC with coding agents" |
| `CHANGELOG.md` | the command line, marked experimental |

Proof: the publish dry run (doc 19 §8) lists four archives and their sums; `tar -tzf` shows both programs; `install.sh` run against a dry-run
release in a clean container puts a working `vnoc` on the PATH. **[check]** once each: `npx skills add` and `gh skill install` from the
repository find the skill (07 §5).

Commit: `cli A13: vnoc in every release`.

### A14 Trying it with agents

The six scenarios of 06 §8 in Claude Code, Codex, and pi or Cursor, **each with the text output and with an experimental `--xml`** (03 §11:
a renderer, one file, built at the start of this step; note bodies inside `<doc>` tags). Recorded per run: done right or not, scenario 5
resisted or not, tokens, calls. What wins decides the default (03 §11), and the losing renderer is deleted, not kept behind a flag. The table of results goes into 06 §8; the skill, the rules block and the
guides are changed from it; the guide test of A10 keeps them honest. Commit: `cli A14: the skill, after six scenarios in three agents`.

**Stop point: release 0.5.** Full test run; `02` §3's row.

## 5. Phase 4: after

### A15 `changed` (optional; recommended)

`crates/vnoc/src/changed.rs`: `git diff -U0 <ref>` parsed into line ranges per file, each to its innermost def (03 §10); `cli/changed.rs`. The
skill's "after you changed code" gains one line. Tests: a temp git repository with one changed function and one added: both listed, the
documented one first; no git → `3`. Commit: `cli A15: vnoc changed`.

### A16 The outline cache (only if A0's numbers are outside the budget)

05 §3's design, in `crates/vnoc/src/index/cache.rs`, used by `cli::project::open` and (a second commit, if start-up wants it) by the server's
first build. Tests: a second run starts no driver when nothing changed; one changed file is the only one outlined; a cache from another driver
version is ignored; a corrupt cache is ignored; the answers of every command are identical with and without the cache (the integration suite run
twice). **[measure]** again, written into 05 §3. Commit: `cli A16: outlines are kept between runs`.

### A17 Docs on the project root (if there is time; not urgent)

[`10-root-docs.md`](10-root-docs.md). Two commits: the library and the commands (§4 there), then the extension (§5 there, with its two
`[check]`s). Tests: `doc new / --title …` creates `.vnoc/root/<slug>.md` and no node folder, no marker, no `node.toml`; `doc list /`, `doc read
/`, `show /`; `.` from the root is `/`, `.` from a subfolder is that folder; `find` finds a root doc by a word and `in:billing/` does not;
a core that does not know `root/` (the 0.2 fixture of `18-release-0.2`) still loads the project; `check` and `missing` say nothing about it.
Commits: `cli A17: docs on the project root`, `root docs in the extension`.

## 6. What each step leaves working

| After | An agent can | A person or CI can |
|---|---|---|
| A2 | `vnoc status` | see the same counts as the sidebar |
| A3 | read one function, by any name for it | — |
| A4 | read notes; get a tree in the shape it wants | — |
| A5 | search names, notes, and code | — |
| A6 | follow calls; see what is wrong | **fail a pull request with `vnoc check`** |
| A9 | attach a note; relink; put an `ID:` back | script the same |
| A11 | be taught all of it by one command | commit the skill for the team |
| A13 | use it with no VS Code | install from a release |

## 7. Tests, by kind

a. **Unit** (Rust): `tree_view`, `grep`, `check`, `agents::plan`, `apply_text_edits`, the exit-code table, the lock.
b. **Integration** (`crates/vnoc/tests/cli.rs`): every command on a temp copy of the sample, text and `--json` (golden), with the real driver.
c. **The guide is true** (`tests/guide.rs`): every command line it shows runs.
d. **The server is unchanged** (A1, A2, A7): `lifecycle.rs` and the window suite pass with no edits.
e. **Two writers** (A8): the race.
f. **By hand**: the PATH command on macOS and Windows; the six scenarios in three agents (A14).
