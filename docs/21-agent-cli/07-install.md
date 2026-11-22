# 21.07 Installing: the skill, the command, the releases

```
installing
├── 1. where each agent looks        read again on 2026-11-19, from each agent's documentation
├── 2. vnoc agent install            what it writes, and what it never overwrites
├── 3. from VS Code                  Set Up Coding Agents · Install vnoc Command in PATH
├── 4. permissions                   reads without a prompt
├── 5. from the repository           other installers, for free
└── 6. from a release                archives · an install script
```

Three things have to be true before an agent can use V-NOC: the **command** is on its PATH, the **skill** is where it looks, and the **rules**
are in the file it always reads. One VS Code command does the first; one `vnoc` command does the other two, for the agents the user picks.

## 1. Where each agent looks

Read from each agent's own documentation on **2026-11-19** (the first reading, on 2026-11-16, was through a tool that summarises pages; this one
was too, except where a source says "raw"). **The shape held: two folders cover everyone, and no row of §2's mapping changed.** What the pages
say, with the page; a cell marked **[check]** was not stated plainly on a page that could be reached.

| Agent | Project skill folders | User skill folders | Always-read file |
|---|---|---|---|
| **Claude Code** | `.claude/skills/`; not `.agents/skills/` (no page mentions it) | `~/.claude/skills/` | `CLAUDE.md` (or `.claude/CLAUDE.md`); `AGENTS.md` **only when there is no `CLAUDE.md`**, from v2.1.277; a `CLAUDE.md` can hold `@AGENTS.md` |
| **Codex** | `.agents/skills/`, in every folder from the working one up to the repository root | `~/.agents/skills/` (and `/etc/codex/skills` for an admin) | `AGENTS.md` (`AGENTS.override.md` first; 32 KiB in all); `CLAUDE.md` only by a setting |
| **Cursor** | `.agents/skills/`, `.cursor/skills/`; also `.claude/skills/`, `.codex/skills/` | `~/.agents/skills/`, `~/.cursor/skills/`; also `~/.claude/skills/`, `~/.codex/skills/` | `AGENTS.md` (root and subfolders), `.cursor/rules/*.mdc` with `alwaysApply`; `CLAUDE.md` **[check]** (the rules page does not name it) |
| **pi** | `.agents/skills/`, `.pi/skills/` **[check]** (the second is from a summary of `configuration.md`) | `~/.agents/skills/`, `~/.pi/agent/skills/` **[check]** | `AGENTS.md` or `CLAUDE.md`, from the agent folder, the working folder and its parents; whether both are read in one folder **[check]** |
| **opencode** | `.agents/skills/`, `.opencode/skills/`, `.claude/skills/` | `~/.agents/skills/`, `~/.config/opencode/skills/`, `~/.claude/skills/` | `AGENTS.md`; "in any directory without an `AGENTS.md`, OpenCode reads `CLAUDE.md` instead" |
| **Gemini CLI** | `.gemini/skills/` or its alias `.agents/skills/` (the alias wins) | `~/.gemini/skills/` or `~/.agents/skills/` | `GEMINI.md` only; `AGENTS.md` only if `context.fileName` lists it |
| **Copilot** (CLI, cloud agent, VS Code) | `.github/skills/`, `.claude/skills/`, `.agents/skills/` | `~/.copilot/skills/`, `~/.agents/skills/`; VS Code also `~/.claude/skills/` | `.github/copilot-instructions.md`, `AGENTS.md`; the CLI and the cloud agent also a root `CLAUDE.md` or `GEMINI.md`. Visual Studio and JetBrains chat read no `AGENTS.md` |
| **Cline** | `.cline/skills/` (recommended), `.clinerules/skills/`, `.claude/skills/`; **not** `.agents/skills/` | `~/.cline/skills/` | `AGENTS.md`, `.clinerules/`, `.cline/rules/` |
| **Amp** | `.agents/skills/`, `.claude/skills/` | `~/.config/agents/skills/`, `~/.agents/skills/`, `~/.config/amp/skills/`, `~/.claude/skills/` | `AGENTS.md` (`AGENT.md` or `CLAUDE.md` where there is none) |
| **Goose** | `.agents/skills/` (also `.goose/skills/`, `.claude/skills/`) | `~/.agents/skills/` (also `~/.claude/skills/`) | `AGENTS.md`, then `.goosehints`, by default |
| **Windsurf** | `.agents/skills/` (its own are now `.devin/skills/`, legacy `.windsurf/skills/`) **[check]** | `~/.agents/skills/` (its own `~/.config/devin/skills/`) **[check]** | `AGENTS.md`, always on |

