//! The core's side of search (`docs/20-search/04-core.md` §2): the corpus `vnoc-search` reads, made from the tree `vnoc/structure` sends, and kept
//! until the index changes.
//!
//! The crate knows no index; this file maps one to the other and back. [`corpus_of`] walks the tree in its own order (the reading order),
//! [`kept`] holds the last corpus under the revision it was made for, and [`answer`] turns the crate's hits into the protocol's rows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use vnoc_domain::{NodeId, NodeKind, NodeToml};
use vnoc_search::{Corpus, DocEntry, DocIndex, NodeEntry, Query, Results, parse};

use crate::index::{self, Index, Kind, Node};
use crate::node_ref::{self, Found, NodeRef};
use crate::project::Project;
use crate::protocol::{SearchDoc, SearchNode, SearchResult};
use crate::userdata::DocTexts;

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

/// Where a node that is not in the tree is, as its `node.toml` says: what its docs show in a result row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// the repo path
    pub path: String,
    pub qualname: String,
    pub kind: Option<NodeKind>,
}

/// The anchor a `node.toml` holds; `None` when it holds none that reads (`NodeToml::parse` checks, so it is not expected).
pub fn anchor_of(toml: &NodeToml) -> Option<Anchor> {
    let place = toml.location().ok()?;
    Some(Anchor {
        path: place.path().to_string(),
        qualname: place.qualname().to_string(),
        kind: toml.kind(),
    })
}

/// The corpus of the tree `Index::tree` gives (see [`entries_of`]) with the docs of `texts` (03 §5), in the texts' own order.
///
/// A doc joins its node through the permanent ID: the tree's nodes that have one are put in a map once. A doc whose ID is not in the tree has no
/// node (`node: None`) and shows what `anchor` says of it: the node is Missing, or it is not in the scan (its code is outside the opened folder,
/// ignored, or past the cap), and the answer cannot tell the two apart. `prefix` is the opened folder's place in the repo (as for `Index::tree`).
///
/// The doc index is kept in `doc_slot` under the texts' generation (03 §9): a change of the code builds the names again and joins the kept doc
/// index to the new nodes. Taken inside the corpus's own lock, never the other way round.
pub fn corpus_of(
    root: &index::Node,
    texts: &DocTexts,
    doc_slot: &Mutex<Option<(u64, Arc<DocIndex>)>>,
    prefix: &str,
    anchor: impl Fn(&NodeId) -> Option<Anchor>,
    is_missing: impl Fn(&NodeId) -> bool,
) -> Corpus {
    let (nodes, by_node_id) = entries_of(root);
    let docs: Vec<DocEntry> = texts
        .iter()
        .map(|(id, file, text)| {
            let node_id = id.to_string();
            let doc = |node, name, kind, path| DocEntry {
                node,
                missing: node.is_none() && is_missing(id),
                node_id: node_id.clone(),
                node_name: name,
                node_kind: kind,
                node_path: path,
                file: file.to_string(),
                doc_path: text.path.to_string_lossy().into_owned(),
                title: text.title.clone(),
                body: text.body.clone(),
            };
            if let Some(&at) = by_node_id.get(node_id.as_str()) {
                let n = &nodes[at];
                return doc(Some(at), n.name.clone(), Some(n.kind), n.path.clone());
            }
            match anchor(id) {
                Some(a) => {
                    let kind = a.kind.as_ref().and_then(kind_of_stored);
                    doc(None, name_of(&a), kind, shown_path(&a.path, prefix))
                }
                None => doc(None, String::new(), None, String::new()),
            }
        })
        .collect();
    let doc_index = kept(doc_slot, texts.generation(), || DocIndex::new(&docs));
    Corpus::with_doc_index(nodes, docs, doc_index)
}

fn kind_of_stored(kind: &NodeKind) -> Option<vnoc_search::Kind> {
    match kind {
        NodeKind::Function => Some(vnoc_search::Kind::Function),
        NodeKind::Class => Some(vnoc_search::Kind::Class),
        NodeKind::File => Some(vnoc_search::Kind::File),
        NodeKind::Folder => Some(vnoc_search::Kind::Folder),
        NodeKind::Other(_) => None,
    }
}

