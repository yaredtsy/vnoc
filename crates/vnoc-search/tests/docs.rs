//! Doc text (03): words, BM25, the prefix, phrases, the snippet, filters by the doc's node, order and limit. Small hand-built corpora: S1's tree
//! (`common::tree_nodes`) with a few docs on its nodes, and Missing nodes' docs.

mod common;

use std::sync::Arc;

use common::*;
use vnoc_search::{Corpus, DocEntry, DocIndex, Kind, Limits, parse, run};

fn body(file: &str, text: &str) -> DocEntry {
    doc_on("total", file, "", text)
}

fn corpus_of(bodies: &[(&str, &str)]) -> Corpus {
    with_docs(bodies.iter().map(|(f, t)| body(f, t)).collect())
}

// ── words ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_camel_case_word_is_found_whole_and_by_its_parts() {
    let c = corpus_of(&[("a", "The retryPolicy applies."), ("b", "other")]);
    for query in [
        "retrypolicy",
        "RETRYPOLICY",
        "retry",
        "policy",
        "retry policy",
    ] {
        assert_eq!(found(&c, query), ["a"], "{query}");
    }
    assert_eq!(found(&c, "retrypol"), ["a"], "a prefix of the whole word");
}

#[test]
fn a_word_with_an_accent_is_one_word() {
    let c = corpus_of(&[("a", "un café noir"), ("b", "other")]);
    assert_eq!(found(&c, "café"), ["a"]);
    assert_eq!(found(&c, "café noir"), ["a"]);
    assert_eq!(found(&c, "caf"), ["a"], "a prefix");
    assert!(found(&c, "cafe").is_empty(), "no folding of accents");
    assert!(found(&c, "é").is_empty(), "`é` is not a word of its own");
    assert!(found(&c, "fé").is_empty());
}

#[test]
fn snake_case_splits() {
    let c = corpus_of(&[("a", "call snake_case here"), ("b", "other")]);
    for query in [
        "snake",
        "case",
        "snake case",
        "case snake",
        "snake_case",
        "snake-case",
    ] {
        assert_eq!(found(&c, query), ["a"], "{query}");
    }
}

#[test]
fn letters_and_digits_split_as_names_do() {
    let c = corpus_of(&[("a", "the x2y gadget"), ("b", "other")]);
    for query in ["x2y", "x", "2", "y", "x 2 y", "gadget x2y"] {
        assert_eq!(found(&c, query), ["a"], "{query}");
    }
}

#[test]
fn query_words_are_split_at_anything_that_is_not_a_letter_or_a_digit() {
    let c = corpus_of(&[("a", "the order service parses"), ("b", "the order only")]);
    // `OrderService.par` is `orderservice` and `par`, both required: `orderservice` is no word of the doc's (it is `order` `service`)
    assert!(found(&c, "OrderService.par").is_empty());
    assert_eq!(found(&c, "order/service"), ["a"]);
    assert_eq!(found(&c, "order.par"), ["a"]);
    assert_eq!(found(&c, "billing/ order"), Vec::<String>::new());
    assert_eq!(
        sorted(found(&c, "order /")),
        ["a", "b"],
        "a word that is only punctuation adds nothing"
    );
    let c = corpus_of(&[("a", "billing order"), ("b", "other")]);
    assert_eq!(found(&c, "billing/"), ["a"]);
}

// ── ranking ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_word_in_the_title_beats_the_same_word_once_in_the_body() {
    // equal length (a title word counts three): 3 + 1 words each
    let c = with_docs(vec![
        doc_on("total", "body-hit", "Other", "retry"),
        doc_on("total", "title-hit", "Retry", "other"),
        doc_on("total", "neither", "Plain", "text"),
    ]);
    assert_eq!(found(&c, "retry"), ["title-hit", "body-hit"]);
    let scores: Vec<f64> = search(&c, "retry").docs.iter().map(|h| h.score).collect();
    assert!(scores[0] > scores[1] && scores[1] > 0.0, "{scores:?}");
}

#[test]
fn every_word_is_required() {
    let c = corpus_of(&[
        ("both", "retry policy"),
        ("one", "retry only"),
        ("other", "policy only"),
        ("title", "x"),
    ]);
    assert_eq!(found(&c, "retry policy"), ["both"]);
    assert_eq!(found(&c, "policy retry"), ["both"]);
    assert_eq!(
        found(&c, "retry"),
        ["both", "one"].map(String::from),
        "a single word finds both"
    );
    // a word in the title and the other in the body is enough
    let c = with_docs(vec![doc_on("total", "split", "Retry", "policy")]);
    assert_eq!(found(&c, "retry policy"), ["split"]);
}

