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

About 3.4 KB. What it leaves to the guide: the budgets in numbers, `--detail`, `--plan`, `--to`, `--under`, `drop`, `resume`, the
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

The results go into this file, as 21.06 §8 has A14's, and the wording of the skill, the guide and the refusals is changed from them in
the same step.

Next: [`05-on-screen.md`](05-on-screen.md)
