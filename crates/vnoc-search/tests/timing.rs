//! A timing, by hand: `cargo test -p vnoc-search --release -- --ignored --nocapture` (04 §2, README §5 row 4).

mod common;

use std::time::{Duration, Instant};

use common::*;
use std::sync::Arc;

use vnoc_search::{Corpus, DocEntry, DocIndex, Kind, Limits, NodeEntry, parse, run};

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

/// A word-like vocabulary, a few common words and many rare ones: the `i`th is a few syllables, and a pick leans toward the small `i`.
struct Text {
    state: u64,
    vocabulary: Vec<String>,
}

impl Text {
    fn new() -> Text {
        const SYLLABLES: &[&str] = &[
            "ba", "re", "ti", "so", "lu", "ma", "ne", "ki", "po", "ra", "de", "vi", "mo", "ta",
            "se", "lo", "gu", "fe", "no", "chi",
        ];
        let vocabulary = (0..4_000)
            .map(|i: usize| {
                let (a, b, c) = (i % 20, (i / 20) % 20, i / 400);
                let mut w = format!("{}{}", SYLLABLES[a], SYLLABLES[b]);
                if c > 0 {
                    w.push_str(SYLLABLES[c % 20]);
                }
                w
            })
            .collect();
        Text {
            state: 0x9E37_79B9_7F4A_7C15,
            vocabulary,
        }
    }

    fn next(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state >> 11) as f64 / (1u64 << 53) as f64
    }

    /// About `bytes` bytes of words, in lines of about ten, with the odd capital.
    fn text(&mut self, bytes: usize) -> String {
        let mut out = String::new();
        let mut on_line = 0;
        while out.len() < bytes {
            let r = self.next();
            let word = self.vocabulary[(self.vocabulary.len() as f64 * r * r * r) as usize].clone();
            if self.next() < 0.1 {
                out.push_str(&word[..1].to_uppercase());
                out.push_str(&word[1..]);
            } else {
                out.push_str(&word);
            }
            on_line += 1;
            out.push(if on_line == 10 { '\n' } else { ' ' });
            if on_line == 10 {
                on_line = 0;
            }
        }
        out
    }
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

#[test]
#[ignore = "a timing, run by hand"]
fn a_thousand_docs_of_two_kb_on_ten_thousand_names() {
    let names = names(9_900);
    let mut nodes: Vec<NodeEntry> = Vec::new();
    for (f, chunk) in names.chunks(99).enumerate() {
        let path = format!("pkg{}/module_{f}.py", f % 10);
        nodes.push(file(&path));
        nodes.extend(chunk.iter().map(|n| def(Kind::Function, &path, n)));
    }
    let mut text = Text::new();
    let docs: Vec<DocEntry> = (0..1_000)
        .map(|d| {
            let at = (d * 9) % nodes.len();
            let node = &nodes[at];
            DocEntry {
                node: Some(at),
                node_id: node.id.clone(),
                node_name: node.name.clone(),
                node_kind: Some(node.kind),
                node_path: node.path.clone(),
                file: format!("{d}.md"),
                doc_path: format!(".vnoc/nodes/{d}/{d}.md"),
                title: text.text(30),
                body: text.text(2_000),
            }
        })
        .collect();
    let bytes: usize = docs.iter().map(|d| d.title.len() + d.body.len()).sum();

    // each build is timed on its own clone, which is not part of it: the first of six, and the median of the other five
    let six = |mut build: Box<dyn FnMut() -> Duration + '_>| -> (Duration, Duration) {
        let times: Vec<Duration> = (0..6).map(|_| build()).collect();
        (times[0], median(times[1..].to_vec()))
    };
    let (first, rest) = six(Box::new(|| {
        let nodes = nodes.clone();
        let start = Instant::now();
        let corpus = Corpus::new(nodes, Vec::new());
        let took = start.elapsed();
        drop(corpus);
        took
    }));
    println!("Corpus::new, 10,000 names, no docs: {first:?} first, {rest:?} median of 5 more");
    let (first, rest) = six(Box::new(|| {
        let start = Instant::now();
        let index = DocIndex::new(&docs);
        let took = start.elapsed();
        assert_eq!(index.len(), 1_000);
        took
    }));
    println!(
        "DocIndex::new, 1,000 docs ({} KB of text): {first:?} first, {rest:?} median of 5 more   (S6a: Corpus::new with the docs took 88 ms first, 74 ms median)",
        bytes / 1024
    );
    let kept = Arc::new(DocIndex::new(&docs));
    let (first, rest) = six(Box::new(|| {
        let (nodes, docs) = (nodes.clone(), docs.clone());
        let start = Instant::now();
        let corpus = Corpus::with_doc_index(nodes, docs, kept.clone());
        let took = start.elapsed();
        drop(corpus);
        took
    }));
    println!(
        "Corpus::with_doc_index, 10,000 names, a kept index: {first:?} first, {rest:?} median of 5 more"
    );
    let corpus = Corpus::with_doc_index(nodes.clone(), docs.clone(), kept.clone());

    // the commonest word of the vocabulary, and a two-letter prefix that opens many words
    for (label, query) in [
        ("a common word", text.vocabulary[0].clone()),
        ("a two-letter prefix", "ba".to_string()),
    ] {
        let query = parse(&query);
        let start = Instant::now();
        let found = run(&corpus, &query, None, Limits::default());
        let first = start.elapsed();
        let rest: Vec<_> = (0..5)
            .map(|_| {
                let start = Instant::now();
                run(&corpus, &query, None, Limits::default());
                start.elapsed()
            })
            .collect();
        println!(
            "one run for {label} `{}`: {first:?} first, {:?} median of 5 more ({} docs found, {} shown; {} nodes found)",
            query.words[0],
            median(rest),
            found.docs_total,
            found.docs.len(),
            found.nodes_total
        );
    }
}
