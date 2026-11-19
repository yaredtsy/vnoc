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

As built in A2 (2026-11-19), on the sample project (`crates/vnoc/tests/sample/`, run as a copy outside the repository):

```
$ vnoc status
V-NOC 0.2.0   root /work/sample   driver python 0.2.0
3 folders · 12 files · 5 classes · 27 functions · 7 docs on 6 nodes
missing 1 · problems 0      → vnoc check
```

- **Line 1:** the version of `vnoc`, the root (**the one absolute path any command prints**: it is the root itself; everything else is relative to
  it, 02 §2), and the driver that ran. A folder with no Python file never starts a driver, and says `driver not started (no Python files to read)`.
- **Line 2:** the counts of `index::structure` (folders *under* the root, files, classes, functions and methods together), then the docs and the
  nodes they are on (`Index::doc_counts`; a Missing node's docs are counted). `no docs` when there are none. Singular where the count is 1
  (`1 file`, `1 class`).
- **Line 3:** `missing N · problems N`, then `failed N` if files could not be outlined and `stopped at 20,000 files` if the scan hit its cap. The
  hint `→ vnoc check` is there only when `missing + problems > 0`, and never with `--quiet`.
- Still to come, not built: the **lost markers** count (A6, with `check`) and the line about the installed skill and rules (A11).
- Always exit `0` when it answered, whatever it found: a Missing node is a finding, not a failure of `status` (`check` is the command that exits `1`).
- `--format` is a usage error here (exit `2`): `status prints no list; --format is for list commands`.

In a project with no `.vnoc/` at the git root: lines 1 and 2 (`no docs`), then
`no .vnoc/ here yet: nothing is documented (vnoc init starts it; ask the user first)`, exit `0`; there is no third line unless files failed or the
scan stopped. The one command to run when anything seems off; the walkthrough's "is it working?" step.

`--json`, one value (golden file `crates/vnoc/tests/golden/status.json`, with the root and the two versions replaced before comparing):

```json
{ "schema": 1, "command": "status", "version": "0.2.0", "root": "/work/sample", "initialized": true,
  "driver": { "name": "python", "version": "0.2.0" },
  "stats": { "folders": 3, "files": 12, "classes": 5, "functions": 27 },
  "docs": { "docs": 7, "nodes": 6 },
  "missing": [ { "id": "7N1HNCPA163HHSN91HX48T9QGE", "kind": "function", "path": "api/routes.py", "qualname": "legacy_export", "docs": 1 } ],
  "problems": [], "failed": [], "truncated": false }
```

`driver` is `null` when none was started. `initialized` is whether `.vnoc/` is a folder at the git root. `missing`, `problems` and `failed` are the
protocol's own types (`MissingNode`, `Problem`, `FailedFile`: snake_case), as `vnoc/structure` sends them.

## 3. `show`, `code`, `doc`

**As built in A3 (2026-11-19)**, on the sample (`orders/service.py` is the sample's, `billing/tax.py` too):

```
$ vnoc show orders/service.py:12
function  orders/service.py::OrderService.parse_order   lines 10-13   ID 31ZBQ9NKY1E881TZWC2CJCN35H
docs (1)
  Why we parse twice  .vnoc/nodes/5H/31ZBQ9NKY1E881TZWC2CJCN35H/why-we-parse-twice.md
  → vnoc doc read orders/service.py::OrderService.parse_order
code
  10      def parse_order(self, raw):
  11          """ID: 31ZBQ9NKY1E881TZWC2CJCN35H"""
  12          lines = [Line(item["sku"], item["qty"], item["price"]) for item in raw["items"]]
  13          return Order(raw["number"], lines)

$ vnoc show billing/tax.py            # a file; a folder is the same with `folder`
file  billing/tax.py
docs (0)
contains (2)
  function  rate_for  lines 6-7  docs 1
  function  apply_tax  lines 10-11  docs 0

$ vnoc show 7N1HNCPA163HHSN91HX48T9QGE   # a Missing node: an answer, exit 0
MISSING  was api/routes.py::legacy_export  ID 7N1HNCPA163HHSN91HX48T9QGE  docs 1
docs (1)
  The old CSV export  .vnoc/nodes/GE/7N1HNCPA163HHSN91HX48T9QGE/the-old-csv-export.md
  → vnoc doc read 7N1HNCPA163HHSN91HX48T9QGE

$ vnoc code orders/service.py::OrderService.parse_order --context 2
 8          self.store = store
 9  
10      def parse_order(self, raw):
…                                       (lines 8 to 15)
```

- **Header:** `<kind>  <location>   lines A-B   ID <id>`, with the lines 1-based and both ends included, and no `lines` for a file or a folder and no
  `ID` for a node with none. The kind is the tree's (`folder`, `file`, `class`, `function`; a method is a `function`). The location is relative to
  the root, `#2` for a second def of one name.
- **Docs:** `docs (N)` (also `docs (0)`), one row per doc with its title and its file **relative to the git root** (the `.vnoc/` folder is there:
  with `--root` below the git root it is not under the root), then the hint to read them. `--quiet` drops the hint.
- **Code:** the lines as `  <number>  <text>`, the numbers right-aligned to the widest. At most **120 lines**, then
  `  … 9 more lines (vnoc code <location>, or --full)` (`--quiet`: `  … 9 more lines`). `--no-code` stops after the docs, and for a file or a
  folder also drops what follows. `--full` prints all.
- **A file or a folder** shows `contains (N)` and one row for each child, one level only: `  <kind>  <name>  lines A-B  docs N`. (`tree` is A4.)
- **A Missing node**, by ID, prints `MISSING  was <anchor>  ID <id>  docs N` and its docs, and the hint reads it by ID. It is exit `0` for `show`.
- **`code`** prints only the lines, as `<number>  <text>` with no indent. With several nodes, each starts with `── <location>  lines A-B`, the
  def's own lines, whatever `--context` adds, and a blank line separates them. `--context N` adds N lines on each side and stops at the file's first
  and last. A **file, a folder or a Missing node is exit `2`**: `billing/charge.py is a file: use vnoc show for a file or a folder`;
  `<anchor> is Missing: there is no code (vnoc show <ID> reads its docs)`.
- **`--format id`** prints one location per argument, an ID for a Missing node, and nothing else: `vnoc show a.py:12 --format id` is the resolver.
  `--format path` prints the file (the anchor's, for a Missing node). On `show` and on `code`.
- **Several nodes, and `-`:** answers are separated by a blank line; `-` reads one reference per line from standard input. An argument that finds
  nothing prints `error: <the sentence of 02 §4>` on standard error **after standard output was flushed**, so a merged stream shows the answers and
  the errors in the order of the arguments; the exit code is `2` at the end. With `--json` standard error stays empty.
- **A relative path** is tried from the current folder first (`..` allowed, never above the root), then as written: from `billing/`, `charge.py`,
  `charge.py:12`, `charge.py::retry_charge` and `billing/charge.py…` name the same node, and `status.py` (two files of that name in the sample)
  is `billing/status.py` from `billing/` and an `Ambiguous` error from the root.

**What a def's range is** (measured on the Python driver in A3; written in `source.rs`): `start.line` is the line of the **first decorator** when
the def has any (`@property` above `def total`), else the `def` or `class` line; `end.line` is the **last line of the def itself**, inclusive and
0-based, and `end.character` is the end of that line, **not column 0 of the next one**. The blank lines after a def are not in it, and a class
ends where its last method does. So the decorators are already in `show` and `code`, and `--context` adds only what lies outside the def.

`--json`, `{ "schema": 1, "command": "show", "results": [ … ] }`, one entry per argument in order (golden files `tests/golden/show.json` and
`code.json`; the arguments of that call include a wrong one):

```json
{ "arg": "orders/service.py::OrderService.parse_order", "kind": "function", "location": "orders/service.py::OrderService.parse_order",
  "path": "orders/service.py", "qualname": "OrderService.parse_order", "lines": [10, 13], "node_id": "31ZBQ9NKY1E881TZWC2CJCN35H",
  "docs": [ { "title": "Why we parse twice", "path": ".vnoc/nodes/5H/31ZBQ9NKY1E881TZWC2CJCN35H/why-we-parse-twice.md" } ],
  "code": { "first": 10, "lines": [ "    def parse_order(self, raw):", … ], "more": 0 } }
{ "arg": "orders", "kind": "folder", …, "lines": null, "code": null,
  "children": [ { "kind": "file", "name": "models.py", "location": "orders/models.py", "lines": null, "docs": 0 }, … ] }
{ "arg": "7N1HNCPA163HHSN91HX48T9QGE", "missing": true, "was": "api/routes.py::legacy_export", "node_id": "7N1H…", "docs": [ … ] }
{ "arg": "orders/service.py::parse_ordr", "error": { "why": "no_def", "message": "no node at orders/service.py::parse_ordr · did you mean: …" } }
```

`code` has the same entries with `code: { first, lines }` (no `more`) and no `docs`. The `why` words are: `no_def`, `no_file`, `not_read`, `failed`,
`ambiguous`, `past_end`, `no_id`, `outside`, `empty`, `unreadable`, `missing` and `no_code` (the last two only from `code`).

**Not built yet:** `doc read` and `doc path` (A4: the hint `→ vnoc doc read …` names a command that does not exist until then).

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
- A path with a `/` as the query (`vnoc find src/orders.py`) is a reference since S9a, as in the box: that file or folder is first, marked
  exact, and the path tiers list what is in it below. A name with no `/` (`orders.py`) is only words. A call's `()` or `(` at the end of a
  word is dropped, and a traceback line (`File "a.py", line 12`) is the reference `a.py:12`.

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
