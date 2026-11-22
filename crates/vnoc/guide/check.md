# check: is the documentation sound

Run `vnoc check --changed` after you change code (`vnoc check` for the whole project). Each problem is one block: what, why, the fix.

```
vnoc check
missing  api/routes.py::legacy_export
  its code is not found (ID 7N1HNCPA163HHSN91HX48T9QGE); 1 doc
  → vnoc relink 7N1HNCPA163HHSN91HX48T9QGE <node>
    or remove it in VS Code if the code is gone

lost marker  billing/tax.py::rate_for
  its ID: line is gone (ID 4KF02248CHCTYAG49B0KKZ5SD7); found by its name only
  → vnoc repair billing/tax.py::rate_for

2 problems
```

A lost marker is mended by `repair`, which writes the `ID:` line back (read that file again before you edit it):

```
vnoc repair billing/tax.py::rate_for
repaired     billing/tax.py::rate_for
             ID: line written at line 7
vnoc check
missing  api/routes.py::legacy_export
  its code is not found (ID 7N1HNCPA163HHSN91HX48T9QGE); 1 doc
  → vnoc relink 7N1HNCPA163HHSN91HX48T9QGE <node>
    or remove it in VS Code if the code is gone

1 problem
```

## The problems
- `missing`: a node with notes whose code is not found. `vnoc missing` lists them. If you know where the code went:
  `vnoc relink <ID> <node>` (the new place must be of the same kind and have no notes of its own). Otherwise tell the user; do not guess.
  A refusal says why:

```
vnoc relink 7N1HNCPA163HHSN91HX48T9QGE api/routes.py  # exit 2
error: this is a file, and the docs belong to a function
```

- `lost marker`: the `ID:` line is gone from the code. `vnoc repair` (all) or `vnoc repair <node>`.
- `broken`: a file in `.vnoc/` cannot be read. The line names the file and the fix.

## Exit codes
`0` done (also "found nothing") · `1` `check` found problems · `2` the request cannot be met (a bad flag, no such node) · `3` the
environment (no driver found, no driver started, a lock that could not be taken).
