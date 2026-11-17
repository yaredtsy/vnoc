//! Running a query over a corpus.
//!
//! A node must pass the query's filters, then each is matched against every word (`names.rs`); the nodes that match are put in order (02 §5), and
//! the best are cut to the limit, with the count before the cut kept so a header can say "200 of 1,340". A query of filters only keeps every node
//! that passes, in reading order. A pasted reference arrives as `exact`: the node it named goes first.

use crate::names::{Found, NodeMatch, fold, match_node, order};
use crate::utf16;
use crate::{Corpus, Kind, KindFilter, Limits, NodeEntry, NodeHit, Query, Results};

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
        let is = |kind: &KindFilter| match kind {
            KindFilter::Folder => node.kind == Kind::Folder,
            KindFilter::File => node.kind == Kind::File,
            KindFilter::Class => node.kind == Kind::Class,
            KindFilter::Function => node.kind == Kind::Function,
            KindFilter::Method => node.kind == Kind::Function && node.in_class,
        };
        self.kinds.is_none_or(|kinds| kinds.iter().any(is))
            && !self.not_kinds.iter().any(is)
            && self.docs.is_none_or(|want| (node.docs > 0) == want)
            && self.inside.iter().all(|p| p.holds(&node.path))
            && !self.not_inside.iter().any(|p| p.holds(&node.path))
    }
}

/// The nodes `query` finds, best first. A query with nothing to search, or that keeps nothing, finds nothing, and so does one of phrases (a phrase is
/// for text, 01 §3) without words. With no words, every node that passes the filters is found, in reading order. `exact`, an index into the corpus's nodes, is put first and marked, and
/// not repeated lower down; it counts once in `nodes_total`.
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results {
    let words: Vec<Vec<char>> = query
        .words
        .iter()
        .map(|w| fold(w))
        .filter(|w| !w.is_empty())
        .collect();
    let nothing_to_search = words.is_empty() && (!query.phrases.is_empty() || !query.has_filters());
    if query.keeps_nothing || nothing_to_search {
        return Results::default();
    }
    let filters = Filters::new(query);
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
        } else if let Some(matched) = match_node(&prepared.view(entry), &words) {
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
    Results {
        nodes,
        nodes_total,
        docs: Vec::new(),
        docs_total: 0,
    }
}
