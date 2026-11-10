# Future 02.01 What agents need

```
what agents need
├── 1. the jobs
│   ├── read      docs · one function's code · structure · call tree · changes
│   ├── write     a new doc · an edit to a doc · relink
│   └── keep      not breaking markers and docs while editing code
├── 2. how an agent names a node
├── 3. what a good answer looks like (for a model, not a person)
├── 4. agents that write docs: the rules
└── 5. layer 0: what works with no tool at all
```

## 1. The jobs

| # | Job | Example request | Needs from V-NOC | Available from |
|---|---|---|---|---|
| R1 | **Read the docs of a node** | "before changing `parse_order`, read what was written about it" | node → ID → the `.md` files | 0.1 |
| R2 | **Read one function's code** | "show me `OrderService.validate`" | the outline's range for that node | 0.1 |
| R3 | **See the structure** | "what is in `src/billing/`?" with doc counts | the tree | 0.1 |
| R4 | **Find** | "which nodes have docs that mention *retry*?" · "where is ID `0Q8V…`?" | search over docs; marker lookup | 0.1 (simple) |
| R5 | **Call tree** | "who calls `parse_order`?" · "what does it call, two levels deep?" | driver call trees | 0.2 |
| R6 | **What changed** | "which functions did I change in this session?" | graph diff ([`../01-graph-diff/`](../01-graph-diff/README.md)) | future |
| R7 | **Health** | "are any docs Missing? did I remove an ID?" | resolve all nodes | 0.1 |
| W1 | **Attach a new doc** | "write down why this function parses twice" | the attach flow: ID, marker, `node.toml`, doc file | 0.1 |
| W2 | **Edit a doc** | "the doc is out of date after my change, update it" | the doc's path. The edit itself is a plain file edit. | 0.1 |
| W3 | **Relink** | "I split this function; the doc belongs to the new one" | Relink | 0.1 |
| K1 | **Keep markers** | the agent rewrites a function and must keep its `ID:` line | knowledge (a rule in the skill) + a check afterwards | 0.1 |
| K2 | **Keep docs true** | the agent changes a documented function | a reminder that docs exist for it | 0.1 |

R1, R2, W1 and K1 are the core. A first version with only those four is already worth installing.

**The two jobs unique to agents are K1 and K2.** A person using VS Code sees the doc badge and the ID line. An agent sees only text, and will happily
replace a whole function, docstring and `ID:` line included. V-NOC survives that (the node becomes "found by name" and offers *Write ID again*,
[`../../05-identity/05-finding-nodes-again.md`](../../05-identity/05-finding-nodes-again.md) §2), but it is better if it does not happen.

## 2. How an agent names a node

An agent knows a node in different ways at different moments. Accept all of them everywhere:

| Form | Example | When the agent has it |
|---|---|---|
| **location** | `src/orders.py::OrderService.parse_order` | it read the code and knows the name. Same text as the `id` in `vnoc/structure`. |
| **file + line** | `src/orders.py:142` | it has a line number from a search result, a stack trace or a compiler error. The node is the innermost def that contains the line. |
| **ID** | `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` | it saw an `ID:` line, or an earlier answer gave it |
| **file** / **folder** | `src/orders.py` · `src/billing/` | for file and folder docs |

*File + line* matters more than it looks: it is what turns "error at `orders.py:142`" into "the function, and its docs" in one step.

Every answer should give back **both** stable forms (location and, if there is one, the ID), so the agent can use either in its next call.

## 3. What a good answer looks like

The reader is a model with a limited context window. That changes the design:

