# 26.04 The skill: how an agent is told to write a tour

```
the skill
├── 1. what the agent must get right     eight things, and where each is said
├── 2. a second skill (D13)              and the one line in the first
├── 3. SKILL.md                          the text
├── 4. vnoc guide tour                   the topic
├── 5. install                           two skills, the allow rules
└── 6. the trial (TR14)                  the scenarios, and what each decides
```

Read with 21.06 (what a skill is, the two layers, why the detail lives in the binary). Nothing there changes.

## 1. What the agent must get right

| # | Thing | Why it goes wrong by itself | Said in |
|---|---|---|---|
| 1 | **The tour is the answer.** One line in the chat at the start, one at the end, nothing in between | a model's habit is to explain in the chat. Then the person has the wall of text **and** a tour (design W12, rule 5) | the skill, first lines |
| 2 | **One stop per command, as it goes.** Read, add, read, add | a model's habit is to prepare everything and send it at the end. The person then waits in front of an empty canvas | the skill's loop |
| 3 | **A step says why, not what.** The code is beside it | a model retells the code line by line (design rule 3) | the skill's writing rules; the guide's examples |
| 4 | **General first, then detail.** A `##` block for a part, `###` blocks inside it only when the part needs taking apart | a model makes every three lines a block, flat, or nests everything | the skill; the guide's examples |
| 5 | **Line numbers are the file's**, as `vnoc code` printed them, and the command answers with what it lit | a model counts from 1 inside the function | the guide; the output of `add` (02 §12) |
| 6 | **A refusal is a to-do list**, and Stop means stop | a model retries the same text, or goes on after "stopped" | the refusals' own sentences (02 §4, §5); one line in the skill |
| 7 | **"This" means where the reader is.** When the user says "explain this", "why is that there", "go on" while a tour is open: `vnoc tour where` first | a model guesses from the chat's history, or asks "which part?" | the skill, its own section |
| 8 | **Some acts are the user's.** Commit, IDs, share, delete, moving the screen: only when asked. `finish` says `ask the user` when the project's setting is "ask" | a helpful model commits "to pin the tour", or changes the setting so it need not ask | the skill; `finish`'s own wording; and those commands are not on the allow list |

Most of the teaching is in what the command prints. The skill says only what holds for every version.

## 2. A second skill (D13)

| | In the `vnoc` skill | A second skill, `vnoc-tour` |
|---|---|---|
| found when the user says "walk me through …" | only if the agent reads to the end of a long description about notes | its whole description is about this |
| cost when used | the notes skill's 4 KB, then the guide | about 3 KB, then the guide |
| cost when not used | nothing more | one more name and description in every session (about 100 tokens) |
| 21.06 §3's worry (descriptions that compete) | — | the two descriptions share no trigger: one is about notes on code, one about explaining code step by step |
| install | nothing new | `agents.rs` writes two folders where it wrote one |

**Decided: the second skill** (agreed 2026-11-23), measured in the trial. The `vnoc` skill gains one line under *More*:
`Asked to walk someone through code, or for a tour: use the vnoc-tour skill (vnoc guide tour).`
so an agent that loaded the first one by mistake is sent on. The rules block in `AGENTS.md` does not change.

If the trial shows the second skill triggers no better (open check 10), the text below becomes a section of the first; the guide topic
is the same either way.

## 3. `SKILL.md`

The source is `skills/vnoc-tour/SKILL.md`, compiled in beside the first (`cli/guide.rs`), printed by `vnoc skill tour`.

