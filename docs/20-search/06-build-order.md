# 20.06 Build order

```
build order
├── 1. rules for every step
├── 2. the steps (S0 – S8; S9 proposed after the review)
├── 3. what each step leaves working
└── 4. tests, by kind
```

On a branch `search` from `main`, after edit-in-canvas is merged. Each step is one commit and leaves the branch green.

## 1. Rules for every step

- **Stage files by path.** Never `git add .` or `git add docs`. Don't touch `docs/futureplan/`.
- **Checks per commit, light:** `cargo test -p vnoc-search` and `cargo test -p vnoc` and `cargo clippy` when `crates/` changed; typecheck and the
  touched packages' unit tests when TypeScript changed; the window suite `VNOC_SUITE=search` on S3, S7, S8 (the suite is added in S3, in
  `extension/src/test/suite/`, registered as `edit` is). Full `pnpm -r test` on S3, S7 and S8. The window suite needs
  `env -u ELECTRON_RUN_AS_NODE`.
- **No logic in TypeScript** (P1). If a step finds itself comparing strings in a page to decide what matches, that belongs in `vnoc-search`.
- **`vnoc-search` gets no dependency** beyond `serde` without a row in the README's decisions.
- A step that settles a `[check]` or a `[measure]` writes the answer into the file that asked, in the same commit or a `docs:` commit after it.
- Spec gaps go into these docs, not into silent choices.

## 2. The steps

### S0 Two checks (no product change)

**Done 2026-11-17, as a reading, not a step.** What `indexChanged` costs is answered in 03 §5. The QuickPick's order needs a person and a
window, and matters only in S3, so it moved there: S3 builds the QuickPick on `vnoc/search` with `alwaysShow` items, and its by-hand pass says
whether the order holds; if not, the fallback of 05 §2 is S3's second commit.

### S1 The crate: names

