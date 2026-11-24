//! Finding a stop's node again (`docs/26-tours/03-core.md` §2, `01-on-disk.md` §6): by the ID the stop kept, then by its exact name.
//!
//! This is the one function a later module replaces (D6), and nothing else of `tours/` asks the index where a stop's node is. It is exact on
//! purpose: an ID, or the same path, qualname and occurrence. A node that moved with no ID, or was renamed, is not found, and the tour says so.

use vnoc_domain::{LocationKey, RepoPath};

use super::fold::TourStop;
use crate::index::{Index, Node};
use crate::node_ref::{Found, NodeRef};
use crate::project::Project;

/// The node of `stop` as it is now, in `tree` (the index's tree: `index.tree(..)`), or `None` when it is gone.
///
/// A stop that has a `node` is looked for by that ID, wherever the def is and whatever it is called now; one that has none, or whose ID is found
/// nowhere (a marker line taken out by hand), by its path, qualname and occurrence exactly.
pub fn find<'t>(
    index: &Index,
    project: &Project,
    tree: &'t Node,
    stop: &TourStop,
) -> Option<&'t Node> {
    if let Some(id) = stop.node
        && let Found::Node(tree_id) = index.resolve_ref(project, &NodeRef::Id(id))
        && let Some(node) = tree.find(&tree_id)
    {
        return Some(node);
    }
    by_name(
        index,
        project,
        tree,
        &stop.path,
        &stop.qualname,
        stop.occurrence,
    )
}

/// The node `text` names, in `tree`: the tree's own key (`Node::id`, what `vnoc/structure` gives a node), else any reference `node_ref` reads (`path::qualname`,
/// a path, an ID). The page asks about the nodes it draws, so it sends their keys; a reference is for a caller that has only that. `None` when no node is named.
pub fn node_named<'t>(
    index: &Index,
    project: &Project,
    tree: &'t Node,
    text: &str,
) -> Option<&'t Node> {
    if let Some(node) = tree.find(text) {
        return Some(node);
    }
    match index.resolve_ref(project, &crate::node_ref::parse(text)?) {
        Found::Node(id) => tree.find(&id),
        _ => None,
    }
}

/// `path` is a repo path; the index is asked in the opened folder's terms, and a path outside the opened folder is in no tree.
fn by_name<'t>(
    index: &Index,
    project: &Project,
    tree: &'t Node,
    path: &RepoPath,
    qualname: &str,
    occurrence: u32,
) -> Option<&'t Node> {
    let shown = match project.prefix() {
        "" => path.as_str(),
        prefix => path.as_str().strip_prefix(prefix)?.strip_prefix('/')?,
    };
    let rest = match (qualname.is_empty(), occurrence) {
        (true, _) => String::new(),
        (false, 1) => qualname.to_string(),
        (false, n) => format!("{qualname}#{n}"),
    };
    let found = index.resolve_ref(
        project,
        &NodeRef::Location {
            path: shown.to_string(),
            rest,
        },
    );
    let Found::Node(tree_id) = found else {
        return None;
    };
    let node = tree.find(&tree_id)?;
    // `resolve_ref` reads a path loosely (a file name alone, the opened folder's name in front): the node must be the one named
    same_place(node, path, qualname, occurrence).then_some(node)
}

/// Whether `node` is at this repo path with this qualname and occurrence.
fn same_place(node: &Node, path: &RepoPath, qualname: &str, occurrence: u32) -> bool {
    match place_of(node) {
        Some((p, q, n)) => p == *path && q == qualname && n == occurrence,
        None => false,
    }
}