#[test]
fn a_frequent_word_beats_a_rare_one_at_equal_length() {
    let c = corpus_of(&[
        ("rare", "retry policy policy policy"),
        ("frequent", "retry retry retry policy"),
        ("none", "unrelated text of the doc"),
    ]);
    assert_eq!(found(&c, "retry"), ["frequent", "rare"]);
}

#[test]
fn bm25_matches_the_hand_computed_scores_of_a_three_doc_corpus() {
    // d0 `retry retry policy` (length 3), d1 `retry backoff jitter timeout` (4), d2 `retry timeout timeout timeout timeout` (5): N = 3, the average
    // length is 4, and df(retry) = 3. With k1 = 1.2 and b = 0.75:
    //   idf(retry)   = ln(1 + (3 - 3 + 0.5) / (3 + 0.5)) = 0.13353139262452257
    //   d0: idf * 2 * 2.2 / (2 + 1.2 * (0.25 + 0.75 * 3 / 4)) = idf * 4.4 / 2.975
    //   d1: idf * 1 * 2.2 / (1 + 1.2 * (0.25 + 0.75 * 4 / 4)) = idf * 2.2 / 2.2 = idf
    //   d2: idf * 1 * 2.2 / (1 + 1.2 * (0.25 + 0.75 * 5 / 4)) = idf * 2.2 / 2.45
    let c = corpus_of(&[
        ("d0", "retry retry policy"),
        ("d1", "retry backoff jitter timeout"),
        ("d2", "retry timeout timeout timeout timeout"),
    ]);
    let hits = search(&c, "retry").docs;
    let got: Vec<(String, f64)> = hits
        .iter()
        .map(|h| (c.docs()[h.index].file.clone(), h.score))
        .collect();
    let want = [
        ("d0", 0.197_491_807_579_125_82),
        ("d1", 0.133_531_392_624_522_57),
        ("d2", 0.121_141_881_968_639_05),
    ];
    assert_eq!(got.len(), 3);
    for ((file, score), (want_file, want_score)) in got.iter().zip(want) {
        assert_eq!(file, want_file);
        assert!(
            (score - want_score).abs() < 1e-9,
            "{file}: {score} vs {want_score}"
        );
    }
    // two words: the sum of two terms. `policy` is in d0 only (idf = ln(1 + (3 - 1 + 0.5) / 1.5) = 0.9808292530117263; tf 1, length 3: * 2.2 / 1.975)
    // and `retry` is the last word, a prefix with the same one word under it
    let hits = search(&c, "policy retry").docs;
    assert_eq!(hits.len(), 1);
    assert!(
        (hits[0].score - 1.290_061_102_073_200_7).abs() < 1e-9,
        "{}",
        hits[0].score
    );
    // `timeout`: df 2 (d1, d2), idf = ln(1 + (3 - 2 + 0.5) / 2.5) = 0.47000362924573563; d2 has it four times
    let hits = search(&c, "timeout").docs;
    let scores: Vec<(String, f64)> = hits
        .iter()
        .map(|h| (c.docs()[h.index].file.clone(), h.score))
        .collect();
    assert_eq!(scores[0].0, "d2");
    assert!(
        (scores[0].1 - 0.762_402_200_435_479_1).abs() < 1e-9,
        "{scores:?}"
    );
    assert!(
        (scores[1].1 - 0.470_003_629_245_735_63).abs() < 1e-9,
        "{scores:?}"
    );
}

#[test]
fn a_title_word_counts_three_times_in_the_count_and_in_the_length() {
    // d0: title `Retry` (3) and body `x y` (2): length 5, tf(retry) 3. d1: `a b c d e` (5), no retry. N = 2, average 5, df 1:
    //   idf = ln(1 + (2 - 1 + 0.5) / 1.5) = ln 2; score = ln 2 * 3 * 2.2 / (3 + 1.2 * 1) = ln 2 * 6.6 / 4.2
    let c = with_docs(vec![
        doc_on("total", "d0", "Retry", "x y"),
        doc_on("total", "d1", "", "a b c d e"),
    ]);
    let score = search(&c, "retry").docs[0].score;
    let want = std::f64::consts::LN_2 * 6.6 / 4.2;
    assert!((score - want).abs() < 1e-9, "{score} vs {want}");
}

// ── the prefix ─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_last_word_is_a_prefix() {
    let c = corpus_of(&[
        ("a", "retry later"),
        ("b", "retries happen"),
        ("c", "other words"),
    ]);
    assert_eq!(found(&c, "ret"), ["a", "b"]);
    assert_eq!(found(&c, "retr"), ["a", "b"]);
    assert_eq!(found(&c, "retri"), ["b"]);
    assert_eq!(found(&c, "RET"), ["a", "b"]);
    assert!(found(&c, "etry").is_empty(), "a prefix, not a substring");
}

