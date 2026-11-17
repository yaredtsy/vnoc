//! Names against words (02): the fuzzy tier, the qualified name, several words, the order, UTF-16 ranges, and the exact node.

mod common;

use common::*;
use vnoc_search::{Corpus, Kind, Limits, parse, run};

/// What `query` finds in a corpus of the one node `name`: its tier, its ranges and the words skipped, or none.
type Found = (u8, Vec<(u32, u32)>, u32);

fn one(name: &str, query: &str) -> Option<Found> {
    let c = corpus(vec![func("m.py", name)]);
    search(&c, query)
        .nodes
        .first()
        .map(|h| (h.tier, h.ranges.clone(), h.skipped))
}

#[test]
fn tier_2_the_table_of_02_3() {
    #[allow(clippy::type_complexity)]
    let rows: &[(&str, &str, Option<(u8, &[(u32, u32)], u32)>)] = &[
        ("chor", "checkout_order", Some((2, &[(0, 2), (9, 11)], 0))),
        ("co", "checkout_order", Some((2, &[(0, 1), (9, 10)], 0))),
        // 02 §3 says `cko` does not match ("no word starts with `k`"), but `checkout` holds `cko`: tier 1. `kor` is a real non-match.
        ("cko", "checkout_order", Some((1, &[(3, 6)], 0))),
        ("kor", "checkout_order", None),
        ("hc", "HTTPClient", Some((2, &[(0, 1), (4, 5)], 0))),
        ("httpcl", "HTTPClient", Some((0, &[(0, 6)], 0))),
        // nothing from `ab`, `abc` from `abc`, `d` from `d`: taking `ab` first leaves `cd` with no word to start
        ("abcd", "ab_abc_d", Some((2, &[(3, 6), (7, 8)], 1))),
        ("or", "checkout_order", Some((1, &[(9, 11)], 0))),
        ("init", "__init__", Some((1, &[(2, 6)], 0))),
    ];
    for &(query, name, want) in rows {
        let want = want.map(|(tier, ranges, skipped)| (tier, ranges.to_vec(), skipped));
        assert_eq!(one(name, query), want, "{query} in {name}");
    }
}

#[test]
fn tier_2_other_cuts() {
    // a plain subsequence is not a match
    assert_eq!(one("checkout_order", "ckor"), None);
    // camel case and digits are word starts too
    assert_eq!(
        one("checkoutOrder", "co"),
        Some((2, vec![(0, 1), (8, 9)], 0))
    );
    assert_eq!(
        one("sha256sum", "s2s"),
        Some((2, vec![(0, 1), (3, 4), (6, 7)], 0))
    );
    // letters may not be used twice, nor out of order
    assert_eq!(one("checkout_order", "oc"), None);
    assert_eq!(one("ab", "aab"), None);
    // two prefixes of one name word are not allowed: each name word gives one prefix
    assert_eq!(one("a_b", "ab"), Some((2, vec![(0, 1), (2, 3)], 0)));
    assert_eq!(one("abc_d", "abcdd"), None);
}

#[test]
fn tier_2_keeps_the_cut_that_skips_fewest_then_the_longest_first_prefix() {
    // `a` + `b` of the first two words skips nothing; `b` of the last would skip two
    assert_eq!(one("a_b_a_b", "ab"), Some((2, vec![(0, 1), (2, 3)], 0)));
    // `a` of the first word and `bc` of the second skip nothing; `ab` + `c` would skip a word
    assert_eq!(one("ab_bc_c", "abc"), Some((2, vec![(0, 1), (3, 5)], 0)));
}

/// Every cut of `query` into prefixes of the words, in order: the words used, as (word, length).
fn cuts(
    words: &[&str],
    query: &str,
    from: usize,
    out: &mut Vec<(usize, usize)>,
    all: &mut Vec<Vec<(usize, usize)>>,
) {
    if query.is_empty() {
        all.push(out.clone());
        return;
    }
    for w in from..words.len() {
        let shared = query
            .chars()
            .zip(words[w].chars())
            .take_while(|(a, b)| a == b)
            .count();
        for len in 1..=shared {
            out.push((w, len));
            cuts(words, &query[len..], w + 1, out, all);
            out.pop();
        }
    }
}

