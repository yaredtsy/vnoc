//! The merge of a call tree (docs/14 T2, T3, T4): siblings that are the same function become one node, with how many there were and where
//! each call is. A pure function over the driver's tree, with no I/O and no index: it is V-NOC's policy, so one function serves every
//! driver and every way of asking.

use std::collections::HashMap;
use std::sync::RwLock;

use vnoc_domain::{LocationKey, NodeId};
use vnoc_driver_protocol::{CallEntry, CallMark, CallNode, CallTarget, Range};

use crate::driver_host::{DriverHost, HostError};
use crate::index::Index;
use crate::project::Project;
use crate::protocol::{CallSite, CallTreeNode, CallTreeResult};

/// One place a node is called: a file and a range in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// relative to the driver's root, `/` separators; `None` when no file is known (the driver gave none and the parent is outside the root)
    pub file: Option<String>,
    pub range: Range,
}

/// A node of the merged tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub target: CallTarget,
    /// how many calls were merged into it; the root is 1
    pub call_count: u32,
    /// every place it is called, by file and then in source order; empty on the root
    pub call_sites: Vec<Site>,
    pub marks: Vec<CallMark>,
    pub children: Vec<Merged>,
}

/// What makes two siblings the same function (T4): the ID when the target has one, else where it is. Never the qualname alone.
#[derive(Hash, PartialEq, Eq)]
enum Key<'a> {
    Id(NodeId),
    Place {
        file: Option<&'a str>,
        qualname: &'a str,
        occurrence: u32,
    },
}

fn key_of(target: &CallTarget) -> Key<'_> {
    // the driver gives the ID text as the code spells it; text that is not an ID is no ID
    match target
        .id
        .as_deref()
        .and_then(|text| NodeId::parse(text).ok())
    {
        Some(id) => Key::Id(id),
        None => Key::Place {
            file: target.file.as_deref(),
            qualname: &target.qualname,
            occurrence: target.occurrence,
        },
    }
}

/// A call, and the file of the node it is a child of: a site that names no file of its own is in that file (docs/14 §10.4). It is worked
/// out before grouping, because after a merge by ID the parents of two children can be in two files.
#[derive(Clone, Copy)]
struct Call<'a> {
    node: &'a CallNode,
    parent_file: Option<&'a str>,
}

/// The calls a node makes, each with the file of that node.
fn calls_of(parent: &CallNode) -> impl Iterator<Item = Call<'_>> {
    parent.children.iter().map(|node| Call {
        node,
        parent_file: parent.target.file.as_deref(),
    })
}

/// How deep a full `vnoc/callTree` goes (docs/16-ui-redesign/05-calls-as-nodes.md §3.1). The driver is asked for this many levels; a node at the cap comes back
/// marked `cut`, and opening it asks for its own tree. Without a cap a deep chain breaks the core, which reads the driver's answer with `serde_json`'s
/// nesting limit of 128 (a level costs two: the node and its list of children), and the size of an answer has no bound.
pub const DEPTH_CAP: u32 = 32;

/// What `vnoc/hasCalls` answers: how many nodes the merged tree has under its root. The marks of a probe's tree (every child is `cut`, because the
/// probe stopped there) are not looked at and do not leave the core.
pub fn count_calls(root: &CallNode) -> u32 {
    merge(root).children.len() as u32
}

pub fn merge(root: &CallNode) -> Merged {
    let children: Vec<Call<'_>> = calls_of(root).collect();
    Merged {
        target: root.target.clone(),
        call_count: 1,
        call_sites: Vec::new(),
        marks: root.marks.clone(),
        children: merge_siblings(&children),
    }
}

