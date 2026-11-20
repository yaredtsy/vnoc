# tree: the structure of a folder, a file or a class

Use it instead of reading many files. The numbers are how many notes (docs) each part has.

```
vnoc tree billing --sig
billing/  4 files · 2 classes · 10 functions · 4 docs inside
  charge.py  · 2 docs
    class ChargeError
    retry_charge(gateway, order, attempts)  1 function
    refund(gateway, order)
  invoice.py
    class Invoice  3 functions · 1 doc
    render_invoice(invoice)
  status.py
    state(invoice)
  tax.py
    rate_for(country)  · 1 doc
    apply_tax(amount, country)
```

What has no note yet, one row each with its whole location (`--flat`: for grep, sort and pasting):

```
vnoc tree billing --where 'no:doc kind:fn' --flat
function  billing/charge.py::retry_charge
function  billing/charge.py::retry_charge.<locals>.one_try
function  billing/charge.py::refund
function  billing/invoice.py::Invoice.__init__
function  billing/invoice.py::Invoice.add_line
function  billing/invoice.py::Invoice.total
function  billing/invoice.py::render_invoice
function  billing/status.py::state
function  billing/tax.py::apply_tax
```

## Keeping it small
- `--depth N` levels below the start (`0` the start alone, `all` every level; default 2, and all with `--where` or `--kind`).
- `--kind` takes `folder`, `file`, `class`, `function`, `method`, several with commas: `vnoc tree billing --kind class`.
  Inside a `--where` query the same word is `kind:fn`.
- `--lines` shows each def's line range, `--docs` the titles of the notes under what they are on.
- `--where` takes the query language of `find` (`vnoc guide find`); `--limit N` caps the rows (default 200).
- Several starts, or `-`: `vnoc tree billing orders/models.py`.
