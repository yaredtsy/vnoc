# read: what a node is, and what is said about it

`show` prints where a node is, the notes attached to it, and its code. Read it before you change a function.

```
vnoc show orders/service.py::OrderService.parse_order
function  orders/service.py::OrderService.parse_order   lines 10-13   ID 31ZBQ9NKY1E881TZWC2CJCN35H
docs (1)
  Why we parse twice
    .vnoc/nodes/5H/31ZBQ9NKY1E881TZWC2CJCN35H/why-we-parse-twice.md
  → vnoc doc read orders/service.py::OrderService.parse_order
code
  10      def parse_order(self, raw):
  11          """ID: 31ZBQ9NKY1E881TZWC2CJCN35H"""
  12          lines = [Line(item["sku"], item["qty"], item["price"]) for item in raw["items"]]
  13          return Order(raw["number"], lines)
```

`doc read` prints the node's line (as `show` does) and then the notes themselves (`doc path`: only their files, for your own read and edit tools):

```
vnoc doc read orders/service.py::OrderService.parse_order
function  orders/service.py::OrderService.parse_order   lines 10-13   ID 31ZBQ9NKY1E881TZWC2CJCN35H
── Why we parse twice
   .vnoc/nodes/5H/31ZBQ9NKY1E881TZWC2CJCN35H/why-we-parse-twice.md
The upstream payload is parsed once to find the number and again, with the lines, once the order is known.
```

`code` prints only the code with line numbers (`--context 2`: two more lines each side). `show --no-code` stops after the notes.

## Naming a node
Four ways to one node: `orders/service.py::OrderService.parse_order`, `orders/service.py:12` (the innermost def holding line 12), its ID
`31ZBQ9NKY1E881TZWC2CJCN35H`, or a path (a file or a folder). A wrong name is an error with a suggestion, exit 2:

```
vnoc show billing/tax.py::rate_fro  # exit 2
error: no node at billing/tax.py::rate_fro
  did you mean
    billing/tax.py::rate_for
```

## The project, several nodes, scripts
`vnoc status` says whether V-NOC works here and what it knows (the root, the counts, `health`). If the root is not your project, add
`--root <folder>` to every command. `show`, `code` and `doc read` take several nodes, or `-` for names on standard input:

```
vnoc find 'no:doc kind:class in:orders/' --format id | vnoc show - --no-code
```

For scripts: `--json` (`{ "schema": 1, "command": …}`; fields are only ever added), `--format id` (a location a line), `--format path`.
