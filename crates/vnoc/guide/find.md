# find: where something lives

`find` looks at names and at the words in notes. Words need no quotes; the best match is first, and each group says its total.

```
vnoc find rate kind:fn
nodes (1)
  function  billing/tax.py::rate_for   lines 6-7   ID 4KF02248CHCTYAG49B0KKZ5SD7   · 1 doc
docs (1)
  Where the rates come from  on billing/tax.py::rate_for
    The table is the finance team's. Do not edit it without asking them.
    .vnoc/nodes/D7/4KF02248CHCTYAG49B0KKZ5SD7/where-the-rates-come-from.md
```

`grep` searches the text of the code (a regular expression) and answers with the function each hit is in. It is how you find callers:
it matches text, not meaning, and the definition line is one of the hits.

```
vnoc grep 'parse_order\('
function  orders/service.py::OrderService.parse_order   lines 10-13   · 1 doc   1 hit
  10      def parse_order(self, raw):
function  orders/service.py::OrderService.place   lines 15-18   1 hit
  16          order = self.parse_order(raw)

2 nodes · 2 hits
```

`vnoc grep 'parse_order\(' --names` leaves the lines out. `--docs` searches the notes instead of the code; `-i` ignores case.

## The query of `find`, `tree --where` and `doc list --where`
- words: all must match, in a name or in a note. `kind:fn` `kind:class` `kind:method` `kind:file` `kind:folder`.
- `in:billing/` inside a folder. `has:doc` or `no:doc`: with or without notes. `tag:plan` and `author:claude`: about a note.
- `-` turns a filter round: `-tag:done`. A filter with a space in it needs quotes.
- a reference answers where that node is now: `vnoc find orders/service.py:12 --nodes`, or an ID.
- `--nodes` or `--docs` for one group, `--limit N`. A misspelt name finds nothing: `show` is the one that suggests.

What a function calls (not who calls it) is `vnoc calls <node> --depth 2`: ask once with the depth you need, it is the slow command.
