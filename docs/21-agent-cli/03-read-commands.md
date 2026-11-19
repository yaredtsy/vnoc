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
V-NOC 0.2.0   root /work/sample   drivers python 0.2.0 (12 files)
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
  "drivers": [ { "name": "python", "version": "0.2.0", "files": 12 } ],
  "stats": { "folders": 3, "files": 12, "classes": 5, "functions": 27 },
  "docs": { "docs": 7, "nodes": 6 },
  "missing": [ { "id": "7N1HNCPA163HHSN91HX48T9QGE", "kind": "function", "path": "api/routes.py", "qualname": "legacy_export", "docs": 1 } ],
  "problems": [], "failed": [], "truncated": false }
```

*(Changed 2026-11-19, built in A4a: the golden file changed with it.)* **`drivers` is a list**, `"drivers": [ { "name": "python", "version": "0.2.0", "files": 12 } ]`,
and line 1 says `drivers python 0.2.0 (12 files)`. Today there is one driver, so the list has one entry, and an empty list when none was
started (`driver not started …` stays the text). The reason is C5: JSON only grows, so a single `driver` object shipped now would have to stay
next to a later list for ever. More languages (a driver saying which files it reads, the scan asking each driver) is not planned here: README §5.
When it comes, a driver that fails is a row of the list with its error, not the whole command's exit `3`.

`files` is how many files that driver outlined (the tree's files). `initialized` is whether `.vnoc/` is a folder at the git root. `missing`, `problems` and `failed` are the
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


**As built in A4 (2026-11-19)**, on the sample:

```
$ vnoc doc list billing/charge.py
file  billing/charge.py   ID 4Q57CGHF0HJ23TTYWXTZXQ8DPS
  Handover notes  .vnoc/nodes/PS/4Q57CGHF0HJ23TTYWXTZXQ8DPS/handover-notes.md
  Why charges are retried  .vnoc/nodes/PS/4Q57CGHF0HJ23TTYWXTZXQ8DPS/why-charges-are-retried.md

$ vnoc doc read orders
── How orders are laid out  .vnoc/nodes/20/0DJ3H8AWWXRWKJ55SNZ3509320/how-orders-are-laid-out.md
# How orders are laid out

One module per concern: models hold the data, service holds the rules, parse reads the upstream format.

$ vnoc doc path billing/charge.py why-charges-are-retried.md
.vnoc/nodes/PS/4Q57CGHF0HJ23TTYWXTZXQ8DPS/why-charges-are-retried.md
```

- **`doc list [<node>…]`:** per node a header, `<kind>  <location>   ID <id>`, then one row per doc, `  <title>  <path>` (`  no docs` when there is
  none; several nodes, a blank line between). A Missing node is `MISSING  was <anchor>  ID <id>  docs N`. **With no node: every doc of the
  project**, grouped by node: the nodes of the tree in its order, then the Missing ones, then any node whose code was not read (outside the
  folder, ignored), shown with the place its record gives. `no docs` when the project has none. `--format id` is a location per node, `--format
  path` is the doc files only.
- **`doc read <node> [file]`:** each doc under `── <title>  <path>`, then the text **as it is in the file with its frontmatter left out**
  (the `# title` line is kept), a blank line between docs. A node with no doc: `no docs on <location>`. `file` is a doc's file name in that node's
  folder, with or without `.md`; an unknown one is exit `2`: `no doc named nope.md on <location>: it has a.md · b.md`. The text is never
  rewritten into advice and never cut (01 §3): the sample's scenario-5 note prints its sentence as it is.
- **`doc path <node> [file]`:** paths only, one per line, for `$(…)` and for the agent's own tools.
- **By ID none of the three starts the driver.** When every argument is an ID, the node's folder in `.vnoc/` is read directly (`open_without_driver`:
  the root and `UserData`, no index, no driver, so `--driver /nonexistent` is no error). The header then says what `node.toml` records (kind,
  where it was) and **cannot say whether the code is still there**: a Missing node prints as an ordinary one. An ID with no folder is
  exit `2`: `no node has the ID …`. Every other form, and `doc list` with no node, goes through the index as `show` does, and a missing driver is
  exit `3`.
- **Doc paths** are relative to the root when `.vnoc/` is under it (the ordinary case), and **absolute** when the root is below the git root
  (`--root`), because a path relative to the git root would be relative to nothing the agent has. One function, `nodes::doc_path`, for `show` and
  every doc command.
- **`--json`:** the entries of `show` for a node (`kind`, `location`, `node_id`, `docs: [{title, path}]`; a Missing one `missing`, `was`,
  `node_id`, `docs`), under `results`, with `arg` when a node was named. `doc read --json` adds `bodies: [{title, path, body}]`. Golden file
  `tests/golden/doc.json`.

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

**As built in A4 (2026-11-19)**, on the sample (`billing --sig --docs` is the golden text `tests/golden/tree-billing.txt`):

```
$ vnoc tree billing --sig --docs
billing/  4 files · 2 classes · 10 functions · 4 docs
  charge.py  · 2 docs
    doc: Handover notes
    doc: Why charges are retried
    class ChargeError
    retry_charge(gateway, order, attempts)  1 function
    refund(gateway, order)
  invoice.py
    class Invoice  3 functions  · 1 doc
      doc: Rounding rule
    render_invoice(invoice)
  …

$ vnoc tree --where 'no:doc kind:fn' --flat
function  api/app.py::create_app
function  api/health.py::ping
…                                      (25 rows: every function with no note)
```

- **Rows.** A folder is `name/`, a file its name, a class `class Name`, a function its name (with `(params)` under `--sig`); then, separated by two
  spaces, what there is: the counts below it, `138-171` under `--lines`, `· 2 docs`. `--docs` adds a line `doc: <title>` under the node. Two spaces
  a level; no box drawing.
