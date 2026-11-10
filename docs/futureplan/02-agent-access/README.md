# Future 02. Agent access: letting coding agents use V-NOC

**Status: idea only. Not scheduled.** The first useful step (§4, layer 0) costs nothing and works the day 0.1 ships.

```
agent access
├── README.md                   this file: the idea, the three options, the answer in short
├── 01-what-agents-need.md      the jobs: read docs · read code · call tree · write docs
├── 02-cli-and-skill.md         option 1: a `vnoc` command + a skill file that teaches it
├── 03-mcp-server.md            option 2: an MCP server
├── 04-native-plugins.md        option 3: a plugin per agent (Claude Code, opencode, pi, VS Code…)
└── 05-comparison-and-path.md   side by side, which is better when, and the order to build
```

## 1. The idea

A coding agent (Claude Code, opencode, pi, Codex, Cursor…) works on a project that has a `.vnoc/` folder. Today it does not know that folder means
anything. It should be able to:

```
READ    "what docs does this function have?"        → the notes a person wrote about it
        "show me this function"                      → exactly its code, not the whole file
        "who calls it? what does it call?"           → the call tree (after 0.2)
WRITE   "attach a doc to this function"              → a new .md in .vnoc/, with the ID and node.toml done correctly
        "this doc is out of date, fix it"            → edit the .md
KEEP    do not break what is there                   → do not delete ID: lines while rewriting code
```

V-NOC is useful to an agent for the same reason it is useful to a person: it holds the **why** that is not in the code, attached to the exact thing it
is about, and it can hand over **one function** instead of a 2,000-line file.

## 2. Why this is easy to start and worth doing well

| Fact about V-NOC | What it means for agents |
|---|---|
| Docs are **plain Markdown files** in the repo | an agent can already read them with its normal file tools |
| The ID is **written in the code** (`ID: 0Q8V…`) | an agent that sees the line can compute the doc folder's path by itself |
| The core is a **separate program** that speaks JSON-RPC | giving it a second front door (a CLI, an MCP server) does not need VS Code |
| The outline gives **exact ranges** | "give me this function" is a cheap, precise answer that saves the agent tokens |
| `.vnoc/` has **strict writing rules** | writing must go through V-NOC's code, not through an agent's guess. This is the part that needs a tool. |

## 3. The three options

```
                         what it is                                    who starts it         works in
1. CLI + skill           a `vnoc` command; a SKILL.md explains         the agent, through    every agent that has a shell
                         when and how to call it                       its shell tool
2. MCP server            `vnoc mcp`: a long-running process that       the agent host        every MCP host, including ones
                         offers typed tools over a standard protocol                         with no shell
3. Native plugin         code or config written for ONE agent's        that agent            that agent only
                         own extension system (hooks, tools, UI)
```

They are **not rivals**. They are three front doors to the same rooms. The real decision is which door to build first, and how to make sure all the
logic lives behind the doors and not in them.

## 4. The answer in short

```
layer 0   plain files + a few lines in AGENTS.md         free · read-only · day one of 0.1
layer 1   `vnoc` CLI  +  one skill                       ← build this first
layer 2   `vnoc mcp`                                     the same functions as typed tools · when a host has no shell
layer 3   thin plugins                                   packaging + hooks that call the CLI · one agent at a time, on demand
```

- **CLI first**, because V-NOC needs a command-line program anyway (CI checks, scripts, the merge-driver idea in
  [`../01-graph-diff/04-resolving.md`](../01-graph-diff/04-resolving.md)), every shell agent can use it, and it costs the agent almost no context.
- **MCP second**, as a thin wrapper, for hosts that cannot run commands and for questions that want a warm index (call trees).
- **Plugins last and thin.** Their one unique power is **hooks**: making the right thing happen without relying on the agent to think of it.

The reasoning is in [`05-comparison-and-path.md`](05-comparison-and-path.md).

## 5. One architecture rule that makes all three cheap

```
                    ┌──────────────── crates/vnoc (library) ────────────────┐
                    │  use cases:  docs_of · show_node · attach_doc · calls  │
                    │              find · missing · relink · diff · check    │
                    └───────▲──────────────▲──────────────▲─────────────────┘
                            │              │              │
                     LSP server        CLI commands    MCP tools
                    (the extension)    (`vnoc show`)   (`vnoc mcp`)
                                           ▲              ▲
                                     skill, plugins    MCP hosts
                                     and hooks call it
```

**Every front door is a thin translation to the same use-case functions.** No door has logic of its own. `crates/vnoc` already has a `lib.rs`, and
the `userdata` module is already designed as "use cases, not file paths" ([`../../04-storage/03-writing-safely.md`](../../04-storage/03-writing-safely.md)
§5). Keeping to that is the whole cost of being ready for this idea.

Next: [`01-what-agents-need.md`](01-what-agents-need.md)