#[test]
fn only_the_last_piece_is_a_prefix() {
    let c = corpus_of(&[
        ("short", "ret policy"),
        ("long", "retry policy"),
        ("long-only", "retry"),
    ]);
    // `ret` must be a whole word when it is not last; `pol` opens into `policy`
    assert_eq!(found(&c, "ret pol"), ["short"]);
    assert_eq!(found(&c, "retry pol"), ["long"]);
    // `pol` is no whole word of any doc, so putting it first finds nothing
    assert!(found(&c, "pol ret").is_empty());
    assert_eq!(sorted(found(&c, "policy ret")), ["long", "short"]);
    // `ret` alone is the last piece: a prefix of `ret` and of `retry`
    assert_eq!(sorted(found(&c, "ret")), ["long", "long-only", "short"]);
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn a_prefix_that_opens_300_words_uses_200_of_them() {
    // one doc per word, `a` and three letters: every df is 1, so the first 200 by the word are used
    let docs: Vec<DocEntry> = (0..300)
        .map(|i| doc_on("total", &format!("{i:03}"), "", &letters('a', i)))
        .collect();
    let c = with_docs(docs);
    let all = Limits {
        nodes: 0,
        docs: 1000,
    };
    let results = run(&c, &parse("a"), None, all);
    assert_eq!(results.docs_total, 200);
    let mut files: Vec<usize> = results
        .docs
        .iter()
        .map(|h| c.docs()[h.index].file.parse().unwrap())
        .collect();
    files.sort();
    assert_eq!(files, (0..200).collect::<Vec<_>>());
    // a prefix that opens fewer words is not capped: `aal` is the words 286 to 299
    assert_eq!(letters('a', 286), "aala");
    assert_eq!(run(&c, &parse("aal"), None, all).docs_total, 14);
    assert_eq!(run(&c, &parse("aaa"), None, all).docs_total, 26);
    assert_eq!(run(&c, &parse("aa"), None, all).docs_total, 200);
}

// ── phrases ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_phrase_is_held_as_written_across_a_line_end() {
    let c = corpus_of(&[
        ("across", "we apply the retry\npolicy twice"),
        ("spaces", "a Retry   Policy b"),
        ("apart", "retry first. policy later"),
        ("reverse", "policy retry"),
        ("hyphen", "retry-policy"),
    ]);
    assert_eq!(sorted(found(&c, "\"retry policy\"")), ["across", "spaces"]);
    assert_eq!(sorted(found(&c, "\"RETRY POLICY\"")), ["across", "spaces"]);
    // the doc with both words but not the phrase is not found, though the words alone find it
    assert_eq!(
        sorted(found(&c, "retry policy")),
        ["across", "apart", "hyphen", "reverse", "spaces"]
    );
}

#[test]
fn a_phrase_alone_finds_docs_and_no_nodes() {
    let c = corpus_of(&[("a", "the retry policy of billing"), ("b", "other")]);
    let results = search(&c, "\"retry policy\"");
    assert_eq!(results.docs.len(), 1);
    assert_eq!(results.docs_total, 1);
    assert!(results.nodes.is_empty());
    assert_eq!(results.nodes_total, 0);
}

#[test]
fn a_phrase_in_the_title_is_found() {
    let c = with_docs(vec![
        doc_on("total", "title", "The Retry Policy", "nothing here"),
        doc_on("total", "body", "Other", "retry policy"),
        doc_on("total", "none", "Retry", "policy"),
    ]);
    assert_eq!(sorted(found(&c, "\"retry policy\"")), ["body", "title"]);
    let hit = &search(&c, "\"retry policy\"").docs[..];
    let title = hit
        .iter()
        .find(|h| c.docs()[h.index].file == "title")
        .unwrap();
    assert_eq!(
        title.title_ranges,
        [(4, 16)],
        "the phrase, merged with its two words"
    );
}

#[test]
fn words_and_a_phrase_find_nodes_by_the_words_and_docs_by_both() {
    let c = corpus_of(&[("a", "billing: the retry policy"), ("b", "billing only")]);
    let results = search(&c, "bill \"retry policy\"");
    let files: Vec<_> = results
        .docs
        .iter()
        .map(|h| c.docs()[h.index].file.as_str())
        .collect();
    assert_eq!(files, ["a"]);
    assert_eq!(
        rows(&c, &results),
        ask(&c, "bill"),
        "nodes are the words' alone"
    );
}

