# Future 02.03 Option 2: an MCP server

```
MCP server
├── 1. what MCP is, in five lines
├── 2. why it fits the core's shape
├── 3. what V-NOC would offer
│   ├── tools
│   ├── resources
│   └── prompts
├── 4. the warm index: the real advantage
├── 5. how a host is told about it
├── 6. building it in Rust
└── 7. advantages and disadvantages
```

## 1. What MCP is

The Model Context Protocol is a standard way for an agent host (Claude Code, Claude Desktop, Cursor, VS Code, opencode…) to talk to a helper program.

- The host **starts the server** as a child process and talks to it over stdin / stdout (or over HTTP for remote servers).
- The messages are **JSON-RPC**: `initialize`, then `tools/list`, `tools/call`, and a few more.
- The server offers **tools** (functions the model may call, each with a name, a description and a JSON Schema for its arguments), **resources**
  (things to read, addressed by URI) and **prompts** (ready-made requests the user can pick).
- The host shows the tools to the model. The model calls one; the host asks the user for permission if needed and passes the call on.
- The server lives **as long as the session**.

## 2. Why it fits the core's shape

Compare with what `vnoc` is today:

| | The V-NOC core today | An MCP server |
|---|---|---|
| Started by | the extension, as a child process | the agent host, as a child process |
| Transport | stdin / stdout | stdin / stdout |
| Messages | JSON-RPC (LSP framing) | JSON-RPC (one JSON object per line) |
| Lifetime | the window | the session |
| State | the in-memory index, kept fresh | free to keep any state |
| stdout discipline | `#![deny(clippy::print_stdout)]` | needed for the same reason |

It is the same kind of program. `vnoc mcp` would be a second router beside `server.rs`: different method names and framing, the same use-case
functions behind them. Everything good in [`../../01-review.md`](../../01-review.md) §1 about the core (stdout is the protocol, unknown messages never
kill it) carries over as it is.

## 3. What V-NOC would offer

### Tools

Few, on purpose (§7 explains the cost of each one). They mirror the CLI commands:

| Tool | Arguments | Returns | CLI twin |
|---|---|---|---|
| `vnoc_show` | `node`, `include_code?` | header, docs list, code with line numbers | `vnoc show` |
| `vnoc_read_doc` | `node`, `name?` | the doc text and its file path | `vnoc doc read` |
| `vnoc_tree` | `path?`, `depth?` | structure with doc counts | `vnoc tree` |
| `vnoc_find` | `query` | nodes and docs that match | `vnoc find` |
| `vnoc_calls` | `node`, `direction`, `depth?` | the call tree (after 0.2) | `vnoc calls` |
| `vnoc_new_doc` | `node`, `title`, `body` | the new doc's path, the ID, whether a marker was written | `vnoc doc new` |
| `vnoc_check` | `paths?` | problems | `vnoc check` |

`node` accepts the same forms everywhere ([`01-what-agents-need.md`](01-what-agents-need.md) §2). Read tools are marked read-only in their
annotations so hosts can allow them without asking **[check the annotation names]**.

### Resources

Docs are a natural fit: each one is a document with an address.

```
vnoc://node/0Q8V5W4T2F7KX9M3B6D1RZ4H7K                     the node: location, state, list of docs
vnoc://node/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/why-we-parse-twice  one doc
```

In hosts that support it, the user can attach a resource to a message by hand ("@ the doc"). Useful, but hosts differ a lot in how well they support
resources; tools are the part to rely on **[check]**.

### Prompts

Ready-made requests that show up in the host's command list:

- **Document this function**: "read the code of `{node}` and its callers; write a V-NOC note about why it exists and what to watch out for".
- **Review docs after my change**: "for every changed function that has docs, check the docs are still true".

Small work, pleasant result. The same two could be slash commands in a plugin ([`04-native-plugins.md`](04-native-plugins.md)).

## 4. The warm index: the real advantage

This is the one thing MCP gives that a one-shot CLI cannot, and it follows from the lifetime:

