//! The core's side of search (`docs/20-search/04-core.md` §2): the corpus `vnoc-search` reads, made from the tree `vnoc/structure` sends, and kept
//! until the index changes.
//!
//! The crate knows no index; this file maps one to the other and back. [`corpus_of`] walks the tree in its own order (the reading order),
//! [`kept`] holds the last corpus under the revision it was made for, and [`answer`] turns the crate's hits into the protocol's rows.

use std::sync::{Arc, Mutex};

use vnoc_search::{Corpus, NodeEntry, Results};

use crate::index::{self, Kind, Node};
use crate::protocol::{SearchNode, SearchResult};

fn kind_in(kind: Kind) -> vnoc_search::Kind {
    match kind {
        Kind::Folder => vnoc_search::Kind::Folder,
        Kind::File => vnoc_search::Kind::File,
        Kind::Class => vnoc_search::Kind::Class,
        Kind::Function => vnoc_search::Kind::Function,
    }
}

fn kind_out(kind: vnoc_search::Kind) -> Kind {
    match kind {
        vnoc_search::Kind::Folder => Kind::Folder,
        vnoc_search::Kind::File => Kind::File,
        vnoc_search::Kind::Class => Kind::Class,
        vnoc_search::Kind::Function => Kind::Function,
    }
}

/// The corpus of the tree `Index::tree` gives (see [`entries_of`]). No doc texts yet.
pub fn corpus_of(root: &index::Node) -> Corpus {
    Corpus::new(entries_of(root), Vec::new())
}

/// Every node under `root`, depth-first in the tree's order, without the root itself.
/// A function whose parent is a class is a method (`in_class`); a function inside a function is not.
fn entries_of(root: &index::Node) -> Vec<NodeEntry> {
    fn walk(node: &Node, parent: Option<Kind>, out: &mut Vec<NodeEntry>) {
        for child in &node.children {
            out.push(NodeEntry {
                id: child.id.clone(),
                kind: kind_in(child.kind),
                name: child.name.clone(),
                qualname: child.qualname.clone(),
                path: child.path.clone(),
                line: child.range.map(|r| r.start.line),
                in_class: child.kind == Kind::Function && parent == Some(Kind::Class),
                docs: child.docs,
            });
            walk(child, Some(child.kind), out);
        }
    }
    let mut nodes = Vec::new();
    walk(root, None, &mut nodes);
    nodes
}

/// The corpus kept in `slot` when it was made for `key`; otherwise `build` makes one, which is kept (instead of the old one) and returned.
/// The lock is held while building, so two searches at once build once.
pub fn kept(
    slot: &Mutex<Option<(u64, Arc<Corpus>)>>,
    key: u64,
    build: impl FnOnce() -> Corpus,
) -> Arc<Corpus> {
    let mut slot = slot.lock().unwrap();
    match slot.as_ref() {
        Some((k, corpus)) if *k == key => corpus.clone(),
        _ => {
            let corpus = Arc::new(build());
            *slot = Some((key, corpus.clone()));
            corpus
        }
    }
}

