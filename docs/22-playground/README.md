# 22. Playgrounds: try one piece of code without running everything

Written 2026-11-17 from the code at `0d5a816` (branch `search`), from the old V-NOC (`ide/v-noc`: `doc/08-playground.md`,
`core/sandbox/code_run.py`, `PlaygroundFormDialog.tsx`, `SettingsDialog.tsx`), and from the conversation of the same day. It replaces the one
line the plan had ("playground files", `02-mvp-and-releases.md` §3, release 0.7+). **Nothing here was run.** Every `[check]` is a claim to verify
before depending on it; every `[measure]` is a number to write here once a test gives it. It is not tied to a release number.

```
playgrounds
├── README (this file)      what ships · decisions · what the old V-NOC did · where it stands · asks · open checks
├── 01-what-it-is.md        the idea, top-down: who uses it, for what, the life of one playground
├── 02-on-disk.md           the play/ folder in a node folder · play.toml · project defaults · this machine's tools
├── 03-running.md           one run, step by step: plan → place → run → clean; the leftovers; the index stays quiet
├── 04-languages.md         a language is a preset (data): Python, TypeScript/JavaScript, Rust; adding one
├── 05-safety.md            not a sandbox · the trust gate · the rules a recipe must pass
├── 06-core.md              the play module, the library functions, the requests and notifications
├── 07-on-screen.md         the panel's second tab · New Playground · Run, Stop, the output · every state
├── 08-agents.md            vnoc play list · show · path · new · run; the lines in the skill
├── 09-build-order.md       steps PG0 – PG12 in six phases, each with files, tests, and its commit message
└── 10-later.md             what is not built, and why the format already has room for it
```

## 1. What it ships

A person or a coding agent picks a node (a function, a class, a file, a folder), writes a small script that imports the real code and calls it,
and runs it. The script is **kept with the node**, as a doc is: hidden in `.vnoc/`, shown on the node in V-NOC, committed, and it follows the
node through renames. For each playground the owner chooses three things: the **language**, **where it runs**, and the **command**.

Running is the same act in every language: the script is copied into the folder it should run in, the command runs, the copy is deleted.

| In | Out (later: [`10-later.md`](10-later.md)) |
|---|---|
| a playground on any node; create, edit, rename, run, stop | a sandbox: a run can do whatever the user's own terminal can |
| languages: **Python**, **TypeScript / JavaScript**, **Rust**, and `custom` (any command) | Rust's private functions (a designed, opt-in step: 04 §4) |
| the three choices per playground, with a default per language and per project | run history, expected output, "this example broke" in CI |
| output as it arrives, Stop, a timeout | arguments, standard input, a debugger, a terminal |
| a leftover file from a crash is removed at the next start | editing the script with full language help (04 §5 says what works today) |
| `vnoc play list / show / path / new / run` for agents, and a guide topic | removing a playground from the command line (a person does it, as for docs) |

## 2. Decisions

