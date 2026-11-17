//! `matches` (04 §5): which of some node ids a query finds. It must agree with `run` given no limits: an id is a hit when `run` gives its node as a
//! node row, or as the node of a doc row, or when it is `exact`. Checked over a table of queries on S1's tree with a few docs.

mod common;

use std::collections::HashSet;

use common::*;
use vnoc_search::{Corpus, Limits, matches, parse, run};

fn everything() -> Limits {
    Limits {
        nodes: usize::MAX / 2,
        docs: usize::MAX / 2,
    }
}

/// The tree with docs on four of its nodes (one of them found by nothing but its doc) and a Missing node's.
fn docs_corpus() -> Corpus {
    with_docs(vec![
        doc_on(
            "total",
            "a",
            "Totals",
            "The retry policy applies. Shared zanzibar.",
        ),
        doc_on("Invoice", "b", "On invoices", "Retry, then the policy."),
        doc_on(
            "add_item",
            "c",
            "Cart notes",
            "Only here: zanzibar and serengeti.",
        ),
        doc_on("pay", "d", "Paying", "billing of the retry policy"),
        doc_missing(
            "gone.py",
            Some(Kind::Function),
            "e",
            "Lost",
            "zanzibar left no code",
        ),
    ])
}

use vnoc_search::Kind;

/// What `run` says, as the hit of every id of the corpus (and one it does not have).
fn by_run(corpus: &Corpus, query: &str, exact: Option<usize>, ids: &[&str]) -> Vec<bool> {
    let results = run(corpus, &parse(query), exact, everything());
    let mut hit: HashSet<&str> = results
        .nodes
        .iter()
        .map(|h| corpus.nodes()[h.index].id.as_str())
        .collect();
    for d in &results.docs {
        if let Some(at) = corpus.docs()[d.index].node {
            hit.insert(corpus.nodes()[at].id.as_str());
        }
    }
    ids.iter().map(|id| hit.contains(id)).collect()
}

#[test]
fn matches_agrees_with_run_without_limits() {
    let corpus = docs_corpus();
    let mut ids: Vec<&str> = corpus.nodes().iter().map(|n| n.id.as_str()).collect();
    ids.push("not/in/the/corpus.py");
    let queries = [
        // words
        "bill",
        "billing",
        "inv pay",
        "total",
        "nothingmatchesthis",
        // a prefix, in names and in docs
        "zan",
        "pol",
        // filters only
        "kind:fn",
        "kind:method",
        "in:billing/",
        "no:doc",
        "has:doc",
        "kind:class in:billing/",
        // words and filters, on names and docs
        "zanzibar kind:fn",
        "zanzibar in:shop/",
        "retry kind:class",
        // a phrase only: docs and their nodes, no node by name
        "\"retry policy\"",
        "\"policy applies\"",
        "\"retry\"",
        // words and a phrase
        "billing \"retry policy\"",
        "zanzibar \"left no code\"",
        // nothing
        "kind:fnn",
        "kind:fnn zanzibar",
        "",
    ];
    let exacts = [None, Some(at("rebill")), Some(at("add_item")), Some(9_999)];
    for query in queries {
        for exact in exacts {
            let got = matches(&corpus, &parse(query), exact, &ids);
            assert_eq!(
                got,
                by_run(&corpus, query, exact, &ids),
                "{query:?} with exact {exact:?}"
            );
        }
    }
}

#[test]
fn the_rows_that_matter() {
    let corpus = docs_corpus();
    let nodes = corpus.nodes();
    let named = |name: &str| nodes[at(name)].id.as_str();
    let ids = [
        named("total"),
        named("add_item"),
        named("pay"),
        named("cart.py"),
        "nope",
    ];
    let hit = |query: &str, exact: Option<usize>| matches(&corpus, &parse(query), exact, &ids);

    // a node found only through its doc: `add_item` holds no `serengeti`
    assert_eq!(hit("serengeti", None), [false, true, false, false, false]);
    // a phrase finds no node by name, and the nodes whose docs hold it
    assert_eq!(
        hit("\"retry policy\"", None),
        [true, false, true, false, false]
    );
    // filters narrow the docs by their node
    assert_eq!(
        hit("zanzibar in:shop/", None),
        [false, true, false, false, false]
    );
    // filters only: no doc is involved
    assert_eq!(hit("in:shop/", None), [false, true, false, true, false]);
    // `keeps_nothing`: nothing, even with `exact`
    assert_eq!(hit("kind:fnn", Some(at("add_item"))), [false; 5]);
    // `exact` is a hit that no word asks for
    assert_eq!(
        hit("serengeti", Some(at("total"))),
        [true, true, false, false, false]
    );
    // an id the corpus does not have is not a hit, however the query reads; and no ids is an empty answer
    assert!(!hit("", None)[4]);
    assert!(matches(&corpus, &parse("billing"), None, &[]).is_empty());
}