// ── the snippet ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

const FILLER: &str = "lorem ipsum dolor sit amet";

fn one(text: &str, query: &str) -> vnoc_search::DocHit {
    let c = corpus_of(&[("a", text)]);
    search(&c, query).docs.remove(0)
}

#[test]
fn a_long_body_is_cut_at_word_boundaries_with_an_ellipsis_at_both_ends() {
    let side = format!("{FILLER} ").repeat(20);
    let hit = one(&format!("{side}needle {side}"), "needle");
    let s = &hit.snippet;
    assert!(s.starts_with('…') && s.ends_with('…'), "{s:?}");
    let inner: &str = s.trim_matches('…');
    assert!(
        inner.chars().count() <= 120,
        "{} chars",
        inner.chars().count()
    );
    assert!(
        inner.chars().count() >= 100,
        "{} chars",
        inner.chars().count()
    );
    let vocabulary: Vec<&str> = FILLER.split(' ').chain(["needle"]).collect();
    assert!(
        inner.split(' ').all(|w| vocabulary.contains(&w)),
        "only whole words: {inner:?}"
    );
    assert!(!inner.starts_with(' ') && !inner.ends_with(' '));
    // the hit is about 40 chars in
    let at = inner.find("needle").unwrap();
    assert!((20..=45).contains(&at), "{at}");
    assert_eq!(hit.snippet_ranges.len(), 1);
    assert_eq!(slice16(s, hit.snippet_ranges[0]), "needle");
}

#[test]
fn a_hit_near_the_start_has_no_leading_ellipsis() {
    let tail = format!(" {FILLER}").repeat(20);
    let hit = one(&format!("The needle is here{tail}"), "needle");
    assert!(
        hit.snippet.starts_with("The needle is here"),
        "{:?}",
        hit.snippet
    );
    assert!(hit.snippet.ends_with('…'));
    assert_eq!(hit.snippet_ranges, [(4, 10)]);
    // a hit near the end: no trailing ellipsis
    let head = format!("{FILLER} ").repeat(20);
    let hit = one(&format!("{head}the needle"), "needle");
    assert!(
        hit.snippet.starts_with('…') && hit.snippet.ends_with("the needle"),
        "{:?}",
        hit.snippet
    );
    assert_eq!(slice16(&hit.snippet, hit.snippet_ranges[0]), "needle");
}

#[test]
fn line_ends_and_runs_of_white_space_become_one_space() {
    let hit = one("first line\nsecond  line\r\nneedle\n\n\tthird", "needle");
    assert_eq!(hit.snippet, "first line second line needle third");
    assert_eq!(hit.snippet_ranges, [(23, 29)]);
    assert_eq!(slice16(&hit.snippet, hit.snippet_ranges[0]), "needle");
    // leading white space is not part of it either
    let hit = one("\n\n  needle", "needle");
    assert_eq!(hit.snippet, "needle");
}

#[test]
fn snippet_ranges_count_utf16_units_with_the_leading_ellipsis() {
    // `é` is one unit (two bytes) and the emoji two units (four bytes)
    let hit = one("é é é needle", "needle");
    assert_eq!(hit.snippet, "é é é needle");
    assert_eq!(hit.snippet_ranges, [(6, 12)]);
    let hit = one("😀 needle 😀", "needle");
    assert_eq!(hit.snippet_ranges, [(3, 9)]);
    assert_eq!(slice16(&hit.snippet, hit.snippet_ranges[0]), "needle");

    // with the `…` in front: it is one unit, and the accents before the hit are counted
    let before = "éàü ".repeat(40);
    let hit = one(
        &format!("{before}needle and café needle {before}"),
        "needle",
    );
    assert!(hit.snippet.starts_with('…'), "{:?}", hit.snippet);
    assert_eq!(hit.snippet_ranges.len(), 2, "every hit inside the snippet");
    for range in &hit.snippet_ranges {
        assert_eq!(slice16(&hit.snippet, *range), "needle");
    }
    let first = hit.snippet_ranges[0].0 as usize;
    let units: Vec<u16> = hit.snippet.encode_utf16().collect();
    assert_eq!(units[0], '…' as u16);
    assert!(first > 1 && first < units.len());
    // the bytes before it are more than the units: the range is not a byte range
    let bytes = hit.snippet.find("needle").unwrap();
    assert!(bytes > first, "{bytes} bytes, {first} units");
}