/// One node per key, in the order each key first appears.
fn merge_siblings(calls: &[Call<'_>]) -> Vec<Merged> {
    let mut groups: Vec<Vec<Call<'_>>> = Vec::new();
    let mut at: HashMap<Key<'_>, usize> = HashMap::new();
    for &call in calls {
        let group = *at.entry(key_of(&call.node.target)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(call);
    }
    groups.iter().map(|group| merge_group(group)).collect()
}

/// `group`: calls of one function, never empty. Their children are put together and merged the same way.
fn merge_group(group: &[Call<'_>]) -> Merged {
    let mut marks = Vec::new();
    for mark in group.iter().flat_map(|call| &call.node.marks) {
        if !marks.contains(mark) {
            marks.push(*mark);
        }
    }
    // The children of calls to one function are calls from one body: the same sites again for each call, so a site is listed once. A site is
    // a file and a range: two copies of one function in two files have the same range in two places.
    let position = |r: &Range| (r.start.line, r.start.character, r.end.line, r.end.character);
    let mut call_sites: Vec<Site> = group
        .iter()
        .filter_map(|call| {
            let file = call.node.call_site_file.as_deref().or(call.parent_file);
            call.node.call_site.map(|range| Site {
                file: file.map(str::to_string),
                range,
            })
        })
        .collect();
    call_sites.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| position(&a.range).cmp(&position(&b.range)))
    });
    call_sites.dedup();
    let children: Vec<Call<'_>> = group.iter().flat_map(|call| calls_of(call.node)).collect();
    Merged {
        target: group[0].node.target.clone(),
        call_count: group.len() as u32,
        call_sites,
        marks,
        children: merge_siblings(&children),
    }
}

/// Why a call tree could not be made. The server maps each case to its response code; a command will map them to its exit codes.
#[derive(Debug, thiserror::Error)]
pub enum CallsError {
    /// the key is not a function or a class, does not parse, is not in the index, or the driver refused the request as invalid
    #[error("{0}")]
    Invalid(String),
    /// the driver does not have `callTree`
    #[error("{0}")]
    MissingCapability(String),
    /// anything else
    #[error("{0}")]
    Internal(String),
}

/// Asks the driver for the tree of the def `key` names, `depth` levels deep. The index is built; the errors are `vnoc/callTree`'s.
///
/// The index is read only to see that the def is there, and the read ends before the driver is asked (a `std` guard must not live across an
/// `.await`).
pub async fn ask(
    project: &Project,
    index: &RwLock<Index>,
    host: &DriverHost,
    key: &str,
    depth: Option<u32>,
) -> Result<vnoc_driver_protocol::CallTreeResult, CallsError> {
    let invalid = |message: String| CallsError::Invalid(message);
    if !key.contains("::") {
        return Err(invalid(format!(
            "`{key}` is not a function or a class: a call tree starts at one"
        )));
    }
    let location: LocationKey = key.parse().map_err(|e| invalid(format!("{e}")))?;
    let in_index = {
        let index = index.read().unwrap();
        index.def_at(&location).is_some()
    };
    if !in_index {
        return Err(invalid(format!(
            "`{key}` is not in the index (is the file saved, and is it Python?)"
        )));
    }

    // The driver is given the opened folder: it is the project the user sees, and what its imports resolve against. Its paths are then
    // relative to that folder, like the editor's; a repo path (what the index's keys are made of) is the folder's prefix and the path.
    let inside = location
        .path()
        .as_str()
        .strip_prefix(project.prefix())
        .map(|p| p.trim_start_matches('/'));
    let file = inside
        .filter(|p| !p.is_empty())
        .ok_or_else(|| CallsError::Internal(format!("`{key}` is not under the opened folder")))?;
    let asked = vnoc_driver_protocol::CallTreeParams {
        root: project.open_dir().to_string_lossy().into_owned(),
        file: file.to_string(),
        entry: CallEntry {
            qualname: location.qualname().to_string(),
            occurrence: location.occurrence(),
        },
        depth,
    };
    host.call_tree(&asked).await.map_err(|e| match e {
        HostError::MissingCapability(_) => CallsError::MissingCapability(format!(
            "{e}: call trees need a driver that has `callTree`"
        )),
        HostError::Remote {
            code: -32602,
            message,
        } => invalid(message),
        other => CallsError::Internal(other.to_string()),
    })
}

