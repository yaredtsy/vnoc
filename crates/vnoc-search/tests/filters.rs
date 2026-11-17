//! Filters on nodes (01 §2, §3), on the ranking tree of S1: which nodes pass, in what order, and the edges.

mod common;

use common::*;
use vnoc_search::{Corpus, Limits, NodeEntry, parse, run};

fn names(c: &Corpus, query: &str) -> Vec<String> {
    ask(c, query).into_iter().map(|(name, _)| name).collect()
}

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|i| i.to_string()).collect()
}

/// `(name, tier)` pairs with every tier 0.
fn tier0(items: &[&str]) -> Vec<(String, u8)> {
    items.iter().map(|n| (n.to_string(), 0)).collect()
}

/// The tree with `docs` docs on the node called `name`.
fn with_docs(name: &str, docs: usize) -> Corpus {
    let nodes: Vec<NodeEntry> = tree()
        .nodes()
        .iter()
        .cloned()
        .map(|n| {
            if n.name == name {
                NodeEntry { docs, ..n }
            } else {
                n
            }
        })
        .collect();
    corpus(nodes)
}

#[test]
fn filters_only_list_what_passes_in_reading_order_at_tier_0() {
    let c = tree();
    let rows: &[(&str, &[&str])] = &[
        (
            "kind:fn",
            &[
                "total",
                "bill_all",
                "pay",
                "rebill",
                "add_item",
                "billing_note",
            ],
        ),
        (
            "kind:function",
            &[
                "total",
                "bill_all",
                "pay",
                "rebill",
                "add_item",
                "billing_note",
            ],
        ),
        ("kind:method", &["pay"]),
        ("kind:class", &["Invoice"]),
        ("kind:cls", &["Invoice"]),
        ("kind:folder", &["billing", "shop"]),
        ("kind:file", &["invoice.py", "rebilling.py", "cart.py"]),
        (
            "kind:file,folder",
            &["billing", "invoice.py", "rebilling.py", "shop", "cart.py"],
        ),
        (
            "kind:file kind:class",
            &["invoice.py", "Invoice", "rebilling.py", "cart.py"],
        ),
        (
            "-kind:fn",
            &[
                "billing",
                "invoice.py",
                "Invoice",
                "rebilling.py",
                "shop",
                "cart.py",
            ],
        ),
        (
            "kind:fn -kind:method",
            &["total", "bill_all", "rebill", "add_item", "billing_note"],
        ),
        (
            "in:billing",
            &[
                "billing",
                "invoice.py",
                "total",
                "bill_all",
                "Invoice",
                "pay",
                "rebilling.py",
                "rebill",
            ],
        ),
        (
            "in:billing/",
            &[
                "billing",
                "invoice.py",
                "total",
                "bill_all",
                "Invoice",
                "pay",
                "rebilling.py",
                "rebill",
            ],
        ),
        (
            "in:billing/invoice.py",
            &["invoice.py", "total", "bill_all", "Invoice", "pay"],
        ),
        (
            "IN:Billing\\Invoice.PY",
            &["invoice.py", "total", "bill_all", "Invoice", "pay"],
        ),
        (
            "in:./shop/",
            &["shop", "cart.py", "add_item", "billing_note"],
        ),
        ("in:bill", &[]),
        ("in:billing/inv", &[]),
        (
            "in:billing in:billing/invoice.py",
            &["invoice.py", "total", "bill_all", "Invoice", "pay"],
        ),
        ("in:shop in:billing", &[]),
        (
            "-in:billing",
            &["shop", "cart.py", "add_item", "billing_note"],
        ),
        ("-in:billing/invoice.py kind:fn in:billing", &["rebill"]),
        ("-in:billing kind:fn", &["add_item", "billing_note"]),
    ];
    for (query, expect) in rows {
        assert_eq!(ask(&c, query), tier0(expect), "{query}");
        let results = search(&c, query);
        assert!(
            results
                .nodes
                .iter()
                .all(|h| h.ranges.is_empty() && !h.exact && h.skipped == 0),
            "{query}: no ranges"
        );
        assert_eq!(results.nodes_total, expect.len(), "{query}");
    }
}