#[test]
fn every_hit_in_the_snippet_is_a_range_and_touching_ones_merge() {
    let hit = one("needle one needle two needle", "needle");
    assert_eq!(hit.snippet_ranges, [(0, 6), (11, 17), (22, 28)]);
    // `retry` and `policy` side by side, found as a phrase: one range
    let hit = one("a retry policy b", "\"retry policy\"");
    assert_eq!(hit.snippet_ranges, [(2, 14)]);
    // a camelCase word: the part that was asked for
    let hit = one("call retryPolicy now", "policy");
    assert_eq!(slice16(&hit.snippet, hit.snippet_ranges[0]), "Policy");
}

#[test]
fn a_hit_only_in_the_title_gives_the_start_of_the_body_and_title_ranges() {
    let long = format!("{FILLER} ").repeat(10);
    let c = with_docs(vec![doc_on("total", "a", "The Needle", &long)]);
    let hit = search(&c, "needle").docs.remove(0);
    assert_eq!(hit.title_ranges, [(4, 10)]);
    assert!(hit.snippet_ranges.is_empty());
    assert!(
        hit.snippet.starts_with("lorem ipsum dolor"),
        "{:?}",
        hit.snippet
    );
    assert!(hit.snippet.ends_with('…') && !hit.snippet.starts_with('…'));
    let inner = hit.snippet.trim_end_matches('…');
    assert!(inner.chars().count() <= 120 && inner.chars().count() >= 100);
    assert!(
        inner.split(' ').all(|w| FILLER.split(' ').any(|f| f == w)),
        "{inner:?}"
    );
    // a short body is all of it, with no ellipsis
    let c = with_docs(vec![doc_on("total", "a", "Needle", "plain body")]);
    let hit = search(&c, "needle").docs.remove(0);
    assert_eq!(
        (hit.snippet.as_str(), hit.title_ranges.as_slice()),
        ("plain body", &[(0, 6)][..])
    );
    // the title in UTF-16: an accent before the hit
    let c = with_docs(vec![doc_on("total", "a", "Café Needle 😀 needle", "x")]);
    let hit = search(&c, "needle").docs.remove(0);
    assert_eq!(hit.title_ranges, [(5, 11), (15, 21)]);
}

#[test]
fn an_empty_body_gives_an_empty_snippet() {
    let c = with_docs(vec![doc_on("total", "a", "Needle", "")]);
    let hit = search(&c, "needle").docs.remove(0);
    assert_eq!(hit.snippet, "");
    assert!(hit.snippet_ranges.is_empty());
    assert_eq!(hit.title_ranges, [(0, 6)]);
}

#[test]
fn one_word_longer_than_the_snippet_is_cut_where_the_snippet_ends() {
    let word = "x".repeat(300);
    let hit = one(&format!("{word} needle"), "needle");
    assert!(
        hit.snippet.chars().count() <= 122,
        "{}",
        hit.snippet.chars().count()
    );
}

// ── filters by the doc's node ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// Docs on a function in billing, a class in billing, a function in shop, a method, and two Missing nodes (a function that was in billing,
/// and one whose kind `node.toml` does not say, in shop). All hold `retry`.
fn filtered() -> Corpus {
    with_docs(vec![
        doc_on("total", "on-total", "", "retry in total"),
        doc_on("Invoice", "on-invoice", "", "retry in invoice"),
        doc_on("add_item", "on-add-item", "", "retry in add item"),
        doc_on("pay", "on-pay", "", "retry in pay"),
        doc_missing(
            "billing/old.py",
            Some(Kind::Function),
            "missing-fn",
            "",
            "retry in missing",
        ),
        doc_missing(
            "shop/gone.py",
            None,
            "missing-unknown",
            "",
            "retry in unknown",
        ),
    ])
}

#[test]
fn filters_narrow_docs_by_their_node() {
    let c = filtered();
    let rows: &[(&str, &[&str])] = &[
        (
            "retry",
            &[
                "on-total",
                "on-invoice",
                "on-pay",
                "on-add-item",
                "missing-fn",
                "missing-unknown",
            ],
        ),
        (
            "in:billing retry",
            &["on-total", "on-invoice", "on-pay", "missing-fn"],
        ),
        ("in:billing/old.py retry", &["missing-fn"]),
        ("in:bill retry", &[]),
        ("in:shop retry", &["on-add-item", "missing-unknown"]),
        ("-in:billing retry", &["on-add-item", "missing-unknown"]),
        ("kind:class retry", &["on-invoice"]),
        (
            "kind:fn retry",
            &["on-total", "on-pay", "on-add-item", "missing-fn"],
        ),
        ("kind:method retry", &["on-pay"]),
        (
            "kind:function -kind:method retry",
            &["on-total", "on-add-item", "missing-fn"],
        ),
        (
            "-kind:class retry",
            &[
                "on-total",
                "on-pay",
                "on-add-item",
                "missing-fn",
                "missing-unknown",
            ],
        ),
        (
            "kind:fn in:billing retry",
            &["on-total", "on-pay", "missing-fn"],
        ),
        (
            "has:doc retry",
            &[
                "on-total",
                "on-invoice",
                "on-pay",
                "on-add-item",
                "missing-fn",
                "missing-unknown",
            ],
        ),
        ("no:doc retry", &[]),
        ("-has:doc retry", &[]),
    ];
    for (query, want) in rows {
        assert_eq!(sorted(found(&c, query)), sorted(strings(want)), "{query}");
    }
}

