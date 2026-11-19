//! Running a query over a corpus.
//!
//! A node must pass the query's filters, then each is matched against every word (`names.rs`); the nodes that match are put in order (02 §5), and
//! the best are cut to the limit, with the count before the cut kept so a header can say "200 of 1,340". A query of filters only keeps every node
//! that passes, in reading order. A pasted reference arrives as `exact`: the node it named goes first.
//!
//! `tag:` and `author:` are filters about a doc: a doc passes when it carries every tag and the author (and none that are excluded), and a node passes
//! when ONE of its docs does. That set of nodes is made once per run, from the docs. A query of filters alone finds no doc, **unless** one of the
//! filters is `tag:` or `author:`: then it lists the docs that pass.

use std::collections::{HashMap, HashSet};

use crate::docs::{self, Terms};
use crate::names::{Found, NodeMatch, fold, match_node, order};
use crate::utf16;
use crate::{
    Corpus, DocEntry, DocHit, Kind, KindFilter, Limits, NodeEntry, NodeHit, Query, Results,
};

/// An `in:` path, ready to compare with many node paths.
struct Inside {
    text: String,
    ascii: bool,
    folded: Vec<char>,
}

impl Inside {
    fn new(text: &str) -> Inside {
        Inside {
            text: text.to_string(),
            ascii: text.is_ascii(),
            folded: fold(text),
        }
    }

    /// `path` is this path, or starts with it and a `/`, ignoring case. An empty path is the opened folder: everything is in it.
    fn holds(&self, path: &str) -> bool {
        if self.text.is_empty() {
            return true;
        }
        if self.ascii && path.is_ascii() {
            let (inner, path) = (self.text.as_bytes(), path.as_bytes());
            return path.len() >= inner.len()
                && path[..inner.len()].eq_ignore_ascii_case(inner)
                && path.get(inner.len()).is_none_or(|&b| b == b'/');
        }
        let path = fold(path);
        path.starts_with(&self.folded) && path.get(self.folded.len()).is_none_or(|&c| c == '/')
    }
}

/// The query's filters, read once per run.
struct Filters<'a> {
    kinds: Option<&'a [KindFilter]>,
    not_kinds: &'a [KindFilter],
    docs: Option<bool>,
    inside: Vec<Inside>,
    not_inside: Vec<Inside>,
    tags: &'a [String],
    not_tags: &'a [String],
    authors: &'a [String],
    not_authors: &'a [String],
    /// With a `tag:` or `author:`: the nodes (indexes into the corpus) that have a doc that passes them. `None` without one.
    with_such_doc: Option<HashSet<usize>>,
}

