# 21.07 Installing: the skill, the command, the releases

```
installing
├── 1. where each agent looks        read on 2026-11-16; check again on the day
├── 2. vnoc agent install            what it writes, and what it never overwrites
├── 3. from VS Code                  Set Up Coding Agents · Install vnoc Command in PATH
├── 4. permissions                   reads without a prompt
├── 5. from the repository           other installers, for free
└── 6. from a release                archives · an install script
```

Three things have to be true before an agent can use V-NOC: the **command** is on its PATH, the **skill** is where it looks, and the **rules**
are in the file it always reads. One VS Code command does the first; one `vnoc` command does the other two, for the agents the user picks.

## 1. Where each agent looks

Read from each agent's own documentation on 2026-11-16, through a tool that summarises pages: **treat every cell as `[check]`** and read it
again in step A11. The shape is what matters, and it is stable: two folders cover everyone.

| Agent | Project skill folders | User skill folders | Always-read file |
|---|---|---|---|
| **Claude Code** | `.claude/skills/` | `~/.claude/skills/` | `CLAUDE.md`; `AGENTS.md` when there is no `CLAUDE.md` |
| **Codex** | `.agents/skills/` | `~/.agents/skills/` | `AGENTS.md` |
| **Cursor** | `.agents/skills/`, `.cursor/skills/`, also `.claude/skills/` | `~/.agents/skills/`, `~/.cursor/skills/`, `~/.claude/skills/` | `AGENTS.md`, `.cursor/rules/` |
| **pi** | `.agents/skills/`, `.pi/skills/` | `~/.agents/skills/`, `~/.pi/agent/skills/` | `AGENTS.md`, `CLAUDE.md` |
| **opencode** | `.agents/skills/`, `.opencode/skills/`, `.claude/skills/` | `~/.agents/skills/`, `~/.config/opencode/skills/`, `~/.claude/skills/` | `AGENTS.md`, else `CLAUDE.md` |
| **Gemini CLI** | `.agents/skills/` (or `.gemini/skills/`) | `~/.agents/skills/`, `~/.gemini/skills/` | `GEMINI.md` |
| **Copilot** (CLI, VS Code) | `.agents/skills/`, `.github/skills/`, `.claude/skills/` | `~/.agents/skills/`, `~/.copilot/skills/` | `AGENTS.md`, `.github/copilot-instructions.md` |
| Amp, Goose, Windsurf | `.agents/skills/` (and others) | `~/.agents/skills/` | `AGENTS.md` |
| Cline | `.claude/skills/`, `.cline/skills/` | `~/.cline/skills/` | `AGENTS.md`, `.clinerules/` |

**So: `.agents/skills/vnoc/` and `.claude/skills/vnoc/`.** The first is read by every agent above but Claude Code and Cline; the second by
those two (and by several of the others as well). No agent needs a third folder. Which of the two is written is the user's pick (§2).

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
| `claude` | `.claude/skills/vnoc/` | `CLAUDE.md` if there is one (unless it imports `@AGENTS.md`), else `AGENTS.md` |
| `cline` | `.claude/skills/vnoc/` | `AGENTS.md` |
| `codex` `cursor` `pi` `opencode` `copilot` `amp` `goose` `windsurf` | `.agents/skills/vnoc/` | `AGENTS.md` |
| `gemini` | `.agents/skills/vnoc/` | `GEMINI.md` (created if missing: Gemini CLI reads only that by default) |
| `other` | `.agents/skills/vnoc/` | `AGENTS.md` |

Picking three agents that share a folder writes that folder once. `--all` is every row: both folders, and each rules file that exists (plus
`AGENTS.md`). An agent that is not in the table is `other`, and gets what the open format says; a new agent, or one that moves, is a row in
this table and never a new mechanism.

**How the pick is made:**