- **Counts** (`4 files · 3 classes · 31 functions · 6 docs`, parts that are 0 left out) are on a row whose children the depth cut, and on the
  start when it is a folder or a file, as the sketch above has it. The docs in them are those **below** the node; its own are the `· N docs` after.
- **The default depth is 2, and `all` when `--where` or `--kind` is given** (a filter asks which nodes, not how deep: `--where 'no:doc kind:fn'`
  at depth 2 would find nothing in a project). An explicit `--depth` wins. If nothing is kept, even the start is not a row: `no nodes match`.
- **`--kind`** takes `folder`, `file`, `class`, `function` (every function, methods included) and `method` (a function in a class). The ancestors
  of a kept row are kept, so it still reads as a tree.
- **`--where`** is `vnoc_search::parse` and `matches` over the id of every node of the tree (the corpus is the one `find` uses, built only when
  `--where` is given). A filter that means nothing is a line `note: …` on standard error, and the command still answers (`no nodes match`).
- **`--flat`** is one row per node, `<kind>  <location>`, then lines, signature, counts, `· N docs`, and no indent. **It lists only the nodes the
  kinds and the query want, not their ancestors**: an ancestor kept only so a tree reads as one is noise in a list to `grep` or `sort`, and it
  would eat the limit. With nothing narrowing the tree every node is wanted, so the rows are the same.
- **`--limit N`** (default 200) counts the rows given; `… 318 more rows (narrow with a path, --depth or --where)` follows, without the
  parenthesis under `--quiet`. With several starts the limit is shared in the order they are printed.
- **Starts** are resolved by `cli/nodes.rs` as in `show` (every form, the current folder first, the same errors), and each start's tree is printed
  as soon as it is found, a blank line between, so an error lands in place. A Missing start is exit `2`: `<anchor> is Missing: there is no tree`.
- **`--format id`** is a location per row, **`path`** the file of each row.
- **`--json`:** `{ "schema": 1, "command": "tree", "rows": [ … ], "cut": N }`, flat with `depth`, in order, over all the starts, and `errors:
  [{ arg, error: { why, message } }]` only when an argument named nothing. A row: `depth`, `kind`, `name`, `location`, `lines` (`[a, b]` or
  `null`), `docs`, and only when there is something to say `method: true`, `context: true` (the row is there only because something below it is
  wanted), `titles` (`--docs`), `params` (`--sig`), `counts: { folders, files, classes, functions, docs }`. Golden file `tests/golden/tree.json`.

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
- *(Settled 2026-11-19, before A5.)* **The limits are the command's own: 20 nodes and 10 docs by default**, not the box's
  `Limits::default()` (200 and 50, right for a list a person scrolls, too much for a model that reads every row). `--limit N` sets both.
  Each group's header always gives the total (`nodes (20 of 143)`), and a cut group ends with `… 123 more (narrow the query, or --limit N)`;
  `--quiet` drops that line. The order is `run`'s, so a bigger limit only adds rows below.
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
- *(Settled 2026-11-19, before A5.)* **Order:** nodes in the tree's order (folders, files, defs in source order), not by hit count, so a
  query gives the same answer every time and `--limit` cuts the end. **At most 5 lines per node**, then `  … 12 more hits`; the node row
  always says the total (`3 hits`). `--names` prints no lines. Lines are printed as they are in the file, with their 1-based numbers; a line
  over 200 characters is cut with `…`.
- **`<path>…`** goes through the node arguments' resolution (02 §2: relative to the current folder, then the root); a path that is not a
  file or folder of the index is the error of 02 §4.
- **`--docs`** reads each doc file from disk (the paths `Index::doc_texts()` lists) and matches the lines after the frontmatter, **numbered as in
  the `.md` file**, so an agent can open the note at that line. Each hit group is `doc  <title>  on <location>  <path>`.
- A pattern that does not compile is `2` with the `regex` crate's message. No match is `no matches`, exit `0`.
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

**Renderers** *(settled 2026-11-19, before A4)*. A command builds its answer as a value (the rows, the node, the results list) and hands it to
one of two renderers in `cli/out.rs`, text or JSON; no command formats as it computes. A new format is then one renderer, and touches no
command. **`--xml` is a trial, not a promise:** A14 adds an experimental renderer that puts each note's body inside
`<doc path="…" title="…">…</doc>` (a closing tag in a body is escaped, so a note cannot end its own tag) and runs the six scenarios with text
and with XML. XML is kept where it wins on tasks done right, scenario 5 resisted, tokens and calls; if it wins only around note bodies,
`doc read` and the docs of `show` use tags by default and the rest stays text. The reason to try it is that XML tags help a model tell content
from instructions (a note is text someone else wrote, 01 §3); the reason not to use it everywhere is that indentation already shows a tree, at
about half the tokens. *As built in A4a:* the seam is `out::Render`, a trait with `fn text(&self, quiet: bool) -> String` on a value that is also `Serialize`, and
`out::render(&value, global)`, which writes the JSON or the text. `status`, `show`, `code`, `doc` and `tree` build such values (`StatusAnswer`-like
structs: the JSON shape is the struct), and format nothing while computing. Chosen over a renderer that walks a generic value because it is one
trait, no generics in the commands, and each command's text stays next to its shape; the cost is that a new format adds a method (with a default
body where the format has nothing to say, as the `--xml` trial would have for everything but note bodies), not a new file per command.
`--format id|path` is a third, tiny renderer in `nodes::emit`.

**User templates** are out: `--json` with `jq` or a script gives any shape; what would bring them back is a user who
needs a shape `--json` cannot give easily.

The whole command line is **experimental until 1.0** and says so in `--help`: names can still move, with a release note.