| # | Decision | Why |
|---|---|---|
| **G1** | **A playground belongs to one node and lives in that node's folder**, beside its docs. Creating the first one on a node is the attach flow (`identity::attach`): the ID, the marker, `node.toml`. | It then follows the node through renames, moves, Relink and the trash with no new code. `04-storage/04-growth-path.md` §2 kept this place free. |
| **G2** | **One playground is one folder: `play/<slug>/`**, holding `play.toml` (the recipe) and the script. | The plan's `play-*.py` had nowhere to put the recipe, and no room for a second file (a fixture, sample input). A folder has both. Older V-NOC versions list only `*.md` files and ignore it. |
| **G3** | **Three choices: `language`, `dir`, `run`.** A fourth, the temporary file's name (`file`), has a default nobody needs to see in Python and that matters in Rust. | It is what the old V-NOC asked, and what you described. |
| **G4** | **A language is a preset: a row of data** (extension, file name, command, where to suggest running). Not code, not a driver. A project can add or change presets in `vnoc.toml`. `custom` always works. | "Future proof" means a new language costs a table row. Go, Java or a shell script must not wait for a release. |
| **G5** | **Run = copy the script into `dir`, run the command, delete the copy.** One mechanism for every language. *(Your call, 2026-11-17; it is what `code_run.py` did.)* | Imports, package lookups and build tools all start from where a file is. Putting the file there is the one trick that works everywhere. Smarter ways per language (Go's overlay, a scratch crate) can come later as presets' options; nothing depends on them. |
| **G6** | **The copy's name always starts with `vnoc_play_`. It never overwrites a file. Each run holds a lock file that names its copy**, so a later start can remove what a crash left. | A file V-NOC put into the user's tree must be recognisable, must never cost them a file, and must not stay. |
| **G7** | **No shell.** The command is split into words and started directly. | The recipe is committed and runs on macOS, Linux and Windows; `sh` and `cmd` do not agree on anything. Someone who wants a shell writes `sh -c "…"`. |
| **G8** | **What runs is committed; which program runs it on this machine is not.** `{python}` in a command is filled from `.vnoc/local/play.toml`, then from the environment. | The old V-NOC stored `executable_path` in the playground: one person's `/Users/…/.venv/bin/python`, committed for everybody. |
| **G9** | **The runner is in the core** (P1, `08` §1c). The extension and `vnoc play run` call the same function; the output goes up as notifications. | An agent and a person must get the same run. A second editor draws a list and a text box. |
| **G10** | **It is not a sandbox, and says so.** The gate is VS Code's Workspace Trust; the full command is always on screen beside Run. No approval dialog per playground (ask 2). | A trusted project already runs its own code (tests, tasks). An approval of the command says nothing about the script, which is the part that does the work. |
| **G11** | **The script is edited as a real file in a normal editor tab.** The recipe is edited in a small form. | Zero new editor code, and the user's keys, formatter and (for Python) language help work. The limit is in 04 §5. |
| **G12** | **Agents get `list`, `show`, `path`, `new`, `run`. No remove.** | As 21 C3: an agent adds, a person removes. |
| **G13** | **Rust reaches `pub` items only.** | A file in `examples/` is its own crate. Reaching private items needs a `mod` line in the user's source: designed as an opt-in later step (04 §4), not built first. |
| **G14** | **Nothing about a run is written into `.vnoc/nodes/`.** The last result of each playground is held in memory. | Output is recomputable and noisy; a committed "last run" would dirty git on every click (`04-storage` §2's question: did a person create it?). |
| **G15** | **The index must not see the copy.** The scan and the `filesChanged` handler skip names that start with `vnoc_play_`. | Otherwise every run adds a file to the tree, bumps the revision, and marks every call tree stale (`shared/calls.ts` `isStale`). |
| **G16** | **A run has a timeout (60 s by default) and an output limit (1 MB kept).** Stop and the timeout kill the whole process group. | `cargo run` and `npx` start a second process; killing only the first leaves the script running. |

## 3. What the old V-NOC did, and what changes

| Old V-NOC (`ide/v-noc`) | Here | Why |
|---|---|---|
| a playground row in TerminusDB: `name`, `description`, `code`, `relative_path`, `filename`, `executable_path`, one owner | a folder in the node folder: `play.toml` + the script as a real file | no database; git shows a script as a script; the owner is the folder it is in |
| owner by database id (`owner_function`…) | the node folder | the ID already follows the code |
| `executable_path` stored per playground | `{python}`, resolved per machine (G8) | a path on one machine is not a team's setting |
| one project-wide setting: examples path + command prefix, in the browser's state | a preset per language, a project default in `vnoc.toml`, a choice per playground | several languages; and the setting is shared |
| `command_prefix + file`, `cwd` = project root, always | `run` with `{file}` and `{name}`; `dir` chosen | `cargo run --example {name}` does not end with a file path |
| temp file `__ide_runner_<uuid>.py`, deleted in `finally` | `vnoc_play_<slug>.<ext>`, never overwrites, a lock file names it, leftovers swept | a crash left the old file in the tree for good; a stable name gives readable tracebacks |
| `subprocess.run(..., timeout=60)`: output after the end, stdout **or** stderr | streamed, both, with the exit code and the time | a run that prints for 30 s should show it |
| Python only | presets | — |
| run from the web server | run from the core, behind the trust gate | — |

## 4. Where it stands

- Nothing of this exists in `crates/`, `extension/` or `webview/`.
- **A node folder may already hold more than docs:** `userdata/read.rs` `markdown_files` lists plain files that end in `.md`; a `play/` folder
  beside them is not read, not touched, and travels with the folder on Remove and Restore (`userdata/trash.rs` moves the whole folder).
  **[check]** in PG0 with the released 0.2.
- **Attach is tied to "make a doc":** `identity/attach.rs` `attach` ends in `create_doc`. PG2 splits "make sure this node has a folder" from
  "create a doc in it". Doc 21's step A7 changes the same function (the `EditSink` seam); whichever lands second builds on the first.
- **V-NOC reads Python only** (`scan.rs`: `extension == "py"`), and the tree is built from the files it read (`index/tree.rs` `build`): a
  folder with no Python file below it is not in the tree. So in a pure TypeScript or Rust repository there is **no node to own a playground
  today**. The presets do not care (a preset is not a driver), but the feature needs an owner: ask 3.
- `extension/package.json` already declares `untrustedWorkspaces: limited`. The gate of 05 adds to it.
- The doc panel (`webview/src/docs/DocPanel.tsx`) is the place a node's attached things are shown; the card's footer (`footerChips.ts`) counts
  docs. Both get a second kind.

## 5. Asks

| # | Question | My pick |
|---|---|---|
| 1 | The first run in a project adds one line, `vnoc_play_*`, to `.git/info/exclude` (a local file, never committed), so the copy does not show in `git status` and an agent's `git add .` cannot stage it. OK? | **Yes.** It is the only way to hide it without touching a committed file. In a worktree or submodule (`.git` is a file) it is skipped. |
| 2 | An approval dialog the first time a playground runs, and again when its command changes? | **No** (G10). Earlier today I said yes; writing it down changed my mind: approving the command does not cover the script, and re-approving on every script edit is noise. Workspace Trust plus an always-visible command is the honest gate. |
| 3 | TypeScript and Rust repositories have no nodes yet. What owns a playground there? | **(a) the project root**, once 21's A17 (`.vnoc/root/`) is in: works in any repository, no new node kind. **(b)** show files of the preset languages in the tree as file nodes with nothing inside (step PG8): more useful, but it changes what the tree is, so it is your call. A real driver for each language is its own plan. |
| 4 | A playground is committed, like a doc. An agent that tries ten things leaves ten folders. A not-committed kind (under `.vnoc/local/`)? | **Not first.** The skill says: one playground per question worth keeping; edit and rerun it. If trial shows clutter, a `--scratch` kind is an addition (10 §6). |
| 5 | Rust's private functions (the temporary `mod` line, 04 §4): build it as PG12, or leave it designed? | **Leave it designed** until someone hits the `pub` limit in real use. It edits a tracked source file, which nothing else in V-NOC does outside the ID marker. |

## 6. Open checks

| # | Check | Step | Decides |
|---|---|---|---|
| 1 | The released 0.2 opens a project whose node folder holds `play/x/`: nothing listed, nothing changed, `git status` clean; Remove and Restore keep it | PG0 | G2 |
| 2 | Killing a process **tree** on macOS, Linux and Windows from the core (a child that starts a child) | PG0 | G16: `libc::killpg` and a Windows job object, or `taskkill /T` |
| 3 | In a window started from the Dock (not from a terminal): are `python3`, `cargo`, `node`, `npx` on the core's `PATH`? | PG0 | whether G8 needs a `PATH` line in `.vnoc/local/play.toml` |
| 4 | `cargo run --example vnoc_play_x` with a file dropped into `examples/`: found without touching `Cargo.toml`; **[measure]** first and second run | PG0 | 04 §4 |
| 5 | `npx --no-install tsx {file}` when `tsx` is not installed: fails at once, asks nothing. Which Node versions run a `.ts` file with no tool | PG0 | 04 §3's default command |
| 6 | On Windows: starting `npx` (a `.cmd`) with no shell | PG0 | G7 on Windows: maybe the preset says `npx.cmd` there |
| 7 | The scan honours `.git/info/exclude` (`git_exclude(true)` is set): a `vnoc_play_*` file is never in the tree, with and without the line | PG3 | G15: the explicit skip is still built |
| 8 | Does Pylance resolve `from src.orders import …` in a script that lives under `.vnoc/nodes/…`? | PG4, by hand | 04 §5 |
| 9 | How the core learns that trust was granted after the window opened | PG5 | 05 §2 |

## 7. What later work stands on

| Later | Uses from here | So this folder must leave |
|---|---|---|
| more languages | the preset table (04) | no language name in the runner's code; only in `presets.rs` and in tests |
| playgrounds as checked examples (CI) | `play::plan`, `play::run`, the exit code | `plan` separate from `run` (06 §2); `--json` on `vnoc play run` |
| a second editor | the requests of 06 §3 | no logic in TypeScript: the form asks the core for the presets and the suggestion |
| `vnoc mcp` | the library functions | 21's C2, kept here too |
| search (`has:play`), the tree's counts | `NodeRecord.plays`, `Node.plays` | the count in the index from PG2 on |
