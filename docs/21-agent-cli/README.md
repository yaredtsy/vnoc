# 21. The `vnoc` command and the agent skill (release 0.5)

Written 2026-11-16 from the code at `0597d12`, as a review and a rewrite of the first draft (`21-agent-cli.md`, same day, now replaced by this
folder), and from the design in [`../futureplan/02-agent-access/`](../futureplan/02-agent-access/README.md), which this folder schedules: its
layers **0, 1a, 1b, 1c and 2c**. MCP (2b), VS Code's own tools (2a) and plugins (3) stay there ([`08-mcp-and-plugins.md`](08-mcp-and-plugins.md)
says why and what would change that). **Nothing here was run.** Every `[check]` is a claim to verify; every `[measure]` is a number to write here.
Starts after search (20) has reached its step S5: the commands stand on `vnoc-search` and `node_ref`.

```
the vnoc command
├── README (this file)        what ships · decisions · what the review changed · asks · what later work stands on
├── 01-what-agents-get.md     the freedoms, the guard rails, and why each
├── 02-naming-a-node.md       location · file:line · ID · path; where paths start; errors that teach
├── 03-read-commands.md       status · show · code · doc · tree · find · grep · calls · missing · check · changed; the output rules
├── 04-write-commands.md      init · doc new · relink · repair; who edits the source file; two writers
├── 05-how-it-runs.md         one binary · the root · one shot and its budget · stdout · exit codes
├── 06-skill.md               what a skill is (a primer) · the two layers · the text · vnoc guide
├── 07-install.md             where each agent looks · vnoc agent install · the PATH · releases
├── 08-mcp-and-plugins.md     postponed: MCP, plugins, callers; why, and what would bring each back
├── 09-build-order.md         steps A0 – A17 in four phases, each with files, tests, and its commit message
└── 10-root-docs.md           docs on the project root: one fixed folder, no ID (step A17, not urgent)
```

## 1. What it ships

A coding agent (Claude Code, Codex, Cursor, pi, opencode, Gemini CLI, Copilot…) in a project can, from its shell:

- **read by structure**: one function's exact code, a tree of a folder with signatures and doc counts, shaped as it asks;
- **read the why**: the notes attached to a function, class, file or folder;
- **find**: by a few letters of a name, by a word in a note, by a filter, by a regular expression over the code, answered **as functions, not
  lines**;
- **write**: attach a note (a why, a plan, a decision: any Markdown) to any node, with the ID, the marker and `node.toml` done by V-NOC;
- **check and repair**: see that nothing went Missing and no `ID:` line was lost, and put a lost one back.

A skill, installed by one command, tells the agent that `vnoc` exists, when to use it, and what never to touch. People and CI get the same
commands: `vnoc check` fails a pull request that left a doc Missing.