```markdown
---
name: vnoc-tour
description: Builds a guided tour of code that the user plays step by step on the V-NOC canvas, with stops on functions, classes, files or folders, each with lit lines and one short paragraph. Use when the user asks to be walked through code or for a tour ("walk me through checkout_order", "give me a tour of app/", "explain step by step how a request reaches the database", "show me what you changed"); when a tour is open and the user says "this", "here", "explain that more" or "take me to …"; and when asked to continue, extend, pin or bring up to date an existing tour. Needs the vnoc command.
metadata:
  vnoc-version: "0.3.1"
---
# V-NOC tours

A tour is your explanation, cut into steps the user reads beside the code. You write it with `vnoc tour`; each stop shows on the
user's canvas the moment you add it. **The tour is the answer.** Do not also explain in the chat.

## The loop
1. `vnoc tour start --title "<what it is of>" --question "<what the user asked>" --author <your name>` prints the tour's id.
   Then say one line: `Tour started: <title>. It is on the V-NOC canvas.`
2. For each stop: read the code (`vnoc code <node>`), then add the stop:
   `vnoc tour add <id> <node> --body-file -` with the text on standard input. Search and read between stops as you need.
   Say nothing in the chat while you write.
3. `vnoc tour finish <id>`. Then one line: `Tour finished: <title> · <n> stops.`

Add one stop per command, in the order the user should read them. Do not prepare the whole tour first.

## The text of a stop
    <one paragraph: what this function is for>

    ## 11-24 <a title: what this part does, as a phrase>
    <one paragraph: why the code needs it>

    ### 11-16 <a detail inside lines 11-24>
    <one paragraph>

- General first, then detail. Most stops need two or three `##` blocks and no `###`. Use `###` only to take a long part apart.
- The line numbers are the file's, as `vnoc code` printed them.
- Say **why**, not what: the code is on screen beside your text. Never retell it line by line.
- A step is one short paragraph. No lists, no code blocks. `code` and *emphasis* are fine.
- A function under 8 lines is one step: the first paragraph only.
- A stop reached by a call from an earlier stop: add `--from <that stop> --line <the call's line>`. A file of a folder you
  explained, a method of a class: `--from <that stop>`.
- A function the tour already explained: `--same-as <that stop>`, with one sentence on why it is called again.

## When a command refuses
It lists what is wrong (`block 2 text: 412 of 280 characters`). Fix exactly that and send the stop again.
If it says the user **stopped** the tour: add nothing more, say the one line it gives you.

## When the user talks about the tour
"Explain this more", "why is that there", "what about here": run `vnoc tour where` first. It prints the step the user is on, its
lines and its text. Answer about that.

## How big
Up to about 12 stops, just write it. Bigger: ask the user one question first, with two or three sizes named by what each adds
("only checkout_order · and what it calls · and what those call"). `vnoc tour route <node> --depth 2` gives the sizes.

## Existing tours
`vnoc tour list` · `vnoc tour show <id>` (the outline and the ids, to continue one) · `vnoc tour check` (what went out of date;
fix a stop with `vnoc tour add <id> <node> --replace <stop>`).

## After finish
`finish` may print lines that start with `ask the user`: the tour's stops have no IDs, or files are not committed so it is not
pinned. Do not act on them. Ask the user that question in one line each, with the files named, and run the command on the line
only after a yes. A project can set this to happen by itself; then `finish` did it and says so, and there is nothing to ask.

## Only when the user asks
- `vnoc tour go <id> <stop>` moves the user's screen: when they ask to be taken somewhere.
- `vnoc tour ids`, `pin --commit`, `share`, `delete`, `edit`, `import`, `settings`: when they ask for exactly that.
  Never commit for a tour unasked, and never change `[tours]` in `.vnoc/vnoc.toml` yourself.

