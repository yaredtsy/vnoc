//! Running a query over a corpus.
//!
//! A node must pass the query's filters, then each is matched against every word (`names.rs`); the nodes that match are put in order (02 §5), and
//! the best are cut to the limit, with the count before the cut kept so a header can say "200 of 1,340". A query of filters only keeps every node
//! that passes, in reading order. A pasted reference arrives as `exact`: the node it named goes first.

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
}

impl<'a> Filters<'a> {
    fn new(query: &'a Query) -> Filters<'a> {
        Filters {
            kinds: query.kinds.as_deref(),
            not_kinds: &query.not_kinds,
            docs: query.docs,
            inside: query.inside.iter().map(|p| Inside::new(p)).collect(),
            not_inside: query.not_inside.iter().map(|p| Inside::new(p)).collect(),
        }
    }

    fn passes(&self, node: &NodeEntry) -> bool {
        self.passes_view(Some(node.kind), node.in_class, node.docs, &node.path)
    }

    /// A doc passes by its node: the node's own entry, or for a Missing node what `node.toml` says (a kind that may be unknown, no class, and a path).
    /// A doc's node has at least this doc, so `has:doc` keeps it and `no:doc` drops it.
    fn passes_doc(&self, corpus: &Corpus, doc: &DocEntry) -> bool {
        match doc.node.and_then(|i| corpus.nodes.get(i)) {
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
/// marked, and not repeated lower down; it counts once in `nodes_total`. **Docs** need words or phrases, and filters narrow them by the doc's node;
/// filters alone find no doc.
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results {
    if query.keeps_nothing {
        return Results::default();
    }
    let filters = Filters::new(query);
    let words: Vec<Vec<char>> = query
        .words
        .iter()
        .map(|w| fold(w))
        .filter(|w| !w.is_empty())
        .collect();
    let nodes_wanted = if words.is_empty() {
        query.phrases.is_empty() && query.has_filters()
    } else {
        true
    };
    let (nodes, nodes_total) = if nodes_wanted {
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
    for (index, (prepared, entry)) in corpus.prepared.iter().zip(&corpus.nodes).enumerate() {
        if Some(index) == exact || !filters.passes(entry) {
            continue;
        }
        if words.is_empty() {
            // filters only: every node that passes is as good as another, so the order falls to reading order
            found.push(Found {
                index,
                matched: NodeMatch {
                    tier: 0,
                    skipped: 0,
                    ranges: Vec::new(),
                },
                name_len: 0,
            });
        } else if let Some(matched) = match_node(&prepared.view(entry), words) {
            found.push(Found {
                index,
                matched,
                name_len: prepared.name_len,
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

/// The doc rows and how many docs matched before the limit: the docs that hold every word and phrase of the query and whose node passes the
/// filters, best score first; equal scores go to the doc whose node comes first in reading order, then to Missing nodes' docs by path, then by file.
fn run_docs(
    corpus: &Corpus,
    query: &Query,
    filters: &Filters,
    limits: Limits,
) -> (Vec<DocHit>, usize) {
    let terms = Terms::of(query);
    if terms.is_empty() {
        return (Vec::new(), 0);
    }
    let (scored, hit_words) = corpus.doc_index.find(&terms);
    let mut found: Vec<(usize, f64)> = scored
        .into_iter()
        .map(|(doc, score)| (doc as usize, score))
        .filter(|&(doc, _)| {
            let entry = &corpus.docs[doc];
            filters.passes_doc(corpus, entry) && docs::holds_phrases(entry, &terms.phrases)
        })
        .collect();
    let docs_total = found.len();
    /// Docs of nodes in the tree first, by the node's place in reading order; then Missing nodes' docs, by path.
    fn place<'a>(corpus: &Corpus, doc: &'a DocEntry) -> (bool, usize, &'a str) {
        match doc.node.filter(|&i| i < corpus.nodes.len()) {
            Some(i) => (false, i, ""),
            None => (true, 0, doc.node_path.as_str()),
        }
    }
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