#[test]
fn tier_2_agrees_with_trying_every_cut() {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    let mut tier_2 = 0;
    for _ in 0..3000 {
        let words: Vec<String> = (0..1 + next(5))
            .map(|_| (0..1 + next(3)).map(|_| ['a', 'b', 'c'][next(3)]).collect())
            .collect();
        let name = words.join("_");
        let query: String = (0..1 + next(5)).map(|_| ['a', 'b', 'c'][next(3)]).collect();
        if name.contains(&query) {
            continue; // tier 0 or 1
        }
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let mut all = Vec::new();
        cuts(&word_refs, &query, 0, &mut Vec::new(), &mut all);
        // the fewest words passed over before the last one used, then the longest first prefix
        let best = all
            .iter()
            .map(|cut| (cut.last().unwrap().0 + 1 - cut.len(), usize::MAX - cut[0].1))
            .min();
        let got = one(&name, &query);
        match best {
            None => assert!(
                got.is_none() || got.as_ref().unwrap().0 > 2,
                "{query} in {name}: {got:?}"
            ),
            Some((skipped, longest)) => {
                tier_2 += 1;
                let (tier, ranges, got_skipped) =
                    got.unwrap_or_else(|| panic!("{query} in {name}: nothing"));
                assert_eq!(
                    (tier, got_skipped as usize),
                    (2, skipped),
                    "{query} in {name}"
                );
                let first = (ranges[0].1 - ranges[0].0) as usize;
                assert_eq!(
                    usize::MAX - first,
                    longest,
                    "{query} in {name}: the first prefix"
                );
                let letters: usize = ranges.iter().map(|r| (r.1 - r.0) as usize).sum();
                assert_eq!(
                    letters,
                    query.len(),
                    "{query} in {name}: every letter used once"
                );
            }
        }
    }
    assert!(
        tier_2 > 200,
        "the generator reached tier 2 only {tier_2} times"
    );
}

#[test]
fn a_word_with_a_dot_finds_the_methods_by_qualified_name() {
    let c = corpus(vec![
        file("src/orders.py"),
        def(Kind::Class, "src/orders.py", "OrderService"),
        method("src/orders.py", "OrderService.parse_order"),
        func("src/orders.py", "parse_order"),
    ]);
    let found = search(&c, "OrderService.par");
    assert_eq!(rows(&c, &found), want(&[("parse_order", 3)]));
    let hit = &found.nodes[0];
    assert_eq!(
        hit.index, 2,
        "the method, not the module-level function (index 3)"
    );
    assert!(c.nodes()[hit.index].in_class);
    assert!(
        hit.ranges.is_empty(),
        "the name did not match, so nothing is bold"
    );
}

#[test]
fn several_words_each_alone_the_worst_tier_counts() {
    let c = corpus(vec![
        func("m.py", "check_order"),
        func("m.py", "checkout_order"),
        func("m.py", "check"),
        func("m.py", "order"),
    ]);
    let found = search(&c, "order check");
    assert_eq!(
        rows(&c, &found),
        want(&[("check_order", 1), ("checkout_order", 1)])
    );
    assert_eq!(
        found.nodes[0].ranges,
        [(0, 5), (6, 11)],
        "both words are bold, sorted"
    );
    assert_eq!(found.nodes[1].ranges, [(0, 5), (9, 14)]);

    let c = corpus(vec![
        func("billing/invoice.py", "pay"),
        func("billing/invoice.py", "total"),
        func("shop/cart.py", "pay_now"),
    ]);
    let found = search(&c, "billing/ pay");
    assert_eq!(
        rows(&c, &found),
        want(&[("pay", 4)]),
        "the path word is the worst"
    );
    assert_eq!(
        found.nodes[0].ranges,
        [(0, 3)],
        "the word that matched the name is bold"
    );
}

