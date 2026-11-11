//! The merge of a call tree (docs/14 T2, T3, T4): siblings that are the same function become one node, with how many there were and where
//! each call is. A pure function over the driver's tree, with no I/O and no index: it is V-NOC's policy, so one function serves every
//! driver and every way of asking.

use std::collections::HashMap;

use vnoc_domain::NodeId;
use vnoc_driver_protocol::{CallMark, CallNode, CallTarget, Range};

/// A node of the merged tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub target: CallTarget,
    /// how many calls were merged into it; the root is 1
    pub call_count: u32,
    /// every place it is called, in source order, in the parent's file; empty on the root
    pub call_sites: Vec<Range>,
    pub marks: Vec<CallMark>,
    pub children: Vec<Merged>,
}

/// What makes two siblings the same function (T4): the ID when the target has one, else where it is. Never the qualname alone.
#[derive(Hash, PartialEq, Eq)]
enum Key<'a> {
    Id(NodeId),
    Place { file: Option<&'a str>, qualname: &'a str, occurrence: u32 },
}

fn key_of(target: &CallTarget) -> Key<'_> {
    // the driver gives the ID text as the code spells it; text that is not an ID is no ID
    match target.id.as_deref().and_then(|text| NodeId::parse(text).ok()) {
        Some(id) => Key::Id(id),
        None => Key::Place { file: target.file.as_deref(), qualname: &target.qualname, occurrence: target.occurrence },
    }
}

pub fn merge(root: &CallNode) -> Merged {
    let children: Vec<&CallNode> = root.children.iter().collect();
    Merged { target: root.target.clone(), call_count: 1, call_sites: Vec::new(), marks: root.marks.clone(), children: merge_siblings(&children) }
}

