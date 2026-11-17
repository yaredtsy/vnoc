# 20.03 Doc text

```
doc text
├── 1. what is kept
├── 2. words
├── 3. ranking            BM25, the title three times, a prefix on the last word
├── 4. phrases
├── 5. keeping it fresh   stamps, its own counter
├── 6. the snippet
└── 7. cost
```

## 1. What is kept

For every `.md` in every node folder `load_all` reads (`userdata/read.rs`): its **title** (the first `# heading` after the frontmatter, else the
file name without `.md`: `title_of`, Q13) and its **body** (the text after the frontmatter, **without** the title's own `# ` line: that line is the title, counted as §3 says, not
counted twice). The frontmatter itself is not searched.

Docs of **Missing** nodes are kept too, and say so in the answer (`missing: true`): a doc someone cannot find any other way is the one search
should find.

A file that is not UTF-8, or larger than 1 MB, is skipped with a `tracing::warn!` (a doc is a note, not a data file).

## 2. Words

Lower case; split at anything that is not a letter or a digit (Unicode's own classes, so `café` is one word); a `camelCase` token is kept whole
**and** split as a name is ([`02-names.md`](02-names.md) §3), so `retryPolicy` is found by `retrypolicy`, `retry` and `policy`. `snake_case`
splits by the first rule already.

No stemming: `retries` does not find `retry` **[decide after use]**. No stop words: the collection is small, and `not` matters in a note.
Text with no spaces between words (Chinese, Japanese) is one long word per run: found by a phrase (§4), not by a word. Noted, not solved.

## 3. Ranking

**BM25** (k1 = 1.2, b = 0.75) over the words of §2. A word in the title counts **three times**. Every word of the query must be in the doc. The
**last** word also matches as a prefix (`ret` finds `retry`) because the user may still be typing it; a prefix that opens more than 200 different
words uses the 200 most common (a one-letter last word must not be slow).

Ties: the doc whose node comes first in reading order, then the file name.

The index is a map from a word to the docs that hold it, with a count each, plus the words in sorted order for the prefix. It is rebuilt whole
when any doc changed (§5): at 1,000 docs that is cheaper than keeping it editable **[measure]** (S6).

## 4. Phrases

`"retry policy"`: the phrase's own words go through §3 as ordinary words (so only docs that hold all of them are looked at), then each of those
docs must **hold the phrase as written**, ignoring case and treating any run of spaces or line ends as one space. The check is on the title and
the body.

## 5. Keeping it fresh

The core already re-reads `.vnoc/` on every update: `index/build.rs` `refresh` calls `UserData::load_all` each time, and the extension's watcher
covers `.vnoc/**` (`extension/src/core.ts`). But a record there is the `node.toml` and the **names** of its `.md` files: an edited body is not a
change to it. So the text has its own store:

```
DocTexts   (crates/vnoc/src/userdata/doc_text.rs)
  per (node ID, file name):   size · modified time · title · body
  refresh(records):           for each doc of each record: same size and time → keep; else read again. Docs that are gone are dropped.
                              returns whether anything changed; if so  generation += 1
```

- Called from `refresh`, right after `load_all`, in the same `spawn_blocking`. The first build reads every doc; later updates stat every doc and
  read only what changed.
- **Held by the `Index`** (a field, with `Index::doc_texts()` and `Index::doc_generation()`), so every caller of `refresh` has it: the server,
  and doc 21's one-shot commands. `refresh` reads the old stamps under the read lock, does the disk work off the lock, and stores the result with
  `Index::set_doc_texts`, the one setter whose answer is **not** part of `changed`.
- **Its `generation` is not the index's `revision`** (Q2). An update that changed only doc bodies announces nothing (`vnoc/indexChanged` is not
  sent): the tree did not change. The next search sees the new text, because the corpus is rebuilt when either number moved
  ([`04-core.md`](04-core.md) §2).
- So a result list on screen can show an old snippet until the next key. That is accepted: the box is being typed in.
- A doc written by `vnoc doc new` (21) or by a teammate's `git pull` arrives the same way: the watcher, `filesChanged`, `refresh`.
- **Checked 2026-11-17 (S0):** `vnoc/indexChanged` makes the host re-fetch every stale call tree 800 ms later (`extension/src/extension.ts`
  → `callTree.ts` → `shared/callScheduler.ts`), the canvas and the sidebar re-fetch the whole structure (`webview/src/state/data.ts`), and the
  doc panel refreshes (`webview/src/docs/DocsApp.tsx`). So a doc save that sent it would cost a structure and a call-tree round trip per save,
  for no change to either. Q2 holds: doc bodies never send it.

## 6. The snippet