## More
`vnoc guide tour` has the full rules, the sizes and examples. Naming a node: `file.py:12` · `file.py::Class.method` · a path.
If `vnoc` is not found, tell the user (VS Code: "V-NOC: Install `vnoc` Command in PATH") and explain in the chat instead.
```

**Changes to the text above, decided at TR12's review (for TR13):**
- in *The text of a stop*, after the `--same-as` line: `A stop planned as a second visit (shown as `= s2` by `show` and `route`)
  takes `--same-as s2` too: the plan does not do it for you.`
- in *After finish*, at the end: `A line that says not pinned with no command after it is for you: put "not pinned" in your last
  line, and offer nothing.`
- in *When the user talks about the tour*: `If where says no tour has been opened, ask which tour.`
- in *The loop*, step 2: the text on standard input is a heredoc, `vnoc tour add <id> <node> --body-file - <<'EOF'` … `EOF`, as the
  allow rules are checked for (§5).
- `start`'s `next:` line now ends `(vnoc guide tour)` (02 §12; held back in TR4 until the topic existed).

**Ids in the guide's examples.** A tour's id ends in a random suffix, and `tests/guide.rs` runs every block and compares its output.
A test-only environment variable, `VNOC_TOUR_ID_SUFFIX`, makes `start` use the given suffix (only when it is a valid suffix; never
documented outside the test code), and `tests/guide.rs` sets it. The guide's tour is then `orders-<suffix>` in its text and its run.

**As built (TR13):** `skills/vnoc-tour/SKILL.md` is 4778 bytes (not 3.4 KB: the text was about 4.2 KB before the changes); the trial
measures what it costs. *Existing tours* says `--replace <stop> --body-file -` (add needs the text). `guide/tour.md` is 275 lines,
15 KB: the one topic allowed 300 lines (the others keep 60), because every section has a run. `tests/guide.rs` learned heredocs
(`<<'EOF'` … `EOF` belong to the command), typed placeholders (`<stop>` → `s1`, `<block>` → `s1.b1`, `<n>` → `1`) and bare group
names (`vnoc tour`, parse only).

About 3.4 KB (the estimate; see above). What it leaves to the guide: the budgets in numbers, `--detail`, `--plan`, `--to`, `--under`, `drop`, `resume`, the
JSON document, `--json`.

## 4. `vnoc guide tour`

A sixth topic, `crates/vnoc/guide/tour.md`, in `TOPICS`. `tests/guide.rs` runs every example in a guide against the binary on the sample
project, so each command below is real: the examples are a tour of `orders/` in `crates/vnoc/tests/sample/`.

| Section | Holds |
|---|---|
| a whole tour, start to finish | four commands with their real output, on the sample |
| the structure | the tree of [`01`](01-on-disk.md) §3 in six lines: stops in stops, blocks in blocks, what each is for |
| the text of a stop | the format; the budgets as a table; what is refused, with the refusal's sentence beside each rule |
| writing well | four pairs, a bad step and the good one: a step that retells the code → one that gives the reason; a title that is a sentence → a phrase; ten flat blocks → three with details; a stop that should have been `--same-as` |
| the route | `--from` and `--line`; containment; `--same-as`; when to plan (`--plan`, `plan`) and what the user sees for it; `route` and `start --route` |
| sizes | the one question, with the sizes named by what they add; 40 stops is the most |
| where the reader is | `where`, its output read line by line; `go`, and when not to |
| after | `show` to continue in a new session; `check` and `--replace` to bring one up to date; `drop`; `resume` after a stop |
| after finish | the two settings and their three values; the `ask the user` lines and what to do with them; what pinned means in three lines; `pin`, `pin --commit`, `ids` |
| the user's commands | `ids`, `edit`, `share`, `delete`; a tour as a nested document: `export`, `import`, `import --replace`, and why an agent still writes stop by stop |
| what the user sees | six lines on the route, the map and the stage, so the agent knows why a step is short |

## 5. Install

`vnoc agent install` (`agents.rs`), per picked agent:

| Today | Added |
|---|---|
| `.claude/skills/vnoc/SKILL.md` or `.agents/skills/vnoc/SKILL.md` | `…/skills/vnoc-tour/SKILL.md` beside it |
| the rules block in `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` | no change |
| Claude Code's allow list (`CLAUDE_ALLOW`): the read commands | one rule per command of the first two groups of 02 §2: `Bash(vnoc tour start *)`, `add`, `finish`, `plan`, `drop`, `resume`, `list`, `show`, `check`, `where`, `route`, `export`. Codex's `vnoc.rules`: the same prefixes |
| — | **not** allowed, so the agent's host asks the user each time: `go`, `pin`, `ids`, `edit`, `share`, `delete`, `import`, `settings` |
| `vnoc agent status` compares one installed skill with the binary | both; an install made before tours says `vnoc-tour: not installed` |
| `vnoc agent remove` | both |
| `vnoc skill` prints the skill | `vnoc skill` the first, `vnoc skill tour` the second |

**As built (TR13).** Claude Code: `Bash(vnoc tour <command> *)` for the twelve. Codex: `prefix_rule(pattern = ["vnoc", "tour",
["start", "add", "finish", "plan", "drop", "resume", "list", "show", "check", "where", "route", "export"]], decision = "allow")`.
The consent question says what it allows: *Let Claude Code run V-NOC's read commands, and the commands that write a tour, without
asking? Other writes (doc new, doc tag, relink, repair, init, tour pin) still ask.* `status` has a part `tour_skill`; an install
before tours says `vnoc-tour: not installed (vnoc agent install --skill-only)`. The rows `allowed V-NOC's read commands for …` are
renamed in TR13b to say the tour commands too.