/// One node per key, in the order each key first appears.
fn merge_siblings(nodes: &[&CallNode]) -> Vec<Merged> {
    let mut groups: Vec<Vec<&CallNode>> = Vec::new();
    let mut at: HashMap<Key<'_>, usize> = HashMap::new();
    for &node in nodes {
        let group = *at.entry(key_of(&node.target)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(node);
    }
    groups.iter().map(|group| merge_group(group)).collect()
}

/// `group`: calls of one function, never empty. Their children are put together and merged the same way.
fn merge_group(group: &[&CallNode]) -> Merged {
    let mut marks = Vec::new();
    for mark in group.iter().flat_map(|node| &node.marks) {
        if !marks.contains(mark) {
            marks.push(*mark);
        }
    }
    // The children of calls to one function are calls from one body: the same sites again for each call, so a site is listed once.
    let position = |r: &Range| (r.start.line, r.start.character, r.end.line, r.end.character);
    let mut call_sites: Vec<Range> = group.iter().filter_map(|node| node.call_site).collect();
    call_sites.sort_by_key(position);
    call_sites.dedup();
    let children: Vec<&CallNode> = group.iter().flat_map(|node| &node.children).collect();
    Merged { target: group[0].target.clone(), call_count: group.len() as u32, call_sites, marks, children: merge_siblings(&children) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{DefKind, Position};

    const ID_A: &str = "3RGJ87CMS6TK775XRJV8BW977Y";
    const ID_B: &str = "7AZ0000000000000000000000G";

    fn at(line: u32, character: u32) -> Range {
        Range { start: Position { line, character }, end: Position { line, character: character + 3 } }
    }

    fn target(file: Option<&str>, qualname: &str, occurrence: u32, id: Option<&str>) -> CallTarget {
        CallTarget { file: file.map(str::to_string), qualname: qualname.to_string(), occurrence, kind: DefKind::Function, id: id.map(str::to_string) }
    }

    /// A call to `qualname` in `src/m.py`, made at `line`.
    fn call(qualname: &str, line: u32, children: Vec<CallNode>) -> CallNode {
        CallNode { target: target(Some("src/m.py"), qualname, 1, None), call_site: Some(at(line, 4)), marks: vec![], children }
    }

    fn root(children: Vec<CallNode>) -> CallNode {
        CallNode { target: target(Some("src/m.py"), "a", 1, None), call_site: None, marks: vec![], children }
    }

    fn names(nodes: &[Merged]) -> Vec<&str> {
        nodes.iter().map(|n| n.target.qualname.as_str()).collect()
    }

    #[test]
    fn a_called_twice_and_c_once_makes_two_nodes_in_the_order_of_first_appearance() {
        let merged = merge(&root(vec![call("b", 3, vec![]), call("c", 4, vec![]), call("b", 5, vec![])]));
        assert_eq!(names(&merged.children), ["b", "c"]);
        let (b, c) = (&merged.children[0], &merged.children[1]);
        assert_eq!((b.call_count, b.call_sites.clone()), (2, vec![at(3, 4), at(5, 4)]));
        assert_eq!((c.call_count, c.call_sites.clone()), (1, vec![at(4, 4)]));
    }

    #[test]
    fn the_root_is_one_call_with_no_sites() {
        let merged = merge(&root(vec![]));
        assert_eq!((merged.call_count, merged.call_sites.len(), merged.children.len()), (1, 0, 0));
        assert_eq!(merged.target.qualname, "a");
    }

    #[test]
    fn the_sites_of_a_node_are_in_source_order() {
        // the driver gives them in source order; two merged calls from different parents are put in order too
        let merged = merge(&root(vec![call("b", 9, vec![]), call("b", 2, vec![]), call("b", 5, vec![])]));
        assert_eq!(merged.children[0].call_sites, [at(2, 4), at(5, 4), at(9, 4)]);
    }

    #[test]
    fn marks_are_the_union_without_repeats() {
        let marked = |line, marks: Vec<CallMark>| CallNode { marks, ..call("b", line, vec![]) };
        let merged = merge(&root(vec![
            marked(1, vec![CallMark::Recursive]),
            marked(2, vec![]),
            marked(3, vec![CallMark::Cut, CallMark::Recursive]),
            marked(4, vec![CallMark::Incomplete]),
        ]));
        assert_eq!(merged.children[0].marks, [CallMark::Recursive, CallMark::Cut, CallMark::Incomplete]);
        assert_eq!(merged.children[0].call_count, 4);
    }

    #[test]
    fn the_children_of_merged_siblings_are_put_together_and_merged() {
        // b is called twice, and each call calls c once (at the same place in b's body) and d once
        let in_b = |line| vec![call("c", 20, vec![]), call("d", line, vec![])];
        let merged = merge(&root(vec![call("b", 3, in_b(21)), call("b", 5, in_b(22))]));
        let b = &merged.children[0];
        assert_eq!(names(&b.children), ["c", "d"]);
        // c: two calls, but one place in the code; d was written at two places
        assert_eq!((b.children[0].call_count, b.children[0].call_sites.clone()), (2, vec![at(20, 4)]));
        assert_eq!((b.children[1].call_count, b.children[1].call_sites.clone()), (2, vec![at(21, 4), at(22, 4)]));
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
        assert_eq!(leaf.call_sites, [at(30, 4), at(31, 4)]);
        assert_eq!((merged.children.len(), top.children.len(), mid.children.len()), (1, 1, 1));
    }

    #[test]
    fn one_qualname_in_two_files_stays_two_nodes() {
        let in_file = |file: &str| CallNode { target: target(Some(file), "helper", 1, None), call_site: Some(at(1, 4)), marks: vec![], children: vec![] };
        let merged = merge(&root(vec![in_file("src/a.py"), in_file("src/b.py"), in_file("src/a.py")]));
        assert_eq!(merged.children.iter().map(|n| (n.target.file.clone().unwrap(), n.call_count)).collect::<Vec<_>>(), [("src/a.py".to_string(), 2), ("src/b.py".to_string(), 1)]);
    }

    #[test]
    fn two_definitions_with_one_qualname_in_a_file_stay_two_nodes() {
        let nth = |occurrence| CallNode { target: target(Some("src/m.py"), "f", occurrence, None), call_site: Some(at(occurrence, 4)), marks: vec![], children: vec![] };
        let merged = merge(&root(vec![nth(1), nth(2), nth(1)]));
        assert_eq!(merged.children.iter().map(|n| (n.target.occurrence, n.call_count)).collect::<Vec<_>>(), [(1, 2), (2, 1)]);
    }

    #[test]
    fn the_id_is_the_key_when_the_target_has_one() {
        let with_id = |id: &str, qualname: &str| CallNode { target: target(Some("src/m.py"), qualname, 1, Some(id)), call_site: Some(at(1, 4)), marks: vec![], children: vec![] };
        // one ID is one function, wherever the code says it is; two IDs are two
        let merged = merge(&root(vec![with_id(ID_A, "old_name"), with_id(ID_B, "old_name"), with_id(ID_A, "new_name")]));
        assert_eq!(merged.children.iter().map(|n| (n.target.qualname.as_str(), n.call_count)).collect::<Vec<_>>(), [("old_name", 2), ("old_name", 1)]);
    }

    #[test]
    fn text_that_is_not_an_id_is_no_id() {
        let text = |id: &str, qualname: &str| CallNode { target: target(Some("src/m.py"), qualname, 1, Some(id)), call_site: Some(at(1, 4)), marks: vec![], children: vec![] };
        let merged = merge(&root(vec![text("not-an-id", "f"), text("not-an-id", "g"), text("not-an-id", "f")]));
        assert_eq!(merged.children.iter().map(|n| (n.target.qualname.as_str(), n.call_count)).collect::<Vec<_>>(), [("f", 2), ("g", 1)]);
    }

    #[test]
    fn two_targets_outside_the_project_merge_on_qualname_and_occurrence() {
        let library = |qualname: &str| CallNode { target: target(None, qualname, 1, None), call_site: Some(at(1, 4)), marks: vec![], children: vec![] };
        let merged = merge(&root(vec![library("dumps"), library("loads"), library("dumps")]));
        assert_eq!(merged.children.iter().map(|n| (n.target.qualname.as_str(), n.call_count)).collect::<Vec<_>>(), [("dumps", 2), ("loads", 1)]);
    }
}
