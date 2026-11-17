//! A timing, by hand: `cargo test -p vnoc-search --release -- --ignored --nocapture` (04 §2, README §5 row 4).

mod common;

use std::time::{Duration, Instant};

use common::*;
use vnoc_search::{Corpus, Kind, Limits, NodeEntry, parse, run};

const WORDS: &[&str] = &[
    "checkout", "order", "parse", "billing", "invoice", "total", "item", "cart", "shop", "user",
    "account", "session", "config", "path", "node", "index", "render", "update", "handle",
    "request",
];

/// A name of two to four words, in snake_case or camelCase, picked by a small deterministic generator.
fn names(count: usize) -> Vec<String> {
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    (0..count)
        .map(|_| {
            let parts: Vec<&str> = (0..2 + next(3)).map(|_| WORDS[next(WORDS.len())]).collect();
            if next(2) == 0 {
                parts.join("_")
            } else {
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if i == 0 {
                            p.to_string()
                        } else {
                            format!("{}{}", p[..1].to_uppercase(), &p[1..])
                        }
                    })
                    .collect()
            }
        })
        .collect()
}

#[test]
#[ignore = "a timing, run by hand"]
fn ten_thousand_names_in_a_hundred_files() {
    let names = names(9_900);
    let mut nodes: Vec<NodeEntry> = Vec::new();
    for (f, chunk) in names.chunks(99).enumerate() {
        let path = format!("pkg{}/module_{f}.py", f % 10);
        nodes.push(file(&path));
        nodes.extend(chunk.iter().map(|n| def(Kind::Function, &path, n)));
    }
    assert_eq!(nodes.len(), 10_000);

    // six builds in one process: the first, and the median of the other five (the clone is not timed)
    let mut builds = Vec::new();
    let mut corpus = None;
    for _ in 0..6 {
        let nodes = nodes.clone();
        let start = Instant::now();
        corpus = Some(Corpus::new(nodes, Vec::new()));
        builds.push(start.elapsed());
    }
    let corpus = corpus.unwrap();

    let query = parse("co");
    let start = Instant::now();
    let found = run(&corpus, &query, None, Limits::default());
    let first = start.elapsed();

    let mut runs: Vec<_> = (0..20)
        .map(|_| {
            let start = Instant::now();
            run(&corpus, &query, None, Limits::default());
            start.elapsed()
        })
        .collect();
    runs.sort();

    let mut rest = builds[1..].to_vec();
    rest.sort();
    let median: Duration = rest[2];
    println!(
        "Corpus::new, 10,000 names in 100 files: {:?} first, {median:?} median of 5 more (all: {builds:?})",
        builds[0]
    );
    println!(
        "one run for `co`: {first:?} first, {:?} median of 20 more ({} found, {} shown)",
        runs[10],
        found.nodes_total,
        found.nodes.len()
    );
}