#[test]
fn only_filters_and_a_value_that_means_nothing_give_no_docs() {
    let c = filtered();
    for query in [
        "kind:fn",
        "in:billing",
        "no:doc",
        "kind:fn in:billing has:doc",
        "kind:fnn retry",
        "has:doc no:doc retry",
        "has:code retry",
    ] {
        let results = search(&c, query);
        assert!(results.docs.is_empty(), "{query}");
        assert_eq!(results.docs_total, 0, "{query}");
    }
    assert!(
        !search(&c, "kind:fn").nodes.is_empty(),
        "the nodes of only filters stay"
    );
}

#[test]
fn a_missing_nodes_doc_says_where_it_was() {
    let c = filtered();
    let hit = &search(&c, "missing").docs[0];
    let entry = &c.docs()[hit.index];
    assert_eq!(
        (entry.node, entry.node_kind, entry.node_path.as_str()),
        (None, Some(Kind::Function), "billing/old.py")
    );
}

// ── order and limit ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn equal_scores_go_by_reading_order_then_missing_by_path_then_the_file() {
    // the same text everywhere, so the scores are equal; given in a scrambled order
    let text = "retry policy";
    let c = with_docs(vec![
        doc_missing("m/y.py", Some(Kind::Function), "missing-m", "", text),
        doc_on("add_item", "add-b", "", text),
        doc_on("total", "total-b", "", text),
        doc_missing("a/x.py", None, "missing-a", "", text),
        doc_on("add_item", "add-a", "", text),
        doc_on("total", "total-a", "", text),
        doc_on("billing", "billing-z", "", text),
    ]);
    assert_eq!(
        found(&c, "retry"),
        [
            "billing-z",
            "total-a",
            "total-b",
            "add-a",
            "add-b",
            "missing-a",
            "missing-m"
        ]
    );
    // a better score comes first, wherever its node is
    let c = with_docs(vec![
        doc_on("billing", "plain", "", "retry policy"),
        doc_missing("a/x.py", None, "better", "Retry", "x"),
    ]);
    assert_eq!(found(&c, "retry"), ["better", "plain"]);
}

#[test]
fn the_limit_cuts_the_rows_and_not_the_count() {
    let docs: Vec<DocEntry> = (0..5)
        .map(|i| doc_on("total", &format!("d{i}"), "", "retry policy"))
        .collect();
    let c = with_docs(docs);
    let results = run(
        &c,
        &parse("retry"),
        None,
        Limits {
            nodes: 200,
            docs: 2,
        },
    );
    let files: Vec<_> = results
        .docs
        .iter()
        .map(|h| c.docs()[h.index].file.as_str())
        .collect();
    assert_eq!(files, ["d0", "d1"]);
    assert_eq!(results.docs_total, 5);
    let none = run(
        &c,
        &parse("retry"),
        None,
        Limits {
            nodes: 200,
            docs: 0,
        },
    );
    assert!(none.docs.is_empty());
    assert_eq!(none.docs_total, 5);
}

#[test]
fn the_hits_say_where_their_doc_is_and_are_best_first() {
    let c = corpus_of(&[("a", "x retry"), ("b", "retry retry retry y"), ("c", "z")]);
    let hits = search(&c, "retry").docs;
    assert_eq!(hits.len(), 2);
    assert_eq!(c.docs()[hits[0].index].file, "b");
    assert!(hits[0].score >= hits[1].score);
}

