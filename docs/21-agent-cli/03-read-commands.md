# 21.03 The read commands

```
read commands
├── 1. the list
├── 2. status
├── 3. show · code · doc
├── 4. tree                 the one with the most freedom
├── 5. find
├── 6. grep                 code text, answered as nodes
├── 7. calls
├── 8. missing
├── 9. check                and the lost marker
├── 10. changed             (optional step A15)
└── 11. output rules        text · --format · --json
```

The sketches show the **shape**, not the final text. Text may change between releases; `--json` only by adding (C5).

## 1. The list

```
vnoc status                                     is V-NOC here · the root · counts · Missing and problems · versions
vnoc show <node>…  [--no-code] [--full]         header · its docs (title and path) · its code
vnoc code <node>…  [--context N]                only the code, with line numbers
vnoc doc list [<node>…] [--where '<query>']     title, tags and path of each doc; with no node, every doc of the project
vnoc doc read <node> [file] [--where '<query>'] print the doc (all of the node's, separated, when no file is given)
vnoc doc path <node> [file]                     the path, for the agent's own read and edit tools
vnoc tree [<node>…] [--depth N] [--kind K,…] [--where '<query>'] [--sig] [--lines] [--docs] [--flat] [--limit N]
vnoc find '<query>' [--limit N] [--nodes | --docs]
vnoc grep '<regex>' [<path>…] [-i] [--docs] [--names] [--limit N]
vnoc calls <node> [--depth N]
vnoc missing
vnoc check [--changed]
vnoc changed [--since <ref>]                    (A15)
global:  --root <dir> · --driver <path> · --json · --format id|path · --quiet · --help · --version
```

## 2. `status`

```
$ vnoc status
V-NOC 0.5.0   root /repo   driver python 0.5.0
12 folders · 84 files · 31 classes · 412 functions · 27 docs on 19 nodes
missing 1 · problems 0 · lost markers 2      → vnoc check
skill installed (0.5.0) · rules in AGENTS.md
```

In a project with no `.vnoc/`: the counts, then `no .vnoc/ here yet: nothing is documented (vnoc init starts it; ask the user first)`. The one
command to run when anything seems off; the walkthrough's "is it working?" step.

## 3. `show`, `code`, `doc`

```
$ vnoc show src/orders.py:142
function  src/orders.py::OrderService.parse_order   lines 138-171   ID 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
docs (2)
  Why we parse twice     .vnoc/nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/why-we-parse-twice.md
  Rounding rule          .vnoc/nodes/7K/0Q8V5W4T2F7KX9M3B6D1RZ4H7K/rounding-rule.md
  → vnoc doc read src/orders.py::OrderService.parse_order
code
  138  def parse_order(self, raw):
  139      """Turn the upstream payload into an Order.
  …
```

- `show` prints at most **120 lines** of code, then `… 84 more lines (vnoc code <node>, or --full)`. `--no-code` stops after the docs.
- `show` on a file or a folder: the header, its docs, and one level of `tree` under it instead of code.
- `code` prints all of it, nothing else. `--context N` adds N lines before and after (the decorators above, the next def below).
- `doc read` prints the body as it is in the file, frontmatter left out, each doc under a line `── <title>  <path>`. The text of a note is
  printed as content, never rewritten into advice (01 §3).
- `doc path` prints only paths, one per line: it is for `$(…)` and for the agent's own tools.
- **`--where`** narrows which docs are fetched, with doc 20's filters (Q9: no second language): `vnoc doc list --where 'tag:plan'` is every
  plan in the project; `vnoc doc read src/billing/ --where 'tag:plan author:claude-code'` prints that folder's plans by that author. Only the
  filters that are about a doc apply (`tag:`, `author:`, and words in its text); on a node given by name, `in:` and `kind:` are already
  answered.
  *(Settled 2026-11-17, from the built code.)* It is `vnoc_search::run` with no node rows and no doc limit; of its doc rows, the ones of the
  nodes named are kept (all of them when no node is named), in `run`'s order. As built, a query of filters alone finds **no** doc
  (20, 01 §3), so `--where 'tag:plan'` needs the rule A8 adds with the two filters ([`04`](04-write-commands.md) §4).
- `doc list`, `doc read` and `doc path` by **ID** read the node folder and do not start the driver.

## 4. `tree`

An agent's question about structure is never the same twice, so the tree has knobs, and every knob has a default that keeps the answer short.