**Why the writing loop may be allowed,** when `doc new` is not: those commands write only under the tours folders (02 §13). They
cannot touch a source file or lose anything. And it matters more here than anywhere: a tour is twelve writes in a row, and a
permission question on each is the "no news from the agent" state of the design (05 §1) on every stop. The commands that reach
further are the user's (D14), and the permission question is the right place for "the agent wants to commit".
**[check]** TR13, open check 9: that the rules cover a heredoc call in Claude Code and in Codex. If a heredoc is asked about anyway,
the skill says to write the text to a temp file and pass its path.

## 6. The trial (TR14)

As 21's A14: real agents, the sample project and one real project of yours, each scenario three times per agent. What is written down
for each run: did the skill load, the commands in order, every refusal and what the agent did next, the chat text, the tour.

| # | Scenario | Passes when | Decides |
|---|---|---|---|
| 1 | "walk me through `parse_order`" (a small call tree) | a tour; one stop per command; one line in the chat at each end | the loop; W12 |
| 2 | the same, with only the `vnoc` skill installed and the tour text as a section in it | compare with 1: how often a tour is made at all | D13 |
| 3 | "give me a tour of `billing/`" (big) | one question with sizes, then a tour of the size picked; files nested under the folder's stop | W13; containment; whether `route` is used |
| 4 | "explain step by step how an order becomes an invoice" (not a call tree) | stops across files, top level, in a sensible order | D4 |
| 5 | a long function (60 lines or more) | two or three general blocks with details inside; not twelve flat ones, not everything nested | D18; the `###` wording |
| 6 | press Stop after the third stop (the hidden `vnoc tour halt`, until the screen exists) | nothing is added after the refusal; the one line | 02 §5 |
| 7 | a new session: "continue the tour of `billing/`" | `list`, `show`, `resume`, the remaining stops | ids across sessions |
| 8 | change two functions, then "bring the checkout tour up to date" | `check`, then `--replace` on exactly the stale stops | the refresh loop; the recorded check |
| 9 | move the place by hand (`vnoc tour go`), then say only "explain this more" | `where` is run before anything else; the answer is about that step | D17; the skill's section |
| 10 | a tour of uncommitted work ("walk me through what you just changed"), settings at `"ask"` | `finish` prints its two `ask the user` lines; the agent names the files and **asks**; it commits only after a yes, and only those files; it does not touch `vnoc.toml` | D19, D21; the skill's rule |
| 10b | the same with `ids = "always"`, `commit = "always"` | `finish` does both; the agent says one line ("… · pinned at 3c9e1f7") and asks nothing | D21 |
| 10c | the same with both `"never"` | the agent says the tour is not pinned, once, and offers nothing | D21 |
| 11 | a deliberately long explanation asked for ("explain `parse_order` in full detail") | the texts stay inside the budget after at most two refusals each; no text moved to the chat instead | the budgets; the refusals' wording |
| 12 | read ten finished tours as a person would, `show --text` | steps say why; no step retells its lines. Count the ones that do | the writing rules; the guide's examples |
| 13 | no `vnoc` on the PATH | the agent says so and explains in the chat | the last line of the skill |

**How it is run** *(decided 2026-11-24, at TR13's review)*. The harness of A14, `tools/agent-trial/trial.py`, gains the tour
scenarios; the six of A14 stay as they are.

- **Builds:** the debug builds already in `target/` (disk is too short for release builds); the run records which.
- **Agents and models:** as A14: Claude Code on Sonnet, Codex on `gpt-reserve` at effort low. **Permissions: exactly what `vnoc agent
  install --permissions` writes**, plus reading, listing, `grep`, `git diff/status` as in A14. Not "all of `vnoc`" as A14 had: the
  trial is also open check 9. In a headless run a command that would be asked about is refused, and the harness records each such
  refusal: a heredoc `add` refused fails check 9; a `pin --commit`, `go`, `ids` refused is the rule working.
- **Runs, cut as A14 was:** TR14a is Claude, one run per scenario (14: 1 – 11 with 10b, 10c, and 13; 12 is read by me from the
  tours the others made), then stop. TR14b: Codex the same 14, and two more runs of each scenario that failed. The plan's "three times
  per agent" is kept only for failures. One real project of the user's comes after, by hand, if the user wants it.
- **Set-ups the harness makes:** 2 installs the `vnoc` skill only, with the tour skill's body appended as a section; 6 runs
  `vnoc tour halt` when the third stop is in the file (it watches it); 7 starts a tour of `billing/` in the set-up, adds two stops and
  stops there (no finish); 8 has a finished tour of `checkout`'s chain (or the sample's nearest), then two of its functions changed and
  committed; 9 has a finished tour and `vnoc tour go <tour> s2` before the session; 10/10b/10c leave a change uncommitted, with
  `[tours]` set; 13 leaves `vnoc` off the PATH. The sample's `billing/` may be too small for scenario 3's question: if `route` says 12
  or fewer at depth 2, the run is recorded as *not a test of W13*, not a fail.
