# 05.03 Files: the `# FileID:` comment

```
file markers
├── 1. what it looks like, and where exactly it goes
├── 2. driver work
├── 3. files that no driver understands
├── 4. if this slips out of 0.1
└── 5. a cheap win right after 0.1
```

## 1. What it looks like

```python
#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Copyright 2026 Acme. Licensed under MIT.
# FileID: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K
"""Orders module."""

import json
```

Placement, in order (unchanged from `plan_vs/02-technology/05-identity-and-archive.md` §2, which thought this through well):

1. after a shebang line (`#!…`), which must stay first for the OS
2. after an encoding line (`# -*- coding: … -*-`), which Python only honours on line 1 or 2
3. after a leading block of comments (a licence header), because licence checkers expect it at the top
4. **before** the first statement, including the module docstring. A comment before the docstring does not change `__doc__`.

In an empty file, it is simply the first line.

Reading is tolerant (`#FileID:`, extra spaces). Writing is one canonical form.

## 2. Driver work

The file marker was a post-MVP item in the plan. You asked for it now. It needs two additions to the driver protocol:

| Addition | Shape |
|---|---|
| `vnoc/outline` also returns the file's ID | a new top-level field next to `defs`: `"file_id": "0Q8V…"` or `null`. Additive: old readers ignore it. |
| `vnoc/planFileId` (capability `ids.file`) | request `{ text, version, id }` → response `{ version, edits, written: { span, text }, text_hash }`. Same pattern as `planIds`. Already prototyped in spikes S2/S3. |

The core then treats it exactly like a def marker: it goes into the index's `markers` map as "ID → this file".

## 3. Files that no driver understands

A README, a YAML config, a SQL migration: there is no driver to say where a comment may go, and some formats (JSON) have no comments at all.

**Rule:** a file gets a marker only if a driver claims its extension *and* announces the `ids.file` capability. Every other file is attached by path only
(`id_in_code = "no"`). It works; it goes Missing if it is renamed.

To document those files in 0.1, the tree must show them. Today the scan lists only `.py` files. Showing non-code files is a UI choice you can make later
(a "show all files" toggle); the identity model already handles them.

## 4. If this slips out of 0.1

If porting `planFileId` takes longer than expected, ship 0.1 with files attached **by path only** and add the comment in 0.1.x:

- no format changes: those nodes have `id_in_code = "no"`, a state that already exists
- when the marker arrives, each such file node gets a "Write ID" action, and new attaches write it automatically

Functions, classes and folders do not depend on it.

## 5. A cheap win right after 0.1

A file marker (or a folder marker) can rescue the *unmarked* nodes inside it.

```
node.toml of a function WITHOUT its own marker:   path = "src/orders.py", qualname = "parse_order"
src/orders.py is renamed to src/order_parsing.py  → the anchor's path no longer exists → Missing

but: the FILE has a marker, and V-NOC can see it moved from src/orders.py to src/order_parsing.py
  → every anchor under the old path can be retried under the new path
  → src/order_parsing.py::parse_order exists → Found
```

The same works one level up: a folder with a `.vnoc-id` that moved lets every anchor under the old folder path be retried under the new one.

This is a small piece of the 0.4 matching work that needs no hashing, no git and no guessing. It only helps nodes in `none` mode or with a failed marker,
so it is not needed for 0.1, but it is the first thing I would add after it.

Next: [`04-folders.md`](04-folders.md)