/// The last part of the qualified name (`Outer.method` is `method`); for a file or a folder, which have none, the last part of the path.
fn name_of(anchor: &Anchor) -> String {
    let (whole, separator) = if anchor.qualname.is_empty() {
        (anchor.path.as_str(), '/')
    } else {
        (anchor.qualname.as_str(), '.')
    };
    whole.rsplit(separator).next().unwrap_or(whole).to_string()
}

/// A repo path as the tree shows it: relative to the opened folder; the repo path itself when it is outside it.
fn shown_path(repo_path: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return repo_path.to_string();
    }
    repo_path
        .strip_prefix(prefix)
        .and_then(|p| p.strip_prefix('/'))
        .unwrap_or(repo_path)
        .to_string()
}

/// Every node under `root`, depth-first in the tree's order, without the root itself; and where each one that has a permanent ID is.
/// A function whose parent is a class is a method (`in_class`); a function inside a function is not.
fn entries_of(root: &index::Node) -> (Vec<NodeEntry>, HashMap<&str, usize>) {
    fn walk<'a>(
        node: &'a Node,
        parent: Option<Kind>,
        out: &mut Vec<NodeEntry>,
        by_node_id: &mut HashMap<&'a str, usize>,
    ) {
        for child in &node.children {
            if let Some(id) = &child.node_id {
                by_node_id.insert(id.as_str(), out.len());
            }
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
            walk(child, Some(child.kind), out, by_node_id);
        }
    }
    let mut nodes = Vec::new();
    let mut by_node_id = HashMap::new();
    walk(root, None, &mut nodes, &mut by_node_id);
    (nodes, by_node_id)
}

/// What is kept in `slot` when it was made for `key` (the corpus under `(revision, generation)`, the doc index under the generation); otherwise
/// `build` makes one, which is kept (instead of the old one) and returned. The lock is held while building, so two searches at once build once.
pub fn kept<K: PartialEq, T>(
    slot: &Mutex<Option<(K, Arc<T>)>>,
    key: K,
    build: impl FnOnce() -> T,
) -> Arc<T> {
    let mut slot = slot.lock().unwrap();
    match slot.as_ref() {
        Some((k, held)) if *k == key => held.clone(),
        _ => {
            let made = Arc::new(build());
            *slot = Some((key, made.clone()));
            made
        }
    }
}

/// The kept corpus with what it was made for: the index revision and the doc-text generation.
pub type CorpusSlot = Mutex<Option<((u64, u64), Arc<Corpus>)>>;

/// What both `vnoc/search` and `vnoc/searchCards` start from: the query read, the corpus for the index as it is, and the node a pasted reference names.
pub struct Prepared {
    pub revision: u64,
    pub corpus: Arc<Corpus>,
    pub query: Query,
    /// the place in the corpus's nodes of the node the reference named, when it named one the tree has
    pub exact: Option<usize>,
}

/// Parses `text`, resolves a reference in it, and gets the corpus (kept under `(revision, generation)`, built on the first call after a change).
/// The one place the two requests share, so they cannot disagree about what a query means. Synchronous: it holds locks (the index first, then the
/// corpus slot, then the doc slot inside it), so nothing here awaits. The caller has waited for the first build.
pub fn prepare(
    project: &Project,
    index: &RwLock<Index>,
    corpus_slot: &CorpusSlot,
    doc_slot: &Mutex<Option<(u64, Arc<DocIndex>)>>,
    text: &str,
) -> Prepared {
    let query = parse(text);
    // a bare path is found by the path tiers, so only the forms they cannot find are tried (04 §4)
    let reference = query
        .reference
        .as_deref()
        .and_then(node_ref::parse)
        .filter(|r| !matches!(r, NodeRef::Path(_)));
    let index = index.read().unwrap();
    let revision = index.revision();
    let texts = index.doc_texts();
    let corpus = kept(corpus_slot, (revision, texts.generation()), || {
        corpus_of(
            &index.tree(&project.name(), project.prefix()),
            &texts,
            doc_slot,
            project.prefix(),
            |id| index.record_toml(id).and_then(anchor_of),
            |id| index.is_missing(id),
        )
    });
    // a node the tree has first; a Missing ID or a reference that names nothing leaves the words to find what they find
    let exact = match reference.map(|r| index.resolve_ref(project, &r)) {
        Some(Found::Node(id)) => corpus.position(&id),
        _ => None,
    };
    Prepared {
        revision,
        corpus,
        query,
        exact,
    }
}