About 120 characters of the **body** around the first hit (a word of the query, or the phrase), cut at word boundaries, line ends turned into
spaces, `…` at a cut end. Markdown is left as it is (no rendering in a result row). When the only hits are in the title, the snippet is the
start of the body after the title line.

The answer gives the hits inside the snippet as ranges (`snippet_match`), and the hits inside the title as ranges (`title_match`), both in UTF-16
units (Q4).

## 7. Cost

Kept in memory: each body once, each title once, the word map. **[measure]** in S6 on 1,000 generated docs of 2 KB: the first read, the memory,
one query, and one update after a single doc changed. Written here. If the first read is over 200 ms it moves off the first build's path (search
answers names until it is done); not before the number says so.

**Measured in S6a (2026-11-17), the crate only.** `cargo test -p vnoc-search --release -- --ignored --nocapture` (`tests/timing.rs`), Apple M1, 8 GB,
rustc 1.94.1. 10,000 names and 1,000 generated docs (a 30-byte title and a 2,000-byte body each, 1,987 KB of text, words from a vocabulary of 4,000
with a few common ones and many rare), attached across the names. Each figure is the first run in the process, then the median of five more:

| | first | median of 5 |
|---|---|---|
| `Corpus::new`, 10,000 names, no docs | 2.0 ms | 4.1 ms |
| `Corpus::new`, 10,000 names and 1,000 docs | 88 ms | 74 ms |
| one `run`, a word in every doc (`baba`: 1,000 docs found, 50 shown) | 2.5 ms | 2.4 ms |
| one `run`, a two-letter prefix (`ba`: 1,000 docs found, 50 shown, and 81 nodes) | 2.8 ms | 2.9 ms |

