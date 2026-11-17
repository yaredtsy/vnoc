# 22.02 On disk

```
on disk
├── 1. the layout
├── 2. play.toml                the recipe of one playground
├── 3. the placeholders
├── 4. where a value comes from   playground → project → preset
├── 5. vnoc.toml [play]         the project's defaults and its own languages
├── 6. .vnoc/local/             this machine: tools, lock files
├── 7. rules                    names, sizes, what is refused
└── 8. what it adds to the frozen format
```

## 1. The layout

```
.vnoc/
├── vnoc.toml                                   + an optional [play] table                    (§5)
├── nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/
│   ├── node.toml
│   ├── why-we-parse-twice.md
│   └── play/                                   NEW: every folder in here is one playground
│       ├── parse-a-bad-order/
│       │   ├── play.toml                       the recipe
│       │   └── script.py                       the script (the name is in the recipe)
│       └── two-currencies/
│           ├── play.toml
│           └── script.py
└── local/                                      not committed (as today)
    ├── play.toml                               this machine's tools                          (§6)
    └── play/runs/*.lock                        one per run in progress                       (03 §5)
```

- **The folder is the list**, as for docs: every folder under `play/` that holds a `play.toml` is a playground of that node. No index file.
- **The folder's name is the slug of the first title**, by the doc rules (`04-storage/01-vnoc-folder.md` §3: legal on three systems, at most
  60 characters, `-2` if taken). It is the playground's name in commands (`vnoc play run <node> parse-a-bad-order`) and it does not change
  when the title does.
- A folder under `play/` with no `play.toml`, or one that does not parse, is **one problem line** ("Problems", with the path) and nothing
  else: the other playgrounds and the node's docs load (rule 3 of `04-storage/01` §5).
- Other files in a playground's folder are left alone. They are not copied on a run (later: 10 §5).

## 2. `play.toml`

```toml
# .vnoc/nodes/7K/0Q8V…H7K/play/parse-a-bad-order/play.toml
title    = "Parse a bad order"      # required. Shown in the list
language = "python"                 # required. A preset's name, or "custom"
script   = "script.py"              # required. A file name in this folder, nothing else
dir      = "."                      # required. Where it runs: a folder, relative to the project root. "." is the root

# Optional. Absent = the project's default, else the preset's (§4).
# file        = "{name}.py"                      # the copy's path, relative to dir
# run         = "{python} {file}"                # the command, in words (no shell: 03 §3)
# timeout     = 60                               # seconds. 0 = no limit
# description = "what quantity 'x' does to the parser"
# author      = "claude-code"                    # as a doc's author: (21 C16)
#
# [env]                                          # added to the environment of the run. Values may use placeholders
# ORDERS_DEBUG = "1"
```

- Written with `toml_edit`, as `node.toml` is: **keys V-NOC does not know are kept**, comments too.
- A new playground gets only the four required keys. The optional ones are written when the person changes them, so a playground follows its
  project's default until someone decides otherwise.
- `language = "custom"` has no preset: `file` and `run` are then required.
- An unknown `language` (a newer V-NOC's preset, or one a teammate's `vnoc.toml` defined on another branch) is not an error to **list**; it is
  an error to **run**, with the name: `no preset "kotlin": set file and run in play.toml, or add [play.language.kotlin] to vnoc.toml`.

## 3. The placeholders

A value of `file`, `run` or `[env]` can hold:

| Placeholder | Becomes | Example |
|---|---|---|
| `{name}` | `vnoc_play_` + the slug, with every character that is not an ASCII letter or digit turned into `_` (`vnoc_play_x` if nothing is left) | `vnoc_play_parse_a_bad_order` |
| `{file}` | the copy's path relative to `dir`, after `file` is resolved (not allowed inside `file`) | `vnoc_play_parse_a_bad_order.py` · `examples/vnoc_play_parse_a_bad_order.rs` |
| `{root}` | the absolute path of the project root | `/Users/me/shop` |
| `{dir}` | the absolute path of `dir` | `/Users/me/shop/services/api` |
| a tool: `{python}` | the program found for this machine (§6) | `/Users/me/shop/.venv/bin/python` |
| `{{` `}}` | a real `{` `}` | — |

Anything else in braces is an error **before anything is placed**: `unknown placeholder {pyhton} in run`. Why `{name}` and not only `{file}`:
`cargo run --example` takes a name, and a Rust example's file must be a legal crate name, which a slug with `-` is not.

