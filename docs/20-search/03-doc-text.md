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
file name without `.md`: `title_of`, Q13) and its **body** (the text after the frontmatter, the title line included). The frontmatter itself is
not searched.

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

## 6. The snippet

About 120 characters of the **body** around the first hit (a word of the query, or the phrase), cut at word boundaries, line ends turned into
spaces, `…` at a cut end. Markdown is left as it is (no rendering in a result row). When the only hits are in the title, the snippet is the
start of the body after the title line.

The answer gives the hits inside the snippet as ranges (`snippetMatch`), and the hits inside the title as ranges (`titleMatch`), both in UTF-16
units (Q4).

## 7. Cost

Kept in memory: each body once, each title once, the word map. **[measure]** in S6 on 1,000 generated docs of 2 KB: the first read, the memory,
one query, and one update after a single doc changed. Written here. If the first read is over 200 ms it moves off the first build's path (search
answers names until it is done); not before the number says so.