Sources, all read on 2026-11-19:
Claude Code: `code.claude.com/docs/en/skills`, `/memory`, `/permissions`. Codex: `learn.chatgpt.com/docs/build-skills`,
`/docs/agent-configuration/agents-md`, `/docs/agent-configuration/rules` (the old `developers.openai.com/codex/...` URLs redirect there), and, raw,
`github.com/openai/codex` `codex-rs/execpolicy/README.md`. Cursor: `cursor.com/docs/context/skills`, `/context/rules`. pi: `github.com/badlogic/pi-mono`
`packages/coding-agent/docs/skills.md`, `configuration.md`, `settings.md`. opencode: `opencode.ai/docs/skills/`, `/rules/`. Gemini CLI:
`geminicli.com/docs/cli/skills/`, `/gemini-md/`. Copilot: `docs.github.com/en/copilot/concepts/agents/about-agent-skills`, `.../how-tos/copilot-cli/customize-copilot/add-skills`,
`.../how-tos/configure-custom-instructions/add-repository-instructions`, `.../reference/custom-instructions-support`,
`code.visualstudio.com/docs/copilot/customization/agent-skills`, `.../custom-instructions`. Cline: `docs.cline.bot/customization/skills`, `/cline-rules`. Amp:
`ampcode.com/docs/customize/skills`, `/agents-md`. Goose: `goose-docs.ai/docs/guides/context-engineering/using-skills`, `/using-goosehints`. Windsurf:
`docs.windsurf.com` now redirects to `docs.devin.ai/desktop/cascade/skills` and `/agents-md`, which describe **Devin Local** (the same editor,
renamed); the Windsurf name itself is **[check]**.

**What the second reading corrected:** Cursor also reads `.codex/skills/` and `~/.codex/skills/`; Cline has a third project folder
(`.clinerules/skills/`) and `.cline/rules/`; Copilot's VS Code build also reads `~/.claude/skills/`; Claude Code reads `AGENTS.md` natively but
**only when there is no `CLAUDE.md`**, and only from v2.1.277, so an `AGENTS.md` block for `claude` reaches an older Claude Code not at all;
Amp, Goose and Windsurf, which were guessed, are read. Nothing moved between `.agents/skills/` and `.claude/skills/`, and no agent needs a third
folder.

**So: `.agents/skills/vnoc/` and `.claude/skills/vnoc/`.** The first is read by every agent above but Claude Code and Cline; the second by
those two (and by several of the others as well). Which of the two is written is the user's pick (§2).

## 2. `vnoc agent install`

```
vnoc agent install [--agent <name>[,<name>…] | --all] [--global] [--skill-only | --rules-only] [--dry-run] [--force]
vnoc agent list                 the names it knows, which it detects here, what each would get
vnoc agent status
vnoc agent remove [--agent …] [--global]
```

**The user says which agents they use (C12).** Why tools ask, and why this one does:

1. **Not to litter.** Every file written is a file in the repository that a teammate reviews. A `.claude/` folder and a block in `CLAUDE.md`
   have no business in a project where nobody uses Claude Code.