## 4. Where a value comes from

For `file`, `run`, `timeout` and `[env]`, the first that has it wins:

```
1. the playground's play.toml
2. the project's vnoc.toml    [play.language.<language>]     (§5)
3. the built-in preset                                       (04)
```

`title`, `language`, `script` and `dir` are the playground's own: there is no default to fall back to at run time. (When a playground is
**created**, the preset proposes `dir`: 04 §1.)

The result of this lookup, with the placeholders replaced, is the **plan** (06 §2). The panel shows it; `vnoc play show` prints it; a run does
exactly it.

## 5. `vnoc.toml`: `[play]`

```toml
# .vnoc/vnoc.toml
format = 1

[play.language.python]                 # change a built-in preset for this project
run = "uv run python {file}"

[play.language.go]                     # a language V-NOC has no preset for
ext     = "go"                         # the script's extension, for a new playground
file    = "{name}_test.go"
run     = "go test -run VnocPlay -v ."
anchor  = ["go.mod"]                   # New Playground proposes the nearest folder above the node that holds one of these
starter = "package orders\n\nimport \"testing\"\n\nfunc TestVnocPlay(t *testing.T) {\n}\n"
```

- Optional, like every table in `vnoc.toml` but `format`. A V-NOC that does not know `[play]` keeps it (`toml_edit`).
- The keys of a language table are the preset's fields (04 §1). A table for a built-in name changes only the keys it gives.
- V-NOC writes into `[play]` only from the form's *Use as this project's default* (07 §4), and only the one key.

## 6. `.vnoc/local/`: this machine

```toml
# .vnoc/local/play.toml          never committed (.vnoc/.gitignore: local/)
[tools]
python = "/Users/me/shop/.venv/bin/python"
```

A **tool** is a program whose place differs from machine to machine. A preset names the tools its command uses and how to find each
(04 §2 has Python's list). The order for `{python}`:

```
1. [tools] python in .vnoc/local/play.toml
2. what the caller passed for this run        (the extension may pass the interpreter the Python extension has selected: later, 10 §2)
3. the preset's own search                    $VIRTUAL_ENV → <dir>/.venv → <root>/.venv → python3 → python on PATH
```

Not found is an error before anything is placed, and it says where to set it. A tool is **never** written into `play.toml` by V-NOC. A person
who types an absolute path into `run` has made their own choice; the form warns once ("this path is on your machine; teammates will get it").

## 7. Rules

Everything in `.vnoc/` arrives by `git pull` and is untrusted input (`04-storage/01` §5 rule 2). A recipe is **refused**, with the reason, when:

| Rule | Why |
|---|---|
| `dir` is not `.` or a valid `RepoPath` (no `..`, not absolute); or, with symlinks followed, it is outside the project root; or it is not a folder | the copy and the command stay inside the project |
| `file`, resolved, has a `..` part, is absolute, or its **last part does not start with `vnoc_play_`** | the copy must be recognisable: the scan skips it (G15), git ignores it (ask 1), the sweep may remove it (03 §5). A pulled recipe cannot ask V-NOC to create `conftest.py` |
| `script` is not a plain file name in the playground's folder | no reading files from elsewhere |
| the script is larger than 256 KB | a playground is small (21's limit for a doc body) |
| `run` resolves to no words | — |
| `timeout` is not a whole number from 0 to 3600 | — |

A refused recipe is still **listed**, with the reason in place of the command, and *Edit* still opens it.

## 8. What it adds to the frozen format

All of it is addition (`04-storage/04-growth-path.md` §2 and §3).

| Addition | An older V-NOC does |
|---|---|
| `play/` in a node folder | does not list it (it reads `*.md` files only), does not touch it; Remove moves the node folder with it inside, Restore brings it back |
| `[play]` in `vnoc.toml` | keeps it when it rewrites the file |
| `.vnoc/local/play.toml`, `.vnoc/local/play/` | nothing: `local/` is ignored by git and unknown names in it are not read |
| a file named `vnoc_play_*` in the tree for a moment | would show it in its tree for a moment if it is a `.py` file. Harmless |

No `format = 2`. The fixture test of `04-growth-path.md` §5 gets a `play/` folder in the newest fixture, so "opening it changes no byte" keeps
being checked.

Next: [`03-running.md`](03-running.md)