impl<'a> Filters<'a> {
    fn new(corpus: &Corpus, query: &'a Query) -> Filters<'a> {
        let mut filters = Filters {
            kinds: query.kinds.as_deref(),
            not_kinds: &query.not_kinds,
            docs: query.docs,
            inside: query.inside.iter().map(|p| Inside::new(p)).collect(),
            not_inside: query.not_inside.iter().map(|p| Inside::new(p)).collect(),
            tags: &query.tags,
            not_tags: &query.not_tags,
            authors: &query.authors,
            not_authors: &query.not_authors,
            with_such_doc: None,
        };
        if query.has_doc_filters() {
            filters.with_such_doc = Some(
                corpus
                    .docs
                    .iter()
                    .filter(|doc| filters.doc_has_what_is_asked(doc))
                    .filter_map(|doc| doc.node)
                    .collect(),
            );
        }
        filters
    }

    /// The filters about the doc itself: it carries every `tag:` and none of the `-tag:`, and has the `author:` (ignoring case) and not a `-author:`.
    /// Always true for a query with none of them.
    fn doc_has_what_is_asked(&self, doc: &DocEntry) -> bool {
        let is_author = |wanted: &String| {
            doc.author
                .as_deref()
                .is_some_and(|a| a.chars().flat_map(char::to_lowercase).eq(wanted.chars()))
        };
        self.tags.iter().all(|t| doc.tags.contains(t))
            && !self.not_tags.iter().any(|t| doc.tags.contains(t))
            && self.authors.iter().all(is_author)
            && !self.not_authors.iter().any(is_author)
    }

    fn passes(&self, index: usize, node: &NodeEntry) -> bool {
        self.with_such_doc
            .as_ref()
            .is_none_or(|nodes| nodes.contains(&index))
            && self.passes_view(Some(node.kind), node.in_class, node.docs, &node.path)
    }

    /// A doc passes by its node: the node's own entry, or for a Missing node what `node.toml` says (a kind that may be unknown, no class, and a path).
    /// A doc's node has at least this doc, so `has:doc` keeps it and `no:doc` drops it.
    fn passes_doc(&self, corpus: &Corpus, doc: &DocEntry) -> bool {
        self.doc_has_what_is_asked(doc)
            && match doc.node.and_then(|i| corpus.nodes.get(i)) {
                Some(node) => {
                    self.passes_view(Some(node.kind), node.in_class, node.docs.max(1), &node.path)
                }
                None => self.passes_view(doc.node_kind, false, 1, &doc.node_path),
            }
    }

    /// The filters on what they read of a node. An unknown kind is none of the kinds a filter names.
    fn passes_view(&self, kind: Option<Kind>, in_class: bool, docs: usize, path: &str) -> bool {
        let is = |filter: &KindFilter| match (filter, kind) {
            (KindFilter::Folder, kind) => kind == Some(Kind::Folder),
            (KindFilter::File, kind) => kind == Some(Kind::File),
            (KindFilter::Class, kind) => kind == Some(Kind::Class),
            (KindFilter::Function, kind) => kind == Some(Kind::Function),
            (KindFilter::Method, kind) => kind == Some(Kind::Function) && in_class,
        };
        self.kinds.is_none_or(|kinds| kinds.iter().any(is))
            && !self.not_kinds.iter().any(is)
            && self.docs.is_none_or(|want| (docs > 0) == want)
            && self.inside.iter().all(|p| p.holds(path))
            && !self.not_inside.iter().any(|p| p.holds(path))
    }
}

/// What `query` finds in `corpus`: the nodes, then the docs, each best first and cut to its limit with the count before the cut.
///
/// A query that keeps nothing finds nothing. **Nodes** need words, or filters alone: a phrase is for text (01 §3), so a query of phrases finds no
/// node. With no words, every node that passes the filters is found, in reading order. `exact`, an index into the corpus's nodes, is put first and
/// marked, and not repeated lower down; it counts once in `nodes_total`. **Docs** need words or phrases, and filters narrow them by the doc's node
/// (and `tag:` and `author:` by the doc itself); filters alone find no doc, except `tag:` and `author:`, which list the docs that pass.
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results {
    if query.keeps_nothing {
        return Results::default();
    }
    let filters = Filters::new(corpus, query);
    let words = folded_words(query);
    let (nodes, nodes_total) = if nodes_wanted(query, &words) {
        run_nodes(corpus, &filters, &words, exact, limits)
    } else {
        (Vec::new(), 0)
    };
    let (docs, docs_total) = run_docs(corpus, query, &filters, limits);
    Results {
        nodes,
        nodes_total,
        docs,
        docs_total,
    }
}

/// The query's words, folded; an empty one is dropped.
fn folded_words(query: &Query) -> Vec<Vec<char>> {
    query
        .words
        .iter()
        .map(|w| fold(w))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Whether the query can find nodes: it has words, or no words and no phrases but a filter.
fn nodes_wanted(query: &Query, words: &[Vec<char>]) -> bool {
    if words.is_empty() {
        query.phrases.is_empty() && query.has_filters()
    } else {
        true
    }
}

/// How the node at `index` matches: `None` when it fails the filters or a word; a match with no tier and no ranges when there are no words.
fn match_at(
    corpus: &Corpus,
    filters: &Filters,
    words: &[Vec<char>],
    index: usize,
) -> Option<NodeMatch> {
    let entry = &corpus.nodes[index];
    if !filters.passes(index, entry) {
        return None;
    }
    if words.is_empty() {
        // filters only: every node that passes is as good as another
        return Some(NodeMatch {
            tier: 0,
            skipped: 0,
            ranges: Vec::new(),
        });
    }
    match_node(&corpus.prepared[index].view(entry), words)
}

/// The node rows and how many nodes matched before the limit. `words` are folded; none means filters only.
fn run_nodes(
    corpus: &Corpus,
    filters: &Filters,
    words: &[Vec<char>],
    exact: Option<usize>,
    limits: Limits,
) -> (Vec<NodeHit>, usize) {
    let exact = exact.filter(|&at| at < corpus.nodes.len());

    let mut found: Vec<Found> = Vec::new();
    for (index, prepared) in corpus.prepared.iter().enumerate() {
        if Some(index) == exact {
            continue;
        }
        if let Some(matched) = match_at(corpus, filters, words, index) {
            found.push(Found {
                index,
                matched,
                // filters only: the order falls to reading order
                name_len: if words.is_empty() {
                    0
                } else {
                    prepared.name_len
                },
            });
        }
    }

    let nodes_total = found.len() + usize::from(exact.is_some());
    let first = exact.filter(|_| limits.nodes > 0);
    let room = limits.nodes - usize::from(first.is_some());
    if found.len() > room {
        // only the best `room` are put in order: the rest are not worth sorting
        found.select_nth_unstable_by(room, order);
        found.truncate(room);
    }
    found.sort_unstable_by(order);

    let mut nodes = Vec::with_capacity(found.len() + 1);
    if let Some(index) = first {
        nodes.push(NodeHit {
            index,
            tier: 0,
            skipped: 0,
            exact: true,
            ranges: Vec::new(),
        });
    }
    for f in found {
        let name = &corpus.nodes[f.index].name;
        let ranges = f
            .matched
            .ranges
            .iter()
            .map(|&(start, end)| utf16::range(name, start, end))
            .collect();
        nodes.push(NodeHit {
            index: f.index,
            tier: f.matched.tier,
            skipped: f.matched.skipped,
            exact: false,
            ranges,
        });
    }
    (nodes, nodes_total)
}

/// The docs that hold every term and phrase and whose node passes the filters, in doc order, each with its score; and the words the prefix opened into.
/// What [`run`] ranks and [`matches`] only asks.
fn matching_docs(
    corpus: &Corpus,
    terms: &Terms,
    filters: &Filters,
) -> (Vec<(usize, f64)>, HashSet<String>) {
    let (scored, hit_words) = corpus.doc_index.find(terms);
    let found = scored
        .into_iter()
        .map(|(doc, score)| (doc as usize, score))
        .filter(|&(doc, _)| {
            let entry = &corpus.docs[doc];
            filters.passes_doc(corpus, entry) && docs::holds_phrases(entry, &terms.phrases)
        })
        .collect();
    (found, hit_words)
}

/// Of these node ids, which `query` finds: a node is a hit when [`run`], with no limits, would give it as a node row or as the node of a doc row;
/// `exact` is a hit when `run` would give it (a query that wants no node rows gives none). One answer for each id, in the order given; an id the corpus does not have is not a hit. No order, no snippets: the matching of
/// [`run`], asked once for each id.
pub fn matches(corpus: &Corpus, query: &Query, exact: Option<usize>, ids: &[&str]) -> Vec<bool> {
    let mut hits = vec![false; ids.len()];
    if query.keeps_nothing || ids.is_empty() {
        return hits;
    }
    let filters = Filters::new(corpus, query);
    let words = folded_words(query);
    let at: HashMap<&str, usize> = corpus
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let exact = exact.filter(|&i| i < corpus.nodes.len());
    let by_name = nodes_wanted(query, &words);
    let terms = Terms::of(query);
    let of_docs: HashSet<usize> = if docs_by_filters_alone(query, &words) {
        docs_passing(corpus, &filters)
            .into_iter()
            .filter_map(|doc| corpus.docs[doc].node)
            .collect()
    } else if terms.is_empty() {
        HashSet::new()
    } else {
        matching_docs(corpus, &terms, &filters)
            .0
            .into_iter()
            .filter_map(|(doc, _)| corpus.docs[doc].node)
            .collect()
    };
    for (hit, id) in hits.iter_mut().zip(ids) {
        let Some(&index) = at.get(id) else { continue };
        *hit = of_docs.contains(&index)
            || (by_name
                && (Some(index) == exact || match_at(corpus, &filters, &words, index).is_some()));
    }
    hits
}

/// The doc rows and how many docs matched before the limit: the docs that hold every word and phrase of the query and whose node passes the
/// filters, best score first; equal scores go to the doc whose node comes first in reading order, then to Missing nodes' docs by path, then by file.
fn run_docs(
    corpus: &Corpus,
    query: &Query,
    filters: &Filters,
    limits: Limits,
) -> (Vec<DocHit>, usize) {
    if docs_by_filters_alone(query, &folded_words(query)) {
        return docs_of_filters_alone(corpus, filters, limits);
    }
    let terms = Terms::of(query);
    if terms.is_empty() {
        return (Vec::new(), 0);
    }
    let (mut found, hit_words) = matching_docs(corpus, &terms, filters);
    let docs_total = found.len();
    found.sort_by(|a, b| {
        let (x, y) = (&corpus.docs[a.0], &corpus.docs[b.0]);
        b.1.total_cmp(&a.1)
            .then_with(|| place(corpus, x).cmp(&place(corpus, y)))
            .then_with(|| x.file.cmp(&y.file))
    });
    found.truncate(limits.docs);
    let hits = found
        .into_iter()
        .map(|(index, score)| {
            let shown = docs::show(&corpus.docs[index], &hit_words, &terms.phrases);
            DocHit {
                index,
                score,
                title_ranges: shown.title_ranges,
                snippet: shown.snippet,
                snippet_ranges: shown.snippet_ranges,
            }
        })
        .collect();
    (hits, docs_total)
}

/// Docs of nodes in the tree first, by the node's place in reading order; then Missing nodes' docs, by path.
fn place<'a>(corpus: &Corpus, doc: &'a DocEntry) -> (bool, usize, &'a str) {
    match doc.node.filter(|&i| i < corpus.nodes.len()) {
        Some(i) => (false, i, ""),
        None => (true, 0, doc.node_path.as_str()),
    }
}

/// A query of filters alone (no word, no phrase) that has a `tag:` or an `author:` lists docs: the other filters alone find none (a doc of every
/// function under `kind:fn` would be noise), but "the docs tagged plan" is a question about docs.
fn docs_by_filters_alone(query: &Query, words: &[Vec<char>]) -> bool {
    words.is_empty() && query.phrases.is_empty() && query.has_doc_filters()
}

/// The docs (indexes) that pass every filter, by the doc itself and by its node.
fn docs_passing(corpus: &Corpus, filters: &Filters) -> Vec<usize> {
    (0..corpus.docs.len())
        .filter(|&i| filters.passes_doc(corpus, &corpus.docs[i]))
        .collect()
}

/// The doc rows of a query of filters alone: the docs that pass, in the reading order of their nodes and then by file name, each with the start of
/// its body as the snippet and nothing to draw bold; no score.
fn docs_of_filters_alone(
    corpus: &Corpus,
    filters: &Filters,
    limits: Limits,
) -> (Vec<DocHit>, usize) {
    let mut found = docs_passing(corpus, filters);
    let docs_total = found.len();
    found.sort_by(|&a, &b| {
        let (x, y) = (&corpus.docs[a], &corpus.docs[b]);
        place(corpus, x)
            .cmp(&place(corpus, y))
            .then_with(|| x.file.cmp(&y.file))
    });
    found.truncate(limits.docs);
    let nothing = HashSet::new();
    let hits = found
        .into_iter()
        .map(|index| {
            let shown = docs::show(&corpus.docs[index], &nothing, &[]);
            DocHit {
                index,
                score: 0.0,
                title_ranges: Vec::new(),
                snippet: shown.snippet,
                snippet_ranges: Vec::new(),
            }
        })
        .collect();
    (hits, docs_total)
}
