//! The ranking rows of `webview/src/sidebar/search.test.ts`, ported (02, 06 S1). The order of every row is the TypeScript's; only the tier
//! numbers moved (old 2 and 3 are 4 and 5), and `INVOICE`'s `pay` is tier 3 because its qualified name `Invoice.pay` holds the word.

mod common;

use common::*;
use vnoc_search::{Limits, parse, run};

#[test]
fn the_tree_is_in_reading_order_with_one_method() {
    let c = tree();
    let names: Vec<_> = c.nodes().iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "billing",
            "invoice.py",
            "total",
            "bill_all",
            "Invoice",
            "pay",
            "rebilling.py",
            "rebill",
            "shop",
            "cart.py",
            "add_item",
            "billing_note"
        ]
    );
    let methods: Vec<_> = c
        .nodes()
        .iter()
        .filter(|n| n.in_class)
        .map(|n| n.name.as_str())
        .collect();
    assert_eq!(methods, ["pay"]);
}

#[test]
fn ranking_rows() {
    let c = tree();
    #[allow(clippy::type_complexity)]
    let rows: &[(&str, &str, &[(&str, u8)])] = &[
        (
            "`bill`: name prefixes, then name substrings, then what holds it only in its path",
            "bill",
            &[
                ("billing", 0),
                ("bill_all", 0),
                ("billing_note", 0),
                ("rebill", 1),
                ("rebilling.py", 1),
                ("pay", 4),
                ("total", 4),
                ("Invoice", 4),
                ("invoice.py", 4),
            ],
        ),
        (
            "case does not matter; `pay` holds the word in its qualified name",
            "INVOICE",
            &[
                ("Invoice", 0),
                ("invoice.py", 0),
                ("pay", 3),
                ("total", 5),
                ("bill_all", 5),
            ],
        ),
        (
            "`billing/` finds by path: everything under that folder, not the folder itself",
            "billing/",
            &[
                ("pay", 4),
                ("total", 4),
                ("rebill", 4),
                ("Invoice", 4),
                ("bill_all", 4),
                ("invoice.py", 4),
                ("rebilling.py", 4),
            ],
        ),
        (
            "a path substring in the middle of the path",
            "illing/inv",
            &[
                ("pay", 5),
                ("total", 5),
                ("Invoice", 5),
                ("bill_all", 5),
                ("invoice.py", 5),
            ],
        ),
        ("a blank query finds nothing", "  ", &[]),
        ("an empty query finds nothing", "", &[]),
        ("nothing matches", "zzz", &[]),
    ];
    for (name, query, expect) in rows {
        assert_eq!(ask(&c, query), want(expect), "{name}");
    }
}

#[test]
fn the_four_tiers_in_order() {
    let c = corpus(vec![
        func("lib/shopping.py", "x1"), // the path holds `shop`, not at its start
        func("shop/b.py", "x2"),       // the path starts with it
        func("c.py", "myshop"),        // the name holds it
        func("d.py", "shopper"),       // the name starts with it
    ]);
    assert_eq!(
        ask(&c, "shop"),
        want(&[("shopper", 0), ("myshop", 1), ("x2", 4), ("x1", 5)])
    );
}

#[test]
fn a_name_prefix_beats_a_path_match() {
    let rows = ask(&tree(), "shop");
    assert_eq!(rows[0], ("shop".to_string(), 0));
    assert!(
        rows[1..].iter().all(|r| r.1 == 4),
        "what is under shop/ matches by path: {rows:?}"
    );
    assert_eq!(rows.len(), 4);
}

#[test]
fn the_cap_shows_the_best_200_of_301() {
    let mut nodes = vec![file("x/z.py")];
    nodes.extend((0..300).map(|i| func("x/z.py", &format!("f{i}"))));
    nodes.push(func("x/z.py", "f")); // a whole-name match: first, wherever it stands
    let c = corpus(nodes);

    let all = search(&c, "f");
    assert_eq!(all.nodes.len(), 200);
    assert_eq!(all.nodes_total, 301);
    assert_eq!(
        c.nodes()[all.nodes[0].index].name,
        "f",
        "the best first, whatever the order in the tree"
    );

    let five = run(&c, &parse("f"), None, Limits { nodes: 5, docs: 50 });
    let names: Vec<_> = five
        .nodes
        .iter()
        .map(|h| c.nodes()[h.index].name.as_str())
        .collect();
    assert_eq!(
        names,
        ["f", "f0", "f1", "f2", "f3"],
        "the shorter names first, then reading order"
    );
    assert_eq!(five.nodes_total, 301);
}

#[test]
fn the_letters_to_draw_bold() {
    let c = tree();
    let results = search(&c, "bill");
    let ranges: Vec<_> = results.nodes.iter().map(|h| h.ranges.clone()).collect();
    let none: Vec<(u32, u32)> = vec![];
    assert_eq!(
        ranges,
        [
            vec![(0, 4)],
            vec![(0, 4)],
            vec![(0, 4)],
            vec![(2, 6)],
            vec![(2, 6)],
            none.clone(),
            none.clone(),
            none.clone(),
            none.clone()
        ],
        "bold for a name match, nothing for a path match"
    );
    assert_eq!(
        search(&c, "BILL").nodes[1].ranges,
        [(0, 4)],
        "`bill_all` with the query in capitals"
    );
    assert!(
        search(&c, "billing/").nodes[0].ranges.is_empty(),
        "a path match has nothing bold in the name"
    );
}