#[test]
fn the_order_of_02_5() {
    // (what the row shows, query, names in reading order, names in the answer)
    let rows: &[(&str, &str, &[&str], &[&str])] = &[
        (
            "the tier beats the length; fewer skipped beats the length",
            "ac",
            &["a_x_c", "a_c_zzz", "zzac", "ac_zzzzzzzz"],
            &["ac_zzzzzzzz", "zzac", "a_c_zzz", "a_x_c"],
        ),
        (
            "the shorter name beats reading order",
            "ac",
            &["a_c_yyyy", "a_c"],
            &["a_c", "a_c_yyyy"],
        ),
        (
            "reading order last",
            "ac",
            &["a_c_p", "a_c_q"],
            &["a_c_p", "a_c_q"],
        ),
        (
            "reading order last, reversed",
            "ac",
            &["a_c_q", "a_c_p"],
            &["a_c_q", "a_c_p"],
        ),
        // 2 words each: a_b_c_d_eeee skips 0 + 2, a_b_x_c_d skips 0 + 3; the longer name comes first
        (
            "two words: skipped adds up",
            "ab cd",
            &["a_b_x_c_d", "a_b_c_d_eeee"],
            &["a_b_c_d_eeee", "a_b_x_c_d"],
        ),
        // length is in chars, not bytes: `xéééé` is 5 chars and 9 bytes, `xabcdef` is 7 and 7
        (
            "the length is in chars",
            "x",
            &["xabcdef", "xéééé"],
            &["xéééé", "xabcdef"],
        ),
    ];
    for &(what, query, names, expect) in rows {
        let c = corpus(names.iter().map(|n| func("m.py", n)).collect());
        let got: Vec<_> = search(&c, query)
            .nodes
            .iter()
            .map(|h| c.nodes()[h.index].name.clone())
            .collect();
        assert_eq!(got, expect, "{what}");
    }

    let c = corpus(vec![
        func("m.py", "a_b_x_c_d"),
        func("m.py", "a_b_c_d_eeee"),
    ]);
    let skipped: Vec<_> = search(&c, "ab cd")
        .nodes
        .iter()
        .map(|h| h.skipped)
        .collect();
    assert_eq!(skipped, [2, 3]);
}

#[test]
fn skipped_counts_only_at_tier_2() {
    // `ab` is tier 2 with one word skipped, but `m.py` makes the node tier 4: skipped plays no part there
    let c = corpus(vec![func("m.py", "a_x_b")]);
    let hit = &search(&c, "ab m.py").nodes[0];
    assert_eq!((hit.tier, hit.skipped), (4, 0));
    let hit = &search(&c, "ab").nodes[0];
    assert_eq!((hit.tier, hit.skipped), (2, 1));
}

#[test]
fn ranges_are_utf16_units() {
    // `é` is two bytes and one unit: `or` starts at byte 6 and unit 5
    assert_eq!(one("café_order", "or"), Some((1, vec![(5, 7)], 0)));
    assert_eq!(one("café_order", "co"), Some((2, vec![(0, 1), (5, 6)], 0)));
    // an emoji is four bytes and two units
    assert_eq!(one("😀_order", "or"), Some((1, vec![(3, 5)], 0)));
    assert_eq!(one("😀_order", "o"), Some((1, vec![(3, 4)], 0)));
    // `İ` lower-cases to two chars; the match after it still points into the name as written
    assert_eq!(one("İx", "x"), Some((1, vec![(1, 2)], 0)));
}