/// A tree node's repo path, qualname and occurrence: a def's from its location key (the tree's id), a file's or a folder's from its path.
/// `None` for the root of the tree, which has no repo path.
pub fn place_of(node: &Node) -> Option<(RepoPath, String, u32)> {
    if node.qualname.is_some() {
        let key: LocationKey = node.id.parse().ok()?;
        return Some((
            key.path().clone(),
            key.qualname().to_string(),
            key.occurrence(),
        ));
    }
    RepoPath::new(node.id.clone())
        .ok()
        .map(|path| (path, String::new(), 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;
    use std::path::PathBuf;
    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    fn def(qualname: &str, parent: &str, kind: DefKind, from: u32, to: u32) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            qualname: qualname.to_string(),
            name: qualname.rsplit('.').next().unwrap().to_string(),
            kind,
            parent: parent.to_string(),
            parent_kind: None,
            range: Range {
                start: at(from),
                end: at(to),
            },
            params: vec![],
            id: None,
            syntax_error: false,
        }
    }

    fn fresh(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vnoc-find-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dunce::canonicalize(dir).unwrap()
    }

    /// The tour of one stop on `path` with `qualname` and `occurrence` and nothing else.
    fn stop_on(path: &str, qualname: &str, occurrence: u32, kind: &str) -> TourStop {
        let occurrence = if occurrence == 1 {
            String::new()
        } else {
            format!(r#","occurrence":{occurrence}"#)
        };
        let text = format!(
            "{}\n{{\"type\":\"stop\",\"id\":\"s1\",\"path\":\"{path}\",\"qualname\":\"{qualname}\"{occurrence},\"kind\":\"{kind}\",\"text\":\"t\"}}\n",
            r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#
        );
        fold(&text).unwrap().stops.remove(0)
    }

    fn index_of(files: &[(&str, Vec<Def>)], folders: &[&str]) -> Index {
        Index::for_tests(
            files
                .iter()
                .map(|(p, d)| (RepoPath::new(*p).unwrap(), d.clone()))
                .collect(),
            vec![],
            folders.iter().map(|f| RepoPath::new(*f).unwrap()).collect(),
        )
    }

    #[test]
    fn a_stop_is_found_by_its_exact_path_qualname_and_occurrence() {
        let project = Project::open(&fresh("exact")).unwrap();
        let index = index_of(
            &[(
                "a.py",
                vec![
                    def("f", "", DefKind::Function, 0, 3),
                    def("C", "", DefKind::Class, 5, 12),
                    def("C.m", "C", DefKind::Function, 6, 9),
                    def("f", "", DefKind::Function, 14, 16),
                ],
            )],
            &["pkg"],
        );
        let tree = index.tree(&project.name(), project.prefix());
        let found = |stop: &TourStop| find(&index, &project, &tree, stop).map(|n| n.id.clone());
        assert_eq!(
            found(&stop_on("a.py", "f", 1, "function")).as_deref(),
            Some("a.py::f")
        );
        assert_eq!(
            found(&stop_on("a.py", "f", 2, "function")).as_deref(),
            Some("a.py::f#2"),
            "the second f is the second f"
        );
        assert_eq!(
            found(&stop_on("a.py", "C.m", 1, "function")).as_deref(),
            Some("a.py::C.m")
        );
        assert_eq!(
            found(&stop_on("a.py", "", 1, "file")).as_deref(),
            Some("a.py")
        );
        // nothing else: a renamed def, a third f, another file, a file named alone
        for (path, qualname, occurrence) in [
            ("a.py", "g", 1),
            ("a.py", "f", 3),
            ("b.py", "f", 1),
            ("a.py", "c.m", 1),
        ] {
            assert_eq!(
                found(&stop_on(path, qualname, occurrence, "function")),
                None,
                "{path}::{qualname}#{occurrence}"
            );
        }
    }

    #[test]
    fn a_folder_is_found_by_its_path() {
        let project = Project::open(&fresh("folder")).unwrap();
        let index = index_of(
            &[("pkg/m.py", vec![def("f", "", DefKind::Function, 0, 1)])],
            &["pkg"],
        );
        let tree = index.tree(&project.name(), project.prefix());
        let found = find(&index, &project, &tree, &stop_on("pkg", "", 1, "folder"));
        assert_eq!(found.map(|n| n.id.as_str()), Some("pkg"));
    }

    #[test]
    fn with_a_folder_opened_below_the_git_root_a_stop_keeps_its_repo_path() {
        let top = fresh("below");
        std::fs::create_dir_all(top.join(".git")).unwrap();
        std::fs::create_dir_all(top.join("sub")).unwrap();
        let project = Project::open(&top.join("sub")).unwrap();
        assert_eq!(project.prefix(), "sub");
        let index = index_of(
            &[
                ("sub/a.py", vec![def("f", "", DefKind::Function, 0, 3)]),
                ("a.py", vec![def("f", "", DefKind::Function, 0, 3)]),
            ],
            &[],
        );
        let tree = index.tree(&project.name(), project.prefix());
        let found = |path: &str| {
            find(&index, &project, &tree, &stop_on(path, "f", 1, "function")).map(|n| n.id.clone())
        };
        assert_eq!(found("sub/a.py").as_deref(), Some("sub/a.py::f"));
        assert_eq!(
            found("a.py"),
            None,
            "a.py of the repo is outside the opened folder, not sub/a.py"
        );
    }

    #[test]
    fn a_tree_node_says_its_repo_path_qualname_and_occurrence() {
        let project = Project::open(&fresh("place")).unwrap();
        let index = index_of(
            &[(
                "pkg/a.py",
                vec![
                    def("f", "", DefKind::Function, 0, 1),
                    def("f", "", DefKind::Function, 3, 4),
                ],
            )],
            &[],
        );
        let tree = index.tree(&project.name(), project.prefix());
        let place =
            |id: &str| place_of(tree.find(id).unwrap()).map(|(p, q, n)| (p.to_string(), q, n));
        assert_eq!(
            place("pkg/a.py::f#2"),
            Some(("pkg/a.py".into(), "f".into(), 2))
        );
        assert_eq!(
            place("pkg/a.py"),
            Some(("pkg/a.py".into(), String::new(), 1))
        );
        assert_eq!(place("pkg"), Some(("pkg".into(), String::new(), 1)));
        assert_eq!(
            place_of(&tree),
            None,
            "the root of the tree has no repo path"
        );
    }
}