| Run | The pick |
|---|---|
| with `--agent …` or `--all` | those |
| in a terminal, with neither | a numbered list of the names, **the detected ones ticked**; Enter takes the ticked ones, or type the numbers. No new dependency: one line read from standard input |
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
- `status`: for each file V-NOC wrote, `current`, `older (0.5.0 → 0.6.0: vnoc agent install)`, `changed by hand`; and which agents that
  amounts to. Nothing is stored about the pick: the files are the record. `vnoc status` shows the same in one line.
- `remove`: deletes the skill folders V-NOC wrote and takes the block out of the files, for the agents named or for all; nothing else.
- All of it is file work with no driver: it is instant, and it is library code (`crates/vnoc/src/agents.rs`) the extension's command calls
  through the binary (P1).

Some agents read **both** skill folders (Cursor, opencode, Copilot). Picking `claude` and `cursor` puts the skill in both. **[check]** in A11
with one of them that two skills of the same name show as one; if one shows twice, `.claude/skills/vnoc` becomes a link to the other where the
system allows it.

`vnoc skill` only prints `SKILL.md` to standard output, for anyone who wants to put it somewhere this command does not.

## 3. From VS Code

**V-NOC: Install `vnoc` Command in PATH** (`vnoc.installCli`; the way VS Code installs `code`).

| System | What it does |
|---|---|
| macOS, Linux | a symlink `~/.local/bin/vnoc` → `<extension>/bin/vnoc`. Says so if `~/.local/bin` is not on the PATH, with the line to add |
| Windows | a `vnoc.cmd` in `%LOCALAPPDATA%\vnoc\bin` that runs `<extension>\bin\vnoc.exe %*`, and that folder added to the **user** PATH **[check]**: `setx` cuts long PATHs, so write `HKCU\Environment` and broadcast the change |

- The extension's folder name holds its version, so **on every activation**, if the link exists and points into another version's folder, it is
  moved to this one.
- *V-NOC: Uninstall `vnoc` Command from PATH* removes what the install made, and nothing else.
- The driver comes along by C8: the binary looks next to its real path.
- Sets the context key `vnoc.cliInstalled` (a walkthrough step completes on it).

**V-NOC: Set Up Coding Agents** (`vnoc.setUpAgents`): asks `vnoc agent list --json` for the names and which are detected, shows them as a
multi-select list with the detected ones ticked, runs `vnoc agent install --agent … --dry-run --json`, shows the files in a confirmation
("V-NOC will write these files: …"), then runs the install and offers to open the rules file. The list, the detection and the files are all
the binary's: the extension only draws the two questions. It **offers itself once**, after
the first doc is attached in a project that has an `AGENTS.md`, `CLAUDE.md`, `.claude/` or `.agents/` (someone here uses an agent); never
silently, never twice. Sets `vnoc.agentsSetUp`.

## 4. Permissions

Most agents ask the user before running a shell command they have not been allowed. Fifteen prompts for fifteen `vnoc show` calls would end the
experiment. Every agent has its own allow-list; V-NOC's part is to make the list easy to write:

- **The command names split cleanly** (01 §3): the writes are `vnoc init`, `vnoc doc new`, `vnoc relink`, `vnoc repair`, `vnoc agent`; every
  other command only reads. A prefix rule can allow the reads and still ask for the writes.
- `vnoc agent install` **prints** the lines for the agents picked (it does not write settings files: those are the user's):

```
to let Claude Code run V-NOC's read commands without asking, add to .claude/settings.json:
  "permissions": { "allow": ["Bash(vnoc status:*)", "Bash(vnoc show:*)", "Bash(vnoc code:*)", "Bash(vnoc doc list:*)", "Bash(vnoc doc read:*)",
                             "Bash(vnoc doc path:*)", "Bash(vnoc tree:*)", "Bash(vnoc find:*)", "Bash(vnoc grep:*)", "Bash(vnoc calls:*)",
                             "Bash(vnoc missing:*)", "Bash(vnoc check:*)", "Bash(vnoc guide:*)"] }
```

**[check]** in A11 the rule syntax of Claude Code and Codex on the day; print only what was checked.

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
  then the rules block in full, for people who want only layer 0.