| Files | What |
|---|---|
| `crates/vnoc-search/Cargo.toml`, `src/lib.rs` | the crate (Q7); `Kind`, `NodeEntry`, `DocEntry`, `Corpus::new`, `Limits`, `Results` (04 §1) |
| `.github/workflows/ci.yml` | `-p vnoc-search` in the `core` test step (clippy and fmt already cover the workspace) |
| `src/words.rs` | `name_words(&str) -> Vec<(start, end)>` (02 §3) |
| `src/names.rs` | the six tiers for one word; fuzzy by word with the retry; several words (02 §4); the order (02 §5) |
| `src/utf16.rs` | byte range → UTF-16 range |
| `src/run.rs` | `run` for nodes only: words, the order, the limit, `nodes_total`; `exact` first, marked, not repeated (it is in `run`'s signature, so it is built here, not in S5); `docs` empty |

`parse` in this step is only "split on spaces into words" (the real one is S4).

Tests (`cargo test -p vnoc-search`):

- every row of `webview/src/sidebar/search.test.ts` that is about ranking, ported, **giving the same order** (the old tiers 2 and 3 are now 4 and 5);
- `name_words`: `checkout_order`, `checkoutOrder`, `HTTPClient`, `sha256sum`, `__init__`, `_private`, `a.b-c d`, an empty name;
- tier 2: every row of 02 §3's table, with the ranges;
- tier 3: `OrderService.par` finds the method `parse_order` of `OrderService`, and not a function `parse_order` at module level;
- several words: 02 §4's two examples;
- UTF-16: `café_order` with `or`, and a name with an emoji before the match;
- the order: tier, fewer skipped, shorter, reading order (a table).
- `exact`: the given node first, `exact: true`, not repeated below.
- `#[ignore]` timing test, run by hand: `Corpus::new` and one `run` on 10,000 generated names **[measure]**, written into 04 §2.

By hand once: `cargo check -p vnoc-search --target wasm32-unknown-unknown` **[check]**, written into the README §5.

Commit: `search S1: the vnoc-search crate, names in six tiers, fuzzy by word`.

### S2 The corpus and `vnoc/search`

| Files | What |
|---|---|
| `crates/vnoc/Cargo.toml` | `vnoc-search = { path = "../vnoc-search" }` |
| `crates/vnoc/src/search.rs` (new) | `corpus_of(&Node)` over `Index::tree` (nodes only: no doc texts yet); `kept`, the kept corpus and its key (04 §2) |
| `crates/vnoc/src/protocol.rs` | `Search`, `SearchParams`, `SearchResult` and its rows (04 §3) |
| `crates/vnoc/src/server.rs` | the `search` handler: wait for the first build, read lock, corpus, `run`. `State.corpus` |
| `shared/messages.ts` | the result types only. The pages' `search` request is declared in S3, with its handler: `relay.ts` must serve every request `Requests` names, or the typecheck fails |

Tests: unit, `corpus_of` on a small tree gives the nodes in the tree's reading order, without the root, a method `in_class`; `kept` builds
once for two calls with one key and again for a new key (a counting closure, `Arc::ptr_eq`); `#[ignore]` timing, `tree::build` + `corpus_of` +
`Corpus::new` on 10,000 defs **[measure]** (04 §2); `lifecycle.rs`, `vnoc/search` on the fixtures answers before and after a file change with
the new revision. Nothing on screen changes yet.

Commit: `search S2: vnoc/search over a kept corpus`.

### S2b The corpus build, cheaper (added 2026-11-17)

S2 measured the build at 17 – 30 ms on 10,000 nodes, over 04 §2's 20 ms. The fix is in `crates/vnoc-search/src/{names.rs, words.rs}`: ASCII
text compared in place, only non-ASCII folded (04 §2, "Decided"). No change to the API, the tiers, the order or any expectation. Tests: every
S1 test unchanged; a check that the in-place path and the folded path agree on generated ASCII names and words (tier, skipped, ranges); a row
each with a non-ASCII name, qualname and path. Both timing tests report the first run and the median of five. **[measure]** into 04 §2.

Commit: `search S2b: ASCII names are matched in place; the corpus build is cheaper`.

### S3 The sidebar and the palette ask the core

*(The palette half was undone in S3b: VS Code re-sorts a QuickPick, so *V-NOC: Search…* now opens the box. 05 §2.)*

| Files | What |
|---|---|
| `shared/messages.ts`, `extension/src/relay.ts` | the `search` request in `Requests`, and its handler: `d.core("vnoc/search", params)` |
| `webview/src/sidebar/useSearch.ts` | asks on change (debounce), numbers the requests, draws only the newest answer (05 §1; the revision is not compared, Q5); asks again on `indexChanged`; `setSearch` and `searchDrawn` for the tests |
| `webview/src/sidebar/search.ts` | `rankNodes`, `matchRange`, `indexNodes`, `Tier` deleted; `splitMatch` takes several ranges; `searchKey`, `pickIntent` stay |
| `webview/src/sidebar/` (the result row) | bold from `match[]`; the header with the count and `of N` |
| `extension/src/search.ts` | the QuickPick on `vnoc/search`: every item `alwaysShow: true`, `items` set again on each answer. By hand: type a query whose core order is not alphabetical; if VS Code re-sorts it, a second commit takes 05 §2's fallback (the command focuses the sidebar's box; this file is deleted) |
| `webview/src/sidebar/search.test.ts` | the ranking rows deleted (they are in Rust); `splitMatch` with two ranges; `searchKey` rows stay |
| `extension/src/test/suite/` | the `search` suite; the two palette tests of `sidebar.test.ts` pass a starting query to `vnoc.search` (an empty value lists nothing now) |

Window tests: type in the box, the rows are the core's, in its order (a fixture where the core's order differs from the old one: a tier 2 hit);
type fast, only the last query's rows are drawn; a file saved while the box has text gives rows from the new tree. **[measure]** key to drawn
rows, written into 05 §1.

Commit: `search S3: the sidebar and the palette ask the core; rankNodes is gone`.