So the index adds about 70 – 85 ms to a build (the names' own 2 – 4 ms is as in 04 §2), and a query that finds every doc, the worst case, costs about
3 ms with the names and the snippets of the 50 shown. The build happens once per change of the corpus (04 §2), not per key. **Still to measure in S6b**
(it reads from disk): the first read, the memory, and one update after a single doc changed.

## 8. Settled for S6a (2026-11-17)

The crate's side, so that S6a's tests have one answer each.

- **What the crate returns:** `DocHit { index, score: f64, title_ranges, snippet: String, snippet_ranges }`. `index` points into the corpus's
  docs; the ranges are UTF-16 (Q4), the snippet's measured in the snippet string as returned (a leading `…` is one unit).
- **Query words for docs:** each query word is split by §2's rule (at anything not a letter or a digit), so `billing/` is `billing` and
  `OrderService.par` is `orderservice` and `par`. Every piece is required. The **last piece of the last word** is the prefix; a word that is
  only punctuation adds nothing.
- **Postings:** a camelCase token gives its whole form and its parts, each a posting at its own byte range; every posting counts toward the
  doc's length. A title posting counts three times, in the term count and in the length.
- **BM25:** idf = ln(1 + (N − df + 0.5) / (df + 0.5)); k1 = 1.2, b = 0.75, the average length over the corpus's docs. The prefix is **one term**:
  its df is the number of docs holding any of its words (at most the 200 with the highest df, ties by the word), its tf the sum of theirs.
- **Phrases:** their words are required words (no prefix); then the title or the body must hold the phrase, case ignored char by char, any run
  of white space as one space.
- **Filters on docs** read the doc's node: in the tree, the corpus's node; for a Missing node, `node_kind` and `node_path`, no `in_class`
  (so `kind:method` never keeps a Missing node's doc), and `docs` at least 1. Only filters, no words and no phrases: no docs. `keeps_nothing`:
  no docs.
- **Ties** (equal score): docs of nodes in the tree by the node's place in reading order, then Missing nodes' docs by `node_path`; then the
  file name.
- **The snippet:** chars, not bytes. The first hit in the body (a posting of a query word or prefix word, or the phrase) is placed about 40 chars
  in: start up to 40 chars before it, moved back to the start of a word; end at most 120 chars after the start, moved back to the end of a word;
  line ends and runs of white space become one space; `…` where text was cut. Hits only in the title: the first 120 chars of the body, cut the
  same way. An empty body: an empty snippet. `snippet_ranges`: every hit inside the snippet; `title_ranges`: every hit in the title.
- **The index** is built in `Corpus::new`, whole. **[measure]** in S6a: 1,000 generated docs of 2 KB, the build and one query, in the crate's
  timing test; S6b measures reading them from disk.
- *(Settled from S6a's tests, 2026-11-17.)* Digits split words in docs as in names (`a001` is `a` `001`). When the last query word is only
  punctuation, the last piece overall is the prefix (`order /`: `order`). A phrase never gives the prefix; its pieces are whole words. A
  repeated piece is required once, and may also be the prefix (`ret ret`). A doc's node has at least that doc for `has:`/`no:` (`max(docs, 1)`),
  in the tree too. A Missing node with no kind fails `kind:` and passes `-kind:`. Only the prefix words inside the cap are highlighted. `…`
  only where text was cut; a word longer than 120 chars is cut where the snippet ends. Ties: docs of tree nodes, then Missing by `node_path`, then
  `file`.

## 9. The doc index is its own (decided 2026-11-17, after S6a)

S6a measured `Corpus::new` at **74 – 88 ms** with 1,000 docs of 2 KB, against **4 ms** without: the doc index is 95% of the build, and over the
watchdog's 50 ms. Two fixes, both in S6a2:

- **Kept apart from the names.** The corpus is rebuilt on every code change (its key is the revision), and the doc text has not changed then.
  So the doc index is a public `DocIndex`, built from the docs' titles and bodies alone, and the core keeps it under the **doc-text
  generation**; `Corpus` takes it as an `Arc`. A code change rebuilds the names (≈ 6 ms) and joins the kept doc index to the new nodes
  (which node each doc belongs to, a scan); a doc change rebuilds the doc index only.
- **Cheaper to build.** It allocated an owned `String` per distinct word **per doc** and inserted each into a `BTreeMap`: about 200,000 of both
  for 1,000 docs. Instead: each word is interned once (a `HashMap<Box<str>, u32>` looked up by `&str`, so no allocation for a word already seen),
  the counts of one doc are kept by word number, postings are `Vec`s by word number, and the sorted word list for the prefix is made once at the
  end. ASCII text is lower-cased in place, as names are (S2b). Target: the doc index's median build **20 ms or less** on the M1. If it is over,
  S6b builds it in `refresh`'s `spawn_blocking`, off the request, only when the generation moved.

**Measured in S6a2 (2026-11-17).** The same timing test (`cargo test -p vnoc-search --release -- --ignored --nocapture`, `tests/timing.rs`), Apple M1,
8 GB, rustc 1.94.1; 10,000 names and 1,000 docs of 2 KB (1,987 KB of text). Each figure is the first run in the process, then the median of five
more; the range is over five invocations on a quiet moment. The machine is shared (a language server and an editor run beside it): in invocations
made while its load average was 9 – 10 the same medians read 19 – 29 ms, so a build is **16 ms on a quiet M1 and up to about 29 ms under load**.

| | before (S6a) | after, first | after, median of 5 more |
|---|---|---|---|
| the doc index: `Corpus::new` with 1,000 docs, then `DocIndex::new` alone | 88 ms · 74 ms | **16 – 21 ms** (29 under load) | **15.3 – 16.9 ms** (19.1 under load) |
| `Corpus::new`, 10,000 names, no docs | 2.0 ms · 4.1 ms | 1.6 – 2.8 ms | 1.3 – 1.6 ms |
| `Corpus::with_doc_index`, 10,000 names, a kept index | (not possible) | 1.3 – 2.8 ms | 1.3 – 1.9 ms |
| one `run`, a word in every doc (`baba`) | 2.5 ms · 2.4 ms | 2.0 – 2.5 ms | 2.0 – 2.5 ms |
| one `run`, a two-letter prefix (`ba`) | 2.8 ms · 2.9 ms | 2.5 – 4.7 ms | 2.4 – 3.9 ms |

The build is **4.5 times cheaper** (74 ms to 16 ms) and a code change now costs only the names, 1.3 – 1.9 ms with the kept index joined, which is
less than building the corpus without docs was in S6a. The queries are unchanged (the ranking did not move: every S6a test passes as it was).

**Decision: the doc index is built in the request**, the first time a search needs it after a doc change (or the first search of the session). Its
median is **under the 20 ms line** on a quiet M1, and the watchdog's 50 ms (`watchdog.rs` `LIMIT`) is met in every run, load included, by a factor of
about 2. S6b therefore keeps it under the doc-text generation in `State` and builds it where `kept` builds the corpus; it does **not** build it in
`refresh`'s `spawn_blocking`. If S6b's measurement on the real disk path (reading 1,000 files first) puts a search over the line, that is where it moves.

**What is left in `DocIndex::new`** (a scratch profile, not kept): cutting the text into words is about two thirds of it (about 11 of 16 ms for
325,000 words; the scan of the characters alone is under 3 ms of that), and looking each word up for its number is the rest. A byte-level scan of the
ASCII text was tried and was no faster, so it is not in the code. The next cut, if one is ever needed, is to skip the per-token copy for words that
need no lower-casing, not to change the index.