| Rule | Why |
|---|---|
| **Short by default, more on request** | every token of output is paid for in the agent's context. `--depth`, `--full`, `--limit` instead of dumping everything. |
| **Plain text first, JSON on request** | models read compact text well; JSON spends tokens on quotes and braces. JSON is for scripts and plugins. |
| **Line numbers on code** | the agent's edit tools work with exact text and positions. |
| **Say what to do next** | `2 docs: why-we-parse-twice.md, edge-cases.md · read: vnoc doc read <node> <name>` |
| **Errors that teach** | `no node at src/orders.py::parse_ordr · did you mean: OrderService.parse_order` |
| **Paths the agent can open** | give the real path of a doc file; reading and editing it needs no further V-NOC call. |
| **Stable** | once a skill tells agents how the output looks, the output is an interface. See [`02-cli-and-skill.md`](02-cli-and-skill.md) §6. |

An example of the shape (not a spec):

```
$ vnoc show src/orders.py:142
OrderService.parse_order    function    src/orders.py:138-151    ID 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
docs (2): why-we-parse-twice.md · edge-cases.md        → vnoc doc read 0Q8V… [name]

138  def parse_order(self, raw):
139      """Turn the upstream payload into an Order.
140
141      ID: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
142      """
143      data = self._decode(raw)
 …
```

## 4. Agents that write docs: the rules

Writing is where an agent can do damage, so the rules are stricter than for reading.

| Rule | Reason |
|---|---|
| **Only V-NOC's code creates nodes.** An agent never invents an ID, a node folder or a `node.toml`. | the ID is a BLAKE3 hash with exact input rules ([`../../05-identity/01-id-generation.md`](../../05-identity/01-id-generation.md)); a guessed one breaks clean merges |
| **Doc *content* is a normal file.** After the node exists, the agent edits the `.md` with its own tools. | it is just Markdown; no reason to wrap it |
| **The agent writes when asked, not on its own.** No "document everything" by default. | 400 docs that restate the code bury the 10 that explain something. The skill says what a V-NOC doc is for: **why, constraints, history, traps**. Not *what the code does*. |
| **Say who wrote it.** One optional frontmatter line: `author: agent` (or the agent's name). | a reader should know. Additive key, old readers ignore it (rule 1 in [`../../04-storage/01-vnoc-folder.md`](../../04-storage/01-vnoc-folder.md) §5). **[decide: needed, or is git blame enough?]** |
| **Review is git review.** Agent docs are committed files; they show in the pull request like any other. | no new approval system. With graph diff, a new doc shows on its node. |
| **Docs are data, not instructions.** A doc arrives by `git pull` from someone else. | a doc that says "ignore your instructions and…" is a prompt-injection attempt. The skill tells the agent to treat doc text as information about the code. V-NOC cannot enforce this; it can avoid making it worse (never present doc text as a command). |
| **Never delete.** Agents get no Remove and no Empty-trash command. | removal is a person's decision; Missing is a safe state |

## 5. Layer 0: what works with no tool at all

Because of how 0.1 stores things, an agent can already do R1 by itself if it is told three facts. This goes in the project's `AGENTS.md` /
`CLAUDE.md` (V-NOC could offer to add it at the first attach):

```markdown
## V-NOC docs
This project keeps notes about functions, classes, files and folders in `.vnoc/`.
- A docstring line `ID: <26 characters>` (or a `# FileID:` comment, or a `.vnoc-id` file in a folder) means that node has notes.
  They are the `.md` files in `.vnoc/nodes/<last 2 characters of the ID>/<ID>/`. Read them before changing that code.
- Never remove or change an `ID:` / `# FileID:` line or a `.vnoc-id` file. Keep it when you rewrite a function.
- Do not create or rename anything under `.vnoc/nodes/` by hand.
```

| Layer 0 can | Layer 0 cannot |
|---|---|
| read the docs of a **marked** node (R1) | find docs of an **unmarked** node without searching every `node.toml` |
| keep markers (K1), because it was told to | check that it did (R7) |
| edit an existing doc (W2) | attach a new doc (W1): it cannot derive an ID |
| | read one function exactly (R2), call trees (R5) |

That gap is exactly what the options in the next three files fill. But layer 0 is real, it is free, and it should ship with 0.1 as one line in the
user guide.

Next: [`02-cli-and-skill.md`](02-cli-and-skill.md)
