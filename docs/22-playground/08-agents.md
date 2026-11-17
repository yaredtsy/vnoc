# 22.08 Agents: `vnoc play`

```
agents
├── 1. what an agent does with it
├── 2. the commands              list · show · path · new · run
├── 3. what each prints          text, --json, exit codes
├── 4. the skill and the guide
├── 5. guard rails
└── 6. what it stands on in doc 21
```

## 1. What an agent does with it

Coding agents already do this by hand: write `test_thing.py` at the root, import the real code, run it, read the output, and (sometimes)
delete it. `vnoc play` is the same loop with three differences: the file is placed and removed for them, in the right folder with the right
command for the language; the script is **kept on the function** it tries; and the next agent, or person, finds it there.

```
vnoc play new src/orders.py:142 --title "Parse a bad order" --lang python --script-file -   <<'EOF'
from src.orders import OrderService
print(OrderService().parse_order("id=7;qty=x"))
EOF
vnoc play run src/orders.py:142 parse-a-bad-order        # read the output
# edit the code, or the script (its path was printed), and run again
```

## 2. The commands

`<node>` is any form doc 21 takes (21.02: `file:line`, `file::Name`, an ID, a path). `<name>` is the playground's slug, or enough of its
title to mean one.

```
vnoc play list [<node>…] [--where <query>]          the playgrounds of these nodes; with no node, of the project
vnoc play show <node> <name>                        the plan for this machine (where, the command), then the script
vnoc play path <node> <name>                        the script's path, and nothing else
vnoc play new  <node> --title "…" --lang <language>
               [--dir <folder>] [--file <pattern>] [--run "<command>"] [--timeout <s>]
               [--script-file <file> | -] [--description "…"] [--author <name>] [--no-marker] [--again]
vnoc play run  <node> <name> [--timeout <s>]
```

