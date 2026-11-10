//! Finding a documented node again: marker → anchor → Missing (`05-identity/05-finding-nodes-again.md` §1).
//!
//! Pure: it reads its arguments and returns a value (no disk, no driver, no clock), so the heart of the product is tested with a table.

use std::collections::{BTreeMap, HashMap};

use vnoc_domain::{LocationKey, NodeId, RepoPath, occurrences};
use vnoc_driver_protocol::Def;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundBy {
    /// the ID is written in the code: the doc follows the code anywhere
    Marker,
    /// only the stored path and name matched: found while nothing moves
    Anchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Found { at: LocationKey, by: FoundBy },
    Missing,
}

/// What the code says: where each V-NOC ID is written, and which ID is written at each place.
#[derive(Default)]
pub struct Markers {
    /// an ID can be in several places (copy-paste); they come in path order, then line order
    by_id: HashMap<NodeId, Vec<LocationKey>>,
    at: HashMap<LocationKey, NodeId>,
}

impl Markers {
    /// `file_ids` are the texts of the `# FileID:` comments, `folder_ids` the parsed `.vnoc-id` files. A file or a folder is a
    /// location with an empty qualname. An ID that is not a V-NOC ID (an old UUID) is not ours: it is neither a marker nor a block.
    pub fn read(files: &BTreeMap<RepoPath, Vec<Def>>, file_ids: &BTreeMap<RepoPath, String>, folder_ids: &BTreeMap<RepoPath, NodeId>) -> Markers {
        let whole = |path: &RepoPath| LocationKey::new(path.clone(), "", 1).expect("occurrences are 1-based");
        let mut found: Vec<(LocationKey, NodeId)> = Vec::new();
        for (path, defs) in files {
            if let Some(id) = file_ids.get(path).and_then(|text| NodeId::parse(text).ok()) {
                found.push((whole(path), id));
            }
            let which = occurrences(defs.iter().map(|d| d.qualname.as_str()));
            for (def, occurrence) in defs.iter().zip(which) {
                let Some(id) = def.id.as_deref().and_then(|text| NodeId::parse(text).ok()) else { continue };
                found.push((LocationKey::new(path.clone(), def.qualname.clone(), occurrence).expect("occurrences are 1-based"), id));
            }
        }
        found.extend(folder_ids.iter().map(|(path, id)| (whole(path), *id)));
        // path order; the sort is stable, so inside one file the file's own marker comes first, then the defs in line order
        found.sort_by(|a, b| a.0.path().cmp(b.0.path()));

        let mut markers = Markers::default();
        for (location, id) in found {
            markers.by_id.entry(id).or_default().push(location.clone());
            markers.at.insert(location, id);
        }
        markers
    }
}

