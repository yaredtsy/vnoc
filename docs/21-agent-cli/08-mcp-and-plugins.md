# 21.08 MCP, plugins and callers: postponed

```
postponed
├── 1. MCP             what it would add here, what it would not, what brings it back
├── 2. plugins         hooks are the one thing worth having; the first one to build
├── 3. callers         why there are none, and the ways to get them
├── 4. VS Code's own agent
└── 5. the order after 0.3
```

The designs are in [`../futureplan/02-agent-access/`](../futureplan/02-agent-access/README.md) (03 MCP, 04 plugins) and still stand. This file
only records the decision for 0.3 (C15) and its conditions.

## 1. MCP

| MCP would add | Is that needed for 0.3? |
|---|---|
| a way in for hosts **with no shell** (desktop chat apps) | no agent on the list lacks a shell: Claude Code, Codex, Cursor, pi, opencode, Gemini CLI, Copilot |
| a **warm index**: build once, answer many times | the only real gain. A0 measures the one-shot; the outline cache (05 §3, step A16) gets most of it for every command, MCP or not. `calls` is the one command a cache does not help |
| **typed arguments** and **per-tool permission** | the command names already split reads from writes for prefix rules (07 §4) |
| the model **sees the tools** without being taught | the skill's description does that job for 100 tokens; and "seen" is not "preferred" |

| MCP would cost | |
|---|---|
| a second front door to keep equal to the first | tool names and schemas become a public interface beside the command line's |
| a long-running process per agent session, beside VS Code's core | two cores, two indexes, on one project |
| per-host configuration | a different file and key in each agent |

Two things written in the futureplan have changed since: **pi has MCP now** (its docs show a native client), and several hosts load tool
definitions **on demand**, so "every schema sits in the context" is weaker than it was. Neither changes the order: what MCP would give V-NOC is
still the warm index, and that has a cheaper answer first.

**What brings it back** (any one):

1. after A16, `calls` or whole-project questions are still too slow for an agent to use in a loop **[measure]**;
2. a real user needs a host with no shell;
3. V-NOC's own tools in VS Code's agent (§4) are being built anyway: then one router serves both.

When it comes, it is `vnoc mcp`: a second router beside `server.rs`, over the same library functions the commands call (C2 is what keeps that
a matter of days). Tools mirror the commands; the descriptions come from the same source as `vnoc guide`.

## 2. Plugins

A plugin gives one agent what a command and a skill cannot: **hooks**, code that runs on the agent's own events whether or not the model thinks
of it. For V-NOC there is exactly one hook worth the trouble:

```
after the agent edits a file   →   vnoc check --changed --quiet   →   if an ID line was lost, put the message in front of the model
```

That turns "keep the `ID:` line" from advice into a guarantee. It is the **first thing to build after 0.3**, for Claude Code, because the pieces
are all here: `check --changed` (03 §9), exit code `1`, `--quiet`, and the skill at `skills/vnoc/` so a `.claude-plugin/` beside it is an
addition. An afternoon and its testing (futureplan 02.04 §2, §7).

Other agents' plugins: on demand, one at a time, when a user of that agent asks.

## 3. Callers

"Who calls this?" is not in 0.3, and not because nobody thought of it. A call tree is **solved lazily, from one entry**: the driver opens the
project and follows the calls *out of* the function it was given (`drivers/python/src/call_tree.rs`, `call_wire` with one `Entry`). Nothing
holds the edges of functions nobody asked about, so there is no map to read backwards.

| Way | How | Cost | Finds |
|---|---|---|---|
| **A. by text** (0.3: `vnoc grep 'name\('`) | the functions whose code mentions the name | none | too much (another `save`) and too little (a call through an alias); said to be "by text" in the guide |
| **B. text, then solve** (the way Jedi finds references) | A's candidates, widened to every function in a file that mentions the name; for each, ask the driver for its tree one level deep (what `vnoc/hasCalls` already does) and keep those whose callees include the target | one solve per candidate, capped (say 50), cancellable. No new driver feature | exactly the callers among the candidates. Misses a call that never spells the name anywhere in its file (a callback passed around, `getattr`) |
| **C. index eagerly** | solve every function once, keep the edges both ways, keep them fresh | the whole-project analysis at start-up and on change; a place to keep it (the SQLite of `04-storage/04-growth-path.md`) | everything the solver can see |

**B is the next step when callers are wanted**, as `vnoc calls <node> --callers` and later a card on the canvas: it is built from pieces that
exist, it is honest about what it is (resolved, among candidates), and it needs no decision about storage. C is the real feature and belongs
with the storage work, not with the command line. **[measure]** before B: one depth-1 solve on a warm project, times 50.

## 4. VS Code's own agent

The extension could give Copilot's agent mode tools that talk to the **running** core (futureplan 02.04 §5): warm, with undo, aware of unsaved
files. It is the best quality of all the doors and it is independent of this folder. Not scheduled; it shares nothing with the command line but
the library functions, which C2 keeps ready.

## 5. The order after 0.3

```
0.3 ships ──► the Claude Code plugin with the after-edit hook        (an afternoon; a guarantee for K1)
          ──► the walkthrough                                        (your next piece; README §5 lists what it stands on)
          ──► callers the Jedi way (§3 B), when someone asks "who calls this" and grep is not enough
          ──► MCP, only on one of §1's three conditions
          ──► more plugins, only when asked
```