| Command | Notes |
|---|---|
| `list` | one line each: `parse-a-bad-order  python  "Parse a bad order"  src/orders.py::OrderService.parse_order`. `vnoc show <node>` and `vnoc tree` gain the count beside the doc count |
| `show` | what a person sees in the panel: `in .`, `$ …`, or the sentence why it cannot run; then the script with line numbers. The thing to read before running someone else's |
| `path` | for the agent's own edit tool. Editing the script needs no command, as editing a doc needs none (21.04 §5) |
| `new` | the attach flow from a terminal (21's `doc new`, with a playground at the end: 06 §6). Without `--dir` the preset's suggestion is used and printed. Without `--script-file` the script is the starter |
| `run` | plans, places, runs, streams, cleans: 03. `--timeout` overrides the recipe's for this run |

There is no `edit`, no `rm`, no `rename`: the first is a file edit, the second a person's act, the third a key in `play.toml`.

## 3. What each prints

**`run`** passes the script's stdout to stdout and its stderr to stderr, as they arrive, and ends with one line on stderr:

```
$ vnoc play run src/orders.py:142 parse-a-bad-order
Traceback (most recent call last):
  …
ValueError: qty: 'x' is not a number
vnoc: exit 1 · 0.8 s · in . · .venv/bin/python vnoc_play_parse_a_bad_order.py
```

| Exit | Means |
|---|---|
| `0` | the script ran and exited `0` |
| `1` | the script ran and did not end well: a non-zero code, the timeout (both said in the last line) |
| `2` | bad arguments; no such node; no such playground (with "did you mean"); the recipe is refused |
| `3` | the environment: the program was not found, the copy's name is taken, it is already running, the project's `format` is too new |

This follows 21's C6: `1` is "the thing looked at has a problem", `2` and `3` are the caller's and the machine's. An agent's harness reads a
non-zero exit as failure, which is right for a script that raised.

`--json` (every command): `run` then prints nothing until the end, and one object: `{ "schema": 1, "command": "play run", "how": "exit",
"code": 1, "ms": 812, "stdout": "…", "stderr": "…", "cut": false, "plan": { "dir": ".", "command": […] } }`.

**`new`** prints what a write prints (21.04 §8): the node and its ID, each file created, the source file if an `ID:` line was added (and
"read the file again before you edit it"), and the next command:

```
attached to  function  src/orders.py::OrderService.parse_order   ID 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
created      .vnoc/nodes/7K/0Q8V…H7K/play/parse-a-bad-order/script.py      ← the script
created      .vnoc/nodes/7K/0Q8V…H7K/play/parse-a-bad-order/play.toml
runs         in .   $ .venv/bin/python vnoc_play_parse_a_bad_order.py
next         vnoc play run src/orders.py:142 parse-a-bad-order
```

## 4. The skill and the guide

Following 21 C7 (the installed skill is short; the detail comes from the binary):

- **`SKILL.md`** gains one bullet in its "when to use" list: *to try a function on real input without running the application, or to see
  how it is meant to be called: `vnoc play` (see `vnoc guide play`).*
- **The rules block in `AGENTS.md`** (the five always-on lines, 21.06 §4) gains nothing. The one rule that could go there, "never commit a
  `vnoc_play_*` file", is kept by the exclude line (ask 1) and does not need an agent's memory.
- **`vnoc guide play`** is the topic, compiled into the binary. Its content:

```
WHEN     you would otherwise write a scratch file to try one function. Do this instead: the file is put in the
         right place, run, and removed, and the script stays on the function for the next reader.
LOOK     vnoc play list <node>        a function may already have one: run it before writing your own
NEW      vnoc play new <node> --title "…" --lang python|typescript|javascript|rust --script-file -
         The script imports the real code. Import paths are as seen from the folder it runs in (printed as "in …").
RUN      vnoc play run <node> <name>   exit 0 = the script exited 0. Its output is the answer; read it.
EDIT     the script is a file: vnoc play path <node> <name>. Edit it and run again. One playground per
         question worth keeping: change it, do not make a second one for each try.
RULES    read one you did not write before you run it (vnoc play show)
         no secrets: it is committed
         never create, edit or commit a file named vnoc_play_*: that is the running copy
         rust: only pub items can be imported; python: {python} is this machine's interpreter
RECIPE   play.toml beside the script: title, language, script, dir; optional file, run, timeout, [env].
         Leave keys you do not know.
```

The guide test of 21's A10 (every command the guide names exists; every example runs on the sample project) covers this topic too: the
sample project gets one playground.

## 5. Guard rails

| Rail | In the command |
|---|---|
| one playground per call | `new` takes one node |
| no accidental twins | a title this node already has is refused, with the existing one's name and "`vnoc play run …` it, edit it, or `--again`" (as `doc new`) |
| small | a script over 256 KB is refused |
| the recipe's rules | the same `plan` as the extension: 02 §7. A `--dir` outside the project, a `--file` that does not start with `vnoc_play_`: exit `2`, the sentence |
| no remove | not offered |
| no `init` on the way | a project with no `.vnoc/` → exit `2` with 21.04 §3's sentence (starting V-NOC is the owner's decision) |
| a run cannot hang the agent | the timeout applies; standard input is closed |

## 6. What it stands on in doc 21

| Needs | From | Without it |
|---|---|---|
| the command line, the root, `<node>` arguments, `--json`, exit codes | A2, A3 (and `node_ref`, 20 S5) | no `vnoc play` at all |
| a write without an editor | A7 (`EditSink::Disk`, the write lock) and A8 (`doc new`) | `list`, `show`, `path`, `run` work; `new` works only on a node that already has a folder |
| `vnoc guide`, the skill | A10 | the commands exist and nothing tells an agent |

So the agent phase (PG9, PG10) is scheduled after 21's A10. Until then an agent can still **run** what a person made in VS Code, once PG9
is in.

Next: [`09-build-order.md`](09-build-order.md)