| Option | Does | Default |
|---|---|---|
| `<node>…` | where to start: folders, files, classes; several are printed one after another | the root |
| `--depth N` | how many levels below each start; `--depth 0` is the start alone; `--depth all` | 2 |
| `--kind K,…` | which kinds get a row: `folder` `file` `class` `function` `method`. Ancestors of a kept row are kept, so it still reads as a tree | all |
| `--where '<query>'` | keeps the nodes doc 20's query finds (words, filters), and their ancestors. `--where 'no:doc kind:fn'` is "the undocumented functions, in place" | none |
| `--sig` | parameters after a function's name: `parse_order(self, raw)` (from the outline's `params`) | off |
| `--lines` | the line range after each def: `138-171` | off |
| `--docs` | the titles of a node's docs under it | off: only the count `· 2 docs` |
| `--flat` | one row per node with its full location, no indent: for `grep`, `sort`, and pasting | off |
| `--limit N` | at most N rows, then `… 318 more rows (narrow with a path, --depth or --where)` | 200 |

```
$ vnoc tree src/billing --depth 2 --sig
src/billing/                        4 files · 3 classes · 31 functions · 6 docs
  invoice.py                        · 2 docs
    class Invoice                   · 1 doc
    total(items, tax)
    bill_all(customers)
  coupons.py
    class Coupon
    apply(order, code)              · 1 doc
  …
```

- A folder or file that the depth cut shows what is under it as counts, so the agent knows whether going deeper is worth a call.
- Rows are in the tree's own order (folders first, then files, as the Explorer sorts; defs in source order): the same order every time.
- No box-drawing characters: two spaces per level. They cost tokens and say nothing an indent does not.
- `--where` is the query language of [`../20-search/01-query.md`](../20-search/01-query.md), unchanged (Q9): the filter an agent learned for
  `find` works here.
- *(Settled 2026-11-17, from the built code.)* A node is kept when `vnoc_search::matches` says so: the query finds it **by name, or finds
  one of its docs**, which is what the canvas dims by (20, 04 §5). So `--where retry` keeps `retry_charge` and also the function whose note
  talks about retries. `tree_view` asks about every id of the tree in one call; nothing is ranked.

## 5. `find`

Doc 20's `vnoc_search::run`, printed: nodes, then docs with their snippets, each group with its total.

```
$ vnoc find 'retry in:billing/'
nodes (2)
  function  src/billing/charge.py::retry_charge      lines 40-71    · 1 doc
  function  src/billing/charge.py::Charger._retry    lines 102-118
docs (1)
  Why three retries      on src/billing/charge.py::retry_charge
    …the gateway drops one request in fifty; a fourth **retry** only doubles the charge when…
    .vnoc/nodes/M3/…/why-three-retries.md
```

`--nodes` or `--docs` keeps one group. `--limit N` is for both. A query that is one reference (`vnoc find 0Q8V…`) answers where that node is
now. Finding nothing prints `no matches`, the `notes` if a filter was wrong, and exits `0`.

*(Settled 2026-11-17, from the built code.)*

- `find` is `search::prepare`, `vnoc_search::run`, and its own printing: the same two calls as `vnoc/search`, so the order cannot differ.
- A node row of the search has the def's first line only, and no ID. `find` prints both lines and (C5) the ID, so each row is **joined to the
  tree by its id** (`Node::find`, 02 §2); the search's types do not grow for it.
- An ID of a **Missing** node: `prepare` keeps only a reference that named a node of the tree. It gains `reference: Option<Found>` (what the
  reference resolved to, kept), and `find` prints the `MISSING  was …` line of 02 §4 above the rows. The box ignores the new field.
- A path as the query (`vnoc find src/orders.py`) is not a reference here, as in the box: the path tiers find it, with what is in it.

## 6. `grep`

`find` knows names and notes. `grep` reads the **code's text**, and answers the way V-NOC can and `rg` cannot: **which functions**.

```
$ vnoc grep 'retry_count' src/
function  src/billing/charge.py::retry_charge        lines 40-71    · 1 doc     3 hits
  44      retry_count = 0
  51      while retry_count < MAX:
  66          retry_count += 1
function  src/api/client.py::Client.send             lines 12-58                1 hit
  31          log.info("retry_count=%s", n)
file      src/settings.py                                                        1 hit      (outside any def)
  8   RETRY_COUNT = 3
```