/// `vnoc/callTree`'s answer: the driver's tree for `key`, merged, with each target given the key the index knows it by.
/// Every full tree is asked for with a depth cap (`DEPTH_CAP`): a node at it is `cut`, and opening it asks for its own tree.
pub async fn tree_for(
    project: &Project,
    index: &RwLock<Index>,
    host: &DriverHost,
    key: &str,
    depth: Option<u32>,
) -> Result<CallTreeResult, CallsError> {
    let answer = ask(project, index, host, key, depth).await?;
    let merged = merge(&answer.root);
    // one read of the index for the whole mapping: the revision says which index the keys are from
    let index = index.read().unwrap();
    let known = |target: &CallTarget| {
        let location = LocationKey::new(
            project.repo_path(target.file.as_deref()?).ok()?,
            target.qualname.clone(),
            target.occurrence,
        )
        .ok()?;
        index
            .def_at(&location)
            .map(|def| (location.to_string(), def.name))
    };
    let root = tree_node(&merged, &known);
    Ok(CallTreeResult {
        revision: index.revision(),
        root,
    })
}

/// The short name of a target the index does not have: the part of its qualname after the last dot, which is right for Python (`C.f` is `f`) and
/// wrong for a language whose names hold dots (Bend's `Nat.double`). The index's own `Def::name`, the driver's word, is used whenever the target
/// is there (nearly always: a node in the tree is a def of the project); only a def the scan did not read gets this.
fn short_name_guess(qualname: &str) -> &str {
    qualname.rsplit('.').next().unwrap_or(qualname)
}