/// `anchor` is what `node.toml` says; `exists` says whether a location is in the index.
pub fn resolve(id: &NodeId, anchor: Option<&LocationKey>, markers: &Markers, exists: impl Fn(&LocationKey) -> bool) -> Resolved {
    // 1. MARKER. With several copies, the one at the anchor is the node; else the first by path, then line (§3).
    if let Some(places) = markers.by_id.get(id) {
        let at = anchor.and_then(|a| places.iter().find(|p| *p == a)).unwrap_or(&places[0]);
        return Resolved::Found { at: at.clone(), by: FoundBy::Marker };
    }
    // 2. ANCHOR, unless a different V-NOC ID is written there: then it is someone else's node
    if let Some(anchor) = anchor.filter(|a| exists(a) && !markers.at.contains_key(*a)) {
        return Resolved::Found { at: anchor.clone(), by: FoundBy::Anchor };
    }
    // 3. MISSING
    Resolved::Missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{DefKind, Position, Range};

    fn id(n: u8) -> NodeId {
        NodeId::parse(&format!("0000000000000000000000000{n}")).unwrap()
    }

    fn at(path: &str, qualname: &str) -> LocationKey {
        LocationKey::new(RepoPath::new(path).unwrap(), qualname, 1).unwrap()
    }

    fn def(qualname: &str, marker: Option<NodeId>) -> Def {
        let p = Position { line: 0, character: 0 };
        Def { qualname: qualname.into(), name: qualname.into(), kind: DefKind::Function, parent: String::new(), parent_kind: None, range: Range { start: p, end: p }, params: vec![], id: marker.map(|m| m.to_string()), syntax_error: false }
    }

    fn read(files: &BTreeMap<RepoPath, Vec<Def>>) -> Markers {
        Markers::read(files, &BTreeMap::new(), &BTreeMap::new())
    }

    /// The code: `a.py` has `f` (marked 1) and `g` (unmarked); `b.py` has `f` marked 2 and a copy of 1's marker on `copy`.
    fn code() -> (BTreeMap<RepoPath, Vec<Def>>, Markers) {
        let files: BTreeMap<_, _> = [("a.py", vec![def("f", Some(id(1))), def("g", None)]), ("b.py", vec![def("f", Some(id(2))), def("copy", Some(id(1)))])]
            .into_iter()
            .map(|(p, d)| (RepoPath::new(p).unwrap(), d))
            .collect();
        let markers = read(&files);
        (files, markers)
    }

    fn exists(files: &BTreeMap<RepoPath, Vec<Def>>) -> impl Fn(&LocationKey) -> bool + '_ {
        move |l| files.get(l.path()).is_some_and(|defs| defs.iter().any(|d| d.qualname == l.qualname()))
    }

    #[test]
    fn the_marker_wins_even_when_the_anchor_is_stale() {
        let (files, markers) = code();
        let stale = at("old/place.py", "renamed");
        assert_eq!(resolve(&id(2), Some(&stale), &markers, exists(&files)), Resolved::Found { at: at("b.py", "f"), by: FoundBy::Marker });
    }

    #[test]
    fn without_a_marker_the_anchor_finds_it_by_name() {
        let (files, markers) = code();
        assert_eq!(resolve(&id(3), Some(&at("a.py", "g")), &markers, exists(&files)), Resolved::Found { at: at("a.py", "g"), by: FoundBy::Anchor });
    }

    #[test]
    fn an_anchor_that_carries_another_nodes_id_is_not_ours() {
        let (files, markers) = code();
        // node 3 was documented at a.py::f, but a.py::f now carries node 1's ID
        assert_eq!(resolve(&id(3), Some(&at("a.py", "f")), &markers, exists(&files)), Resolved::Missing);
    }

    #[test]
    fn nothing_there_is_missing() {
        let (files, markers) = code();
        assert_eq!(resolve(&id(3), Some(&at("a.py", "gone")), &markers, exists(&files)), Resolved::Missing);
        assert_eq!(resolve(&id(3), None, &markers, exists(&files)), Resolved::Missing);
    }

    #[test]
    fn an_id_in_two_places_is_the_one_at_the_anchor_else_the_first() {
        let (files, markers) = code();
        assert_eq!(resolve(&id(1), Some(&at("b.py", "copy")), &markers, exists(&files)), Resolved::Found { at: at("b.py", "copy"), by: FoundBy::Marker });
        assert_eq!(resolve(&id(1), Some(&at("x.py", "y")), &markers, exists(&files)), Resolved::Found { at: at("a.py", "f"), by: FoundBy::Marker }, "a.py comes before b.py");
    }

    #[test]
    fn an_old_uuid_is_not_a_marker() {
        let mut files = BTreeMap::new();
        files.insert(RepoPath::new("a.py").unwrap(), vec![Def { id: Some("e4efd15c-958e-47c5-abb3-7ffd7c7920da".into()), ..def("main", None) }]);
        let markers = read(&files);
        // the anchor still finds the node: an old UUID there does not block it
        assert_eq!(resolve(&id(3), Some(&at("a.py", "main")), &markers, exists(&files)), Resolved::Found { at: at("a.py", "main"), by: FoundBy::Anchor });
    }

    #[test]
    fn a_file_and_a_folder_are_found_by_their_markers_wherever_they_went() {
        let files: BTreeMap<_, _> = [(RepoPath::new("new/place.py").unwrap(), vec![def("f", None)])].into_iter().collect();
        let file_ids = BTreeMap::from([(RepoPath::new("new/place.py").unwrap(), id(1).to_string())]);
        let folder_ids = BTreeMap::from([(RepoPath::new("new").unwrap(), id(2))]);
        let markers = Markers::read(&files, &file_ids, &folder_ids);
        let exists = |_: &LocationKey| false;
        assert_eq!(resolve(&id(1), Some(&at("old/place.py", "")), &markers, exists), Resolved::Found { at: at("new/place.py", ""), by: FoundBy::Marker });
        assert_eq!(resolve(&id(2), Some(&at("old", "")), &markers, exists), Resolved::Found { at: at("new", ""), by: FoundBy::Marker });
    }

    #[test]
    fn a_file_without_a_marker_is_found_by_its_path_while_it_stays() {
        let files = BTreeMap::new();
        let markers = read(&files);
        assert_eq!(resolve(&id(1), Some(&at("a.py", "")), &markers, |l| l.path().as_str() == "a.py"), Resolved::Found { at: at("a.py", ""), by: FoundBy::Anchor });
        assert_eq!(resolve(&id(1), Some(&at("a.py", "")), &markers, |_| false), Resolved::Missing);
    }

    #[test]
    fn a_copied_folder_marker_leaves_the_node_at_its_anchor() {
        let folder_ids = BTreeMap::from([(RepoPath::new("billing").unwrap(), id(1)), (RepoPath::new("billing2").unwrap(), id(1))]);
        let markers = Markers::read(&BTreeMap::new(), &BTreeMap::new(), &folder_ids);
        assert_eq!(resolve(&id(1), Some(&at("billing2", "")), &markers, |_| true), Resolved::Found { at: at("billing2", ""), by: FoundBy::Marker });
        assert_eq!(resolve(&id(1), Some(&at("gone", "")), &markers, |_| true), Resolved::Found { at: at("billing", ""), by: FoundBy::Marker }, "else the first by path");
    }
}
