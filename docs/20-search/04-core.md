# 20.04 The crate and the requests

```
the core's side
├── 1. the vnoc-search crate     what it may and may not know
├── 2. the corpus                built once per change, kept
├── 3. vnoc/search
├── 4. a reference               node_ref: parse and resolve
├── 5. vnoc/searchCards          which of these cards match
└── 6. who calls what
```

## 1. The `vnoc-search` crate (Q7)

```
crates/vnoc-search/                 depends on: serde (derive). Nothing else.
├── src/lib.rs        Corpus · NodeEntry · DocEntry · Kind · Limits · Results · parse · run · matches
├── src/words.rs      a name into words · a text into words
├── src/names.rs      the tiers, fuzzy by word, the name's ranges
├── src/utf16.rs      a byte range of a string → a UTF-16 range
├── src/query.rs      parse (01)
├── src/docs.rs       the word index, BM25, the prefix, the phrase check, the snippet (03)
└── src/run.rs        filters → names → docs → the order → the limits
```

| It knows | It does not know |
|---|---|
| a list of nodes: id, kind, name, qualified name, path, line, in a class or not, how many docs | the index, `RepoPath`, `NodeId`, the driver |
| a list of docs: its node, title, body, missing or not | files, `.vnoc/`, stamps |
| a query string | the editor, LSP, `tokio`, the clock |

That is the whole porting story: **an editor, a CLI or a web page that can produce those two lists gets the same search.** `crates/vnoc` produces
them from the index (§2). The crate has no `unsafe`, no threads and no I/O, so it builds for `wasm32-unknown-unknown` **[check]** (S1, by hand).

```rust
pub struct NodeEntry { pub id: String, pub kind: Kind, pub name: String, pub qualname: Option<String>,
                       pub path: String, pub line: Option<u32>, pub in_class: bool, pub docs: usize }
pub struct DocEntry  { pub node: Option<usize>,          // index into the nodes; None for a Missing node
                       pub node_id: String, pub node_name: String, pub node_kind: Option<Kind>, pub node_path: String,
                       pub file: String, pub doc_path: String, pub title: String, pub body: String,
                       pub tags: Vec<String>,            // the frontmatter's tags: (lower case)
                       pub author: Option<String> }      // the frontmatter's author:
pub struct Corpus    { /* private: the nodes in reading order with their words split, the doc index */ }
impl Corpus { pub fn new(nodes: Vec<NodeEntry>, docs: Vec<DocEntry>) -> Corpus }

pub struct Limits { pub nodes: usize, pub docs: usize }       // default 200 and 50
pub fn parse(query: &str) -> Query
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results
pub fn matches(corpus: &Corpus, query: &Query, exact: Option<usize>, ids: &[&str]) -> Vec<bool>     // §5

pub struct Results { pub nodes: Vec<NodeHit>, pub nodes_total: usize, pub docs: Vec<DocHit>, pub docs_total: usize }
pub struct NodeHit { pub index: usize,                // into the corpus's nodes
                     pub tier: u8, pub skipped: u32,  // skipped: 02 §5, 0 unless the tier is 2
                     pub exact: bool,
                     pub ranges: Vec<(u32, u32)> }    // in the name, UTF-16 units (Q4), sorted and merged
pub struct DocHit  { /* S6 (03); an empty struct until then */ }
```

`exact` is the node a reference resolved to (§4): the crate puts it first; it does not find it.

Its row has `tier: 0`, `exact: true` and no ranges; it counts once in `nodes_total` and takes one slot of the limit. A query with no words gives
nothing even with `exact`: the core passes `exact` only for a one-token query (§4), which is a word. (Settled in S1, 2026-11-17.)

## 2. The corpus

The index has no reading order of its own (`Index::tree` builds and sorts the tree on each call), and splitting 10,000 names into words on every
key would be waste. So the core builds a `Corpus` **once per change** and keeps it:

```
crates/vnoc/src/search.rs
  corpus_of(root: &index::Node) -> Corpus             the tree `Index::tree` gives (the one vnoc/structure sends), walked depth-first in
                                                       its order, without the root; a function whose parent is a class is in_class;
                                                       line = range.start.line. S6 adds the doc texts (03 §5) as a second argument
  kept(slot: &Mutex<Option<(u64, Arc<Corpus>)>>, key: u64, build: impl FnOnce() -> Corpus) -> Arc<Corpus>
                                                       the same key → the kept Arc; another → build, keep, return. The lock is held while
                                                       building, so two searches at once build once
  State.corpus: Mutex<Option<(u64, Arc<Corpus>)>>      the key is the index revision; S6b keeps the doc index apart, under the doc-text
                                                       generation (03 §9), and the corpus's key becomes (revision, generation)
  the handler:  index read lock, then State.corpus (nothing else takes it, so the order cannot deadlock); run on the Arc
```