### S3b The palette's order, proven; no debounce (added 2026-11-17; done: VS Code re-sorts, so the fallback was taken, `6f8b188`)

| Files | What |
|---|---|
| `webview/src/sidebar/{useSearch.ts, search.ts, search.test.ts}` | no debounce (05 §1, "Decided"); the request numbering as a pure function, unit-tested with answers that arrive out of order |
| `extension/src/search.ts` | no debounce; the same numbering; `placeOf` from `shared/` (one copy) |
| `extension/src/test/suite/search.test.ts` | the order check walks every row of `e` and first asserts that the core's order is not the labels' sorted order (else it could not tell) |

If VS Code re-sorts, a second commit takes 05 §2's fallback. Commit: `search S3b: no debounce; the palette's order is checked on a query that can tell`.

### S4 The query: filters and phrases parsed

| Files | What |
|---|---|
| `crates/vnoc-search/src/query.rs` | `parse` (01), the `Query` type, `notes` |
| `src/run.rs` | filters applied to nodes: kinds, docs, inside, the `not_` forms; only-filters gives reading order |
| `protocol.rs`, `shared/messages.ts`, the sidebar | `notes` carried and drawn under the box |
| `crates/vnoc/src/search.rs` | `answer()` copies the query's `notes` into `SearchResult.notes` |
| `extension/src/test/suite/search.test.ts` | a filter query in the box; the timing hooks inside the suite (they were root hooks, S3b) |

Tests: the parse table (every row of 01 §2 and §4); `kind:fn no:doc in:billing/` on a fixture corpus; `-in:tests/`; `kind:method`; an unknown
value gives no nodes and a note; only filters, in reading order; phrases are parsed (and ignored by nodes).

Commit: `search S4: filters in the query`.

### S5 A pasted reference

| Files | What |
|---|---|
| `crates/vnoc/src/node_ref.rs` (new) | `NodeRef`, `parse`, `Found`, `Index::resolve_ref` (04 §4) |
| `crates/vnoc/src/index/mod.rs` | what `resolve_ref` needs: the innermost def at a line, where an ID was found |
| `crates/vnoc/src/server.rs` | a one-token query that parses as a location, file:line or ID is resolved; the node goes to `run` as `exact` |

Tests (the crate's side of `exact` is S1's): the `parse` table (every form, `#2`, `:12:5`, `C:\a.py:12`, a 26-character non-ID, an absolute path); `resolve_ref`: a line inside a
method gives the method, a line between defs gives the file, an ID gives its node, an ID of a Missing node gives `Missing`; `lifecycle.rs`:
`vnoc/search` with `nested.py:<line>` answers that def first.

Commit: `search S5: a pasted file:line, location or ID finds its node`.

### S6 Doc text (two steps: S6a the crate, S6b the core; split 2026-11-17)