/// The crate's results as the protocol's answer, with the notes `query` gave about its filters. A doc with no node in the tree (`node: null`) is
/// sent as `missing`.
pub fn answer(revision: u64, corpus: &Corpus, query: &Query, results: Results) -> SearchResult {
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
    let docs = results
        .docs
        .into_iter()
        .map(|hit| {
            let doc = &corpus.docs()[hit.index];
            SearchDoc {
                node: doc.node.map(|at| corpus.nodes()[at].id.clone()),
                node_id: doc.node_id.clone(),
                node_name: doc.node_name.clone(),
                node_kind: doc.node_kind.map(kind_out),
                node_path: doc.node_path.clone(),
                missing: doc.missing,
                file: doc.file.clone(),
                doc_path: doc.doc_path.clone(),
                title: doc.title.clone(),
                title_match: hit.title_ranges,
                snippet: hit.snippet,
                snippet_match: hit.snippet_ranges,
            }
        })
        .collect();
    SearchResult {
        revision,
        nodes,
        nodes_total: results.nodes_total,
        docs,
        docs_total: results.docs_total,
        notes: query.notes.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::userdata::UserData;
    use std::collections::BTreeMap;
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
        let corpus = corpus_of(
            &tree,
            &DocTexts::default(),
            &Mutex::new(None),
            "",
            |_| None,
            |_| false,
        );
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

    /// A temp git root with a `.vnoc/`, a node folder for each of `nodes` (its anchor, `kind`) holding one doc with this text; the doc texts read from it.
    fn texts_of(
        label: &str,
        nodes: &[(NodeId, &str, &str, &str, &str)],
    ) -> (DocTexts, std::path::PathBuf) {
        let git_root =
            std::env::temp_dir().join(format!("vnoc-search-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&git_root);
        let data = UserData::new(&git_root);
        for (id, path, qualname, kind, text) in nodes {
            let toml = NodeToml::parse(&format!(
                "path = \"{path}\"\nqualname = \"{qualname}\"\nkind = \"{kind}\"\n"
            ))
            .unwrap();
            data.create_node(id, &toml).unwrap();
            let doc = data.create_doc(id, "A doc", "hint").unwrap();
            std::fs::write(doc, text).unwrap();
        }
        let records = data.load_all().records;
        (
            data.refresh_doc_texts(&DocTexts::default(), &records).0,
            git_root,
        )
    }

    #[test]
    fn a_doc_joins_its_tree_node_and_a_node_that_is_not_in_the_tree_shows_its_node_toml() {
        let (in_tree, missing, unknown) = (
            NodeId::parse("0000000000000000000000000A").unwrap(),
            NodeId::parse("0000000000000000000000000B").unwrap(),
            NodeId::parse("0000000000000000000000000C").unwrap(),
        );
        let (texts, git_root) = texts_of(
            "join",
            &[
                (
                    in_tree,
                    "pkg/a.py",
                    "Outer",
                    "class",
                    "# On Outer\nfound zanzibar",
                ),
                (
                    missing,
                    "gone.py",
                    "Old.run",
                    "function",
                    "# Lost\nserengeti",
                ),
                (unknown, "pkg/b.py", "", "file", "# Odd\nkilimanjaro"),
            ],
        );
        let tree = index::tree::build(
            "repo",
            "",
            &files(vec![
                ("pkg/a.py", vec![def("Outer", "", DefKind::Class, 3)]),
                ("pkg/c.py", vec![def("other", "", DefKind::Function, 0)]),
            ]),
            &HashMap::from([(
                "pkg/a.py::Outer".to_string(),
                index::Attached {
                    id: in_tree,
                    docs: 1,
                    by: crate::identity::FoundBy::Anchor,
                },
            )]),
        );
        let anchors = |id: &NodeId| {
            let toml = NodeToml::parse(&match id.to_string().as_str() {
                x if x == missing.to_string() => {
                    "path = \"gone.py\"\nqualname = \"Old.run\"\nkind = \"function\"\n".to_string()
                }
                _ => "path = \"pkg/b.py\"\nqualname = \"\"\nkind = \"file\"\n".to_string(),
            })
            .unwrap();
            anchor_of(&toml)
        };
        let slot = Mutex::new(None);
        let corpus = corpus_of(&tree, &texts, &slot, "", anchors, |id| *id == missing);

        let docs: Vec<_> = corpus
            .docs()
            .iter()
            .map(|d| {
                (
                    d.node.map(|n| corpus.nodes()[n].id.as_str()),
                    d.node_name.as_str(),
                    d.node_kind,
                    d.node_path.as_str(),
                    d.title.as_str(),
                    d.body.as_str(),
                )
            })
            .collect();
        use vnoc_search::Kind as K;
        assert_eq!(
            docs,
            [
                (
                    Some("pkg/a.py::Outer"),
                    "Outer",
                    Some(K::Class),
                    "pkg/a.py",
                    "On Outer",
                    "found zanzibar"
                ),
                (
                    None,
                    "run",
                    Some(K::Function),
                    "gone.py",
                    "Lost",
                    "serengeti"
                ),
                (
                    None,
                    "b.py",
                    Some(K::File),
                    "pkg/b.py",
                    "Odd",
                    "kilimanjaro"
                ),
            ],
            "the node in the tree by its permanent ID; the others by what their node.toml says"
        );
        assert!(
            corpus
                .docs()
                .iter()
                .all(|d| std::path::Path::new(&d.doc_path).is_absolute()
                    && d.doc_path.starts_with(git_root.to_str().unwrap()))
        );
        assert_eq!(corpus.docs()[1].node_id, missing.to_string());
        assert_eq!(
            corpus.docs().iter().map(|d| d.missing).collect::<Vec<_>>(),
            [false, true, false],
            "Missing only for the node the index lists as Missing, not for any node without a tree place"
        );

        // the doc index is kept under the generation: a rebuilt corpus joins the same one
        let first = slot.lock().unwrap().as_ref().unwrap().1.clone();
        corpus_of(&tree, &texts, &slot, "", |_| None, |_| false);
        let (generation, second) = slot.lock().unwrap().as_ref().unwrap().clone();
        assert_eq!(generation, texts.generation());
        assert!(Arc::ptr_eq(&first, &second));

        // a node whose `node.toml` cannot be read has no fields to show, and is still found by its words
        let bare = corpus_of(&tree, &texts, &Mutex::new(None), "", |_| None, |_| false);
        assert_eq!(bare.docs()[1].node_path, "");
        assert!(bare.docs()[1].node_kind.is_none());
    }

    #[test]
    fn a_stored_path_is_shown_below_the_opened_folder_or_as_it_is() {
        assert_eq!(shown_path("services/api/x.py", "services/api"), "x.py");
        assert_eq!(
            shown_path("services/other.py", "services/api"),
            "services/other.py"
        );
        assert_eq!(
            shown_path("services/apix/y.py", "services/api"),
            "services/apix/y.py"
        );
        assert_eq!(shown_path("x.py", ""), "x.py");
        let name = |path: &str, qualname: &str| {
            name_of(&Anchor {
                path: path.into(),
                qualname: qualname.into(),
                kind: None,
            })
        };
        assert_eq!(name("a/b.py", "Outer.method"), "method");
        assert_eq!(name("a/b.py", "run.<locals>.step"), "step");
        assert_eq!(name("a/b.py", ""), "b.py");
        assert_eq!(name("a/dir", ""), "dir");
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
            let entries = entries_of(&tree).0;
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