- Built on the first search after a change, not on every update: a project nobody searches pays nothing.
- **Measured** (S1, 2026-11-17; `cargo test -p vnoc-search --release -- --ignored --nocapture`; Apple M1, macOS 14.7.6): 10,000 generated names in 100 files.
  `Corpus::new` took **14 – 20 ms** (19.0, 15.3 and 14.2 ms in three runs; 20.5 ms in a fourth), and one `run` for `co` (2,796 found, 200 shown)
  **1.6 – 2.4 ms**. The run is small; the build sits **on the 20 ms line**: under it, it stays in the request; over it, it is built at the end of
  `refresh` instead, off the request. S2 measures `corpus_of` and `Corpus::new` together on 10,000 nodes and applies the rule. If it is over, the first fix is in
  the crate, not a move: `Prepared::new` folds each node's path on its own, so the 100 paths of the test are folded 10,000 times; folding each
  distinct path once and sharing it is cheaper than building off the request.
- **Measured** (S2, 2026-11-17; `cargo test -p vnoc --release --lib -- --ignored --nocapture search`; Apple M1, macOS 14.7.6): 10,000 defs in 100 files
  across 10 folders (10,110 nodes with the files and folders). Four runs: `tree::build` **4.2 – 5.4 ms**, `corpus_of`'s walk **1.0 – 1.5 ms**,
  `Corpus::new` **11.8 – 23.2 ms**; the total **17.0, 21.1, 29.9 and 23.1 ms**. The first search after a change therefore costs about the 20 ms line
  and above it in three runs of four, and the build is mostly `Corpus::new`, as in S1. **Not applied:** the rule says that over 20 ms the design does
  not change here and nothing is optimised; the choice between the crate's path folding (above) and building at the end of `refresh` is the planner's.
  Under the watchdog's 50 ms (`watchdog.rs` `LIMIT`) in every run: the build runs inside one poll of the handler (a debug build is slower, and
  was not measured).
- **Decided** (2026-11-17, after S2): over the line, so the fix is in the crate (step S2b), not a move. `Prepared::new` made about seven
  allocations per node (the folded name, a 16-byte span per char, the words and a scratch list, the folded qualname and path), and the spread
  of 12 – 23 ms is the allocator's, not the computing. Lower-casing ASCII never changes a length, so **ASCII text is compared in place**
  (`eq_ignore_ascii_case`, byte offsets are char offsets): no folded copy and no spans. Only a name, qualname or path that is not ASCII is folded
  as before. The words are kept in one `Vec<(u32, u32)>` and split without a scratch list. Target: the median total of S2's timing at **20 ms or
  less** on the M1. If S2b does not reach it, the next step is "keep it warm": after an update, if a corpus is kept, rebuild it in the
  background, so the first key after a save does not pay. Building at the end of every `refresh` stays out: a project nobody searches pays
  nothing.
- **Measured** (S2b, 2026-11-17; the two `--ignored` timing tests in release, six builds in one process each: the first, and the median of the other
  five; Apple M1, macOS 14.7.6; three invocations each). Before, in the same order: S1's `Corpus::new` on 10,000 names **14 – 20 ms**; S2's core
  total **17.0 – 29.9 ms** (`Corpus::new` 11.8 – 23.2 ms, the walk 1.0 – 1.5 ms, `tree::build` 4.2 – 5.4 ms).

  | | before | first run | median of 5 more |
  |---|---|---|---|
  | `Corpus::new`, the crate's test (10,000 names) | 14 – 20 ms | **1.3 – 1.7 ms** | **3.2 – 4.2 ms** |
  | `Corpus::new`, the core's test (10,110 nodes) | 11.8 – 23.2 ms | **1.0 – 1.3 ms** | **0.86 – 1.0 ms** |
  | `corpus_of`'s walk | 1.0 – 1.5 ms | 1.0 – 1.5 ms | 1.1 – 1.3 ms |
  | `tree::build` | 4.2 – 5.4 ms | 3.9 – 5.7 ms | 3.7 – 4.9 ms |
  | **the core's total** | **17.0 – 29.9 ms** | **5.9 – 8.5 ms** | **5.6 – 7.3 ms** |

  The median total is under 20 ms in every invocation, and under the watchdog's 50 ms by a wide margin: **the corpus stays in the request**; no
  "keep it warm". `Corpus::new` is now the smallest part of the build; what is left is `tree::build` (the tree `vnoc/structure` also builds). One
  `run` for `co` went from 1.6 – 2.4 ms to 1.3 – 1.6 ms. (In the crate's test the median is above the first run: each build there is given a clone of
  the 10,000 entries, and the allocator's state after the first few is what it shows; the core's test, which builds from a fresh tree, does not.)
- A node's `id` and `path` are the tree's: the id is what the pages already use (the repo path, or the location key); the path is relative to the
  opened folder. `in:` therefore means what the user sees.

## 3. `vnoc/search`

```
vnoc/search  { query: string, nodes?: number, docs?: number }          → SearchResult
SearchResult {
  revision: number,
  nodes: [{ id, kind, name, path, line?, in_class, docs, tier, exact, match: [start, end][] }],
  nodes_total: number,                      // how many matched before the limit
  docs:  [{ node: string | null,           // the tree's node id; null for a Missing node
            node_id, node_name, node_kind?, node_path, missing,
            file, doc_path, title, title_match: [start, end][], snippet, snippet_match: [start, end][],
            tags: string[], author: string | null }],        // A8b: the doc's frontmatter, [] and null when it has none
  docs_total: number,
  notes: string[],                         // 01 §2: what was wrong with a filter
}
```

