//! Running a query over a corpus.
//!
//! Each node is matched against every word (`names.rs`), the nodes that match are put in order (02 §5), and the best are cut to the limit, with
//! the count before the cut kept so a header can say "200 of 1,340". A pasted reference arrives as `exact`: the node it named goes first.

use crate::names::{Found, fold, match_node, order};
use crate::utf16;
use crate::{Corpus, Limits, NodeHit, Query, Results};

/// The nodes `query` finds, best first. An empty query finds nothing. `exact`, an index into the corpus's nodes, is put first and marked, and
/// not repeated lower down; it counts once in `nodes_total`.
pub fn run(corpus: &Corpus, query: &Query, exact: Option<usize>, limits: Limits) -> Results {
    let words: Vec<Vec<char>> = query
        .words
        .iter()
        .map(|w| fold(w))
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return Results::default();
    }
    let exact = exact.filter(|&at| at < corpus.nodes.len());

    let mut found: Vec<Found> = Vec::new();
    for (index, (prepared, entry)) in corpus.prepared.iter().zip(&corpus.nodes).enumerate() {
        if Some(index) == exact {
            continue;
        }
        if let Some(matched) = match_node(&prepared.view(entry), &words) {
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