2. **Some things really are per agent:** which file it always reads (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`), its permission syntax (§4), and
   later its hooks or plugin ([`08`](08-mcp-and-plugins.md)). The command can only print or write the right one if it knows.
3. **It is a decision a person should see being made**, once, with the list of files in front of them, not a side effect.

Older tools asked for a harder reason: every agent had its own folder and its own format. That is mostly gone (§1), which is what keeps the
question cheap here: **a name is only a label on one of two skill folders and one rules file.**

| You pick | The skill goes to | The rules block goes to |
|---|---|---|
| `claude` | `.claude/skills/vnoc/` | `CLAUDE.md` if there is one (unless it imports `@AGENTS.md`), else `AGENTS.md` **and a one-line `CLAUDE.md`: `@AGENTS.md`** (A11d) |
| `cline` | `.claude/skills/vnoc/` | `AGENTS.md` |
| `codex` `cursor` `pi` `opencode` `copilot` `amp` `goose` `windsurf` | `.agents/skills/vnoc/` | `AGENTS.md` |
| `gemini` | `.agents/skills/vnoc/` | `GEMINI.md` (created if missing: Gemini CLI reads only that by default) |
| `other` | `.agents/skills/vnoc/` | `AGENTS.md` |

Picking three agents that share a folder writes that folder once. `--all` is every row: both folders, and each rules file that exists (plus
`AGENTS.md`). An agent that is not in the table is `other`, and gets what the open format says; a new agent, or one that moves, is a row in
this table and never a new mechanism.

**The import file for `claude`** *(decided 2026-11-20, review of A11; built in A11d)*. With no `CLAUDE.md` (and no `.claude/CLAUDE.md`), the
block goes to `AGENTS.md`, which Claude Code reads only from v2.1.277 (§1). So `install --agent claude` (and `--all`) also creates `CLAUDE.md`
holding exactly one line, `@AGENTS.md`: the import is old, so every version reads the block, and a newer one is not changed (it reads
`AGENTS.md` either way, once). The block stays in one file. `remove` deletes that `CLAUDE.md` only while it is still exactly that line;
`status` counts it as V-NOC's. A `CLAUDE.md` that exists is never touched by this rule (it gets the block, as before, unless it already
imports `@AGENTS.md`).

**How the pick is made:**

| Run | The pick |
|---|---|
| with `--agent …` or `--all` | those |
| in a terminal, with neither | **a menu** *(changed 2026-11-20, A12b: the user expected to move and tick, "it's everyone's expectation")*: ↑/↓ to move, Space to tick or untick, Enter to accept, **the detected ones ticked** at the start; Esc or q cancels and writes nothing. It is `dialoguer`'s `MultiSelect` on standard error, so standard output stays the answer. If the terminal cannot draw it (`TERM=dumb`, or the library fails), the numbered list and one typed line, as before |
| not in a terminal (an agent or a script runs it), with neither | the detected ones. Nothing detected: exit `2`, listing the names and `vnoc agent install --agent <name>`. An agent knows what it is; `vnoc guide` tells it to pass its own name |

**Detecting** is looking for what each agent leaves in a project: `.claude/` or `CLAUDE.md` → `claude`; `.codex/` → `codex`; `.cursor/` →
`cursor`; `.pi/` → `pi`; `.opencode/` or `opencode.json` → `opencode`; `.gemini/` or `GEMINI.md` → `gemini`;
`.github/copilot-instructions.md` → `copilot`; `.clinerules/` or `.cline/` → `cline`; an `AGENTS.md` or `.agents/` with none of those →
`other`. It only ticks boxes; it never writes on its own in a terminal.

**What is never overwritten:**

| File | If it is already there |
|---|---|
| a `SKILL.md` in either folder | written by V-NOC before (its header has `metadata.vnoc-version`): replaced. Anything else: left alone, said so, exit `2`; `--force` replaces |
| a rules file | only the block between its markers is replaced; the rest of the file is byte-for-byte as it was. No file: created with the block |

- It prints each file it wrote or left, and one last line: `commit these files so your team's agents get them`.
- `--global` writes the skill to the picked agents' user folders (`~/.claude/skills/vnoc/`, `~/.agents/skills/vnoc/`) and **no rules**: a rule
  about `.vnoc/` does not belong in every project's context.
- `--dry-run` prints what it would write. Nothing is ever written outside the root (or the home folder with `--global`).
- `status`: for each file V-NOC wrote, `current`, `older (0.3.0 → 0.4.0: vnoc agent install)`, `changed by hand`; and which agents that
  amounts to. Nothing is stored about the pick: the files are the record. `vnoc status` shows the same in one line.
- `remove`: deletes the skill folders V-NOC wrote and takes the block out of the files, for the agents named or for all; nothing else.
- All of it is file work with no driver: it is instant, and it is library code (`crates/vnoc/src/agents.rs`) the extension's command calls
  through the binary (P1).

Some agents read **both** skill folders (Cursor, opencode, Copilot). Picking `claude` and `cursor` puts the skill in both. **[check]** in A11
with one of them that two skills of the same name show as one; if one shows twice, `.claude/skills/vnoc` becomes a link to the other where the
system allows it.

`vnoc skill` only prints `SKILL.md` to standard output, for anyone who wants to put it somewhere this command does not.

**As built (A11).** `crates/vnoc/src/agents.rs` (the table, `detect`, `plan`, `apply`, `status`, `plan_remove`; `plan` reads the disk and writes nothing) and
`cli/agent.rs` (the pick, the printing, the exit codes). What the plan above did not say, or said differently:

- **What a line says.** `created` / `replaced` / `added` (the block, after what was there) / `current` (nothing to do) / `left` (and why) / `deleted` /
  `removed`, each with a line under it: `the skill, for claude` (the names picked that map to that file). With `--dry-run` the verbs are `will create`… and
  nothing is written; `--json` gives `{ schema, command: "agent install", dry_run, root, agents, actions: [{ part, path, change, agents, readers, why }],
  permissions: [{ agent, text }] }` (the extension's A12 reads it). `commit these files so your team's agents get them` follows only when something was written in
  the project (not for `--global`, not with `--quiet`). `~/` paths are the home folder's.
- **Exit codes.** `install`: `2` when a file was left (a `SKILL.md` that is not V-NOC's; markers that cannot be trusted: a `<!-- vnoc:start` with no
  `<!-- vnoc:end -->` after it), and `2` for an unknown name, `--global` with `--rules-only`, no agent found with no flag; the files that could be written are written all
  the same. `remove` leaves a `SKILL.md` that is not V-NOC's and says so, exit `0` (it had nothing of V-NOC's to take back). `3` for a write that failed or no home folder.
- **The rules file for `claude`** also looks at `.claude/CLAUDE.md` (Claude Code reads it as `CLAUDE.md`, and then ignores `AGENTS.md`): `CLAUDE.md`, else
  `.claude/CLAUDE.md`, else `AGENTS.md`; each unless it holds an `@AGENTS.md` import, which sends the block to `AGENTS.md`. Claude Code reads `AGENTS.md` itself
  only from v2.1.277 (§1).
- **`--all`** is the whole table (`other` included), so the rules go to `AGENTS.md`, and to `CLAUDE.md` or `GEMINI.md` only when that file already exists: it does not
  create the two that are for one agent.
- **The block** is replaced from the line that starts `<!-- vnoc:start` to the end of `<!-- vnoc:end -->`; the rest of the file is kept byte for byte, line endings
  included. Appended: after exactly one blank line (none added when the file already ends in one), then the block and a line break. `remove` undoes that
  to the byte (the blank line it added goes with it); a file with nothing left is deleted, and a rules file V-NOC created (`GEMINI.md`, `AGENTS.md`) is that case.
- **`status`** reads the two project skill folders (and the two user-level ones: `~/…` lines), and the rules files `AGENTS.md`, `CLAUDE.md`, `.claude/CLAUDE.md`,
  `GEMINI.md`. A skill file is V-NOC's when its header has `metadata.vnoc-version`. States: `current`; `older` / `newer` (its version against this binary's);
  `changed` (this version's number, other words: `changed by hand`; for a rules block, which carries no version: it differs from this V-NOC's text, edited
  or from another version); `missing` is a half with no file while the other half is there. A `SKILL.md` that is not V-NOC's is not listed. Each file says who reads it
  (`serves`): a folder is read by every name that maps to it.
- **`remove --agent codex`** takes the skill folder and the rules file of `codex`, and each says who else reads it (`also read by cursor, pi…`): a shared file is removed
  for all. For `claude`, `AGENTS.md` is only a candidate while that is where `claude`'s block would be written now, so taking back `claude` does not take the block
  the other agents read. `remove` with no `--agent` takes everything V-NOC wrote. The `vnoc` skill folder is removed only when it is empty after its `SKILL.md`; the
  folders above it (`.claude/skills/`) are left.
- **The terminal pick** (stdin and stdout both terminals, neither flag): the twelve names numbered on standard error, `[x]` on the ones found, one line read: Enter takes the
  ticked, or numbers separated by spaces or commas; anything else is exit `2` and nothing is written. Not tried in a pty (the parsing is a unit test).
- **The permission lines** are printed for `claude` and `codex` only, and not with `--global`; §4 has their syntax and its sources.
- **Not done:** the `[check]` above, with an agent that reads both folders, that two skills of the same name show once. It needs an agent in front of it (A14).

## 3. From VS Code

**V-NOC: Install `vnoc` Command in PATH** (`vnoc.installCli`; the way VS Code installs `code`).

| System | What it does |
|---|---|
| macOS, Linux | a symlink `~/.local/bin/vnoc` → `<extension>/bin/vnoc`. Says so if `~/.local/bin` is not on the PATH, with the line to add |
| Windows | a `vnoc.cmd` in `%LOCALAPPDATA%\vnoc\bin` that runs `<extension>\bin\vnoc.exe %*`, and that folder added to the **user** PATH **[check]**: `setx` cuts long PATHs, so write `HKCU\Environment` and broadcast the change |

- The extension's folder name holds its version, so **on every activation**, if the link exists and points into another version's folder, it is
  moved to this one.
- *(Settled 2026-11-20, before A12.)* **What the link points to:** the core binary the extension itself runs (`core.ts`'s choice: the
  `vnoc.server.path` setting, else `<extension>/bin/vnoc` installed, else the repository's `target/debug/vnoc` in a development window),
  as a resolved absolute path. **Who moves it:** only an extension in **production** mode (`ExtensionMode.Production`), and only when the
  link points into **another version's V-NOC extension folder** (`…/<publisher>.vnoc-<version>/bin/vnoc`). A development window (F5) or the
  test suite never moves a link it did not make in that session, and a link that points anywhere else is the user's and is never touched
  (install says so and stops). So a dev window and an installed V-NOC never fight over `~/.local/bin/vnoc`.
- **Where:** `~/.local/bin` (macOS, Linux) or `%LOCALAPPDATA%\vnoc\bin` (Windows); `VNOC_CLI_DIR` overrides it, for the tests and for
  anyone who keeps tools elsewhere.
- In a development window the drivers are not next to `target/debug/vnoc` (they are in `drivers/<language>/target/debug/`), so the linked
  command needs `VNOC_DRIVERS` (or `VNOC_DRIVER` for one) there; install says so in that mode only.
- *V-NOC: Uninstall `vnoc` Command from PATH* removes what the install made, and nothing else.
  *(Narrowed 2026-11-20, review of A12, built in A12b.)* "What the install made" is a link whose target is **a V-NOC extension folder's
  `bin/vnoc`** (any version, the same pattern the mover uses) or **exactly the binary this extension runs** (so a development window can undo
  its own), and our wrapper on Windows. A link to any other program that happens to be named `vnoc` (a person's own build, `cargo install`)
  is theirs: refused and left, as a foreign file is. The refusal says what to do, with the command:
  `V-NOC left ~/.local/bin/vnoc alone: it points to /usr/bin/true, which V-NOC did not make. If you don't need it: rm ~/.local/bin/vnoc,
  then run this command again.` (Windows: `del "<path>"`.) The same sentence shape for a foreign file. Seen by the user, 2026-11-20. `vnoc.cliInstalled` keeps the wider test (any program named `vnoc`): it only says the
  command exists.
- The drivers come along by C8: the binary runs every `vnoc-driver-*` it finds next to its real path.
- Sets the context key `vnoc.cliInstalled` (a walkthrough step completes on it).

**V-NOC: Set Up Coding Agents** (`vnoc.setUpAgents`): asks `vnoc agent list --json` for the names and which are detected, shows them as a
multi-select list with the detected ones ticked, runs `vnoc agent install --agent … --dry-run --json`, shows the files in a confirmation
("V-NOC will write these files: …"), then runs the install and offers to open the rules file. The list, the detection and the files are all
the binary's: the extension only draws the two questions. It **offers itself once**, after
the first doc is attached in a project that has an `AGENTS.md`, `CLAUDE.md`, `.claude/` or `.agents/` (someone here uses an agent); never
silently, never twice. Sets `vnoc.agentsSetUp`.
*(Settled 2026-11-20, before A12.)* "Once" is per workspace (`workspaceState`), and the offer is skipped when `vnoc agent status --json`
already finds V-NOC's files. `vnoc.agentsSetUp` is set on activation from the same status. The command takes an optional argument
`{ agents: string[], confirm: true }` that skips the two questions: for the window suite, like the other commands that take one. It runs the
binary the extension runs, in the workspace's git root; a failure shows the binary's own error line.

*(As built, A12, 2026-11-20.)*

- **Files.** `extension/src/cliInstall.ts` is the pure part (what to make, move, leave or refuse, the PATH advice, the Windows `vnoc.cmd` text and the
  user-PATH edits; no `vscode`, no file system; `cliInstall.test.ts` runs it on each platform × each state of the link's place × each mode).
  `agentsJson.ts` is the pure reading of the binary's `agent … --json` (and the offer's decision; `agentsJson.test.ts` reads fixtures the real binary
  printed, in `extension/src/fixtures/`). `cliSetup.ts` is the impure part: it reads the machine, does what the plans say, and draws the two questions.
  `commands.ts` registers the three commands; `Core` now says which core and driver binary it runs (`binaryPath`, `driverPath`), and the link is made to
  that one.
- **The link** is a symlink made beside a temporary one and renamed over (nobody sees half of it); the target is `path.resolve` of the binary, not its
  `realpath`, so the `…/<publisher>.<name>-<version>/bin/vnoc` shape that identifies another version's link is kept. "Another version's folder" is read
  from `package.json`'s `publisher` and `name`, not written in the code. `vnoc.cliInstalled` is true when the command is there, points to a program named
  `vnoc` (or `vnoc.exe`) and that program exists. Uninstall removes a link to a program named `vnoc`, or our wrapper, and refuses anything else.
- **What the commands answer** (for the window suite; the user sees the messages): `{ done: "create" | "move" | "leave" | "remove" | "refuse", message }`
  for install and uninstall, `{ wrote, left }` for set up (`undefined` when the user stopped at a question or the binary failed).
- **Windows [check], still open:** the wrapper and the user-PATH edit (`reg.exe query/add` on `HKCU\Environment` `Path` as `REG_EXPAND_SZ`, then a
  `WM_SETTINGCHANGE` broadcast through PowerShell) are written and their decisions are unit-tested, but no Windows machine has run them. The window
  suite never installs on Windows (it would write the real user PATH): there it only runs the refusal of a foreign `vnoc.cmd`. Whether install added the
  folder is remembered in `globalState`, so uninstall takes the entry out only then.
- **Set Up Coding Agents** runs the binary in the workspace folder (the binary finds the git root from there; no structure read is made at activation).
  The QuickPick lists the binary's rows: label the name, description `found in this project` for a detected one (ticked), detail `skill: <file> · rules:
  <file>`. The dry run is always made, also with `{ agents, confirm: true }`, which skips the pick and the modal only. A dry run or an install the binary
  exits `2` on because it left a file still prints its JSON: that is read, and the left files are said with their reason. A failure with no JSON shows the
  binary's `error:` line.
- **The offer** is decided by the binary: "somebody here uses an agent" is `agent list --json` finding any agent (its detection, which includes
  `AGENTS.md`, `CLAUDE.md`, `.claude/`, `.agents/` and the other agents' signs), so TypeScript names no folder. **The first attach settles it:** the flag
  `vnoc.agentsOffered` (`workspaceState`) is set at the first attach whether or not anything was shown, so a workspace gets the offer at most once and
  only at its first attach.

## 4. Permissions

Most agents ask the user before running a shell command they have not been allowed. Fifteen prompts for fifteen `vnoc show` calls would end the
experiment. Every agent has its own allow-list; V-NOC's part is to make the list easy to write:

- **The command names split cleanly** (01 §3): the writes are `vnoc init`, `vnoc doc new`, `vnoc doc tag`, `vnoc tag`, `vnoc relink`,
  `vnoc repair`, `vnoc agent`; every other command only reads, and no read starts with a write's name (hence `vnoc tags`, A10a). A prefix rule can allow
  the reads and still ask for the writes. `vnoc agent list` and `vnoc agent status` read, but stay under the one `vnoc agent` prefix, which
  is not allowed by default: setting up agents is the user's step.
- `vnoc agent install` **prints** the lines for the agents picked (it does not write settings files: those are the user's), **only for an agent whose
  syntax was read on 2026-11-19**: Claude Code and Codex.

```
to let Claude Code run V-NOC's read commands without asking, add to .claude/settings.json:
  "permissions": { "allow": ["Bash(vnoc status *)", "Bash(vnoc show *)", "Bash(vnoc code *)", "Bash(vnoc doc list *)", "Bash(vnoc doc read *)",
                             "Bash(vnoc doc path *)", "Bash(vnoc tree *)", "Bash(vnoc find *)", "Bash(vnoc grep *)", "Bash(vnoc calls *)",
                             "Bash(vnoc missing *)", "Bash(vnoc check *)", "Bash(vnoc tags *)", "Bash(vnoc guide *)",
                             "Bash(vnoc skill *)"] }

to let Codex run them without asking, add to ~/.codex/rules/default.rules:
  prefix_rule(pattern = ["vnoc", ["status", "show", "code", "tree", "find", "grep", "calls", "missing", "check", "tags", "guide", "skill"]], decision = "allow")
  prefix_rule(pattern = ["vnoc", "doc", ["list", "read", "path"]], decision = "allow")
```

**Read on 2026-11-19.** *Claude Code* (`code.claude.com/docs/en/permissions`): the rule is `Bash(<command> *)`; the trailing `:*` is "an equivalent way to
write a trailing wildcard" and is not deprecated, but "the permission dialog writes the space-separated form", so that is what is printed. The space
before the `*` is part of the rule (`Bash(ls *)` does not match `lsof`), and `ls *` also matches a bare `ls`, so `Bash(vnoc show *)` does not
match `vnoc showx`. Rules are evaluated deny, then ask, then allow; the first match decides. The pages call Bash rules "not a security boundary".
*Codex* (`learn.chatgpt.com/docs/agent-configuration/rules`; and, raw, `openai/codex` `codex-rs/execpolicy/README.md`):
rules are `prefix_rule(pattern = [...], decision = "allow" | "prompt" | "forbidden")` in a `.rules` file under a `rules/` folder next to a config layer; the user
one is `~/.codex/rules/` (a project's `.codex/rules/` loads only when that project's `.codex/` is trusted, so the user file is what is printed);
a list inside `pattern` means alternatives; the most restrictive match wins; Codex is restarted to read a new file. The page marks rules
**experimental**. `codex execpolicy check --rules <file> -- <command>` tests one (Codex is not installed here, so the printed lines were not run through it).
Other agents' allow-lists: not read, so nothing is printed for them.

**Writing them, when the user says yes** *(decided 2026-11-20, A12c; the user: "give the option, most users want the default")*.

| Run | What happens |
|---|---|
| in a terminal, no flag | after the files, one question: `Let Claude Code run V-NOC's read commands without asking? Writes (doc new, doc tag, relink, repair, init) still ask. [Y/n]`. **Enter = yes** (the default). No → the lines are printed, as before |
| `--permissions` | yes, without the question (scripts, the VS Code command) |
| `--no-permissions` | no, without the question; the lines are printed |
| not a terminal, neither flag | no question, nothing written: the lines are printed (an agent or a script never widens its own permissions) |
| *Set Up Coding Agents* (VS Code) | a checkbox in the confirmation, "Let Claude Code run V-NOC's read commands without asking", **ticked**; it passes `--permissions` or `--no-permissions` |

- **Where:** Claude Code → `.claude/settings.local.json` (only this user; not committed). `--permissions=project` writes the shared
  `.claude/settings.json` instead. Codex → `~/.codex/rules/vnoc.rules` (a file of its own, so nothing of the user's is edited). Other
  agents: printed only, until their format is checked. The question names the agents it covers.
- **Merge, never replace:** JSON is parsed, the missing entries are added to `permissions.allow`, and it is written back with the file's
  indentation; the other keys and the user's own entries stay. A file that does not parse → left alone, exit 2, the lines printed. An entry
  already there is not added twice.
- **`vnoc agent remove` takes back only its own entries** (the exact strings V-NOC adds), and deletes `vnoc.rules` and a
  `settings.local.json` it created and emptied. `vnoc agent status` says whether the permissions are in place.
- Output: `allowed      V-NOC's read commands for Claude Code` / `in           .claude/settings.local.json`.

**As built in A12c (2026-11-20; reviewed).** What the table above left open:

- **The files are written first, then the question.** `n` or Ctrl-C at it leaves the skill and the rules in place; Ctrl-C gives the cursor
  back and exits 130, as at the menu. A **dry run never asks**: in a terminal it shows what the default (yes) would write; with no terminal,
  the printed lines.
- **No question** with `--global`, `--skill-only`, `--rules-only`, `--json`, or when no picked agent has a syntax V-NOC knows. An explicit
  `--permissions` still writes with `--skill-only` / `--rules-only`; with `--global` it is a usage error, and so is `--permissions` with
  `--no-permissions`. `--permissions=local` is the default's own name.
- **The merge** keeps the keys in their order (its own JSON tree; `serde_json::Value` sorts them), the indentation, CRLF, a BOM is read past. One
  cost, accepted: a one-line array comes back one entry per line (keeping it would mean editing the text in place). A repeated key: the last
  one counts, as JSON readers take it.
- **Remove** deletes a settings file only when nothing but `{"permissions":{"allow":[]}}` is left, and `vnoc.rules` only when its first line is
  V-NOC's mark. `agent status` prints one line per agent: `allowed` / `partly` / `changed` (Codex: V-NOC's file says something else) / `not
  allowed`; `--json` has a `permissions` array. `vnoc status`'s embedded agent block does not show it.
- **No "commit these files" line** when the only file written is `settings.local.json` (it is not committed).
- `--json` of install gains `can_allow` (the picked agents V-NOC can allow, by name), so *Set Up Coding Agents* names them without a list of
  its own (P1). The VS Code question is a one-item multi-select, ticked, asked before the confirmation (a modal holds no checkbox), so the
  confirmation lists exactly what will be written. The window suite runs the binary with a temporary `HOME`.

## 5. From the repository

The skill's source is `skills/vnoc/SKILL.md` at the repository's root (06 §3). That one path is also what the wider installers look for, so
these work with no code of ours **[check]** in A13, each once:

| Installer | Command | Gives |
|---|---|---|
| the `skills` CLI (Vercel) | `npx skills add <owner>/<repo>` | the skill in the folders of the agents it finds |
| GitHub CLI | `gh skill install <owner>/<repo>` | the same, with the version pinned |

They install the **skill**, not the command and not the rules block: the skill's first line (`vnoc status`; "if the command is not found…")
is what catches that. `vnoc agent install` stays the recommended way; these are for people who manage all their skills with one tool.

A Claude Code **plugin** (`.claude-plugin/` beside `skills/`) would add hooks; that is [`08`](08-mcp-and-plugins.md), later, and this layout
is already the one it needs.

## 6. From a release

For CI, cloud agents, and people without VS Code.

- `publish.yml` also attaches, per target, `vnoc-<target>.tar.gz` (`.zip` on Windows) holding `vnoc` and `vnoc-driver-python` side by side
  (C8 then finds the driver), with their checksums in the existing `SHA256SUMS` (doc 19 §5). The binaries are the ones `scripts/package.mjs`
  already builds into `extension/bin/` for the VSIX, so nothing is compiled twice.
- **An install script** (`install.sh`, attached to the release): picks the target from `uname`, downloads the archive and `SHA256SUMS`, checks
  the sum, unpacks into `~/.local/bin`. It is what a cloud agent's set-up step or a CI job runs in one line. No Windows script at first (the
  archive and the VS Code command cover Windows).
- The README gets **"Using V-NOC with coding agents"**: install the command (VS Code, or the script), `vnoc agent install`, commit the files;
  then the rules block in full, for people who want only layer 0. *(Moved to A13r, the README rewrite.)*

**Settled 2026-11-20, before A13.**

- **The archive** is made by `scripts/package.mjs`, right after it copies the two programs into `extension/bin/`, so a local run makes it too
  and it is tested before CI. Name `vnoc-cli-<target>.tar.gz` (`vnoc-cli-win32-x64.zip`), the VSIX's target names (`darwin-arm64`, `darwin-x64`,
  `linux-x64`, `win32-x64`); the `-cli` keeps it apart from `vnoc-<target>.vsix` in one release. **Flat**, four files: `vnoc`,
  `vnoc-driver-python` (mode 755), `LICENSE`, `THIRD_PARTY_NOTICES.md` (the programs hold other people's code; the VSIX check of package.mjs
  holds for the archive too).
