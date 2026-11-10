# Future 02.05 Comparison, the better one, and the path

```
comparison and path
├── 1. side by side
├── 2. which door for which situation
├── 3. the recommendation, and why
├── 4. the path: four layers
├── 5. what to do NOW so this stays cheap     (the only part that touches 0.1–0.3)
├── 6. risks
└── 7. open questions
```

## 1. Side by side

| | **Layer 0: plain files + AGENTS.md** | **CLI + skill** | **MCP server** | **Native plugin** |
|---|---|---|---|---|
| Works in | every agent that reads files | every agent with a shell | every MCP host | one agent each |
| Works with no shell | ✅ | ❌ | ✅ | depends |
| Read docs of a marked node | ✅ | ✅ | ✅ | via CLI / MCP |
| Read docs of an unmarked node | ❌ | ✅ | ✅ | via CLI / MCP |
| One function's exact code | ❌ | ✅ | ✅ | via CLI / MCP |
| Attach a new doc correctly | ❌ | ✅ | ✅ | via CLI / MCP |
| Call trees | ❌ | ✅ slow without a cache | ✅ **fast (warm)** | via CLI / MCP |
| Guards that do not depend on the model (hooks) | ❌ | ❌ | ❌ | ✅ **only here** |
| Context cost when unused | a few lines, always | ~2 lines (skill description) | every tool schema, always | small |
| Context cost per use | file contents | command output | tool output | — |
| Setup for the user | none | get `vnoc` on PATH | one config entry per host | one install per agent |
| Typed arguments, per-tool permissions | — | ❌ | ✅ | depends |
| Composable (pipes, scripts, CI) | — | ✅ | ❌ | ❌ |
| Useful without any agent | — | ✅ `vnoc check` in CI | ❌ | ❌ |
| Work to build | an hour | 1–2 weeks | days, **after** the CLI | an afternoon each, after the CLI |
| Work to keep alive | none | a public interface to keep stable | follow the protocol | follow each agent's API |
| New surface to keep stable | none | commands, flags, `--json` | tool names and schemas | — |

## 2. Which door for which situation

```
Does the agent have a shell?
├── yes → CLI + skill.                                      (Claude Code, opencode, pi, Codex, Gemini CLI…)
│         └── do you want guarantees, not advice?
│               └── yes → add that agent's thin plugin for its hooks.
└── no  → MCP.                                              (desktop chat apps, some IDE agents)

Is the agent inside VS Code with the V-NOC extension?
└── yes → language model tools that talk to the running core.   (the best quality of all: warm, undo, unsaved buffers)

Is the question a call tree, or anything that needs the whole project, asked many times?
└── yes → MCP (warm), or the CLI once there is a SQLite cache.
```

## 3. The recommendation

**Build the CLI first, teach it with one skill, and make every other door a thin wrapper around the same library.**

Why the CLI and not MCP as the base:

1. **V-NOC needs a CLI regardless of agents.** `vnoc check` in CI and pre-commit, scripting, a possible git merge driver
   ([`../01-graph-diff/04-resolving.md`](../01-graph-diff/04-resolving.md) §5), debugging the core without VS Code. The agent use rides on work that
   pays for itself.
2. **Reach.** Every serious coding agent has a shell. Not every one has MCP (pi), and not every one will keep its plugin API.
3. **Context cost.** A skill costs two lines until it is needed. For a tool that matters in *some* turns of *some* sessions, that is the right shape.
4. **The slow part is not here yet.** MCP's real advantage is the warm index for call trees, and call trees are 0.2. Until then one-shot is fast
   enough for the questions that exist.
5. **It forces the right architecture.** A CLI can only be built if the use cases are plain library functions with no editor attached. Once they are,
   MCP is a few days and plugins are afternoons.

Why not *only* the CLI:

- Hosts without a shell exist, and VS Code's own agent is one click away from the extension V-NOC already ships.
- Advice is weaker than a hook. The marker-loss problem (K1) deserves a guarantee where one is available.

Why plugins are last: they multiply maintenance, and under the thin rule ([`04-native-plugins.md`](04-native-plugins.md) §8) nothing is lost by
waiting. Each one is an afternoon whenever a real user of that agent asks.

## 4. The path

| Layer | What | Depends on | Size |
|---|---|---|---|
| **0** | Three paragraphs for `AGENTS.md` / `CLAUDE.md` in the user guide; offer to add them at the first attach | 0.1 | an hour |
| **1a** | `vnoc` read commands: `status` · `show` · `code` · `doc list/read/path` · `tree` · `find` · `check` · `missing` — lazy one-shot, `--json` | 0.1's library with use cases free of the LSP layer | ~1 week |
| **1b** | `vnoc skill` + the skill text · "Install `vnoc` in PATH" command · standalone binaries in releases | 1a | days |
| **1c** | write commands: `doc new` · `relink` | 1a + the two-writers check ([`02-cli-and-skill.md`](02-cli-and-skill.md) §2) | days |
| **2a** | VS Code language model tools from the extension (talks to the running core) | 0.1; independent of the CLI | days |
| **2b** | `vnoc mcp` with the read tools, then `vnoc_new_doc` | 1a–1c | days |
| **2c** | `vnoc calls` / `vnoc_calls` | 0.2 call trees | small once 0.2 exists |
| **3a** | Claude Code plugin: skill + two commands + hooks (session start, after edit) | 1b | an afternoon + testing |
| **3b** | pi extension / opencode plugin: the after-edit check | 1a | an afternoon each, on demand |
| **later** | `vnoc diff`, checkpoints, "what did the agent change" | graph diff phase 1 | — |