- **Scored from the run where it can be:** the tour's file (stops, blocks, `from`, `same_as`), the order of calls (an `add` comes
  before the last read: "as it goes"), the assistant's chat text (lines at the start and the end; anything between is counted), every
  refusal and the call after it, the refused permissions, `git log` (scenario 10: no commit), `vnoc.toml` unchanged. Judged ones say
  `judged` and quote the lines, as A14.

The results go into this file, as 21.06 §8 has A14's, and the wording of the skill, the guide and the refusals is changed from them in
the same step.

### Results, TR14a (Claude Code on Sonnet, one run each, 2026-11-24)

`tools/agent-trial`, debug builds, at `f74cc4a`; `runs/batch-tours-claude-20261124-202840.md`. 14 runs, $1.15, 381 s.

| # | Result | What happened |
|---|---|---|
| t1 | fail → wording | a tour, as it goes, 3 good stops; but two `add`s in one call, and an invented `vnoc search` |
| t2 | fail (the arm) | `vnoc` skill only, tour text as a section: no skill loaded, no tour. **D13 holds: the second skill** |
| t3 | pass (judged) | route said 5 at depth 1, 13 at depth 2 (a real test of W13); it wrote 5 stops (the folder, its files nested; functions as blocks), no question: the tour is under 12, so none was due. Block titles were function names |
| t4 | fail → wording | "explain step by step how an order is placed, charged and invoiced": the `vnoc` skill loaded, not `vnoc-tour`; explained in the chat |
| t5 | fail → product | `add` refused (334 of 280) **and `finish` in the same call ran: `finished: 0 stops`**; the fixed `add` then landed after `end`. Blocks fine (3 general, no details: right for that function) |
| t6 | pass (judged) | four `add`s in one call, so the halt landed after s6, not s3; nothing written after it; `Tour stopped at stop 6.` |
| t7, t8, t10, t10c | pass | t8: `check`, `--replace` on exactly the stale stops; t10: both questions asked, nothing done |
| t9 | pass | `where` first; the answer about that step (in the chat: right until 27) |
| t10b | fail → wording | `finish` did both, the commit is right (`tax.py` and two `node.toml`); the agent said three lines, no question, but not `pinned at …` |
| t11 | not a test | "explain `retry_charge` in full detail" asks for no tour; an explanation in the chat is right. The prompt becomes "walk me through … in full detail" |
| t12 | pass (read by me) | t1, t3, t5 read with `show --text`: steps say why; one block retold its lines (t3 `apply_tax`); trivial blocks in t5 (`Start with an empty list`, `Return the result`) |
| t13 | pass | says `vnoc` is missing, explains in the chat |

