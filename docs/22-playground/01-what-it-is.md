# 22.01 What a playground is

```
what it is
├── 1. the idea in one picture
├── 2. who uses it, and for what
├── 3. the life of one playground
├── 4. the three choices
├── 5. what it is not
└── 6. words used in this folder
```

## 1. The idea in one picture

```
the user's project                              .vnoc/  (hidden, committed)
├── src/                                        └── nodes/7K/0Q8V…H7K/           the node: OrderService.parse_order
│   └── orders.py   ← the real code                 ├── node.toml
│        ▲                                          ├── why-we-parse-twice.md     a doc
│        │ import                                   └── play/
│        │                                              └── parse-a-bad-order/    a playground
└── vnoc_play_parse_a_bad_order.py                          ├── play.toml         language · where · command
     ▲   the copy: exists only while it runs                └── script.py         the script
     └────────────── copied here on Run, deleted after ──────────┘
```

The script is small. It imports the real function and calls it with real values:

```python
# script.py
from src.orders import OrderService

print(OrderService().parse_order("id=7;qty=x"))
```

It does not copy the function. It does not start the application. It is what a person writes in a scratch file to answer "what does this do
with a bad quantity?", and what a coding agent writes when it wants to try one function before it trusts a change. The difference is where the
file goes afterwards: not into the bin, and not into the tree as `test2.py`, but **onto the node**, where the next reader of that function finds
a working example of how to call it.

## 2. Who uses it, and for what

| Who | Wants to | Does |
|---|---|---|
| a person reading code | see what one function returns for one input, without running the app | *New Playground* on the card, writes four lines, Run |
| a person fixing a bug | reproduce it on the one function, change the code, run again | Run, edit the code, Run |
| a person writing a doc | keep "this is how you call it" beside the why | a playground next to the doc, on the same node |
| a coding agent | check its change on the real code before it says "done" | `vnoc play new`, `vnoc play run`, reads the output |
| a coding agent | learn how a function is meant to be called | `vnoc play list <node>`, `vnoc play show` |
| the next person | not ask "how do I even call this?" | opens the node, sees the playground, presses Run |

## 3. The life of one playground

```
create ──► edit the script ──► run ──► read the output ──┐
  │              ▲                                       │
  │              └──────────── edit again ◄──────────────┘
  │
  ├──► change the recipe     another folder, another command, another title
  ├──► follows its node      rename · move · Relink · Remove and Restore   (nothing to build: it is in the node folder)
  └──► remove                a person's act: the folder goes to the system trash
```

| Act | In VS Code | For an agent | What changes on disk |
|---|---|---|---|
| **create** | *New Playground…* on a card, a row, or the panel | `vnoc play new <node> --title … --lang …` | the node folder is made if the node had none (the attach flow: ID, marker, `node.toml`); then `play/<slug>/play.toml` and the script |
| **edit the script** | *Edit Script* opens it in an editor tab | its own edit tool on the path from `vnoc play path` | the script file |
| **edit the recipe** | the ⚙ form: Run in · File name · Command · Timeout | edits `play.toml` (the guide says which keys) | `play.toml`, keys it does not know kept |
| **rename** | the title in the form | the `title` key | `play.toml` only; the folder's name (the slug) stays, so a link to it stays good |
| **run** | ▶ Run | `vnoc play run <node> <name>` | nothing under `.vnoc/nodes/`; a copy in the tree for the length of the run |
| **stop** | ■ Stop | Ctrl-C | — |
| **remove** | *Remove Playground* (asks; system trash) | not offered | the playground's folder |

## 4. The three choices

| Choice | Key | Example | Who fills it first |
|---|---|---|---|
| **Language** | `language` | `python` · `typescript` · `javascript` · `rust` · `custom` | the person, from a list. V-NOC proposes the owner node's language when there is a preset for it |
| **Where it runs** | `dir` | `.` (the project root) · `services/api` · `crates/vnoc` | the preset proposes: for Rust the nearest folder above the node that has a `Cargo.toml`, for TypeScript the nearest `package.json`, for Python the project root |
| **The command** | `run` | `{python} {file}` · `npx tsx {file}` · `cargo run --quiet --example {name}` | the preset; the project's `vnoc.toml` can change it for everybody; one playground can have its own |

`dir` is both where the copy is placed and where the command starts. A language that needs the copy in a sub-folder of that (Rust:
`examples/`) says so in the preset's file name, not in a second choice: [`02-on-disk.md`](02-on-disk.md) §3.

## 5. What it is not

| It is not | Because |
|---|---|
| **a test** | nothing asserts, nothing is collected, CI does not run it. A playground that deserves to be a test should become one (later: 10 §4) |
| **a REPL or a notebook** | every run starts a fresh process; nothing is kept between runs |
| **a sandbox** | the script runs as the user, with their files and their network: [`05-safety.md`](05-safety.md) |
| **a copy of the code** | it imports the real code. When the function changes, the playground runs the new function |
| **a place for secrets** | it is committed |

## 6. Words used in this folder

| Word | Means |
|---|---|
| **playground** | the folder `play/<slug>/`: a recipe and a script, owned by one node |
| **script** | the code file in it |
| **recipe** | `play.toml`: language, where, command, and the optional keys |
| **preset** | a language's defaults: a row of data in the core, or in `vnoc.toml` |
| **the copy** | the temporary file in the user's tree, `vnoc_play_…`, that exists during a run |
| **plan** | a recipe with every default filled and every placeholder replaced: exactly what a run will do, before it does it |
| **tool** | a program a command names through a placeholder (`{python}`), found per machine |

Next: [`02-on-disk.md`](02-on-disk.md)