```
0.1 ships ──► layer 0 ──► 1a read CLI ──► 1b skill ──► 1c write ──► 2b MCP ──► 3a/3b plugins
                              │                                        ▲
                              └── `vnoc check` in CI (useful alone)    └── 2c call trees when 0.2 exists
                 2a VS Code tools: any time after 0.1, on its own track
```

Stop points: after **1b** V-NOC is usable by every shell agent for reading. After **1c** agents can write docs. Everything beyond that is reach and
comfort.

## 5. What to do now so this stays cheap

Nothing here is new work for 0.1–0.3. It is three habits while writing code that is being written anyway:

| Habit | Where | Why it matters later |
|---|---|---|
| **Handlers in `server.rs` stay thin**: parse the message, call one library function, convert the result | `crates/vnoc/src/server.rs` (already the intent: "one line per message") | that library function is the CLI command and the MCP tool |
| **Use cases do not take LSP types or the client socket.** The attach flow gets "something that can apply an edit" as a parameter, not VS Code itself | `identity/attach.rs` | the CLI passes "write the file on disk"; the extension passes "`workspace/applyEdit`"; tests pass a fake |
| **Index building is callable for one file, one folder, or everything** | `index/build.rs` (already has "update for a list of changed files") | the lazy one-shot in [`02-cli-and-skill.md`](02-cli-and-skill.md) §2 is that same call |

> **🦀 Rust note: passing "something that can apply an edit".** This is the one place where a small trait earns its keep *before* a second
> implementation exists in the product, because the test harness is already a second implementation:
> ```rust
> pub trait EditSink {                       // who puts planned edits into a source file?
>     async fn apply(&self, file: &RepoPath, version: Option<i32>, edits: &[TextEdit]) -> Result<bool, EditError>;
> }
> // extension:  sends workspace/applyEdit to VS Code
> // CLI (later): checks the file hash, writes temp + rename
> // tests:       answers `true` and records what it was given
> ```
> It replaces nothing in the 0.1 design ([`../../05-identity/01-id-generation.md`](../../05-identity/01-id-generation.md) §4); it names the seam that
> design already has.

## 6. Risks

| Risk | What limits it |
|---|---|
| **Agents flood the project with low-value docs** | the skill's first line says what a note is for; writing is on request; no bulk command; review happens in git |
| **Agents drop `ID:` lines when rewriting functions** | the rule in the skill; the after-edit hook where plugins exist; and the existing safety net: the node falls back to "found by name" with *Write ID again*, never to data loss |
| **Prompt injection through doc text** (docs arrive by `git pull`) | docs are returned as plain content, never formatted as instructions; the skill says so; write tools ask for permission in hosts that support it |
| **A public CLI freezes design too early** | marked experimental until 1.0; few commands; only `--json` is a promise |
| **Three doors drift apart** | one library; tool descriptions and the skill generated from one source; a test that runs the same scenario through CLI and MCP and compares |
| **Two cores write at once** (VS Code + CLI/MCP) | atomic writes; node creation by `create_dir`; a test that races the two |
| **Chasing agent APIs** | the thin-plugin rule; build a plugin only when a real user of that agent asks |
| **This pulls attention from 0.1** | the whole folder is unscheduled; §5 is the only part that applies now, and it costs nothing |

## 7. Open questions

| # | Question | Lean |
|---|---|---|
| A1 | Does the CLI write the marker into the source file by itself, print the edit, or skip the marker? | Write it (file unchanged since read, atomic), `--no-marker` to skip. [`02-cli-and-skill.md`](02-cli-and-skill.md) §2 |
| A2 | An `author:` line in doc frontmatter for agent-written docs? | Probably yes, optional and additive. Decide before the first write command ships, because it is a frozen-format addition. |
| A3 | One binary (`vnoc serve` / `vnoc mcp` / `vnoc show`) or a separate `vnoc-cli`? | One. The VSIX stays at two programs. |
| A4 | Should the extension offer to write the `AGENTS.md` lines and install the skill at the first attach? | Offer once, never silently. It edits a file the user owns. |
| A5 | Should `vnoc mcp` forward to VS Code's running core when there is one? | Not at first. Two cores are correct, only wasteful. |
| A6 | Do agents get a way to mark a doc "needs review by a person"? | Wait for real use. Git review covers it. |
| A7 | Which agents do V-NOC's first users actually use? | Ask them. That decides the order inside layer 3, and whether 2a (VS Code tools) comes before 1a. |

Back to the map: [`README.md`](README.md) · the other idea: [`../01-graph-diff/`](../01-graph-diff/README.md)
