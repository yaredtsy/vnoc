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
                       pub file: String, pub doc_path: String, pub title: String, pub body: String }
pub struct Corpus    { /* private: the nodes in reading order with their words split, the doc index */ }
impl Corpus { pub fn new(nodes: Vec<NodeEntry>, docs: Vec<DocEntry>) -> Corpus }

pub struct Limits { pub nodes: usize, pub docs: usize }       // default 200 and 50
pub fn parse(query: &str) -> Query
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results
pub fn matches(corpus: &Corpus, query: &Query, ids: &[&str]) -> Vec<bool>     // §5

pub struct Results { pub nodes: Vec<NodeHit>, pub nodes_total: usize, pub docs: Vec<DocHit>, pub docs_total: usize }
pub struct NodeHit { pub index: usize,                // into the corpus's nodes
                     pub tier: u8, pub skipped: u32,  // skipped: 02 §5, 0 unless the tier is 2
                     pub exact: bool,
                     pub ranges: Vec<(u32, u32)> }    // in the name, UTF-16 units (Q4), sorted and merged
pub struct DocHit  { /* S6 (03); an empty struct until then */ }
```

`exact` is the node a reference resolved to (§4): the crate puts it first; it does not find it.

## 2. The corpus

The index has no reading order of its own (`Index::tree` builds and sorts the tree on each call), and splitting 10,000 names into words on every
key would be waste. So the core builds a `Corpus` **once per change** and keeps it:

```
crates/vnoc/src/search.rs
  corpus_of(&Index, &Project) -> Corpus                the tree, walked in reading order (as indexNodes does today), without the root;
                                                       a function under a class is in_class; the index's doc texts (03 §5) joined to
                                                       their node, or Missing
  State.corpus: Mutex<Option<(revision, generation, Arc<Corpus>)>>
  the handler:  the pair (index revision, doc-text generation) is the key; a different pair → build, store; then run on the Arc
```

- Built on the first search after a change, not on every update: a project nobody searches pays nothing.
- **[measure]** (S1, S2) the build on 10,000 generated names. Under 20 ms it stays in the request. Over that, it is built at the end of `refresh`
  instead, off the request.
- A node's `id` and `path` are the tree's: the id is what the pages already use (the repo path, or the location key); the path is relative to the
  opened folder. `in:` therefore means what the user sees.

## 3. `vnoc/search`

```
vnoc/search  { query: string, nodes?: number, docs?: number }          → SearchResult
SearchResult {
  revision: number,
  nodes: [{ id, kind, name, path, line?, inClass, docs, tier, exact, match: [start, end][] }],
  nodesTotal: number,                      // how many matched before the limit
  docs:  [{ node: string | null,           // the tree's node id; null for a Missing node
            nodeId, nodeName, nodeKind?, nodePath, missing,
            file, docPath, title, titleMatch: [start, end][], snippet, snippetMatch: [start, end][] }],
  docsTotal: number,
  notes: string[],                         // 01 §2: what was wrong with a filter
}
```

- The limits default to 200 nodes and 50 docs.
- **Waits for the first build**, as `vnoc/structure` does.
- The handler takes the read lock, gets the corpus (§2), resolves a reference (§4), calls `run`. No `await` after the lock: it cannot be
  cancelled, and need not be (Q8).
- Types in `crates/vnoc/src/protocol.rs` and `shared/messages.ts`, as every request. The crate's `Results` and the protocol's `SearchResult` are
  two types with a plain conversion: the crate does not know the revision.

## 4. A reference: `node_ref`

`crates/vnoc/src/node_ref.rs`. Built here (S5) because the box needs it; doc 21's every `<node>` argument uses the same two functions.

| Form | Example | Resolved by |
|---|---|---|
| location | `src/orders.py::OrderService.parse_order` (`#2` for a second def of the same name) | `Index::def_at` |
| file + line | `src/orders.py:142` (`:142:7` is taken, the column ignored) | the innermost def whose range holds the line, else the file |
| ID | `0Q8V5W4T2F7KX9M3B6D1RZ4H7K` | `NodeId::parse`, then where the index found it (or Missing) |
| file / folder | `src/orders.py` · `src/billing/` | `Index::is_file`, `Index::is_folder` |

```rust
pub enum NodeRef { Location(String), FileLine { path: String, line: u32 }, Id(NodeId), Path(String) }
pub fn parse(text: &str) -> Option<NodeRef>                 // pure, a table test; a Windows drive's colon is not a line's
pub enum Found { Node(String /* the tree's id */), Missing(NodeId), Nothing }
impl Index { pub fn resolve_ref(&self, project: &Project, r: &NodeRef) -> Found }
```

- A path in a reference is relative to the **opened folder** (what the user sees and pastes), or absolute; `resolve_ref` turns it into a repo
  path with `Project::repo_path` / `repo_path_of`.
- In the box only a query that is **one token** is tried as a reference, and a bare path (`src/orders.py`) is not: it is already found by the
  path tiers, in order. The CLI tries all four forms.
- An ID that resolves to a Missing node gives no node row; its docs are found if the query's words find them. (The CLI says where it was.)

## 5. `vnoc/searchCards` (Q12)

```
vnoc/searchCards  { query: string, ids: string[] }      → { revision: number, hits: string[] }
```

"Of these nodes, which does the query find?" `hits` is the subset of `ids` that would be in the **unlimited** answer: by name, or because a doc
of the node is a doc hit. The canvas sends the ids of the cards it shows (tens), so the answer is exact where the first draft's 200-id list was
not. `vnoc_search::matches` does it without ranking or snippets.

## 6. Who calls what

| Caller | Request | Step |
|---|---|---|
| the sidebar's box | `vnoc/search` on each change | S3, S7 |
| the palette's QuickPick | `vnoc/search` on each change | S3, S7 |
| the canvas | `vnoc/searchCards` when the query or its cards change | S8 |
| `vnoc find`, `vnoc tree --where`, "did you mean" (21) | `vnoc_search::run` directly, on a corpus built by the same `corpus_of` | 21 |