/// A merged node as the editor gets it. `known`: the key and the name (the outline's) of a target the index has.
fn tree_node(
    node: &Merged,
    known: &dyn Fn(&CallTarget) -> Option<(String, String)>,
) -> CallTreeNode {
    let target = &node.target;
    let (key, name) = match known(target) {
        Some((key, name)) => (Some(key), name),
        None => (None, short_name_guess(&target.qualname).to_string()),
    };
    CallTreeNode {
        key,
        name,
        qualname: target.qualname.clone(),
        path: target.file.clone(), // already relative to the opened folder, `null` outside it
        kind: target.kind,
        call_count: node.call_count,
        call_sites: node
            .call_sites
            .iter()
            .map(|site| CallSite {
                path: site.file.clone(),
                range: site.range,
            })
            .collect(),
        marks: node.marks.clone(),
        children: node
            .children
            .iter()
            .map(|child| tree_node(child, known))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{DefKind, Position};

    const ID_A: &str = "3RGJ87CMS6TK775XRJV8BW977Y";
    const ID_B: &str = "7AZ0000000000000000000000G";

    fn at(line: u32, character: u32) -> Range {
        Range {
            start: Position { line, character },
            end: Position {
                line,
                character: character + 3,
            },
        }
    }

    /// Sites in `file`, at the given ranges.
    fn sites(file: &str, ranges: impl IntoIterator<Item = Range>) -> Vec<Site> {
        ranges
            .into_iter()
            .map(|range| Site {
                file: Some(file.to_string()),
                range,
            })
            .collect()
    }

    fn target(file: Option<&str>, qualname: &str, occurrence: u32, id: Option<&str>) -> CallTarget {
        CallTarget {
            file: file.map(str::to_string),
            qualname: qualname.to_string(),
            occurrence,
            kind: DefKind::Function,
            id: id.map(str::to_string),
        }
    }

    /// A call to `qualname` in `src/m.py`, made at `line`.
    fn call(qualname: &str, line: u32, children: Vec<CallNode>) -> CallNode {
        CallNode {
            target: target(Some("src/m.py"), qualname, 1, None),
            call_site: Some(at(line, 4)),
            call_site_file: None,
            marks: vec![],
            children,
        }
    }

    fn root(children: Vec<CallNode>) -> CallNode {
        CallNode {
            target: target(Some("src/m.py"), "a", 1, None),
            call_site: None,
            call_site_file: None,
            marks: vec![],
            children,
        }
    }

    fn names(nodes: &[Merged]) -> Vec<&str> {
        nodes.iter().map(|n| n.target.qualname.as_str()).collect()
    }

    #[test]
    fn a_called_twice_and_c_once_makes_two_nodes_in_the_order_of_first_appearance() {
        let merged = merge(&root(vec![
            call("b", 3, vec![]),
            call("c", 4, vec![]),
            call("b", 5, vec![]),
        ]));
        assert_eq!(names(&merged.children), ["b", "c"]);
        let (b, c) = (&merged.children[0], &merged.children[1]);
        assert_eq!(
            (b.call_count, b.call_sites.clone()),
            (2, sites("src/m.py", [at(3, 4), at(5, 4)]))
        );
        assert_eq!(
            (c.call_count, c.call_sites.clone()),
            (1, sites("src/m.py", [at(4, 4)]))
        );
    }

    #[test]
    fn the_root_is_one_call_with_no_sites() {
        let merged = merge(&root(vec![]));
        assert_eq!(
            (
                merged.call_count,
                merged.call_sites.len(),
                merged.children.len()
            ),
            (1, 0, 0)
        );
        assert_eq!(merged.target.qualname, "a");
    }

    #[test]
    fn the_sites_of_a_node_are_in_source_order() {
        // the driver gives them in source order; two merged calls from different parents are put in order too
        let merged = merge(&root(vec![
            call("b", 9, vec![]),
            call("b", 2, vec![]),
            call("b", 5, vec![]),
        ]));
        assert_eq!(
            merged.children[0].call_sites,
            sites("src/m.py", [at(2, 4), at(5, 4), at(9, 4)])
        );
    }

    #[test]
    fn a_probe_counts_the_nodes_opening_would_show() {
        // calls nothing (or only library code, which the driver leaves out): no node
        assert_eq!(count_calls(&root(vec![])), 0);
        // two calls to one callee are one node, as they are in the tree
        assert_eq!(
            count_calls(&root(vec![call("b", 3, vec![]), call("b", 9, vec![])])),
            1
        );
        assert_eq!(
            count_calls(&root(vec![
                call("b", 3, vec![]),
                call("c", 4, vec![]),
                call("b", 9, vec![])
            ])),
            2
        );
    }

    #[test]
    fn the_cut_of_a_depth_one_probe_is_not_counted_as_anything() {
        // the probe stops one level down, so every child is marked cut (and has no children); the count is the children, whatever their marks
        let cut = |name, line| CallNode {
            marks: vec![CallMark::Cut],
            ..call(name, line, vec![])
        };
        assert_eq!(count_calls(&root(vec![cut("b", 1), cut("c", 2)])), 2);
        assert_eq!(
            count_calls(&CallNode {
                marks: vec![CallMark::Cut],
                ..root(vec![])
            }),
            0,
            "and a root marked cut with nothing under it has none"
        );
    }

    #[test]
    fn marks_are_the_union_without_repeats() {
        let marked = |line, marks: Vec<CallMark>| CallNode {
            marks,
            ..call("b", line, vec![])
        };
        let merged = merge(&root(vec![
            marked(1, vec![CallMark::Recursive]),
            marked(2, vec![]),
            marked(3, vec![CallMark::Cut, CallMark::Recursive]),
            marked(4, vec![CallMark::Incomplete]),
            marked(5, vec![CallMark::Likely, CallMark::Cut]),
        ]));
        assert_eq!(
            merged.children[0].marks,
            [
                CallMark::Recursive,
                CallMark::Cut,
                CallMark::Incomplete,
                CallMark::Likely
            ]
        );
        assert_eq!(merged.children[0].call_count, 5);
    }

    #[test]
    fn the_children_of_merged_siblings_are_put_together_and_merged() {
        // b is called twice, and each call calls c once (at the same place in b's body) and d once
        let in_b = |line| vec![call("c", 20, vec![]), call("d", line, vec![])];
        let merged = merge(&root(vec![call("b", 3, in_b(21)), call("b", 5, in_b(22))]));
        let b = &merged.children[0];
        assert_eq!(names(&b.children), ["c", "d"]);
        // c: two calls, but one place in the code; d was written at two places
        assert_eq!(
            (b.children[0].call_count, b.children[0].call_sites.clone()),
            (2, sites("src/m.py", [at(20, 4)]))
        );
        assert_eq!(
            (b.children[1].call_count, b.children[1].call_sites.clone()),
            (2, sites("src/m.py", [at(21, 4), at(22, 4)]))
        );
    }

    #[test]
    fn a_merge_three_levels_deep() {
        let leaf = |line| call("leaf", line, vec![]);
        let mid = |line| call("mid", line, vec![leaf(30), leaf(31)]);
        let top = |line| call("top", line, vec![mid(20), mid(21)]);
        let merged = merge(&root(vec![top(1), top(2)]));
        let top = &merged.children[0];
        let mid = &top.children[0];
        let leaf = &mid.children[0];
        assert_eq!((top.call_count, mid.call_count, leaf.call_count), (2, 4, 8));
        assert_eq!(leaf.call_sites, sites("src/m.py", [at(30, 4), at(31, 4)]));
        assert_eq!(
            (
                merged.children.len(),
                top.children.len(),
                mid.children.len()
            ),
            (1, 1, 1)
        );
    }

    #[test]
    fn one_qualname_in_two_files_stays_two_nodes() {
        let in_file = |file: &str| CallNode {
            target: target(Some(file), "helper", 1, None),
            call_site: Some(at(1, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![],
        };
        let merged = merge(&root(vec![
            in_file("src/a.py"),
            in_file("src/b.py"),
            in_file("src/a.py"),
        ]));
        assert_eq!(
            merged
                .children
                .iter()
                .map(|n| (n.target.file.clone().unwrap(), n.call_count))
                .collect::<Vec<_>>(),
            [("src/a.py".to_string(), 2), ("src/b.py".to_string(), 1)]
        );
    }

    #[test]
    fn two_definitions_with_one_qualname_in_a_file_stay_two_nodes() {
        let nth = |occurrence| CallNode {
            target: target(Some("src/m.py"), "f", occurrence, None),
            call_site: Some(at(occurrence, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![],
        };
        let merged = merge(&root(vec![nth(1), nth(2), nth(1)]));
        assert_eq!(
            merged
                .children
                .iter()
                .map(|n| (n.target.occurrence, n.call_count))
                .collect::<Vec<_>>(),
            [(1, 2), (2, 1)]
        );
    }

    #[test]
    fn the_id_is_the_key_when_the_target_has_one() {
        let with_id = |id: &str, qualname: &str| CallNode {
            target: target(Some("src/m.py"), qualname, 1, Some(id)),
            call_site: Some(at(1, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![],
        };
        // one ID is one function, wherever the code says it is; two IDs are two
        let merged = merge(&root(vec![
            with_id(ID_A, "old_name"),
            with_id(ID_B, "old_name"),
            with_id(ID_A, "new_name"),
        ]));
        assert_eq!(
            merged
                .children
                .iter()
                .map(|n| (n.target.qualname.as_str(), n.call_count))
                .collect::<Vec<_>>(),
            [("old_name", 2), ("old_name", 1)]
        );
    }

    #[test]
    fn text_that_is_not_an_id_is_no_id() {
        let text = |id: &str, qualname: &str| CallNode {
            target: target(Some("src/m.py"), qualname, 1, Some(id)),
            call_site: Some(at(1, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![],
        };
        let merged = merge(&root(vec![
            text("not-an-id", "f"),
            text("not-an-id", "g"),
            text("not-an-id", "f"),
        ]));
        assert_eq!(
            merged
                .children
                .iter()
                .map(|n| (n.target.qualname.as_str(), n.call_count))
                .collect::<Vec<_>>(),
            [("f", 2), ("g", 1)]
        );
    }

    #[test]
    fn two_targets_outside_the_project_merge_on_qualname_and_occurrence() {
        let library = |qualname: &str| CallNode {
            target: target(None, qualname, 1, None),
            call_site: Some(at(1, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![],
        };
        let merged = merge(&root(vec![
            library("dumps"),
            library("loads"),
            library("dumps"),
        ]));
        assert_eq!(
            merged
                .children
                .iter()
                .map(|n| (n.target.qualname.as_str(), n.call_count))
                .collect::<Vec<_>>(),
            [("dumps", 2), ("loads", 1)]
        );
    }

    /// `Child` is in `a.py`; its `__init__` is inherited from `b.py`, and the call is written there.
    #[test]
    fn a_site_is_in_the_file_the_driver_names() {
        let from_b = CallNode {
            call_site_file: Some("b.py".to_string()),
            ..call("helper", 3, vec![])
        };
        let merged = merge(&CallNode {
            target: target(Some("a.py"), "Child", 1, None),
            ..root(vec![from_b])
        });
        assert_eq!(merged.children[0].call_sites, sites("b.py", [at(3, 4)]));
    }

    #[test]
    fn a_site_with_no_file_is_in_its_parents_file() {
        let merged = merge(&CallNode {
            target: target(Some("a.py"), "main", 1, None),
            ..root(vec![call("helper", 3, vec![])])
        });
        assert_eq!(merged.children[0].call_sites, sites("a.py", [at(3, 4)]));
    }

    #[test]
    fn two_copies_of_one_function_in_two_files_keep_a_site_for_each_file() {
        // one ID in two files merges into one node (T4); each copy calls `helper` at the same line and column of its own file
        let copy = |file: &str| CallNode {
            target: target(Some(file), "f", 1, Some(ID_A)),
            call_site: Some(at(1, 4)),
            call_site_file: None,
            marks: vec![],
            children: vec![call("helper", 5, vec![])],
        };
        let merged = merge(&root(vec![copy("src/a.py"), copy("src/b.py")]));
        let f = &merged.children[0];
        assert_eq!(f.call_count, 2);
        let helper = &f.children[0];
        assert_eq!(helper.call_count, 2);
        let expected: Vec<Site> = sites("src/a.py", [at(5, 4)])
            .into_iter()
            .chain(sites("src/b.py", [at(5, 4)]))
            .collect();
        assert_eq!(helper.call_sites, expected);
    }

    /// The two refusals that need no driver: `host` has no path, so reaching it would be an error of another kind.
    async fn refused(key: &str) -> CallsError {
        let dir = std::env::temp_dir().join(format!("vnoc-calls-ask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let project = Project::open(&dir).unwrap();
        let (index, host) = (RwLock::new(Index::default()), DriverHost::default());
        ask(&project, &index, &host, key, None).await.unwrap_err()
    }

    #[tokio::test]
    async fn a_key_without_a_def_is_refused_as_invalid() {
        match refused("src/orders.py").await {
            CallsError::Invalid(text) => assert_eq!(
                text,
                "`src/orders.py` is not a function or a class: a call tree starts at one"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn a_def_the_index_does_not_have_is_refused_as_invalid() {
        match refused("src/orders.py::parse_order").await {
            CallsError::Invalid(text) => assert_eq!(
                text,
                "`src/orders.py::parse_order` is not in the index (is the file saved, and is it Python?)"
            ),
            other => panic!("{other:?}"),
        }
    }

    /// A tree of one call, shown as the editor gets it, with `known` standing for the index.
    fn shown(
        qualname: &str,
        known: impl Fn(&CallTarget) -> Option<(String, String)>,
    ) -> CallTreeNode {
        tree_node(&merge(&root(vec![call(qualname, 3, vec![])])), &known)
    }

    #[test]
    fn a_node_is_named_by_the_outline_not_by_cutting_its_qualname() {
        // Bend: the dot is a character of the name
        let bend = shown("Nat.double", |t| {
            Some((
                format!("lib.bend::{}", t.qualname),
                "Nat.double".to_string(),
            ))
        });
        let node = &bend.children[0];
        assert_eq!(
            (
                node.name.as_str(),
                node.qualname.as_str(),
                node.key.as_deref()
            ),
            ("Nat.double", "Nat.double", Some("lib.bend::Nat.double"))
        );
        // Python: a method is named `f`, its qualname is `C.f`
        let python = shown("C.f", |t| {
            Some((format!("m.py::{}", t.qualname), "f".to_string()))
        });
        assert_eq!(python.children[0].name, "f");
        assert_eq!(python.children[0].qualname, "C.f");
    }

    #[test]
    fn a_target_the_index_does_not_have_is_named_by_the_last_part_of_its_qualname() {
        // the one place the qualname is cut: no outline to ask. Right for Python, a guess for a dotted name
        let unknown = shown("Nat.double", |_| None);
        assert_eq!(unknown.children[0].name, "double");
        assert_eq!(unknown.children[0].key, None);
        assert_eq!(shown("C.f", |_| None).children[0].name, "f");
        assert_eq!(shown("plain", |_| None).children[0].name, "plain");
    }
}