#[test]
fn words_are_tried_on_the_nodes_that_pass() {
    let c = tree();
    assert_eq!(
        ask(&c, "bill in:shop"),
        vec![("billing_note".to_string(), 0)]
    );
    assert_eq!(
        names(&c, "bill kind:fn"),
        s(&["bill_all", "billing_note", "rebill", "pay", "total"])
    );
    // the tiers are the words', not the filters': `bill` holds `rebill`, and `pay` and `total` hold it only in their path
    assert_eq!(
        ask(&c, "bill kind:fn"),
        want(&[
            ("bill_all", 0),
            ("billing_note", 0),
            ("rebill", 1),
            ("pay", 4),
            ("total", 4)
        ])
    );
    assert_eq!(names(&c, "bill kind:class"), s(&["Invoice"]));
    assert_eq!(names(&c, "bill -in:billing"), s(&["billing_note"]));
    assert_eq!(names(&c, "BILL IN:SHOP"), s(&["billing_note"]));
}

#[test]
fn no_doc_and_has_doc_look_at_the_docs_count() {
    let c = with_docs("total", 1);
    assert_eq!(
        names(&c, "kind:fn no:doc in:billing/"),
        s(&["bill_all", "pay", "rebill"]),
        "`total` has a doc, so it is left out"
    );
    assert_eq!(names(&c, "kind:fn has:doc in:billing/"), s(&["total"]));
    assert_eq!(
        names(&c, "kind:fn -has:doc in:billing/"),
        s(&["bill_all", "pay", "rebill"])
    );
    assert_eq!(names(&c, "has:doc"), s(&["total"]));
    assert_eq!(names(&c, "-no:doc"), s(&["total"]));
    assert_eq!(names(&c, "kind:fn has:doc"), s(&["total"]));
    assert_eq!(names(&c, "no:doc kind:class"), s(&["Invoice"]));
    // more than one doc is still "has"
    assert_eq!(names(&with_docs("pay", 3), "has:doc"), s(&["pay"]));
    // a tree where no node has a doc
    assert_eq!(names(&tree(), "has:doc"), Vec::<String>::new());
    assert_eq!(names(&tree(), "kind:method no:doc"), s(&["pay"]));
}

#[test]
fn a_value_that_means_nothing_keeps_nothing() {
    let c = tree();
    for query in [
        "kind:fnn",
        "kind:fnn bill",
        "kind:class,fnn",
        "-kind:fnn",
        "has:code",
        "no:code kind:fn",
        "has:doc no:doc",
        "has:doc -has:doc kind:fn",
    ] {
        let results = search(&c, query);
        assert!(results.nodes.is_empty(), "{query}");
        assert_eq!(results.nodes_total, 0, "{query}");
        assert!(parse(query).keeps_nothing, "{query}");
        assert!(!parse(query).notes.is_empty(), "{query}");
    }
    // even with `exact`: there is nothing to search
    let exact = run(&c, &parse("kind:fnn"), Some(0), Limits::default());
    assert!(exact.nodes.is_empty());
}

#[test]
fn a_phrase_is_for_text_not_for_nodes() {
    let c = tree();
    for query in [
        "\"a phrase\"",
        "\"bill\"",
        "kind:fn \"a phrase\"",
        "\"in:billing\" in:billing",
    ] {
        let results = search(&c, query);
        assert!(results.nodes.is_empty(), "{query}");
        assert_eq!(results.nodes_total, 0, "{query}");
    }
    assert!(!parse("\"a phrase\"").keeps_nothing);
    // with words, the nodes are found by the words alone
    assert_eq!(
        ask(&c, "bill in:shop \"a phrase\""),
        ask(&c, "bill in:shop")
    );
    assert_eq!(ask(&c, "\"x\" bill"), ask(&c, "bill"));
}

#[test]
fn nothing_to_search_finds_nothing() {
    let c = tree();
    for query in ["", "  ", "\"\"", "\"  \""] {
        let results = search(&c, query);
        assert!(results.nodes.is_empty(), "{query:?}");
        assert_eq!(results.nodes_total, 0, "{query:?}");
    }
    // a half-typed filter is a word, and finds by name like one
    assert!(search(&c, "kind:").nodes.is_empty());
    assert!(search(&c, "-in:").nodes.is_empty());
}