/// The crate's results as the protocol's answer. No doc is searched yet, so `docs` is empty and so are `notes`.
pub fn answer(revision: u64, corpus: &Corpus, results: Results) -> SearchResult {
    let nodes = results
        .nodes
        .into_iter()
        .map(|hit| {
            let node = &corpus.nodes()[hit.index];
            SearchNode {
                id: node.id.clone(),
                kind: kind_out(node.kind),
                name: node.name.clone(),
                path: node.path.clone(),
                line: node.line,
                in_class: node.in_class,
                docs: node.docs,
                tier: hit.tier,
                exact: hit.exact,
                ranges: hit.ranges,
            }
        })
        .collect();
    SearchResult {
        revision,
        nodes,
        nodes_total: results.nodes_total,
        docs: Vec::new(),
        docs_total: results.docs_total,
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use vnoc_domain::RepoPath;
    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    fn def(qualname: &str, parent: &str, kind: DefKind, line: u32) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            qualname: qualname.into(),
            name: qualname.rsplit('.').next().unwrap().into(),
            kind,
            parent: parent.into(),
            parent_kind: None,
            range: Range {
                start: at(line),
                end: at(line + 1),
            },
            params: vec![],
            id: None,
            syntax_error: false,
        }
    }

    fn files(list: Vec<(&str, Vec<Def>)>) -> BTreeMap<RepoPath, Vec<Def>> {
        list.into_iter()
            .map(|(p, d)| (RepoPath::new(p).unwrap(), d))
            .collect()
    }

    #[test]
    fn the_corpus_is_the_tree_in_reading_order_without_the_root() {
        let tree = index::tree::build(
            "repo",
            "",
            &files(vec![
                (
                    "pkg/orders.py",
                    vec![
                        def("OrderService", "", DefKind::Class, 3),
                        def("OrderService.parse", "OrderService", DefKind::Function, 4),
                        def("run", "", DefKind::Function, 10),
                        def("run.<locals>.step", "run", DefKind::Function, 11),
                    ],
                ),
                ("pkg/util.py", vec![def("helper", "", DefKind::Function, 0)]),
                ("top.py", vec![def("main", "", DefKind::Function, 7)]),
            ]),
            &HashMap::new(),
        );
        let corpus = corpus_of(&tree);
        let rows: Vec<_> = corpus
            .nodes()
            .iter()
            .map(|n| (n.id.as_str(), n.in_class, n.line))
            .collect();
        assert_eq!(
            rows,
            [
                ("pkg", false, None),
                ("pkg/orders.py", false, None),
                ("pkg/orders.py::OrderService", false, Some(3)),
                ("pkg/orders.py::OrderService.parse", true, Some(4)),
                ("pkg/orders.py::run", false, Some(10)),
                ("pkg/orders.py::run.<locals>.step", false, Some(11)),
                ("pkg/util.py", false, None),
                ("pkg/util.py::helper", false, Some(0)),
                ("top.py", false, None),
                ("top.py::main", false, Some(7)),
            ]
        );
        let method = &corpus.nodes()[3];
        assert_eq!(method.name, "parse");
        assert_eq!(method.qualname.as_deref(), Some("OrderService.parse"));
        assert_eq!(method.path, "pkg/orders.py");
    }

    #[test]
    fn a_corpus_is_kept_for_its_key_and_only_one_is_kept() {
        let slot = Mutex::new(None);
        let calls = AtomicUsize::new(0);
        let build = || {
            calls.fetch_add(1, Ordering::SeqCst);
            Corpus::new(Vec::new(), Vec::new())
        };
        let a = kept(&slot, 1, build);
        let b = kept(&slot, 1, build);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&a, &b));
        kept(&slot, 2, build);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        kept(&slot, 1, build);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "key 2 replaced key 1: one corpus is kept"
        );
    }

    /// By hand: `cargo test -p vnoc --release --lib -- --ignored --nocapture search`. 04 §2's measurement.
    #[test]
    #[ignore = "a timing, run by hand"]
    fn timing_10000_defs() {
        let list: Vec<(String, Vec<Def>)> = (0..100)
            .map(|f| {
                let defs = (0..100)
                    .map(|d| def(&format!("handler_{f}_{d}"), "", DefKind::Function, d))
                    .collect();
                (format!("dir{}/module_{f}.py", f % 10), defs)
            })
            .collect();
        let files: BTreeMap<RepoPath, Vec<Def>> = list
            .into_iter()
            .map(|(p, d)| (RepoPath::new(p).unwrap(), d))
            .collect();

        // six builds in one process: the first, and the median of the other five
        let mut runs: Vec<[Duration; 3]> = Vec::new();
        let mut nodes = 0;
        for _ in 0..6 {
            let started = Instant::now();
            let tree = index::tree::build("repo", "", &files, &HashMap::new());
            let built = started.elapsed();

            let started = Instant::now();
            let entries = entries_of(&tree);
            let walked = started.elapsed();

            let started = Instant::now();
            let corpus = Corpus::new(entries, Vec::new());
            let new = started.elapsed();

            nodes = corpus.nodes().len();
            runs.push([built, walked, new]);
        }
        let total = |run: &[Duration; 3]| run.iter().sum::<Duration>();
        let median = |part: &dyn Fn(&[Duration; 3]) -> Duration| {
            let mut rest: Vec<Duration> = runs[1..].iter().map(part).collect();
            rest.sort();
            rest[2]
        };
        for (what, part) in [
            (
                "tree::build",
                &(|r: &[Duration; 3]| r[0]) as &dyn Fn(&[Duration; 3]) -> Duration,
            ),
            ("corpus_of's walk", &|r| r[1]),
            ("Corpus::new", &|r| r[2]),
            ("total", &total),
        ] {
            println!(
                "{nodes} nodes, {what}: {:?} first, {:?} median of 5 more",
                part(&runs[0]),
                median(part)
            );
        }
    }
}