| In | Out (later) |
|---|---|
| `status` `show` `code` `doc list/read/path` `tree` `find` `grep` `calls` `missing` `check` | `vnoc mcp`, VS Code language-model tools, plugins and hooks ([`08`](08-mcp-and-plugins.md)) |
| `init` `doc new` `relink` `repair` | remove, trash, empty trash: **never** for agents |
| `guide` `skill` `agent install/status/remove` | `diff`, `history` (graph diff, function history: futureplan 01, 03) |
| plain text by default, `--json` on every command | Homebrew, `cargo install`, an npm wrapper |
| *Install `vnoc` Command in PATH* · *Set Up Coding Agents* · `vnoc` and the driver in each GitHub release | callers (`calls --callers`): call trees are solved lazily, one entry at a time, so there is no reverse map to ask. `grep` is the honest stand-in; [`08`](08-mcp-and-plugins.md) §3 has the two ways to build the real thing |
| `changed` (optional, A15) · an outline cache (A16, only if A0's numbers ask) · docs on the root (A17, if there is time) | a process that stays warm between calls |

## 2. Decisions

The three asks of the first version of this table were answered on 2026-11-16: C12 (the user picks the agents), C16 (yes), C17 (planned, not urgent).

| # | Decision | Why |
|---|---|---|
| **C1** | **Subcommands of the existing `vnoc` binary.** No arguments, or `serve`, runs today's LSP core. | The VSIX still ships two files; an old extension with a new core still works. |
| **C2** | **Every command is a thin handler over a library function** in `crates/vnoc` or `crates/vnoc-search`, the same one the LSP handler calls. A handler parses, calls, prints. | P1 (`08` §1c): the CLI, the server and later MCP must not disagree. |
| **C3** *(changed)* | **Read, check, and write docs.** `init`, `doc new`, `relink`, `repair` are in. **No remove, no trash, no bulk.** | Your call on 2026-11-16: agents write docs and plans. The first draft was read-only because writing needs the two-writers check; that check is now a step (A7), not a reason to wait. Removing stays a person's decision. |
| **C4** | **One shot, and exact:** each run opens the project, starts the driver, builds the whole index, answers, exits. Commands that need no outline skip the driver. A budget decides whether a cache is built (05 §3). | Always correct: a partial index can attach a doc to the wrong one of two nodes (05 §3). The cost is measured first (A0). |
| **C5** | **Plain text by default, short; `--json` for scripts**, with `"schema": 1`. Every answer about a node gives back its location and, if it has one, its ID. Text may change between releases; JSON only by adding. | The reader is a model paying for each token. JSON is the promise; text is for reading. |
| **C6** *(changed)* | Exit codes: `0` done (also "found nothing") · `1` `check` found problems · `2` bad arguments, or no such node · `3` the environment (no driver, a driver that failed, `format` too new). **A project with no `.vnoc/` is not an error for a read command.** | The extension browses a project before its first attach; so does the CLI: `vnoc code file:line` is useful anywhere. An agent's harness treats a non-zero exit as a failure, so "no matches" is `0`. |
| **C7** *(changed)* | **The instructions are compiled into the binary**, in two sizes: `vnoc skill` prints the short `SKILL.md`; `vnoc guide [topic]` prints the detail. **The installed `SKILL.md` is short and points at `vnoc guide`.** | An installed skill is a committed copy that every teammate's binary reads, at whatever version each has. The less it says about flags, the less of it can be wrong. The detail always comes from the binary being run. |
| **C8** | The driver is found by `--driver`, then `VNOC_DRIVER`, then **next to the binary** (its real path, symlinks followed). | A symlink in `~/.local/bin` must still find `vnoc-driver-python` in the extension's `bin/`. |
| **C9** *(new)* | **The project is the repository.** The root is the nearest folder at or above the current one that holds `.vnoc/`, else `.git`, else the current folder; `--root` overrides. Path arguments are relative to the current folder; every path printed is relative to the root. | The first draft said "found from the current folder" and "as the extension's opened folder" at once. An agent that runs `vnoc` from `src/` must not get an index of `src/` only. |
| **C10** *(new)* | A write is the **same library flow as the extension's** (`identity::attach`, `relink`, `write_id`), with the one thing that differs passed in: **who applies the planned edit to the source file**: the editor, or the disk. Writers in different processes take one lock file. | Today `attach.rs` holds a `ClientSocket` and asks VS Code to apply the edit; a terminal has no VS Code. futureplan 02.05 §5 named this seam (`EditSink`); it was not built. |
| **C11** *(new)* | **Two layers of instruction:** five always-on lines in `AGENTS.md` (the never-rules), and the skill (the how), loaded when the task matches. | A skill is read only if the agent decides the task matches. "Keep the `ID:` line" must be known **before** the agent rewrites a function it never thought of as documented. |
| **C12** *(new; changed 2026-11-16)* | **The user says which agents they use; V-NOC writes only for those.** `vnoc agent install` asks (the ones it detects are ticked), or takes `--agent claude,codex`. Each name maps to one of **two skill folders** (`.claude/skills/vnoc/`, `.agents/skills/vnoc/`) and one rules file (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`). | Your call. Asking keeps a `.claude/` folder out of a repository where nobody uses Claude Code, and lets the command print the permission lines for the right agent. Because every name is only a label on two folders, an agent that moves or a new one costs a table row, and `other` always works (07 §2). |
| **C13** *(new)* | `vnoc grep <regex>` searches code text and **answers in nodes**: the enclosing function of each hit, with its doc count. | It is what `rg` cannot say, and it is the honest stand-in for "who calls this" until the core has callers. |
| **C14** *(new)* | Every node command takes **several nodes**; lists can come from standard input. | One call instead of five; `vnoc find … --format id \| vnoc show -`. |
| **C15** *(new)* | **MCP is postponed.** | Every agent on the list has a shell; the CLI must exist anyway; MCP's one real gain here is a warm index, and a cache gives the CLI most of it. [`08`](08-mcp-and-plugins.md) lists what would bring it back. |
| **C16** *(yes, 2026-11-16)* | Doc frontmatter gains two optional keys: `author:` and `tags:` (a list). `vnoc doc new --author … --tag plan`. **They filter everywhere docs are asked for, not only in search:** `tag:` and `author:` in the query (so `find`, the box, `tree --where`), `doc list` and `doc read` with `--where 'tag:plan'`, and `vnoc/docs` carries both so a page can show and filter them (04 §4). | "Plans, any Markdown": a reader and a search should be able to tell a plan from a why, and an agent's note from a person's. It is an **addition to a frozen format** (`04-storage/01` §5 rule 1 allows additions), so it is decided once, before the first writer ships (A8). |
| **C17** *(planned, not urgent, 2026-11-16)* | **The project root can have docs, in one fixed folder, `.vnoc/root/`, with no ID.** Named `/`. Step A17, if there is time. | A plan for the whole project has no node today (`index/tree.rs`: "the root has none"; `RepoPath` refuses an empty path). An ID exists so a doc can follow code that moves; the root does not move, and there is one. So no ID, no marker, no `node.toml`, and `vnoc-domain` is not touched. An addition to the layout: older versions read only `nodes/`. [`10-root-docs.md`](10-root-docs.md) |

## 3. What the review changed

| # | First draft | What the code or the day says | Now |
|---|---|---|---|
| 1 | read and check only (C3, R3) | you asked for agents that write docs and plans | C3, C10; `08` §1c R3 rewritten |
| 2 | "each run … builds the index", after futureplan 02.02 had a lazy design | lazy is not exact: which record a def carries depends on markers in **other** files (`identity::resolve`) | C4 keeps "exact"; a budget and a pre-designed cache step (05 §3) replace "a cache is its own plan" |
| 3 | exit `3` for "not a V-NOC project" | the extension works before `.vnoc/` exists | C6 |
| 4 | "the root is found from the current folder, as the extension's is the opened folder" | `Project::open(dir)` indexes `dir` only | C9 |
| 5 | `vnoc skill --install` writes `.claude/skills/vnoc/SKILL.md`; other agents' folders `[check]` | checked 2026-11-16: `.agents/skills/` is the shared folder; Claude Code does not read it | C12, 07 §1: the user picks, two folders serve every pick |
| 6 | `--agents` appends to `AGENTS.md` (an option) | the never-rules must not depend on a skill being loaded | C11: the block is part of the install, not an option |
| 7 | the skill text matches the binary because the binary prints it | true when printed; false once installed and committed | C7: short skill, `vnoc guide`, `vnoc agent status` says when an installed copy is older |
| 8 | "`print_stdout` stays denied everywhere else" | the `deny` is in `main.rs` (the binary crate); `lib.rs` has none | 05 §4: the lint goes on the library too |
| 9 | `tree [path] [--depth N]` | you asked for a tree an agent can shape | 03 §4: kinds, a `--where` query, signatures, lines, doc titles, flat, a limit |
| 10 | `find` is the only search | — | C13 `grep`; "did you mean" on every unknown node (02 §4) |
| 11 | `check`: Missing, duplicate IDs, broken `node.toml`, unread files | the one thing agents do wrong, dropping an `ID:` line, leaves a node "found by name" and is in none of those lists | 03 §9: **lost markers** are a `check` failure, and `vnoc repair` fixes them |
| 12 | no `"schema"` in `--json` | futureplan 02.02 §6 had it | C5 |
| 13 | pi has no MCP (futureplan 02.03, 02.04) | pi's docs show native MCP now | noted in 08; does not change the order |

## 4. Where it stands

- `crates/vnoc/src/main.rs` starts the LSP main loop on stdio, nothing else.
- The pieces a read command needs are plain library code already: `Project::open`, `DriverHost` with `set_path`,
  `index::refresh(project, host, &RwLock<Index>, Touched::All)`, `Index::{tree, missing, problems, def_at}`, `UserData::{docs_of, load_all}`,
  `calls::merge`. After doc 20: `vnoc_search`, `search::corpus_of`, `node_ref`.
- **Not yet library code:** `structure` builds its answer inline in `server.rs`; `driver_tree` and `call_tree` ask the driver and map keys inside
  the server. A1 moves those bodies out, unchanged.
- **Not yet free of the editor:** `identity::attach::Env` holds `client: ClientSocket`, and `apply_edits` calls `workspace/applyEdit`. A7 puts
  the seam there.
- The core's integration test (`crates/vnoc/tests/lifecycle.rs`) finds the driver by `VNOC_DRIVER` or `drivers/python/target/debug`, and copies
  `crates/vnoc-domain/tests/fixtures` to a temp folder. The CLI tests do the same.

## 5. What later work stands on

| Later | Uses from here | So this folder must leave |
|---|---|---|
| **the walkthrough and tutorials** | the commands, the two VS Code commands, the skill | stable ids (`vnoc.installCli`, `vnoc.setUpAgents`); context keys a walkthrough step can complete on (`vnoc.cliInstalled`, `vnoc.agentsSetUp`); `vnoc status` as the one "is it working?" command; `vnoc guide` text that a tutorial can quote instead of restating; a small **sample project** (new in A2, `crates/vnoc/tests/sample/`) that the tests, every example in the guide, the trial of A14 and later the tutorial all run on |
| a Claude Code plugin with hooks (futureplan 02.04) | `vnoc check --changed --quiet`, the skill | the skill at `skills/vnoc/SKILL.md` in the repository, so a `.claude-plugin/` beside it is an addition |
| `vnoc mcp` | every library function behind a command | C2 kept strictly |
| CI for users | `vnoc check`, the release archives | exit codes (C6) and `--json` |
| more languages | everything | nothing here names Python except the driver's file name and the scan |

The last row is the largest lever on how useful this is to agents, and it is not in this folder: an agent works in whatever language the project
is in, and V-NOC reads Python. Worth weighing against 0.6 when 0.5 ships.