// ── names unchanged ────────────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn docs_change_nothing_about_the_node_rows() {
    let plain = tree();
    let with = with_docs(vec![
        doc_on("total", "a", "Billing", "bill the billing"),
        doc_missing("billing/old.py", None, "b", "", "bill"),
    ]);
    for query in [
        "bill",
        "BILL",
        "billing/",
        "kind:fn",
        "in:billing",
        "bill in:shop",
        "kind:class",
        "inv",
        "zzz",
        "\"bill\"",
    ] {
        let (a, b) = (search(&plain, query), search(&with, query));
        assert_eq!(a.nodes, b.nodes, "{query}");
        assert_eq!(a.nodes_total, b.nodes_total, "{query}");
    }
    assert!(
        !search(&with, "bill").docs.is_empty(),
        "the docs are found as well"
    );
    // `exact` is as it was
    let results = run(
        &with,
        &parse("billing"),
        Some(at("shop")),
        Limits::default(),
    );
    assert!(results.nodes[0].exact && results.nodes[0].index == at("shop"));
    assert_eq!(
        results
            .nodes
            .iter()
            .filter(|n| n.index == at("shop"))
            .count(),
        1
    );
    assert!(!results.docs.is_empty());
}

#[test]
fn a_query_that_keeps_nothing_finds_no_docs() {
    let c = corpus_of(&[("a", "retry")]);
    assert!(search(&c, "kind:fnn retry").docs.is_empty());
    assert!(search(&c, "").docs.is_empty());
    assert!(search(&c, "   ").docs.is_empty());
    assert!(search(&c, "\"\"").docs.is_empty());
    assert!(search(&c, "/").docs.is_empty(), "no word in it");
}

// ── the doc index kept apart ───────────────────────────────────────────────────────────────────────────────────────────────────────

/// The same corpus built the long way and around a doc index built beforehand.
fn both(nodes: Vec<vnoc_search::NodeEntry>, docs: Vec<DocEntry>) -> (Corpus, Corpus) {
    let index = Arc::new(DocIndex::new(&docs));
    (
        Corpus::new(nodes.clone(), docs.clone()),
        Corpus::with_doc_index(nodes, docs, index),
    )
}

/// A doc hit as the comparison reads it: the file, the score, the snippet and its ranges.
type Row = (String, f64, String, Vec<(u32, u32)>);

/// 200 generated docs on the tree's nodes: a few words picked by a small deterministic generator, in titles and bodies, with line ends.
fn generated() -> Vec<DocEntry> {
    const WORDS: &[&str] = &[
        "retry",
        "policy",
        "alpha",
        "beta",
        "gamma",
        "billing",
        "retryPolicy",
        "café",
        "snake_case",
        "x2y",
        "Order",
        "service",
        "timeout",
        "backoff",
        "needle",
        "total",
    ];
    let names = [
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
        "billing_note",
    ];
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    (0..200)
        .map(|d| {
            let title: Vec<&str> = (0..1 + next(3)).map(|_| WORDS[next(WORDS.len())]).collect();
            let body: String = (0..20 + next(40))
                .map(|i| {
                    format!(
                        "{}{}",
                        WORDS[next(WORDS.len())],
                        if i % 7 == 6 { "\n" } else { " " }
                    )
                })
                .collect();
            doc_on(
                names[d % names.len()],
                &format!("d{d:03}.md"),
                &title.join(" "),
                &body,
            )
        })
        .collect()
}

const QUERIES: &[&str] = &[
    "retry",
    "ret",
    "retry policy",
    "retry pol",
    "\"retry policy\"",
    "\"alpha beta\" gamma",
    "in:billing retry",
    "kind:fn ret",
    "kind:class needle",
    "-in:shop alpha",
    "has:doc beta",
    "bill",
    "café",
    "x 2 y",
    "zzz",
    "retry zzz",
    "kind:fnn retry",
    "kind:fn",
    "",
];

#[test]
fn a_corpus_around_a_prebuilt_index_gives_the_same_results() {
    let mut corpora = vec![
        (tree_nodes(), filtered_docs()),
        (tree_nodes(), generated()),
        (tree_nodes(), Vec::new()),
    ];
    corpora.push((
        tree_nodes(),
        vec![
            doc_on("total", "a", "Retry", "The retryPolicy.\nretry policy"),
            doc_on("pay", "b", "", "unrelated"),
        ],
    ));
    for (nodes, docs) in corpora {
        let (fresh, kept) = both(nodes, docs);
        for query in QUERIES {
            assert_eq!(search(&fresh, query), search(&kept, query), "{query:?}");
        }
        let all = Limits {
            nodes: 1000,
            docs: 1000,
        };
        for query in QUERIES {
            assert_eq!(
                run(&fresh, &parse(query), None, all),
                run(&kept, &parse(query), None, all),
                "{query:?}"
            );
        }
    }
    // the generated corpus finds docs, so the comparison above is not of two empty answers
    let (fresh, _) = both(tree_nodes(), generated());
    assert!(search(&fresh, "retry").docs_total > 20);
    assert!(!search(&fresh, "\"retry policy\"").docs.is_empty());
}