```
CLI        start → build what is needed → answer → exit                    every call pays again
MCP        start → build the index once → answer, answer, answer…          later calls are instant
```

It matters for exactly the expensive questions:

| Question | CLI (no cache) | MCP |
|---|---|---|
| docs / code of one node | fast | fast |
| search, Missing, tree of the whole project | a full build per call | built once |
| **call trees** | the driver's whole-project analysis per call | analysed once, then cheap to expand |

So the honest rule is: **until call trees exist (0.2), MCP has no speed advantage worth its cost. After they exist, it has a real one.**

Keeping the index fresh without VS Code's watcher: the MCP server checks file stamps (mtime + size, already in `FileEntry`) for the files a call
touches, or watches the folder itself with the `notify` crate **[check: prefer stamps first, simpler]**.

One more thing to settle: if VS Code is also open, there are now **two cores** with two indexes on one project. Reading is harmless. Writing is the
"two writers" case, already covered in [`02-cli-and-skill.md`](02-cli-and-skill.md) §2. Memory is doubled. A later refinement is for `vnoc mcp` to
forward to the running core when it finds one; do not start there.

## 5. How a host is told about it

Every host has its own configuration file, but the content is the same three facts: a name, a command, arguments.

```json
{ "mcpServers": { "vnoc": { "command": "vnoc", "args": ["mcp"] } } }
```

Claude Code reads a project-level `.mcp.json` like this, so committing that file gives the tools to everyone on the team who uses it. Other hosts use
their own file and key names **[check each]**. A `vnoc mcp --install <host>` command could write the right one.

Two hosts need **no** configuration, because V-NOC is already inside them:

- **VS Code's own agent mode.** An extension can register an MCP server (or plain "language model tools") from code, so installing the V-NOC
  extension would make the tools appear in Copilot's agent mode by itself. See [`04-native-plugins.md`](04-native-plugins.md) §5.
- **A Claude Code plugin** can carry the MCP entry in its bundle (same file, §2 there).

The server finds the project the same way the CLI does: walk up from the working directory to a `.vnoc/`, else the git root.

## 6. Building it in Rust

Two ways:

| | The official Rust SDK (`rmcp`) **[check name and state]** | By hand |
|---|---|---|
| Work | derive macros turn functions into tools with schemas | about five methods: `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, plus `ping` |
| Risk | a dependency that moves with a young protocol | you track the protocol yourself |
| Fits V-NOC | yes | yes: the core already has a JSON-RPC loop and serde types |

Lean: the SDK, in its own small module (`crates/vnoc/src/mcp.rs`), behind a Cargo feature so the extension's build does not need it. If the SDK
becomes a burden, the hand-written version is small enough to switch to.

The tool descriptions are prompt text. They deserve the same care as the skill: say *when* to call the tool, not only what it returns. Keep one
source for that text, shared with `vnoc skill`.

## 7. Advantages and disadvantages

| Advantages | Disadvantages |
|---|---|
| Works in hosts with **no shell** (desktop chat apps, some IDE agents) | **Context cost**: every tool's name, description and schema sits in the model's context for the whole session, used or not. Seven tools are a few hundred tokens; this is why the list is short. (Some hosts now load tool definitions on demand **[check]**.) |
| **Warm index**: instant answers after the first; the only good home for call trees | A long-running process per agent session, with its own memory, beside VS Code's core |
| **Typed arguments**: the host checks them against the schema before the call | **Per-host configuration**; hosts differ in which parts of MCP they support |
| **Per-tool permissions**: "always allow `vnoc_show`, ask for `vnoc_new_doc`" | Not composable: no pipes, no `grep` over the result |
| Discoverable: the model sees the tools without being taught | Harder to debug than a command you can run yourself |
| A standard: one server, many hosts | Not every agent speaks it: pi leaves MCP out on purpose **[check]** |
| Resources and prompts come almost free | A young protocol that still changes |

MCP is the better door when the host has no shell or when the question needs a warm index. It is the worse door for quick single-node reads in a
terminal agent, where a CLI call costs less context and no setup.

Next: [`04-native-plugins.md`](04-native-plugins.md)
