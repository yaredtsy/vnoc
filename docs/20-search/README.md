# 20. Search in the core (release 0.4)

Written 2026-11-16 from the code at `0597d12` (edit-in-canvas 9.5c), as a review and a rewrite of the first draft (`20-search.md`, same day, now
replaced by this folder). Read for it: `webview/src/sidebar/search.ts` and `useSearch.ts`, `extension/src/search.ts`, `extension/src/relay.ts`,
`crates/vnoc/src/{server.rs, index/, userdata/}`, `shared/calls.ts`. **Nothing here was run.** Every `[check]` is a claim to verify before
depending on it; every `[measure]` is a number to write here once a test gives it. Starts after edit-in-canvas (17) is merged.

```
search
├── README (this file)      what ships · decisions · what the review changed · open checks
├── 01-query.md             words, phrases, filters, a pasted node reference
├── 02-names.md             name words, the six tiers, fuzzy by word, the ranges to draw bold
├── 03-doc-text.md          which text is kept, the word index, BM25, phrases, the snippet, keeping it fresh
├── 04-core.md              the vnoc-search crate, the corpus, vnoc/search, vnoc/searchCards, revisions
├── 05-on-screen.md         the sidebar's two groups · the QuickPick · the canvas dims
└── 06-build-order.md       steps S0 – S8, each with files, tests, and its commit message
```

## 1. What it ships

The search box finds a node by a **few letters of its name** (`chor` → `checkout_order`), by its **qualified name** (`OrderService.par`), by a
**pasted reference** (`src/orders.py:142`, an ID), narrows by **filters** (`kind:fn no:doc in:billing/`), and finds **docs by the words in
them** (`retry`, `"retry policy"`), with a snippet. While the box has text, the canvas **dims every card that is not a hit**.

All of it is one pure Rust crate. The sidebar, the palette, `vnoc find` and `vnoc tree --where` (doc 21) and any other editor call it and draw the
answer. None of them ranks, filters or parses anything.

