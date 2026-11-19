//! `tag:` and `author:` (`docs/21-agent-cli/04-write-commands.md` §4): filters about a doc itself. A doc passes when it carries every tag and has the
//! author; a node passes when ONE of its docs does; a query of filters alone lists docs when one of the filters is `tag:` or `author:`.

mod common;

use common::*;
use vnoc_search::{Corpus, DocEntry, Kind, Limits, parse, run};

/// The tree with tagged docs on it (reading order of the nodes: total, bill_all, Invoice, pay, rebill, add_item):
///
/// | file | node | tags | author |
/// |---|---|---|---|
/// | a  | total    | plan, billing | claude-code |
/// | a2 | total    | draft         | Ana |
/// | x  | bill_all | a             | (none) |
/// | y  | bill_all | b             | (none) |
/// | b  | Invoice  | plan          | Ana |
/// | d  | pay      | plan          | (none) |
/// | r  | rebill   | draft         | (none) |
/// | c  | add_item | billing       | claude-code |
/// | e  | gone.py (Missing, a function) | plan | claude-code |
fn tagged_corpus() -> Corpus {
    with_counts(vec![
        tagged(
            doc_on(
                "total",
                "a",
                "Split total",
                "Split it in two. Shared zanzibar.",
            ),
            &["plan", "billing"],
            Some("claude-code"),
        ),
        tagged(
            doc_on("total", "a2", "Draft", "Not yet."),
            &["draft"],
            Some("Ana"),
        ),
        tagged(doc_on("bill_all", "x", "X", "one"), &["a"], None),
        tagged(doc_on("bill_all", "y", "Y", "two"), &["b"], None),
        tagged(
            doc_on("Invoice", "b", "On invoices", "Zanzibar again."),
            &["plan"],
            Some("Ana"),
        ),
        tagged(
            doc_on("pay", "d", "Paying", "billing of the retry policy"),
            &["plan"],
            None,
        ),
        tagged(
            doc_on("rebill", "r", "Rebill", "draft only"),
            &["draft"],
            None,
        ),
        tagged(
            doc_on("add_item", "c", "Cart notes", "cart"),
            &["billing"],
            Some("claude-code"),
        ),
        tagged(
            doc_missing("gone.py", Some(Kind::Function), "e", "Lost", "left no code"),
            &["plan"],
            Some("claude-code"),
        ),
    ])
}

/// The tree with these docs, and each node's `docs` the number of them it has (as the core builds it: `no:doc` reads that number).
fn with_counts(docs: Vec<DocEntry>) -> Corpus {
    let mut nodes = tree_nodes();
    for doc in &docs {
        if let Some(at) = doc.node {
            nodes[at].docs += 1;
        }
    }
    Corpus::new(nodes, docs)
}

fn nodes_of(c: &Corpus, query: &str) -> Vec<String> {
    ask(c, query).into_iter().map(|(name, _)| name).collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    strings(items)
}

#[test]
fn the_filters_parse_lower_case_and_a_value_is_never_wrong() {
    let q = parse("tag:Plan -tag:Draft author:Claude-Code -author:Ana tag:plan");
    assert_eq!(q.tags, strs(&["plan"]), "the same tag twice is one");
    assert_eq!(q.not_tags, strs(&["draft"]));
    assert_eq!(q.authors, strs(&["claude-code"]));
    assert_eq!(q.not_authors, strs(&["ana"]));
    assert!(q.has_doc_filters() && q.words.is_empty() && !q.keeps_nothing && q.notes.is_empty());
    // an author with a space, quoted
    assert_eq!(parse("author:\"Ana Perez\"").authors, strs(&["ana perez"]));
    // `tag:plan,billing` is one tag, which nothing has; nothing is said
    let q = parse("tag:plan,billing");
    assert_eq!(q.tags, strs(&["plan,billing"]));
    assert!(q.notes.is_empty() && !q.keeps_nothing);
    // no value yet: a word, as a half-typed `kind:` is
    let q = parse("tag: author:");
    assert_eq!(q.words, strs(&["tag:", "author:"]));
    assert!(!q.has_doc_filters());
    assert!(!parse("kind:fn in:billing has:doc").has_doc_filters());
}

#[test]
fn tag_alone_lists_the_docs_that_carry_it_and_the_nodes_that_have_one() {
    let c = tagged_corpus();
    let r = search(&c, "tag:plan");
    // docs: in the reading order of their nodes, then by file; the Missing node's last
    assert_eq!(found(&c, "tag:plan"), ["a", "b", "d", "e"]);
    assert_eq!(r.docs_total, 4);
    // nodes: in reading order, at tier 0 (as every query of filters alone)
    assert_eq!(
        ask(&c, "tag:plan"),
        want(&[("total", 0), ("Invoice", 0), ("pay", 0)])
    );
    // the start of the body is the snippet, and nothing is drawn bold; no score
    let first = &r.docs[0];
    assert_eq!(first.snippet, "Split it in two. Shared zanzibar.");
    assert!(first.title_ranges.is_empty() && first.snippet_ranges.is_empty());
    assert_eq!(first.score, 0.0);
}

#[test]
fn other_filters_alone_still_find_no_doc() {
    let c = tagged_corpus();
    for query in [
        "kind:fn",
        "has:doc",
        "in:billing/",
        "kind:class in:billing/",
        "no:doc",
    ] {
        let r = search(&c, query);
        assert!(r.docs.is_empty() && r.docs_total == 0, "{query}");
    }
    assert!(!search(&c, "kind:fn").nodes.is_empty());
}

