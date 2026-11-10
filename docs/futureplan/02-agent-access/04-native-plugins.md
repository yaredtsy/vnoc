# Future 02.04 Option 3: native plugins, one per agent

Everything in this file about other tools is **[check]**: their extension systems change every few months. The shapes are stable; the names are not.

```
native plugins
├── 1. what "native" adds over a CLI or MCP
├── 2. Claude Code       a plugin = skill + commands + hooks + MCP entry, in one install
├── 3. opencode          plugins in TypeScript: tools and event hooks
├── 4. pi                extensions in TypeScript: tools, commands, events, its own UI
├── 5. VS Code itself    language model tools from the V-NOC extension
├── 6. others            Codex · Cursor · Gemini CLI …
├── 7. hooks: the part worth the trouble
└── 8. advantages, disadvantages, and the rule that keeps plugins thin
```

## 1. What "native" adds

A CLI and an MCP server both wait to be called. The model must decide to use them. A native plugin runs **inside the agent's own loop**, so it can do
three things the others cannot:

| Power | Meaning | Example |
|---|---|---|
| **Hooks** | code that runs on the agent's events, whether or not the model thinks of it | after every file edit, check that no `ID:` line was lost |
| **Install and packaging** | one step gives the user the skill, the commands and the configuration | `/plugin install vnoc` |
| **UI** | commands, status lines, pickers inside the agent | `/vnoc-doc` · "3 documented functions in this change" |

Notice what is **not** on the list: new abilities. Reading docs, showing code, attaching a doc are the CLI's or the MCP server's job. A plugin decides
*when* they run and makes them easy to get.

## 2. Claude Code

A plugin is a folder (shared through a git repository that acts as a marketplace) that can bundle:

```
vnoc-plugin/
├── .claude-plugin/plugin.json     name, version
├── skills/vnoc/SKILL.md           the skill from 02-cli-and-skill.md §4
├── commands/                      slash commands:  /vnoc-doc  ·  /vnoc-review-docs
├── hooks/hooks.json               hooks (§7)
└── .mcp.json                      optional: start `vnoc mcp`
```

So for Claude Code, "native plugin" is mostly **packaging of the other two options plus hooks**. There is no V-NOC logic in it: the skill is text, the
commands are prompts, the hooks are shell commands that call `vnoc`.

That makes it cheap, and it is the natural first plugin: the smallest step from "CLI + skill" to "installed in one command, with guard rails".

## 3. opencode

opencode is configured with a JSON file and can load plugins written in JavaScript / TypeScript from the project or the user's config folder. A plugin
can add **custom tools** (a name, a schema, a function) and subscribe to **events** (before / after a tool runs, session start, file edited…). It
also speaks MCP and reads `AGENTS.md`.

What a V-NOC plugin would be: a ~100-line TypeScript file that registers two or three tools which run `vnoc … --json`, and one "after edit" handler
that runs `vnoc check` on the edited file.

Or nothing at all: with MCP configured and the `AGENTS.md` lines in place, opencode already has every ability. The plugin only adds the automatic
check.

## 4. pi

pi is a small terminal agent whose design is "few built-in tools, everything else by extension". Extensions are TypeScript modules that can register
tools and commands, react to events, and draw their own UI in the terminal. It supports skills. It **does not include MCP**, on purpose: its author's
position is that a CLI tool plus a short description costs less context than an MCP server.

That makes pi the clearest case for option 1:

| In pi | How |
|---|---|
| reading and writing V-NOC | the `vnoc` CLI through its shell tool, taught by the skill. **No plugin needed.** |
| automatic checks | a small extension: on "file edited" → `vnoc check <file>` → put the result in front of the model |
| nice extras | a command that opens a picker of documented nodes; a footer line with the Missing count |

If V-NOC had only an MCP server, pi users would get nothing. That is a strong argument for the order in
[`05-comparison-and-path.md`](05-comparison-and-path.md).

## 5. VS Code itself