#[test]
fn filters_only_respect_the_limit_and_count_all_that_passed() {
    let c = tree();
    let two = run(&c, &parse("kind:fn"), None, Limits { nodes: 2, docs: 50 });
    let got: Vec<_> = two
        .nodes
        .iter()
        .map(|h| c.nodes()[h.index].name.as_str())
        .collect();
    assert_eq!(got, ["total", "bill_all"], "the first in reading order");
    assert_eq!(two.nodes_total, 6);

    let none = run(&c, &parse("kind:fn"), None, Limits { nodes: 0, docs: 50 });
    assert!(none.nodes.is_empty());
    assert_eq!(none.nodes_total, 6);

    // more than the default limit
    let mut nodes = vec![file("x/z.py")];
    nodes.extend((0..300).map(|i| func("x/z.py", &format!("f{i}"))));
    let big = corpus(nodes);
    let all = search(&big, "kind:fn");
    assert_eq!(all.nodes.len(), 200);
    assert_eq!(all.nodes_total, 300);
    let got: Vec<_> = all
        .nodes
        .iter()
        .map(|h| big.nodes()[h.index].name.clone())
        .collect();
    let want: Vec<_> = (0..200).map(|i| format!("f{i}")).collect();
    assert_eq!(got, want, "reading order, though `f10` is longer than `f1`");
}

#[test]
fn exact_comes_first_with_a_filter_and_is_not_repeated() {
    let c = tree();
    let at = |name: &str| c.nodes().iter().position(|n| n.name == name).unwrap();
    let rows = |results: &vnoc_search::Results| -> Vec<(String, bool)> {
        results
            .nodes
            .iter()
            .map(|h| (c.nodes()[h.index].name.clone(), h.exact))
            .collect()
    };
    let t = |name: &str, exact: bool| (name.to_string(), exact);

    // a node that passes: first, and not again below
    let found = run(
        &c,
        &parse("kind:fn bill"),
        Some(at("billing_note")),
        Limits::default(),
    );
    assert_eq!(
        rows(&found),
        [
            t("billing_note", true),
            t("bill_all", false),
            t("rebill", false),
            t("pay", false),
            t("total", false)
        ]
    );
    assert_eq!(found.nodes_total, 5);

    // a node the filters would drop is still first: the user pasted it
    let found = run(
        &c,
        &parse("kind:fn bill"),
        Some(at("shop")),
        Limits::default(),
    );
    assert_eq!(rows(&found)[0], t("shop", true));
    assert_eq!(found.nodes.len(), 6);
    assert_eq!(found.nodes_total, 6);

    // only filters: the same
    let found = run(
        &c,
        &parse("kind:class"),
        Some(at("shop")),
        Limits::default(),
    );
    assert_eq!(rows(&found), [t("shop", true), t("Invoice", false)]);
}

#[test]
fn in_ignores_case_and_reads_paths_that_are_not_ascii() {
    let c = corpus(vec![
        folder("café"),
        file("café/a.py"),
        func("café/a.py", "f"),
        folder("cafe"),
        folder("Ünï"),
        file("Ünï/b.py"),
        folder("straße"),
        file("straße/c.py"),
    ]);
    assert_eq!(names(&c, "in:café"), s(&["café", "a.py", "f"]));
    assert_eq!(names(&c, "in:CAFÉ/"), s(&["café", "a.py", "f"]));
    assert_eq!(names(&c, "in:cafe"), s(&["cafe"]));
    assert_eq!(names(&c, "in:ünï"), s(&["Ünï", "b.py"]));
    assert_eq!(names(&c, "in:ÜNÏ\\b.py"), s(&["b.py"]));
    assert_eq!(names(&c, "in:straße"), s(&["straße", "c.py"]));
    assert_eq!(names(&c, "-in:café -in:Ünï -in:straße"), s(&["cafe"]));
    assert_eq!(names(&c, "in:caf"), Vec::<String>::new());
}

#[test]
fn the_opened_folder_is_every_path() {
    let c = tree();
    assert_eq!(search(&c, "in:./").nodes_total, 12);
    assert_eq!(search(&c, "kind:class in:/").nodes_total, 1);
    assert_eq!(search(&c, "-in:./").nodes_total, 0);
}