#[test]
fn the_node_filters_narrow_the_docs_a_tag_lists() {
    let c = tagged_corpus();
    // functions: total and pay (a method is a function) and the Missing function's doc
    assert_eq!(found(&c, "kind:fn tag:plan"), ["a", "d", "e"]);
    assert_eq!(nodes_of(&c, "kind:fn tag:plan"), ["total", "pay"]);
    assert_eq!(found(&c, "kind:class tag:plan"), ["b"]);
    assert_eq!(found(&c, "kind:method tag:plan"), ["d"]);
    assert_eq!(found(&c, "in:billing/ tag:billing"), ["a"]);
    assert_eq!(found(&c, "in:shop/ tag:billing"), ["c"]);
    // `no:doc` keeps nodes with no docs, so no doc of a node can pass it
    assert!(search(&c, "no:doc tag:plan").docs.is_empty());
    assert!(nodes_of(&c, "no:doc tag:plan").is_empty());
    assert_eq!(found(&c, "has:doc tag:plan"), ["a", "b", "d", "e"]);
}

#[test]
fn two_tags_need_one_doc_that_carries_both() {
    let c = tagged_corpus();
    assert_eq!(found(&c, "tag:plan tag:billing"), ["a"]);
    assert_eq!(nodes_of(&c, "tag:plan tag:billing"), ["total"]);
    // bill_all has a doc tagged a and another tagged b: it has each tag, but no doc has both
    assert_eq!(nodes_of(&c, "tag:a"), ["bill_all"]);
    assert_eq!(nodes_of(&c, "tag:b"), ["bill_all"]);
    assert!(nodes_of(&c, "tag:a tag:b").is_empty());
    assert!(found(&c, "tag:a tag:b").is_empty());
}

#[test]
fn a_tag_with_a_minus_excludes_docs_that_carry_it() {
    let c = tagged_corpus();
    // every doc without `draft`; a node counts when one of its docs does (total has `a` too; rebill has only a draft)
    assert_eq!(found(&c, "-tag:draft"), ["a", "x", "y", "b", "d", "c", "e"]);
    assert_eq!(
        nodes_of(&c, "-tag:draft"),
        ["total", "bill_all", "Invoice", "pay", "add_item"]
    );
    assert_eq!(found(&c, "tag:plan -tag:billing"), ["b", "d", "e"]);
}

#[test]
fn author_is_compared_ignoring_case_and_two_different_ones_keep_nothing() {
    let c = tagged_corpus();
    assert_eq!(found(&c, "author:claude-code"), ["a", "c", "e"]);
    assert_eq!(found(&c, "author:Claude-Code"), ["a", "c", "e"]);
    assert_eq!(found(&c, "author:ana"), ["a2", "b"]);
    assert_eq!(found(&c, "tag:plan author:ana"), ["b"]);
    assert_eq!(found(&c, "-author:claude-code tag:plan"), ["b", "d"]);
    assert!(found(&c, "author:ana author:claude-code").is_empty());
    // a doc with no author is not by anyone
    assert!(!found(&c, "author:claude-code").contains(&"d".to_string()));
    assert_eq!(nodes_of(&c, "author:claude-code"), ["total", "add_item"]);
}

#[test]
fn words_find_docs_as_before_and_a_tag_narrows_them() {
    let c = tagged_corpus();
    // the shorter doc ranks first (BM25)
    assert_eq!(found(&c, "zanzibar"), ["b", "a"]);
    assert_eq!(found(&c, "zanzibar tag:billing"), ["a"]);
    assert_eq!(found(&c, "zanzibar author:ana"), ["b"]);
    assert_eq!(found(&c, "zanzibar tag:draft"), Vec::<String>::new());
    // the ranking of words is not changed by a tag: the same order as without it, less the docs that do not pass
    assert_eq!(found(&c, "zanzibar tag:plan"), ["b", "a"]);
    // a phrase and a tag
    assert_eq!(found(&c, "\"retry policy\" tag:plan"), ["d"]);
}

#[test]
fn a_tag_nothing_has_finds_nothing_and_says_nothing() {
    let c = tagged_corpus();
    let q = parse("tag:nothing-has-this");
    assert!(q.notes.is_empty());
    let r = run(&c, &q, None, Limits::default());
    assert!(r.nodes.is_empty() && r.docs.is_empty());
    assert_eq!((r.nodes_total, r.docs_total), (0, 0));
    // and in a corpus with no docs at all
    assert!(search(&tree(), "tag:plan").docs.is_empty());
    assert!(ask(&tree(), "tag:plan").is_empty());
}

#[test]
fn the_doc_limit_cuts_the_list_and_the_total_counts_all() {
    let c = tagged_corpus();
    let r = run(
        &c,
        &parse("tag:plan"),
        None,
        Limits {
            nodes: 200,
            docs: 2,
        },
    );
    assert_eq!(r.docs.len(), 2);
    assert_eq!(r.docs_total, 4);
    let files: Vec<&str> = r
        .docs
        .iter()
        .map(|d| c.docs()[d.index].file.as_str())
        .collect();
    assert_eq!(files, ["a", "b"], "the first in reading order");
}