- The limits default to 200 nodes and 50 docs.
- **Waits for the first build**, as `vnoc/structure` does.
- The handler takes the read lock, gets the corpus (§2), resolves a reference (§4), calls `run`. No `await` after the lock: it cannot be
  cancelled, and need not be (Q8).
- Field names are snake_case on the wire, as every other request's are (`git_root`, `syntax_error`, `call_count`); no `rename_all`.
- Types in `crates/vnoc/src/protocol.rs` and `shared/messages.ts`, as every request. The crate's `Results` and the protocol's `SearchResult` are
  two types with a plain conversion: the crate does not know the revision.

## 4. A reference: `node_ref`

`crates/vnoc/src/node_ref.rs`. Built here (S5) because the box needs it; doc 21's every `<node>` argument uses the same two functions.

| Form | Example | Resolved by |
|---|---|---|
| location | `src/orders.py::OrderService.parse_order` (`#2` for a second def of the same name) | `Index::def_at` |
| file + line | `src/orders.py:142` (`:142:7` is taken, the column ignored), `:L142`, `#L142`, `#L142-L150` (line 142) | the innermost def whose range holds the line, else the file |
| ID | `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` | `NodeId::parse`, then where the index found it (or Missing) |
| file / folder | `src/orders.py` · `src/billing/` | `Index::is_file`, `Index::is_folder` |

```rust
pub enum NodeRef { Location(String), FileLine { path: String, line: u32 }, Id(NodeId), Path(String) }
pub fn parse(text: &str) -> Option<NodeRef>                 // pure, a table test; a Windows drive's colon is not a line's
pub enum Found { Node(String /* the tree's id */), Missing(NodeId), Nothing }
impl Index { pub fn resolve_ref(&self, project: &Project, r: &NodeRef) -> Found }
```

- A path in a reference is tried in this order, and the first the index has is used: (a) relative to the **opened folder** (what the user sees
  and pastes); (b) as a **repo path**, relative to the git root (the tree's own ids, e.g. `svc/pkg/nested.py` with `repo/svc` open); (c) with
  the opened folder's own name dropped from the front (`backend-api/core/metering.py:110`). An absolute path is only itself. If none names
  anything and the path has no `/`, **a file name alone** names the one file of the index with that file name (a file that failed to outline
  counts); two or more, and nothing is exact. `#L0` is line 0, like `:0` (the file); `#Lx` is not a line, so the text is a path.
- In the box only a query that is **one token** is tried as a reference. A bare path with a `/` (`src/orders.py`, `src/billing/`) is one, so a
  file or folder the index has is `exact` and first; a path that names nothing leaves the words as they are. A name with no `/` and no line
  (`orders.py`) is no reference in the box: the path tiers find it. The CLI tries all the forms.
- An ID that resolves to a Missing node gives no node row; its docs are found if the query's words find them. (The CLI says where it was.)
- *(Settled for S5, 2026-11-17.)* A pasted line is **1-based**, as an editor and a stack trace show it; a def's `range` is 0-based. "Innermost" is
  the holding def that starts last (ties: the one that ends first). A line in no def, or past the end, gives the file.
- `Found::Node` carries the **tree's** id, the one the corpus has: the location key's text for a def, the repo path for a file or a folder (a
  file's or folder's location key has an empty qualname). The server finds its place with `Corpus::position(id)`, a scan: one lookup per query.
- `resolve_ref` uses only public `Index` methods; what it needs and the index does not have yet is added in `index/mod.rs`: the innermost def at
  a line, and where an ID was found (from `attached`). Those two get unit tests there; `resolve_ref` itself is tested through `vnoc/search` in
  `lifecycle.rs`, on the real driver and real records.

## 5. `vnoc/searchCards` (Q12)

```
vnoc/searchCards  { query: string, ids: string[] }      → { revision: number, hits: string[] }
```

"Of these nodes, which does the query find?" `hits` is the subset of `ids` that would be in the **unlimited** answer: by name, or because a doc
of the node is a doc hit. The canvas sends the ids of the cards it shows (tens), so the answer is exact where the first draft's 200-id list was
not. `vnoc_search::matches` does it without ranking or snippets.

*(Settled for S8.)* The handler parses and resolves a reference exactly as `vnoc/search` does (one shared function in `search.rs`), and
`matches` takes the `exact` place too: a pasted `nested.py:25` keeps that card bright. `matches(corpus, query, exact, ids)` agrees with
`run` given no limits: an id is a hit when its node is a node row, or the node of a doc row. An id the corpus does not have is not a hit.

## 6. Who calls what

| Caller | Request | Step |
|---|---|---|
| the sidebar's box | `vnoc/search` on each change | S3, S7 |
| the canvas | `vnoc/searchCards` when the query or its cards change | S8 |
| `vnoc find`, `vnoc tree --where`, "did you mean" (21) | `vnoc_search::run` directly, on a corpus built by the same `corpus_of` | 21 |
