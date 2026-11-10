# 05. Identity: IDs, markers, and finding a node again

The question users will judge V-NOC on: **"I wrote a note, then I refactored. Is my note still on the right thing?"**

```
identity
├── README.md                     this file: the model in one page
├── 01-id-generation.md           how an ID is made, my opinion of the hash approach, the write flow
├── 02-functions-and-classes.md   marker = ID: line in the docstring (a comment style later)
├── 03-files.md                   marker = # FileID: comment at the top
├── 04-folders.md                 marker = .vnoc-id hidden file           ← new in this plan
└── 05-finding-nodes-again.md     marker → anchor → Missing, Relink
```

## 1. Two names for every node

| | **Location** | **ID** |
|---|---|---|
| Looks like | `src/orders.py::OrderService.parse_order` | `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` |
| Changes when you rename or move? | yes, every time | **never** |
| Who has one | every node in the tree | only nodes a person attached something to |
| Rust type | `LocationKey` | `NodeId` |
| Used for | drawing the tree, the canvas | docs: the node folder's name is the ID |

Most nodes never get an ID. An ID is made at the moment of the first attach and not before.

## 2. Three ways V-NOC knows where an ID is

From strongest to weakest:

```
1. MARKER    the ID is written next to the thing itself, so it travels with it
             ├── function / class   ID: line in the docstring
             ├── file               # FileID: comment at the top
             └── folder             .vnoc-id file inside the folder
             survives: rename, move, git mv, terminal mv, edits by an AI agent, VS Code closed

2. ANCHOR    node.toml remembers the last known path + name
             survives: nothing moves. A fresh clone finds the node if it is still where it was.

3. MATCHING  (0.4) guess where it went: same content, git rename detection, similar body
             survives: most refactors, without anything written in the code
```

0.1 has levels 1 and 2. Level 3 is what the plan calls the matching ladder; it is the expensive one and it comes later.

## 3. Everything about tracking is optional

You asked for ID tracking to be optional. The model gives that for free, because **the anchor always exists** and the marker is an extra on top:

| Level | Setting | Effect |
|---|---|---|
| Project | `[ids] default = "on-attach"` in `vnoc.toml` (the default) | attaching a doc writes a marker |
| Project | `[ids] default = "none"` | V-NOC never writes into code or creates `.vnoc-id`. Docs are attached by path + name only. |
| One node | `id_in_code = "never"` in its `node.toml` | this node never gets a marker, whatever the project says |
| One node | "Write ID" action | add the marker later |

A node without a marker is a fully working node. It just goes **Missing** when it is renamed or moved, until you Relink it (or until 0.4 can find it).

```
                   marker written?
attach ──► ID ──►  ├── yes  → id_in_code = "yes"   solid link icon     follows the code anywhere
                   └── no   → id_in_code = "no"    outlined link icon  found only while path + name stay the same
```

## 4. Who does what

| Job | Who | Why there |
|---|---|---|
| Decide **whether** a node gets a marker, and **which** ID | the core | one rule for every language |
| Decide **where in a source file** the marker goes, and produce the edit | the language driver | only it knows the language's syntax |
| Apply the edit | VS Code | the file may be open and unsaved |
| Read and write `.vnoc-id` | the core | no language involved; it is V-NOC's own file |
| Remember the anchor | `node.toml` | so a fresh clone works |

Next: [`01-id-generation.md`](01-id-generation.md)
