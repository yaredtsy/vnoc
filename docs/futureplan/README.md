# Future plan: ideas that are not scheduled

Written 2026-11-10. **Nothing in this folder is being built.** None of it is in a release in [`../02-mvp-and-releases.md`](../02-mvp-and-releases.md),
and none of it should delay 0.1–0.3. The folder exists so the ideas are written down once, with what is possible, what each option costs, and which one
I would pick, instead of being re-thought every time they come up.

```
futureplan
├── 01-graph-diff/        see what changed as FUNCTIONS and as a GRAPH, not as files and lines
│   ├── the idea          "parse_order was updated, old_helper was removed", on the tree and the canvas
│   ├── how               two outlines of two versions, matched node by node
│   └── resolving         revert / keep / merge one function at a time        (design only)
│
└── 02-agent-access/      let coding agents (Claude Code, opencode, pi, Codex…) use what V-NOC knows
    ├── the idea          an agent reads docs, function code and call trees, and writes docs
    ├── the options       CLI + skill · MCP server · native plugin per agent
    └── the choice        one core, a CLI first, the others as thin wrappers
```

## The two ideas in one paragraph each

**Graph diff.** Git shows changes per file and per line. V-NOC already knows where every function starts and ends, and (for documented nodes) has an ID
that survives renames. So it can answer a better question: *which functions were added, removed, updated or moved between two versions?* It can show that
on the tree and the canvas, and later let the user fix things one function at a time ("keep my version of this function, take theirs for that one").
Git stays the version control system. V-NOC is a different **view** of what git stores. → [`01-graph-diff/`](01-graph-diff/README.md)

**Agent access.** A coding agent that works in a V-NOC project should be able to ask "what docs does this function have?", "show me this function's code",
"who calls it?", and to write a doc itself. There are three ways to offer that: a command-line program plus a skill file that teaches the agent to use it,
an MCP server, or a plugin written for each agent. → [`02-agent-access/`](02-agent-access/README.md)

## How the two meet

They are separate, but they help each other:

- "What did the agent change in this session?" is a graph diff between two moments. It is the most useful review screen for agent work.
- A `vnoc diff` command gives an agent a short, exact summary of a change ("3 functions updated, 1 removed") in place of a long line diff.
- Both need the same new thing in the core: a way to run **without VS Code**. See [`02-agent-access/02-cli-and-skill.md`](02-agent-access/02-cli-and-skill.md) §2.

## What these ideas must not break

The rule from [`../02-mvp-and-releases.md`](../02-mvp-and-releases.md) §1 still holds: only what users commit is frozen. Both ideas were checked against it.

| Idea | Adds to frozen formats | Notes |
|---|---|---|
| Graph diff | **nothing** | A diff is computed from git and the code. It belongs in the index (place C in [`../04-storage/README.md`](../04-storage/README.md)). |
| Agent access | at most one optional frontmatter key in docs (`author`) | Agents write the same `.vnoc/` files, through the same writer. |

## Conventions

Same as the rest of `docs/`: every file starts with a small tree, then goes branch by branch. **[check]** means I believe it but did not verify it.
Facts about other tools (Claude Code, opencode, pi, MCP) change quickly; treat every one of them as **[check]** on the day you start building.