- **CI:** `package.yml` uploads it as the artifact `cli-<target>`; `publish` downloads `cli-*` beside `vsix-*`, checks there are four of
  each, puts **every file** (four VSIX, four archives, `install.sh`) in one `SHA256SUMS`, attests the archives as well, and uploads them all.
  The `install.sh` it uploads is stamped with the release's version (below).
- **`install.sh` is POSIX `sh`** (it is run as `curl -fsSL … | sh`), no `bash`, no `sudo`, writes only into its install folder.

  | Step | What |
  |---|---|
  | target | `uname -s` / `uname -m`: Darwin arm64 → `darwin-arm64`; Darwin x86_64 → `darwin-x64`, **unless** `sysctl -n sysctl.proc_translated` says 1 (Rosetta: take `darwin-arm64`); Linux x86_64 → `linux-x64`. Anything else: `error: no vnoc build for <os> <arch>; the releases have darwin-arm64, darwin-x64, linux-x64, win32-x64`, exit 1 |
  | version | `VNOC_VERSION` (`0.3.0` or `v0.3.0`), else the version stamped in the script at release (the line `VERSION="@VERSION@"`, replaced by `publish`), else, unstamped (run from the repository), **the newest release, pre-releases included**, from `api.github.com/repos/yaredtsy/vnoc/releases?per_page=1`. Not `releases/latest`: every V-NOC release so far is a pre-release, and `latest` skips them |
  | download | `curl -fsSL` (else `wget -qO-`; neither: say so, exit 1) the archive and `SHA256SUMS` from `${VNOC_BASE_URL:-https://github.com/yaredtsy/vnoc/releases/download}/v<version>/` into a `mktemp -d` folder, removed on exit (`trap`) |
  | check | the archive's line of `SHA256SUMS` against `sha256sum` (else `shasum -a 256`); a missing line or a different sum: `error: …`, exit 1, nothing installed |
  | install | the two programs into `${VNOC_INSTALL_DIR:-$HOME/.local/bin}` (made if missing), each written to a temporary name there and moved into place (a running `vnoc` is not cut off halfway); then `vnoc --version` from there is run and printed |
  | after | if the folder is not on `PATH`: one line to add it (`export PATH="$HOME/.local/bin:$PATH"` in the shell's rc file), not done for the user |

  `VNOC_BASE_URL` also takes a `file://` folder: that is how the test runs it with no network. The one-liner in the README names the release:
  `curl -fsSL https://github.com/yaredtsy/vnoc/releases/download/v0.3.0/install.sh | sh`.
- **macOS:** a file fetched by `curl` gets no quarantine attribute, so Gatekeeper does not stop the programs; the Rust linker signs them ad hoc,
  which Apple silicon needs. The 1.1 s first run (05, "Measured in A0") is measured again on an installed copy in A13: first run against second,
  after a fresh copy. If it is the system's check of a new binary, it is written down (and goes in the README's troubleshooting), not fixed.