| Option | Does |
|---|---|
| `<path>…` | folders or files to search; the root when none |
| `-i` | ignore case |
| `--names` | only the node rows, no lines |
| `--docs` | search the bodies of the notes instead of the code (a regular expression, where `find` has words) |
| `--limit N` | at most N nodes (default 50); the total is always said |

- The files are the index's (Python, not git-ignored). The pattern is the `regex` crate's syntax, matched line by line.
- A hit is given to the **innermost** def whose range holds its line; a hit in no def goes to the file.
- **The stand-in for callers.** `vnoc grep 'parse_order\('` lists the functions that mention the call. It is by text, not by resolution, and the
  guide says so in those words. Real callers need either a solve per candidate or an eager index: [`08`](08-mcp-and-plugins.md) §3.

## 7. `calls`

What a function calls, merged as the canvas shows it (`calls::merge`), default depth 2, each row with its location or `(outside the project)`.
Callees only: the core has no callers today (`16-ui-redesign/06` §5). This is the slow command (the driver analyses the whole project): the guide
says to ask once with the depth wanted, not level by level.

## 8. `missing`

Documented nodes that are not found, each with its ID, its last known place, its doc count, and the two things that can be done:
`→ vnoc relink <ID> <node>` if the code moved; a person removes it in VS Code if it is gone.

## 9. `check`

Exit `1` if any of these, each as one line with the file and the fix:

| Problem | From | Fix it names |
|---|---|---|
| a documented node is Missing | `Index::missing` | `vnoc relink <ID> <node>` |
| **an `ID:` line was lost**: `node.toml` says the ID is in the code, and the node was found only by its name | new: `Index::lost_markers` (a record with `id_in_code = "yes"` resolved `FoundBy::Anchor`) | `vnoc repair` |
| one ID is written in two places | `Index::problems` | open the copy, delete its `ID:` line |
| a `node.toml` that does not parse, a folder in `nodes/` that is not a node, a `.vnoc-id` that cannot be read | `Index::problems` | the file to open |
| a file the driver could not read | `Index::failed` | a warning only: it does not fail the check (a syntax error mid-edit is normal) |

The first, third and fourth are what the NEEDS ATTENTION view and the Problems lines show, from the same functions, so CI and the editor agree.
The second is new to both: the extension shows it as a link icon, not as a problem **[decide in A6]** whether NEEDS ATTENTION lists it too.

`--changed` keeps only the problems in files that differ from `HEAD` (`git status --porcelain`): fast enough to run after every edit, and what a
hook would call ([`08`](08-mcp-and-plugins.md)). With no git, it is the full check.

## 10. `changed` (optional, step A15)

"Which documented things did this session touch?" `git diff -U0 <ref>` (default `HEAD`), each changed line range given to its innermost def:

```
$ vnoc changed
function  src/orders.py::OrderService.parse_order   lines 138-171   · 2 docs   → re-read: vnoc doc read …
function  src/orders.py::OrderService.validate      lines 173-190
```

Documented nodes first. It is the smallest piece of the graph-diff idea (futureplan 01), and the one that keeps notes true: the skill's "after
you changed code" step becomes one command. Needs `git` on the PATH; without it the command says so and exits `3`.

## 11. Output rules

| | Rule |
|---|---|
| **text** | for reading; may change between releases. Two-space indents, no colour unless standard output is a terminal and `NO_COLOR` is unset |
| **`--format id`** | one location per line and nothing else (an ID for a Missing node). For pipes into `vnoc … -` |
| **`--format path`** | one file path per line (the source file of a node; the `.md` for a doc) |
| **`--json`** | one JSON value: `{ "schema": 1, "command": "show", … }`. Field names in **`snake_case`, as the protocol's** (`node_id`, `nodes_total`, `git_root`: corrected 2026-11-17, the first version said camelCase, which the protocol is not). A command prints the library's own types where it can, so the CLI and the extension cannot name one thing two ways. Fields are only ever added. The shapes are golden files in `crates/vnoc/tests/golden/` (A2 on): a changed shape is a failing test, on purpose |
| **`--quiet`** | no "next step" lines, no totals: only the rows. For hooks |
| **standard error** | errors and the hints of 02 §4. Logs only with `VNOC_LOG` |
| **nothing found** | a line saying so, exit `0` |

The whole command line is **experimental until 1.0** and says so in `--help`: names can still move, with a release note.