#[test]
fn the_exact_node_is_first_marked_and_not_repeated() {
    let c = corpus(vec![
        func("m.py", "f_a"),
        func("m.py", "f"),
        func("m.py", "f_b"),
        func("m.py", "zzz"),
    ]);
    let limits = Limits::default();

    // the query finds f_a, f and f_b; the reference named f_b, which comes first and is not shown again
    let found = run(&c, &parse("f"), Some(2), limits);
    let got: Vec<_> = found
        .nodes
        .iter()
        .map(|h| (c.nodes()[h.index].name.as_str(), h.exact))
        .collect();
    assert_eq!(got, [("f_b", true), ("f", false), ("f_a", false)]);
    assert_eq!(found.nodes_total, 3, "once");

    // a node the words do not find, named by the reference, still comes first
    let found = run(&c, &parse("f"), Some(3), limits);
    let got: Vec<_> = found
        .nodes
        .iter()
        .map(|h| (c.nodes()[h.index].name.as_str(), h.exact))
        .collect();
    assert_eq!(
        got,
        [("zzz", true), ("f", false), ("f_a", false), ("f_b", false)]
    );
    assert_eq!(found.nodes_total, 4);

    // the limit counts it
    let found = run(&c, &parse("f"), Some(2), Limits { nodes: 2, docs: 50 });
    let got: Vec<_> = found
        .nodes
        .iter()
        .map(|h| c.nodes()[h.index].name.as_str())
        .collect();
    assert_eq!(got, ["f_b", "f"]);
    assert_eq!(found.nodes_total, 3);

    // an index outside the corpus is ignored, and an empty query finds nothing
    assert_eq!(run(&c, &parse("f"), Some(99), limits).nodes.len(), 3);
    assert_eq!(run(&c, &parse(""), Some(1), limits).nodes_total, 0);
}

#[test]
fn the_corpus_keeps_the_order_given_and_the_docs_stay_empty() {
    let c = Corpus::new(vec![func("m.py", "b"), func("m.py", "a")], Vec::new());
    assert_eq!(c.nodes()[0].name, "b");
    let found = search(&c, "a");
    assert_eq!((found.docs.len(), found.docs_total), (0, 0));
}

/// What `query` finds in a corpus of the one node: its tier and ranges.
fn only(node: vnoc_search::NodeEntry, query: &str) -> Option<(u8, Vec<(u32, u32)>)> {
    let c = corpus(vec![node]);
    search(&c, query)
        .nodes
        .first()
        .map(|h| (h.tier, h.ranges.clone()))
}

#[test]
fn a_name_that_is_not_ascii_is_matched_folded_and_its_ranges_are_utf16() {
    let name = |n: &str| func("m.py", n);
    // `É` folds to `é` on its own, and `é` is two bytes and one unit
    assert_eq!(only(name("École_Order"), "éc"), Some((0, vec![(0, 2)])));
    assert_eq!(only(name("résumé_order"), "umé"), Some((1, vec![(3, 6)])));
    assert_eq!(
        only(name("résumé_order"), "ro"),
        Some((2, vec![(0, 1), (7, 8)]))
    );
    assert_eq!(
        only(name("résumé_order"), "RÉS"),
        Some((0, vec![(0, 3)])),
        "the query's case is ignored too"
    );
}

#[test]
fn a_qualified_name_or_a_path_that_is_not_ascii_is_matched_folded() {
    let qualified = node(
        Kind::Function,
        "shop/a.py",
        "order",
        Some("Café.order"),
        true,
    );
    assert_eq!(only(qualified.clone(), "café.o"), Some((3, vec![])));
    assert_eq!(only(qualified, "CAFÉ.O"), Some((3, vec![])));

    let in_path = func("café/m.py", "zzz");
    assert_eq!(only(in_path.clone(), "café/m"), Some((4, vec![])));
    assert_eq!(only(in_path.clone(), "fé/m"), Some((5, vec![])));
    assert_eq!(only(in_path, "CAFÉ"), Some((4, vec![])));
}

#[test]
fn an_ascii_text_and_a_word_that_is_not_ascii_do_not_match_and_do_not_panic() {
    // the name, the qualified name and the path are all ASCII here; the query is not
    let plain = node(
        Kind::Function,
        "shop/cafe.py",
        "cafe",
        Some("Shop.cafe"),
        false,
    );
    for query in ["é", "café", "éa", "shop é"] {
        assert_eq!(only(plain.clone(), query), None, "{query}");
    }
    // one corpus with both kinds: the ASCII node is found by an ASCII word, the other by both
    let c = corpus(vec![plain, func("shop/m.py", "café")]);
    assert_eq!(ask(&c, "caf"), want(&[("cafe", 0), ("café", 0)]));
    assert_eq!(ask(&c, "café"), want(&[("café", 0)]));
}