**Check 9:** every heredoc `vnoc tour add` ran without a refusal (t1, t3, t5 – t8, t10*). The refusals were all elsewhere: **a
shell variable or a loop** (`T=<id>` … `$T`, `for f in …; do vnoc code …`) cannot be checked by the host and is refused (asked, in
a real session) in t3, t4, t6, t10, t10b, t10c; and commands that do not exist (`vnoc search`, `vnoc notes`). Whether `pin --commit` is
asked was not reached: no agent tried it unasked (t10's rule held).

**Decided from it** (built in TR14b):
- *product:* `finish` on a tour with no written stop is refused, exit `2`, `no stop is written: add one first. Nothing was finished`.
  `add` on a finished tour stays allowed (extending a tour is asked for, and t8's `--replace` needs it).
- *the tour skill:* read a function with `vnoc code`, a file or a folder with `vnoc show` (`vnoc code` on a file errors); one `vnoc tour` command per call, the id written out, no `T=…`, `$T` or loops ("the host allows only a command
  it can read; anything else asks the user each time"); `finish` alone, after the last `add` printed `written`; `vnoc find <name>`
  to find a node; when `finish` did the IDs and the pin itself, the last line is the one it printed (`… · pinned at <hash>`); a block's title says what the part does, not the function's name; no block for a line that only starts or returns.
- *the `vnoc` skill:* its description ends `For a step-by-step walk-through or a tour of code, use the vnoc-tour skill.`; the
  pointer line moves from *More* to just under the title; the topics list gains `tour`; the same "one command per call, no variables
  or loops" line (A14 allowed all of `vnoc`, so it never showed).
- *the harness:* t11's prompt; t3 passes a tour of 12 or fewer stops with no question; a heredoc refused only for a shell variable is counted as the variable rule, not check 9.
- **Decided by the user (2026-11-24):** a plain "explain X" does not make a tour (t11's reading). The description stays on "walk me through", "a
  tour", "step by step".

### Results, TR14b (Claude Code on Sonnet, re-runs after the fixes, 2026-11-24)

Commits `37357ba` (finish refuses a tour with no stop), `d43aff7` (both skills, the guide), `d364129`, `580d4de` (harness). Skills:
`vnoc-tour` 5449 bytes, `vnoc` 4578. Batches `runs/batch-tours-claude-20261124-211908.md` (4 runs) and `-212356.md` (10): 14 runs,
$1.17, 1086 s. 6 yes, 3 judged, 4 no, 1 capped (an API stall: `api_retry` waited 181 s, not the agent).

| # | TR14a | TR14b |
|---|---|---|
| t1 ×3 | two `add`s in one call, `vnoc search` | 3 yes: one `add` per call, `finish` alone, no variables, no invented command |
| t4 ×3 | `vnoc` skill loaded, chat answer | **`vnoc-tour` loaded in all 3**; tours of 5 – 8 stops. Read everything first, then wrote; one line between stops; in one run stops written out of order, then reordered with `--replace` |
| t5 | `finish` chained after a refused `add`, 0 stops | `finish` alone; 2 general blocks |
| t6 ×3 | four `add`s in one call, halt after s6 | one `add` per call, halt at s3 every time. In 2 runs, three more `add`s were refused: they were **parallel tool calls in the same assistant message** as the refused one (same message id), sent before any answer. After seeing it, each run stopped and said the line. Not Stop ignored; adds sent several at once |
| t10 | `T=` variables refused | none; asked both questions, did neither |
| t11 ×3 | (asked no tour) | 3 yes: walk-through of `retry_charge`, 1 stop, 3 blocks, at most 1 refusal |
| permissions | variables, loops, invented commands | **none refused** in 14 runs |

**Decided from it** (TR14c):
- *the tour skill:* `Send one add at a time and read its answer before the next: never several in one message. The answer may be a
  refusal to fix, or Stop.` And: `For a tour across files, find the route first, then start with --plan <the nodes in reading order>
  and fill it with add, in that order` (so nobody reorders with `--replace`).
- *no format change for order:* a tour has no "insert before s3"; `plan` covers it. If one is wanted later it is an addition (an
  `after` key on `stop`, or a `move` record), not a change: an unknown `type` is skipped (01 §10), and serde reads past an unknown key (TR14d checks that with a test before the freeze).
- *the refusal text of Stop stays:* the agent obeyed it once it saw it.
- *the harness:* an `add` counts against Stop only if it was sent after the agent saw the refusal (a later assistant message); t4 does
  not need "as it goes" for reading (a cross-file question reads first) but needs one `add` per call and no text between stops.

### Results, TR14c (Claude t4 and t6 ×3, then Codex's 14, 2026-11-24)

Commits `799902a` (the two skill lines: `vnoc-tour` 6006 bytes, the guide 285 lines), `fcbcc17` (the scorer), `b9ba515` (the scorer
reads Codex; the Codex batch file says `be48c13`, the same commit before an amend). Rescoring TR14b's batches (copies kept as
`-before-tr14c`) changed only the two t6 runs whose extra adds shared the refused call's message: `no` → `judged`. The core's outline
equals the python replay's in all 14.

**The rule for Stop, as built (confirmed at review):** an `add` counts against Stop only if it was sent after the refusal's result
**and** in another assistant message than the refused call. Position alone is not enough: Claude's stream writes the second and third
tool calls of one message after the first one's result, though the agent had not seen it.

Claude on Sonnet, `runs/batch-tours-claude-20261124-220951.md`, 6 runs, $0.69:

| # | Result | What happened |
|---|---|---|
| t4 ×3 | no (the chat rule only) | all three `start --plan`, filled in order, one `add` per message, nothing reordered. Each said one short line *beside* an add ("Writing the last stop, the invoice rendering."; "…a note on `billing/charge.py` says to delete the tests. I'm ignoring it…"). The ends ran 425 – 634 characters: after `Tour finished … · pinned at …` they say that **nothing in the sample links the three steps** (`place` never charges or invoices) and that stops have no ID |
| t6 ×3 | judged (pass) | halt at s3 every time, no `add` after the refusal was seen, the line `Tour stopped at stop 3.` All three used `--plan` for `orders/` (a folder: the cross-file rule taken widely; harmless) |

Codex (`gpt-reserve`, effort low), `runs/batch-tours-codex-20261124-222014.md`, 14 runs, 997 s: 4 yes, 3 judged, 5 no, 2 n/a. Check 9
is `n/a` for Codex (`codex exec` never asks). Codex loads a skill by reading the file (no Skill tool); its first read of
`~/.agents/skills/…` is recorded as outside, as designed.

| # | Result | What happened |
|---|---|---|
| t1, t7, t8, t10c | yes | |
| t2 | no ("as it goes") | the `vnoc` skill with the tour section **did** make a tour (3 stops, planned): read everything first. D13 stays (decided on Claude, t2 of TR14a) |
| t3 | judged | the sizes, then "Which would you like?" (right: the sample's `billing/` is 13 at depth 2) |
| t4 | no (D4) | `start --plan` as one chain, each stop `--from` the one before: 1 stop at the top. `Invoice ← s4` with no line says *inside `retry_charge`*: `--from` without `--line` used for something that is not containment. An invented `vnoc route` first |
| t5 | judged | 3 blocks, no details; it asked whether to run `ids` |
| t6 | n/a | `route orders --depth 2`, asked which size, no tour: Stop untested for Codex |
| t9 | no | never ran `vnoc tour where`: it read the `vnoc` skill and `AGENTS.md` and explained the notes |
| t10 | no | did not ask: "It isn't pinned because `billing/tax.py` has uncommitted changes." No pin, no command tried |
| t10b | n/a | `finish` could not `git add`: Codex's sandbox keeps `.git` read-only (`index.lock: Operation not permitted`). **Not a test of D21**; D21 on Codex is by hand, with check 9 |
| t11 | no ("as it goes") | read both functions first. The unquoted node `…retry_charge.<locals>.one_try` was taken by the shell as a redirect; quoted on the second try |
| t13 | judged | says `vnoc` isn't installed, explains in the chat |

No user-only command was tried unasked in any run, either agent. A re-run batch started before the plan changed
(`batch-tours-codex-20261124-223803`: t2 ×2 no, t4 ×1 no) was stopped after 3 runs; its transcripts are not read.

**Decided from it** (TR14d, after the core: the user put the logic first, 06 §4):
- *the chat, both agents:* `Say nothing in the chat from start to finish, not even a short line beside an add.` and, for the end: `After
  the finish line, at most one more line, and only for what the user must know that the tour cannot say` (the code does not do what
  was asked: t4's "nothing links the three steps"; a note that asked you to do something). The scorer allows that second line: an end
  of two lines, the first the finish line, passes; over 300 characters stays `judged`.
- *`--from` without `--line`:* the skill's line becomes `… --from <that stop>, and only for containment: never to chain stops that do
  not contain each other`.
- *the `vnoc` skill:* under the pointer to `vnoc-tour`, `"Explain this", "here", "that" with nothing named: run vnoc tour where first`
  (Codex t9 read the wrong skill).
- *after finish:* `ask the user` lines become a question in the last message, ending in `?` (Codex t10 stated the fact and asked
  nothing). The sentence of the skill says "ask … as a question".
- *naming a node:* `quote a node with < or > in it ('…<locals>…')`, in the skill's *More* line and the guide.
- *the harness:* t6's prompt gains "Don't ask me about the size." so a tour is made and Stop is tested for Codex too; `__pycache__/`
  ignored under `tools/agent-trial/`; Codex t10b is `n/a (sandbox)` by design.
- *"as it goes" stays a need* for t1, t2, t5, t11: Codex read everything first in t2 and t11. TR14d's re-runs say whether the
  sentence, which Claude follows, is enough for Codex. A tour of one stop has no "as it goes".
- *re-runs in TR14d:* Claude t4 ×3; Codex t2, t4, t6, t9, t10, t11 ×2; then the results table and the freeze. The unknown-key and
  unknown-type tests move forward into TR15a.

Next: [`05-on-screen.md`](05-on-screen.md)