S6a is the `vnoc-search` rows of the table below (`src/docs.rs`, `src/run.rs`, the crate's tests and timing), on `DocEntry` lists the tests
build by hand: 03 §8 pins its details. S6b is the rest (`DocTexts`, `refresh`, the corpus's docs, the protocol rows, the core's tests).
Commits: `search S6a: doc text ranked in the crate` and `search S6b: the core reads and keeps the doc text`.
S6a2 (added after S6a's timing): the doc index apart from the corpus, and cheaper to build (03 §9). Commit: `search S6a2: the doc index is
its own, and cheaper`.


| Files | What |
|---|---|
| `crates/vnoc/src/userdata/doc_text.rs` (new) | `DocTexts`: stamps, titles, bodies, `refresh`, `generation` (03 §5); `title_of` shared with `read.rs` |
| `crates/vnoc/src/index/{mod.rs, build.rs}` | the field, `set_doc_texts` (not part of `changed`), the call in `refresh` |
| `crates/vnoc/src/search.rs` | docs into the corpus, joined to their node or Missing; the key gets the generation |
| `crates/vnoc-search/src/docs.rs` | words of a text, the index, BM25, the title's weight, the prefix, phrases, the snippet (03) |
| `src/run.rs` | docs in the answer; filters by the doc's node; `docs_total` |
| `protocol.rs`, `shared/messages.ts` | the doc rows |

Tests (crate): title beats body; every word required; the prefix on the last word only; the 200-word prefix cap; a phrase across a line end; a
phrase that the words find but the text does not hold; the snippet's cut and its ranges, with `é` before the hit; a hit only in the title.
Tests (core): a doc body edited on disk → the same revision, a new generation, the next search has the new text, and **no `indexChanged` was
sent**; a doc of a Missing node is found and says so; `in:` and `kind:` narrow docs by their node. `#[ignore]` timing: 1,000 docs of 2 KB
**[measure]**, written into 03 §7.

Commit: `search S6: the words of every doc, ranked, with a snippet`.

### S7 Docs on screen

Its first commit is two core fixes from S6b's review (03, the last item): `DocEntry.missing`, and CRLF frontmatter.

| Files | What |
|---|---|
| the sidebar's result list | the two groups, the doc row, one list for the keys (05 §1) |
| `useSearch.ts` | a doc pick: reveal the node and open the doc (`openDoc`); a Missing node's doc opens in a tab |

Window tests: a word only a doc holds finds it; the pick reveals the node and the doc panel shows that doc; a Missing node's doc opens in a tab;
`↓` walks from the last node row to the first doc row.

Commit: `search S7: docs in the results`.

### S8 The canvas dims

| Files | What |
|---|---|
| `crates/vnoc-search/src/run.rs` (or `lib.rs`) | `matches` (04 §5) |
| `protocol.rs`, `server.rs`, `shared/messages.ts`, `relay.ts` | `vnoc/searchCards` |
| `shared/session.ts` | `search: string | null`, the intent that sets it; cleared with the box |
| `webview/src/sidebar/useSearch.ts` | dispatches it |
| the canvas | asks `searchCards` for its cards, dims the rest (05 §3), the `searchDim` note |

Tests: crate, `matches` agrees with `run` without limits on a table; session, the intent and its reset; window, with a query a hit card is not
dimmed and another is, a card whose **doc** is the hit is not dimmed, an empty box dims nothing. By hand: the dim level, in a light and a dark
theme.

Commit: `search S8: the canvas dims what the query does not find`.

### S9 After the review (proposed 2026-11-17, not built)

The whole branch was read at `e531e01`: the crate and the core's side line by line, the pages by a second reader. All tests pass
(`vnoc-search` 85, `vnoc` 106 and 42, shared 357, webview 749; the window suites were not run again). A scratch program ran the crate on
edge queries, and the probe drove the real core on a folder opened below its git root. **Nothing wrong was found in the ranking, the tiers,
the filters, the parse, BM25, the UTF-16 ranges, the wire types, the request numbering or the locks.** What was found:

| # | What | Seen | Fix | Tests |
|---|---|---|---|---|
| 1 | **A long pasted token stalls the core.** `fuzzy` (names.rs) allocates a table of (letters + 1) × (words + 1) for every node that has a word starting with the token's first letter, before anything can fail | on 10,000 names, release: a token of 2,000 chars 34 ms, of 20,000 chars **1.6 s**, per key, inside the handler | a word longer than the name (in folded chars) cannot be cut into its words' prefixes: `fuzzy` returns `None` first | unit: such a word is no match; the `#[ignore]` timing gets a 20,000-char word, under 5 ms |
| 2 | **The snippet can miss its own hit.** Its start is moved back to the start of a "word", meaning to the last white space; after a long URL or path that is hundreds of chars back, and the 120 chars end before the hit | a body with a 240-char URL and then `tail`: the query `tail` shows 120 chars of the URL, nothing bold; the same for a hit inside the URL | the start moves back to white space only if it is within `SNIPPET_BEFORE` more chars, else it is cut where it is (with `…`); the end is never before the end of the first hit | a hit right after a 300-char run, and one inside it: in the snippet, and bold |
| 3 | **A reference by repo path finds nothing when a folder below the git root is open** (S5b's second form, and more: it is the text of the tree's own ids, of a doc's `hint:`, and of what `vnoc` will print) | `repo/svc` open: `pkg/nested.py:25` and the absolute path resolve; `svc/pkg/nested.py:25` and `svc/pkg/nested.py::Outer.method` do not | `resolve_ref` tries the path, in order: relative to the opened folder; as a repo path; with the opened folder's own name dropped from its front (`backend-api/core/metering.py:110`, no git). The first the index has | `lifecycle.rs`, a temp folder with `.git` and the fixtures one level down: the three forms, each for `:line` and `::name` |
| 4 | **A traceback line is not a reference** (S5b's first form; proposed before, your call) | `File "core/metering.py", line 110, in add` → six words, nothing found | `parse` (query.rs): a whole query of the shape `File "<path>", line <n>` (anything after it ignored) is the reference `<path>:<n>`, and that token is its one word | the parse table: with and without `, in <name>`, a path with a space |
| 5 | **The dim outlives the box.** The page clears `session.search` in a React unmount, which does not run when VS Code disposes or reloads the page; the new page's box is empty and says nothing | text in the box, then *Developer: Reload Webviews* (or the view moved to another container): the canvas stays dimmed for a query nobody can see, until something is typed and cleared | the host clears it: `adoptSidebar`'s `onDidDispose` dispatches `search: null` and `highlight: null` | session unit: the intent; by hand: the reload |
| 6 | A request the core refuses leaves the last query's rows under the new text, and the last dim | core stopped, `foo` → `bar`: `foo`'s rows stay, `Enter` picks one | on a refusal that is still the newest: no rows, no dim | unit, on the two hooks' reject path if it can be reached without a window; else by hand |
| 7 | `sidebarDrew` is reset only when a page is adopted, not when one is disposed | read, not seen: a `focusSearch` sent between a dispose and the next adopt would be lost | reset in `onDidDispose` | — |
| 8 | Comments that describe an earlier step | `search.rs` `answer` ("sent as `missing`"), `protocol.rs` `SearchDoc` ("no doc is searched yet"), `lib.rs` `DocEntry` ("nothing reads it"), `shared/search.ts` and `shared/messages.ts` ("the palette"), `search.test.ts` ("over the debounce") | say what is | — |
| 9 | Two window tests check less than their names say | "only the newest query's answer is drawn" passes without the numbering (one core answers in order); the third pick (a doc of a node that is neither in the tree nor Missing → `openDoc`) has no test | the first: rename it to what it checks (the crossing is `newest`'s unit test); the second: add it | window `search` |
| 10 | **A file's own path does not put the file first.** A bare path is not tried as a reference (04 §4: "already found by the path tiers"), but the file is tier 4 with its own defs | on `backend-api`: `core/metering.py` → the file is row 29 of 52, under 28 of its defs; `core/retry.py` row 4 | a one-token query with a `/` is reference-shaped (`is_reference`), and the core no longer drops `NodeRef::Path`: a file or folder the index has is `exact`; a path that names nothing leaves the words as today | the crate's parse table; `lifecycle.rs`: `pkg/nested.py` answers the file first, `pkg/` the folder |
| 11 | **A line written as GitHub and editors write it** finds nothing | `core/metering.py:L110`, `core/metering.py#L110`, `…#L110-L120` → nothing | `parse` (node_ref.rs) and `is_reference`: `:L<n>`, `#L<n>` and `#L<n>-L<m>` are line `n` | the parse table: each form, `L` either case, `#L0`, `#Lx` (not a line) |
| 12 | **A file name without its folder** finds nothing (VS Code's Quick Open takes it) | `metering.py:110` → nothing | after the four tries of #3: if exactly one file in the index has that file name, that file; two or more, nothing exact (the words still find) | `lifecycle.rs`: a unique name resolves; a name two folders share does not |
| 13 | A name pasted from code with its call parens finds nothing | `parse_order(` and `parse_order()` → nothing (moved here from the by-hand list) | the parse: a word loses a trailing `()` or `(` | the parse table |

*(Built in S9a, settled where the rows did not say.)* A file name alone also serves `Location` and a bare `Path` (CLI), not only `:line`; in the
box a bare `orders.py` is still no reference. A file that failed to outline counts for a file name. `#L25-L30` is line 25 (the end is checked
for its shape, not used). `:L25:9` is line 25. A URL is reference-shaped (it has a `/`) and names nothing.

*(Built in S9b.)* #1: the guard is in `fuzzy` only; `find`, `starts_with` and the other tiers already return at once for a needle longer than the text,
so nothing else did work per node that grew with the token (a 20,000-char word on 10,000 names: 300 ms before, 0.4 ms after, release). #2: a hit
longer than 80 chars may not fit the 120-char window, so the end is never moved back past the end of the first hit, up to the window's end; a longer
hit is cut with the window and is not bold. #13: `f(x)` is left as it is (only a trailing `(` or `()` goes), and `()` alone gives no word.

Three commits, in this order:

- **S9a** `search S9a: references as people paste them`: #3, #4, #10, #11, #12 (node_ref.rs, query.rs, search.rs, index).
- **S9b** `search S9b: a long token, a snippet's hit, call parens`: #1, #2, #13, and #8's Rust comments.
- **S9c** `search S9c: the dim ends with the box's page`: #5, #6, #7, #9, and #8's TypeScript comments.

04 §4 and 01 §5 change with S9a (a bare path with a `/` is tried; the new line forms; the four tries and the file name). A `docs:` commit after: 05 §3 says "until the first answer for a query arrives, nothing is dimmed", and as built the last
query's dim stays until the new answer (about 5 ms; no flash between keys): the built behaviour is kept and the sentence corrected; 05 §4
still lists "the debounce".

**For the by-hand pass, not fixes:** a doc pick of a node in the tree also runs the click rule, so in `code` click mode it opens the source
file beside the docs tab (05 §1 says reveal and docs tab); the dim stays while the STRUCTURE view is hidden with text in its box (the page is
kept).

**Not in S9:** *Add Doc* setting a tag or an author waits for doc 21's A8 (`tags:` and `author:` exist only from there); a step after A8 if
wanted.

## 3. What each step leaves working

| After | A user can | A caller can |
|---|---|---|
| S1 | nothing new | `vnoc_search::run` on names |
| S3 | search as today, plus `chor` and `OrderService.par` | `vnoc/search` |
| S4 | `kind:fn no:doc in:billing/` | the same query string anywhere |
| S5 | paste `orders.py:142` | `node_ref` (21 starts from it) |
| S7 | find a doc by a word in it | — |
| S8 | see the hits on the canvas | `vnoc/searchCards` |

S1 – S5 are enough for doc 21 to start: the CLI needs the crate, the corpus and `node_ref`, not the screens.

**All steps done, 2026-11-17:** S0 – S8 with S2b, S3b, S6a, S6a2 and S6b, on branch `search`. Left before the PR: the by-hand pass
(the dim level in a light and a dark theme; *V-NOC: Search…* puts the keyboard in the box; a doc pick opens that doc), and S9 if you
take it (reviewed 2026-11-17: nine findings, none in what the search finds or how it orders it; four more from the hands-on pass on
`backend-api`, #10 – #13).

## 4. Tests, by kind

a. **Crate** (`cargo test -p vnoc-search`): names, the parse, filters, docs, `matches`. No disk, no async: tables.
b. **Core** (`cargo test -p vnoc`): `corpus_of`, `node_ref`, `DocTexts`, and `lifecycle.rs` for the two requests.
c. **Pages** (unit): `splitMatch`, `searchKey`, the session's `search` field.
d. **Window** (`VNOC_SUITE=search`): the core's order on screen, the newest answer wins, a doc hit and its pick, the dim.
e. **By hand**: the wasm check, the timings, the dim level.