V-NOC **is** a VS Code extension, and VS Code has its own agent (Copilot's agent mode). An extension can give that agent tools in two ways:

| Way | What it is | For V-NOC |
|---|---|---|
| **Language model tools** | the extension registers a tool (name, description, schema) and implements it in TypeScript | the implementation just sends a request to the **core that is already running**: warm index, unsaved buffers known, edits applied by VS Code with undo |
| **MCP server registration** | the extension tells VS Code "here is an MCP server definition" from code | VS Code starts `vnoc mcp` itself; no user configuration |

The first is the better one here, and it is the only door where **every rule of the editor world still holds**: the marker edit goes through
`workspace/applyEdit` as in the normal attach flow, and there is no second core.

It is also the smallest: the tools are a few lines each in `extension/src/`, on top of messages that already exist (`vnoc/docs`, `vnoc/attach`,
`vnoc/structure`).

## 6. Others

| Agent | What it reads today | What V-NOC needs for it |
|---|---|---|
| Codex CLI | `AGENTS.md`, MCP, skills | nothing new: the skill or the `AGENTS.md` lines, optionally the MCP entry |
| Cursor | project rules, MCP | a rules file with the layer-0 text; the MCP entry |
| Gemini CLI | its own context file, MCP, extensions | the same text; the MCP entry |
| Claude Desktop and other chat apps | MCP only (no shell) | the MCP server is the only door |

The pattern: **text for knowledge, CLI or MCP for ability.** Almost every agent accepts one of each, with no plugin.

## 7. Hooks: the part worth the trouble

The two "keep" jobs from [`01-what-agents-need.md`](01-what-agents-need.md) §1 (K1: do not lose markers; K2: keep docs true) are where advice in a
skill is weakest. A model forgets advice in a long session. A hook does not forget.

| Event | Hook does | Effect |
|---|---|---|
| **Session start** | `vnoc status` → one line into the context | "V-NOC project · 42 documented nodes · 0 problems. Use the vnoc skill before changing documented code." The agent knows from the first turn. |
| **Session start** | `vnoc checkpoint` (graph diff, [`../01-graph-diff/02-how-to-compute.md`](../01-graph-diff/02-how-to-compute.md) §7) | "what did the agent change?" becomes answerable later |
| **After a file edit** | `vnoc check <that file>` | **K1.** If an `ID:` line vanished: the hook's output tells the model at once: "you removed `ID: 0Q8V…` from `parse_order`; put it back". Fixed in the same turn, while the old text is still in context. |
| **After a file edit** | `vnoc show` for the functions that changed and have docs | **K2.** "The function you edited has 2 docs: … Check they are still true." |
| **Before finishing** | `vnoc check` + `vnoc diff` | a closing summary: "changed 3 documented functions; docs updated for 2". |
| **Before a file edit** (blocking) | refuse edits under `.vnoc/nodes/**/node.toml` | the agent cannot hand-edit V-NOC's own files |

Rules for hooks, learned the hard way by everyone who writes them:

- **Fast.** A hook runs on every edit. `vnoc check <one file>` must take well under a second, which the lazy one-shot design gives
  ([`02-cli-and-skill.md`](02-cli-and-skill.md) §2).
- **Quiet when fine.** Print nothing if there is nothing to say. Every line a hook prints costs context on every edit.
- **Never block on its own failure.** If `vnoc` is not installed or crashes, the hook exits cleanly. V-NOC must not be the reason an agent session
  breaks.
- **Opt-in.** Hooks run commands on the user's machine; they are installed by a clear user action, never by cloning a repo.

## 8. Advantages, disadvantages, and the rule

| Advantages | Disadvantages |
|---|---|
| **Hooks**: the right thing happens without relying on the model | **One per agent.** N agents = N small codebases, in each agent's language and API |
| **One-step install** of skill + commands + config | **Churn**: these APIs are young and change; each change is a maintenance task with no user-visible gain |
| **UI** inside the agent | Users of agents you did not write a plugin for get nothing extra |
| In VS Code: uses the running core, the only door with editor-quality edits | Testing needs each agent installed; hard to automate |
| Can be tuned to each agent's strengths | Tempting to put logic in the plugin, which then exists N times |

**The rule that keeps this affordable:**

> A plugin contains **configuration, prompt text, and calls to `vnoc`**. Nothing else. If a plugin needs to *know* something about `.vnoc/`, IDs or
> the index, that knowledge is missing from the CLI, and that is where it goes.

Under that rule a plugin is 50–150 lines and an afternoon of work, and losing one to an API change costs an afternoon.

Next: [`05-comparison-and-path.md`](05-comparison-and-path.md)
