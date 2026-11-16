# 20.01 The query

```
the query
├── 1. the parts          words · phrases · filters · a reference
├── 2. filters
├── 3. what each part matches
├── 4. edge cases
└── 5. the parse, as a type
```

One language (Q9). The box, the palette, `vnoc find '<query>'` and `vnoc tree --where '<query>'` all hand the same string to `vnoc_search::parse`.

## 1. The parts

A query is split on spaces, except inside double quotes.

| Part | Looks like | Example |
|---|---|---|
| **word** | anything else | `chor` · `OrderService.par` · `billing/` |
| **phrase** | `"…"` | `"retry policy"` |
| **filter** | `name:value`, where `name` is one of §2's | `kind:fn` · `no:doc` · `in:billing/` |
| **excluding filter** | `-name:value` | `-in:tests/` · `-kind:file` |
| **reference** | the whole query is one token that names a node | `src/orders.py:142` · `src/orders.py::OrderService.parse_order` · `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` |

## 2. Filters

| Filter | Keeps | Notes |
|---|---|---|
| `kind:folder` `kind:file` `kind:class` `kind:function` `kind:method` | nodes of that kind | short forms `fn`, `cls`. A method is a function inside a class: `kind:function` keeps methods too, `kind:method` keeps only them. Several with a comma: `kind:class,fn` |
| `has:doc` | nodes with at least one doc | |
| `no:doc` | nodes with none | "functions with no docs in billing" is `kind:fn no:doc in:billing/` |
| `in:<path>` | nodes whose path starts with it | relative to the opened folder, `/` separators, case ignored. `in:billing` and `in:billing/` are the same |

- `-` in front turns a filter round: `-in:tests/` drops what is under `tests/`. `-has:doc` is `no:doc`. A `-` in front of a **word** is not special
  (`-1` is a word).
- Two filters of the same name: the node must pass both for `in:` (`in:a/ in:a/b/` is the narrower), and either for `kind:` (`kind:fn kind:class`
  is `kind:fn,class`).
- A filter with a value that means nothing (`kind:fnn`, `has:code`) keeps **no node** and adds a line to the answer's `notes` saying what the
  values are. It is not an error: the box is being typed in.
- A name that is not a filter (`http://x`, `self:`) is a **word**.
- With their keys (21 C16, built in step A8): `tag:<t>` and `author:<a>`. On **docs** they keep the docs that carry the tag or the author. On
  **nodes** they keep the nodes that have at least one such doc (`kind:fn tag:plan` is "the functions with a plan"). The parser takes a list of
  known names, so adding one is a row.

## 3. What each part matches

| The query has | Nodes | Docs |
|---|---|---|
| nothing (empty, or only spaces) | none | none (the page does not ask) |
| only filters | every node that passes, in reading order | none |
| words | names, qualified names, paths ([`02-names.md`](02-names.md)); filters narrow | the docs that hold every word ([`03-doc-text.md`](03-doc-text.md)); filters narrow **by the doc's node** |
| a phrase | none (a phrase is for text) | the docs that hold the phrase |
| a reference | that node, first, marked `exact` | none, unless the token is also a word some doc holds |

"By the doc's node": `in:billing/ retry` finds docs attached to nodes under `billing/`; `kind:class retry` docs attached to classes. A Missing
node's doc passes `in:` by its last known path and `kind:` by the kind in its `node.toml`; it never passes `has:doc`-style questions differently
(it has docs).

A query with words **and** a phrase: nodes are matched by the words only; docs must hold the words and the phrase.

## 4. Edge cases

| Input | Result |
|---|---|
| `"unclosed phrase` | the quote is dropped; the rest are words |
| `""` | ignored |
| `kind:` (no value) | a word (the user is still typing the value) |
| `KIND:FN` | the filter (names and values ignore case) |
| `in:"my folder/"` | a quoted value keeps its space |
| a 26-character token that is not a valid ID | a word |
| `a.py:12:5` (line and column) | a reference to line 12 |
| `C:\repo\a.py:12` | a reference: the drive's colon is not the line's (`node_ref::parse`, S5) |
| two tokens, one a reference | no reference: both are words (a reference is the whole query) |

## 5. The parse, as a type

```rust
pub struct Query {
    pub words: Vec<String>,          // lower case
    pub phrases: Vec<String>,        // lower case, inner spaces single
    pub kinds: Option<Vec<KindFilter>>,   pub not_kinds: Vec<KindFilter>,
    pub docs: Option<bool>,          // has:doc → Some(true), no:doc → Some(false)
    pub inside: Vec<String>,         pub not_inside: Vec<String>,
    pub reference: Option<String>,   // the raw token; the core resolves it (04 §4)
    pub notes: Vec<String>,          // what was wrong with a filter
}
pub fn parse(query: &str) -> Query      // pure; never fails
```

`vnoc-search` only **recognises the shape** of a reference (it has `::`, or ends in `:<digits>`, or is 26 ID characters). Finding the node needs the
index, so that is the core's (`crates/vnoc/src/node_ref.rs`): the crate stays free of paths and disks.