| In | Out (later) |
|---|---|
| node names, qualified names and paths, fuzzy by word | search by meaning (embeddings): agents do that, through doc 21 |
| `kind:` `has:doc` `no:doc` `in:`, and `-` in front of a filter to exclude | searching code bodies in the box (`vnoc grep`, 21, answers in nodes; the editor's own search answers in lines) |
| the words of every doc, title weighted; `"a phrase"` | typo tolerance, stemming (`retries` does not find `retry`) |
| a pasted `file:line`, `file::Name` or ID finds that node | `tag:` `author:` filters (they arrive with the keys, 21 §write) |
| the canvas dims what is not a hit | saved searches, a search history |

## 2. Decisions

Q7 was the one ask; you said yes on 2026-11-16. The others follow from P1 or from the code.

| # | Decision | Why |
|---|---|---|
| **P1** | **Logic in the Rust core; UI code only draws and calls the core.** (`08` §1c.) | V-NOC will be ported to other editors. |
| **Q1** | Search runs in the core, behind `vnoc/search`. The TypeScript `rankNodes` is deleted; its table tests move to Rust. | P1. The round trip is a local pipe **[measure]** (S3). |
| **Q2** | The doc text lives **in memory**, read when the core loads `.vnoc/`, re-read only for files whose size or time changed. Nothing on disk. It has **its own counter and never moves the index revision**. | A thousand docs of a few KB is a few MB. The index revision means "the code or the records changed": call trees are marked stale by it (`shared/calls.ts` `isStale`), and a doc's body is neither. |
| **Q3** | **No search library**: the word index and BM25 are about 300 lines of our own. `tantivy` or SQLite FTS5 only if a measured project needs them. | Size, and nothing here needs them. |
| **Q4** | Ranges in the answer are in **UTF-16 code units**, as LSP's are. | Rust indexes by byte, JavaScript by UTF-16: a name with `é` would draw the wrong letters bold. |
| **Q5** | The answer carries the index's **revision**; a page drops an answer older than the tree it shows. | The tree can change between the key and the answer. |
| **Q6** | No meaning search in the extension. | A 30–100 MB model download, or code sent to an API. |
| **Q7** *(new; yes, 2026-11-16)* | Search is its **own crate, `crates/vnoc-search`**: no disk, no async, no LSP types, one dependency (`serde`). It takes plain lists (names, doc texts) and gives plain results. | "Portable" then means something the compiler checks: the crate cannot reach the editor or the disk, so it also builds for a browser (`wasm32`) where no native program runs. The cost is one `Cargo.toml`. As a module of `crates/vnoc` it would work the same today and nothing would stop it from growing a dependency on the server. |
| **Q8** *(new)* | **No cancelling.** A page numbers its requests and draws only the answer to the newest. | The first draft's cancel cannot work as written: `relay.ts` has no cancel message, and `ConcurrencyLayer` drops a request only at an `await`, which a search (a few ms of plain computing under a read lock) does not have. |
| **Q9** *(new)* | **One query language** for the box, the palette, `vnoc find` and `vnoc tree --where`. A CLI flag is never a second way to say a filter. | One parser, one table test, one thing to teach (a person or an agent). |
| **Q10** *(new)* | A query that **names one node** (`file:line`, `file::Name`, an ID) is answered exactly, first, by `node_ref`, the same function doc 21's commands use. | Pasting a stack-trace line into the box is the same act as `vnoc show src/orders.py:142`. Built once, here. |
| **Q11** *(new)* | **Quoted phrases are in.** | The first draft left them to a search library. With the text in memory a phrase is a substring check on the docs the words already found. |
| **Q12** *(new)* | The canvas **asks the core which of its cards match** (`vnoc/searchCards`); the session carries only the query text. | The first draft sent the hit list (at most 200) through the session: a card that matched but was hit 201 would be dimmed. The canvas shows tens of cards; asking about those is exact and small. |
| **Q13** *(new)* | A doc's title is its **first `# heading`**, else the file name: `userdata/read.rs` `title_of`, reused. | The first draft read a `title:` key from the frontmatter. There is none: `create_doc` writes `node:` and `hint:` only. |

## 3. What the review changed

The first draft's shape was right (one request, tiers, BM25, P1). These are the places where it did not match the code, or left a hole:

| # | First draft | What the code says | Now |
|---|---|---|---|
| 1 | cancel the request in flight (§7) | `relay.ts` forwards requests only; the webview's `request()` has no cancel; a search never awaits | Q8: newest answer wins. One step shorter |
| 2 | title from frontmatter `title:` (§6) | no such key | Q13 |
| 3 | "the doc index is rebuilt … under the same revision" | `refresh` calls `load_all` on every update, but `NodeRecord` equality is the `node.toml` text and the **file names**: an edited body changes nothing, so nothing would rebuild | 03 §5: per-file stamps (size and time) decide what is read again |
| 4 | (implied) a doc edit moves the revision | the revision marks call trees stale and re-fetches them | Q2: its own counter |
| 5 | `run(&Index, …)` "over the index" | the index has no reading order: `Index::tree` builds and sorts the tree on each call | 04 §2: a **corpus** (flat, in reading order, words split) built once per revision and kept |
| 6 | "all words together are matched … every word must match" (§5, twice, differently) | — | 02 §4: each word alone, the node's tier is its worst word's |
| 7 | names and paths only | a method's `name` is `parse_order`; `OrderService.parse` finds nothing | a tier for the qualified name (02 §2) |
| 8 | `searchHits: string[]` in the session, at most the limit | the limit is 200 | Q12 |
| 9 | limits with no totals | — | `nodes_total`, `docs_total`: the header can say "200 of 1,340" |
| 10 | one `match` on a doc result | the title and the snippet are two strings | `title_match` and `snippet_match` |
| 11 | QuickPick `matchOnLabel: false` | VS Code has no such property (`matchOnDescription`, `matchOnDetail` only) | 05 §2: `alwaysShow` on each item **[check]** in S0, with a fallback that needs no check |
| 12 | phrases need a search crate | the text is in memory | Q11 |
| 13 | a wrong filter value finds nothing, silently | — | `notes` in the answer: "`kind:fnn` is not a kind (folder, file, class, function, method)" |

## 4. Where it stands

| Where | What | How |
|---|---|---|
| `webview/src/sidebar/search.ts` (`rankNodes`), used by `useSearch.ts` | the sidebar's box (`16-ui-redesign/06-sidebar.md` §5) | over the tree the page holds: four tiers (name starts with it, name holds it, path starts with it, path holds it), shorter name first, then reading order, at most 200 |
| `extension/src/search.ts` (`searchNodes`) | the palette's *V-NOC: Search…* | a `QuickPick` over the same tree, matched by VS Code's own fuzzy matcher on label and description |

Two searches that rank differently, both in TypeScript, neither reads a doc. After this folder there is one, in Rust.

## 5. Open checks

| # | Check | Step | Decides |
|---|---|---|---|
| 1 | Does a `QuickPick` whose items all have `alwaysShow: true` keep the order it was given while its box has text? | S3, by hand | 05 §2: the QuickPick stays, or *V-NOC: Search…* focuses the sidebar's box |
| 2 | What do the pages do on `indexChanged`? (Confirms Q2: a doc save must not look like a code change.) | **done** (S0, 2026-11-17) | Q2 holds: 03 §5 |
| 3 | `cargo check -p vnoc-search --target wasm32-unknown-unknown` passes | **done** (S1, 2026-11-17): **it passes**, Rust 1.94.1 | Q7 holds: the crate builds for the browser |
| 4 | **[measure]** corpus build and one `run` on 10,000 generated names | **done** (S1, 2026-11-17): build 14 – 20 ms, one `run` 1.6 – 2.4 ms, Apple M1 | the build is on the 20 ms line: 04 §2 says where it goes; S2 measures it again |
| 5 | **[measure]** key to drawn results, in the window | S3 | the debounce (05 §1) |
| 6 | **[measure]** 1,000 docs of 2 KB: first read, memory, one query | S6 | Q3 |

## 6. What later work stands on

| Later | Uses from here |
|---|---|
| `vnoc find`, `vnoc tree --where`, "did you mean" in every CLI error (21) | `vnoc_search::{parse, run}`, the corpus glue in `crates/vnoc/src/search.rs` |
| every `<node>` argument of the CLI (21) | `node_ref` (S5) |
| a second editor | `vnoc/search` and `vnoc/searchCards`; its UI draws two lists and sets an opacity |
| the walkthrough | the command id `vnoc.search` (kept), and one sentence per feature in §1: each is a walkthrough step |