fn filtered_docs() -> Vec<DocEntry> {
    vec![
        doc_on("total", "on-total", "", "retry in total"),
        doc_on("Invoice", "on-invoice", "", "retry in invoice"),
        doc_on("add_item", "on-add-item", "", "retry in add item"),
        doc_on("pay", "on-pay", "", "retry in pay"),
        doc_missing(
            "billing/old.py",
            Some(Kind::Function),
            "missing-fn",
            "",
            "retry in missing",
        ),
        doc_missing(
            "shop/gone.py",
            None,
            "missing-unknown",
            "",
            "retry in unknown",
        ),
    ]
}

#[test]
fn a_kept_index_serves_a_corpus_whose_nodes_are_different() {
    let docs = vec![
        doc_on("total", "a", "Retry", "retry policy of the billing"),
        doc_on("add_item", "b", "", "retry retry in the shop"),
        doc_missing(
            "billing/old.py",
            Some(Kind::Function),
            "c",
            "",
            "a retry that is missing",
        ),
    ];
    let kept = Arc::new(DocIndex::new(&docs));
    let first = Corpus::with_doc_index(tree_nodes(), docs.clone(), kept.clone());

    // the code changed: another tree, with the nodes in another order and fewer of them, and the same docs joined to where they are now
    let mut nodes = tree_nodes();
    nodes.reverse();
    nodes.retain(|n| n.name != "billing_note" && n.name != "rebill");
    let join = |doc: &DocEntry| {
        let mut doc = doc.clone();
        if let Some(at) = nodes.iter().position(|n| n.id == doc.node_id) {
            doc.node = Some(at);
        } else if doc.node.is_some() {
            doc.node = None;
        }
        doc
    };
    let moved: Vec<DocEntry> = docs.iter().map(join).collect();
    let second = Corpus::with_doc_index(nodes, moved, kept.clone());

    for query in ["retry", "ret", "\"retry policy\"", "billing"] {
        let (a, b) = (search(&first, query), search(&second, query));
        let key = |c: &Corpus, r: &vnoc_search::Results| -> Vec<Row> {
            let mut rows: Vec<_> = r
                .docs
                .iter()
                .map(|h| {
                    (
                        c.docs()[h.index].file.clone(),
                        h.score,
                        h.snippet.clone(),
                        h.snippet_ranges.clone(),
                    )
                })
                .collect();
            rows.sort_by(|x, y| x.0.cmp(&y.0));
            rows
        };
        assert_eq!(
            key(&first, &a),
            key(&second, &b),
            "{query}: the same docs, the same scores and snippets"
        );
        assert_eq!(a.docs_total, b.docs_total, "{query}");
    }
    // only their node fields differ: the doc of `total` is at another place in the nodes now
    let place = |c: &Corpus| c.docs()[0].node;
    assert_ne!(place(&first), place(&second));
    assert_eq!(second.docs()[0].body, first.docs()[0].body);
    // and the filters read the new nodes
    assert_eq!(
        found(&second, "in:billing retry").len(),
        2,
        "the doc of `total` and the Missing one"
    );
    assert_eq!(found(&second, "in:shop retry"), ["b"]);
    // the index itself was kept, not rebuilt: both corpora share it
    assert_eq!(Arc::strong_count(&kept), 3);
}

/// A debug build refuses a list of docs the index was not built from.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "the doc index was built from another list of docs")]
fn another_number_of_docs_is_a_bug_a_debug_build_says_so() {
    let docs = vec![doc_on("total", "a", "", "retry")];
    let index = Arc::new(DocIndex::new(&docs));
    Corpus::with_doc_index(tree_nodes(), Vec::new(), index);
}

/// A release build builds a fresh index and gives what `Corpus::new` gives, and never reads past the end of the old one. (The fallback itself is
/// also tested in every build, through the function `with_doc_index` calls: `an_index_for_another_number_of_docs_is_replaced_not_used`, in `lib.rs`.)
#[cfg(not(debug_assertions))]
#[test]
fn another_number_of_docs_gives_a_fresh_index_in_a_release_build() {
    let few = vec![doc_on("total", "a", "", "retry")];
    let many = generated();
    for (built_from, given) in [
        (few.clone(), many.clone()),
        (many.clone(), few.clone()),
        (many.clone(), Vec::new()),
    ] {
        let stale = Arc::new(DocIndex::new(&built_from));
        let got = Corpus::with_doc_index(tree_nodes(), given.clone(), stale);
        let want = Corpus::new(tree_nodes(), given);
        for query in QUERIES {
            assert_eq!(search(&got, query), search(&want, query), "{query:?}");
        }
    }
}
