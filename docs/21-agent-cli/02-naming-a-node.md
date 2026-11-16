# 21.02 Naming a node

```
naming a node
├── 1. the four forms
├── 2. where a path starts        the current folder in, the root out
├── 3. several nodes, and standard input
└── 4. when nothing is found      errors that teach
```

## 1. The four forms

Every command that takes `<node>` takes any of these. The parse and the lookup are doc 20's `node_ref` ([`../20-search/04-core.md`](../20-search/04-core.md) §4),
built there in step S5: the search box and the command line resolve a reference with the same two functions.

| Form | Example | Resolved by |
|---|---|---|
| **location** | `src/orders.py::OrderService.parse_order` (`#2` for a second def of the same name) | the tree's node id |
| **file + line** | `src/orders.py:142` (`:142:7` is taken, the column ignored) | the innermost function or class whose range holds the line, else the file |
| **ID** | `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` | where the index found that ID, or its Missing record |
| **file / folder** | `src/orders.py` · `src/billing/` | the path |

*File + line* matters more than it looks: it is what turns "error at `orders.py:142`" into "the function, and its notes" in one call.

A **qualified name alone** (`OrderService.parse_order`, no file) is not a fifth form: it can name several nodes. It is a search, and the error of
§4 answers it.

## 2. Where a path starts

| | Rule |
|---|---|
| **in** | a relative path is relative to the **current folder**, as every command-line tool's is. If nothing is there, it is tried relative to the **root** (so a location copied from an earlier answer works from any folder) |
| **in** | an absolute path is taken as it is, if it is under the root |
| **out** | every path and location printed is relative to the **root** (C9), with `/` separators, on every system |

The CLI opens the root as its project (`Project::open(root)`), so "relative to the opened folder" and "relative to the repository" are the same
thing here, and a location printed by the CLI is the same text as the node's id in `.vnoc/` hints and in the extension when it has the
repository open.

Windows: `C:\repo\a.py:12` is a file and a line (the drive's colon is not the line's); `\` is accepted in and never printed.

## 3. Several nodes, and standard input

`vnoc show a.py::f b.py:30 0Q8V…` answers each in turn, separated by a blank line (in `--json`, a list). `-` reads one reference per line from
standard input: `vnoc find 'kind:fn no:doc in:billing/' --format id | vnoc code -`.

One that cannot be found does not stop the others: it prints its error in place, and the exit code is `2` at the end.

## 4. When nothing is found

| Case | Answer (standard error, exit `2`) |
|---|---|
| no def of that name in that file | `no node at src/orders.py::parse_ordr · did you mean: src/orders.py::OrderService.parse_order` (the three best of `vnoc_search::run` on the name, in that file first) |
| a qualified name with no file | `"OrderService.parse_order" names no file · found: src/orders.py::OrderService.parse_order` (one hit: the hint is the whole answer; several: the first five) |
| the file is not in the index | `src/app.ts is not read by V-NOC (it reads Python files that git does not ignore)` |
| the file failed in the driver | `src/bad.py could not be read: <the driver's message>` |
| a line past the end | `src/orders.py has 210 lines` |
| an ID nobody has | `no node has the ID 0Q8V…` |
| an ID of a Missing node | not an error for `show`, `doc`: `MISSING  was src/old.py::parse_order  ID 0Q8V…  docs 2` and its docs are readable. An error (`2`) for `code`, `calls`: there is no code |
| a path outside the root | `…/other/x.py is outside this project (root: /repo)` |

The hint is always something that can be pasted back as the argument.
