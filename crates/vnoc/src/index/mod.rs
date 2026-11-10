//! The index: everything V-NOC knows about the project right now, held in memory (`04-storage/02-index.md`).
//!
//! The fields are private on purpose: the methods below are the whole interface, so a SQLite copy can replace the storage later
//! without touching callers. They speak in domain words, never in storage words.
//!
//! It holds the outline of every file in scope, the records read from `.vnoc/`, and which node each record is (resolved).
//! Files and folders are nodes too: a location with an empty qualname, found again by their `# FileID:` comment or `.vnoc-id` file.

mod build;
mod tree;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vnoc_domain::{LocationKey, NodeId, RepoPath, occurrences};
use vnoc_driver_protocol::Def;

pub use build::{BuildError, Touched, refresh};
pub use tree::{Kind, Node};

use crate::identity::{FoundBy, Markers, Resolved, resolve};
use crate::protocol::{FailedFile, MissingNode, Problem};
use crate::scan::Folder;
use crate::userdata::{LoadProblem, LoadReport, NodeRecord};

/// A documented node that was found: what the tree shows on it.
#[derive(Debug, Clone, Copy)]
pub struct Attached {
    pub id: NodeId,
    pub docs: usize,
    pub by: FoundBy,
}

impl Attached {
    /// What a tree node shows of it: the permanent ID, the doc count, and how it was found.
    fn shown(&self) -> (Option<String>, usize, Option<tree::Link>) {
        (Some(self.id.to_string()), self.docs, Some(if self.by == FoundBy::Marker { tree::Link::Marker } else { tree::Link::Name }))
    }
}

#[derive(Default)]
pub struct Index {
    /// +1 on every change; the UI uses it to know "my copy is old"
    revision: u64,
    /// every file in scope the driver could outline; sorted, so the tree comes out in order
    files: BTreeMap<RepoPath, Vec<Def>>,
    /// the ID text of each file's `# FileID:` comment, for the files that have one
    file_ids: BTreeMap<RepoPath, String>,
    /// every folder in scope, with the ID its `.vnoc-id` holds (`None`: no marker, or one that cannot be read)
    folders: BTreeMap<RepoPath, Option<NodeId>>,
    /// `.vnoc-id` files that could not be read: one line each, with the repo path of the file
    marker_problems: Vec<(String, String)>,
    /// files in scope the driver could not outline, with its message
    failed: BTreeMap<RepoPath, String>,
    /// the scope had more files than the cap; the rest are not read
    truncated: bool,
    /// every node folder in `.vnoc/nodes/`
    records: BTreeMap<NodeId, NodeRecord>,
    /// what was wrong in `.vnoc/` (a `node.toml` that does not parse…): one line each
    problems: Vec<LoadProblem>,
    /// where each record was found, by location key text. Computed from `files` and `records` at the end of every update.
    attached: HashMap<String, Attached>,
    /// records no code matches. Computed with `attached`.
    missing: Vec<MissingNode>,
    /// an ID written in more than one place: one entry per copy that is **not** the node. Computed with `attached`.
    duplicates: Vec<Duplicate>,
}

/// A copy of an ID that is not where the node was found. Nothing is changed for it: the user is told.
struct Duplicate {
    copy: LocationKey,
}

impl Index {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn failed(&self) -> Vec<FailedFile> {
        self.failed.iter().map(|(path, error)| FailedFile { path: path.to_string(), error: error.clone() }).collect()
    }

    /// Documented nodes the code does not match, sorted by path (then name).
    pub fn missing(&self) -> Vec<MissingNode> {
        self.missing.clone()
    }

    /// What the user should look at: broken files first, then IDs written twice. `git_root` makes the files absolute.
    pub fn problems(&self, git_root: &std::path::Path) -> Vec<Problem> {
        let absolute = |repo_path: &str| git_root.join(repo_path).to_string_lossy().into_owned();
        let broken = self.problems.iter().map(|p| Problem { text: p.text.clone(), file: Some(p.file.to_string_lossy().into_owned()), line: None });
        let markers = self.marker_problems.iter().map(|(text, file)| Problem { text: text.clone(), file: Some(absolute(file)), line: None });
        let duplicates = self.duplicates.iter().map(|d| {
            // a def is shown at its line; a file or a folder at its top (a folder has no file to open, so its marker is)
            let line = self.def_at(&d.copy).map(|def| def.range.start.line);
            let file = match (d.copy.qualname().is_empty(), self.is_folder(d.copy.path())) {
                (true, true) => format!("{}/{}", d.copy.path(), crate::identity::folder_marker::FILE_NAME),
                _ => d.copy.path().to_string(),
            };
            let text = format!("this ID is also at {}", place(&d.copy));
            Problem { text, file: Some(absolute(&file)), line }
        });
        broken.chain(markers).chain(duplicates).collect()
    }

    /// The tree the sidebar and the canvas draw. `prefix`: the opened folder's place in the repo.
    pub fn tree(&self, root_name: &str, prefix: &str) -> Node {
        tree::build(root_name, prefix, &self.files, &self.attached)
    }

    /// The def a location names, as outlined from the saved file. `None` when the file or the def is not in the index.
    pub fn def_at(&self, location: &LocationKey) -> Option<Def> {
        let defs = self.files.get(location.path())?;
        let which = occurrences(defs.iter().map(|d| d.qualname.as_str()));
        defs.iter().zip(which).find(|(d, n)| d.qualname == location.qualname() && *n == location.occurrence()).map(|(d, _)| d.clone())
    }

    /// Whether this path is a file in scope (outlined or not).
    pub fn is_file(&self, path: &RepoPath) -> bool {
        self.knows(path)
    }

    /// Whether this path is a folder in scope (the opened folder itself is not one).
    pub fn is_folder(&self, path: &RepoPath) -> bool {
        self.folders.contains_key(path)
    }

    /// The V-NOC ID written in the file's `# FileID:` comment, if it is one.
    pub fn file_id(&self, path: &RepoPath) -> Option<NodeId> {
        self.file_ids.get(path).and_then(|text| NodeId::parse(text).ok())
    }

    /// The ID in the folder's `.vnoc-id`, if there is a readable one.
    pub fn folder_id(&self, path: &RepoPath) -> Option<NodeId> {
        self.folders.get(path).copied().flatten()
    }

    /// Whether a location names something that is there now: a def, a file (empty qualname) or a folder.
    fn exists(&self, location: &LocationKey) -> bool {
        if location.qualname().is_empty() { self.knows(location.path()) || self.is_folder(location.path()) } else { self.def_at(location).is_some() }
    }

    /// Whether the index already has an answer (an outline or an error) for this file.
    fn knows(&self, path: &RepoPath) -> bool {
        self.files.contains_key(path) || self.failed.contains_key(path)
    }

    /// Stores what the driver said about a file. Returns whether that differs from what was held.
    fn set_outline(&mut self, path: RepoPath, outcome: Result<(Vec<Def>, Option<String>), String>) -> bool {
        match outcome {
            Ok((defs, file_id)) => {
                let had_error = self.failed.remove(&path).is_some();
                let old_id = match &file_id {
                    Some(id) => self.file_ids.insert(path.clone(), id.clone()),
                    None => self.file_ids.remove(&path),
                };
                let id_changed = old_id != file_id;
                self.files.insert(path, defs.clone()).is_none_or(|old| old != defs) || had_error || id_changed
            }
            Err(error) => {
                let was_outlined = self.files.remove(&path).is_some();
                let had_id = self.file_ids.remove(&path).is_some();
                self.failed.insert(path, error.clone()).is_none_or(|old| old != error) || was_outlined || had_id
            }
        }
    }

    /// Replaces the folders with what the walk found. Returns whether that differs.
    fn set_folders(&mut self, found: Vec<Folder>) -> bool {
        let mut problems = Vec::new();
        let folders: BTreeMap<RepoPath, Option<NodeId>> = found
            .into_iter()
            .map(|f| {
                let id = match f.marker {
                    Some(Ok(id)) => Some(id),
                    Some(Err(e)) => {
                        let file = format!("{}/{}", f.path, crate::identity::folder_marker::FILE_NAME);
                        problems.push((format!("{file}: {e}"), file));
                        None
                    }
                    None => None,
                };
                (f.path, id)
            })
            .collect();
        let changed = folders != self.folders || problems != self.marker_problems;
        self.folders = folders;
        self.marker_problems = problems;
        changed
    }

    /// Forgets every file that is not in `keep`. Returns whether any was forgotten.
    fn retain_only(&mut self, keep: &BTreeSet<RepoPath>) -> bool {
        let before = self.files.len() + self.failed.len();
        self.files.retain(|p, _| keep.contains(p));
        self.failed.retain(|p, _| keep.contains(p));
        self.file_ids.retain(|p, _| keep.contains(p));
        self.files.len() + self.failed.len() != before
    }

    /// Replaces the records with what `.vnoc/` holds now. Returns whether that differs.
    fn set_records(&mut self, report: LoadReport) -> bool {
        let records: BTreeMap<NodeId, NodeRecord> = report.records.into_iter().collect();
        let changed = records != self.records || report.problems != self.problems;
        self.records = records;
        self.problems = report.problems;
        changed
    }

    /// Finds every record again in the code. Call after `files` or `records` changed.
    fn resolve_all(&mut self) {
        let folder_ids = self.folders.iter().filter_map(|(path, id)| Some((path.clone(), (*id)?))).collect();
        let markers = Markers::read(&self.files, &self.file_ids, &folder_ids);
        let mut attached = HashMap::new();
        let mut missing = Vec::new();
        let mut duplicates = Vec::new();
        for (id, record) in &self.records {
            let anchor = record.toml.location().ok();
            match resolve(id, anchor.as_ref(), &markers, |location| self.exists(location)) {
                Resolved::Found { at, by } => {
                    if by == FoundBy::Marker {
                        duplicates.extend(markers.places(id).iter().filter(|copy| **copy != at).map(|copy| Duplicate { copy: copy.clone() }));
                    }
                    attached.insert(at.to_string(), Attached { id: *id, docs: record.docs.len(), by });
                }
                Resolved::Missing => {
                    // an unreadable anchor cannot be here: `NodeToml::parse` checked it
                    let Some(anchor) = anchor else { continue };
                    missing.push(MissingNode { id: id.to_string(), kind: record.toml.kind().map(|k| k.as_str().to_string()), path: anchor.path().to_string(), qualname: anchor.qualname().to_string(), docs: record.docs.len() });
                }
            }
        }
        missing.sort_by(|a, b| (&a.path, &a.qualname, &a.id).cmp(&(&b.path, &b.qualname, &b.id)));
        self.attached = attached;
        self.missing = missing;
        self.duplicates = duplicates;
    }

    fn set_truncated(&mut self, truncated: bool) -> bool {
        std::mem::replace(&mut self.truncated, truncated) != truncated
    }

    fn next_revision(&mut self) {
        self.revision += 1;
    }
}

/// A location for people: `src/orders.py::parse_order`, or the bare path for a file or a folder.
fn place(location: &LocationKey) -> String {
    if location.qualname().is_empty() { location.path().to_string() } else { format!("{}::{}", location.path(), location.qualname()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_domain::{NodeKind, NodeToml};
    use vnoc_driver_protocol::{DefKind, Position, Range};

    fn id(n: u8) -> NodeId {
        NodeId::parse(&format!("0000000000000000000000000{n}")).unwrap()
    }

    fn path(p: &str) -> RepoPath {
        RepoPath::new(p).unwrap()
    }

    fn at(p: &str, qualname: &str) -> LocationKey {
        LocationKey::new(path(p), qualname, 1).unwrap()
    }

    fn def(qualname: &str, line: u32, marker: Option<NodeId>) -> Def {
        let (start, end) = (Position { line, character: 0 }, Position { line: line + 1, character: 0 });
        Def { qualname: qualname.into(), name: qualname.into(), kind: DefKind::Function, parent: String::new(), parent_kind: None, range: Range { start, end }, params: vec![], id: marker.map(|m| m.to_string()), syntax_error: false }
    }

    fn record(at: &LocationKey, kind: Option<NodeKind>, docs: usize) -> NodeRecord {
        let mut toml = NodeToml::new(at);
        if let Some(kind) = kind {
            toml.set_kind(&kind);
        }
        NodeRecord { toml, docs: (0..docs).map(|n| format!("{n}.md")).collect() }
    }

    fn index(files: &[(&str, Vec<Def>)], records: Vec<(NodeId, NodeRecord)>) -> Index {
        let mut index = Index::default();
        index.files = files.iter().map(|(p, d)| (path(p), d.clone())).collect();
        index.records = records.into_iter().collect();
        index.resolve_all();
        index
    }

    #[test]
    fn a_record_nothing_matches_is_missing_with_its_stored_anchor_and_sorted_by_path() {
        let index = index(
            &[("a.py", vec![def("f", 0, None)])],
            vec![
                (id(1), record(&at("z.py", "gone"), Some(NodeKind::Function), 2)),
                (id(2), record(&at("a.py", "f"), None, 1)),
                (id(3), record(&at("b/old.py", "g"), None, 1)),
            ],
        );
        let missing = index.missing();
        assert_eq!(missing.iter().map(|m| (m.path.as_str(), m.qualname.as_str(), m.kind.as_deref(), m.docs)).collect::<Vec<_>>(), [("b/old.py", "g", None, 1), ("z.py", "gone", Some("function"), 2)]);
        assert_eq!(missing[1].id, id(1).to_string());
        assert_eq!(index.tree("r", "").counts()[3], 1, "the found one is in the tree, the missing ones are not");
    }

    #[test]
    fn an_id_in_two_places_is_one_problem_and_the_winner_does_not_change() {
        let files = [("a.py", vec![def("f", 3, Some(id(1)))]), ("copy.py", vec![def("x", 0, None), def("parse_order", 7, Some(id(1)))])];
        // the anchor says a.py::f, so a.py wins
        let at_anchor = index(&files, vec![(id(1), record(&at("a.py", "f"), None, 1))]);
        let problems = at_anchor.problems(std::path::Path::new("/repo"));
        assert_eq!(problems, [Problem { text: "this ID is also at copy.py::parse_order".into(), file: Some("/repo/copy.py".into()), line: Some(7) }]);
        assert_eq!(at_anchor.attached.get("a.py::f").map(|a| a.id), Some(id(1)));

        // the anchor is gone: the first by path still wins, and it is the same winner
        let no_anchor = index(&files, vec![(id(1), record(&at("old.py", "f"), None, 1))]);
        assert_eq!(no_anchor.attached.keys().collect::<Vec<_>>(), ["a.py::f"]);
        assert_eq!(no_anchor.problems(std::path::Path::new("/repo")).len(), 1);

        // an ID written once is not a problem
        assert!(index(&files[..1], vec![(id(1), record(&at("a.py", "f"), None, 1))]).problems(std::path::Path::new("/repo")).is_empty());
    }

    #[test]
    fn broken_files_are_problems_with_the_file_to_open() {
        let mut index = Index::default();
        index.problems.push(LoadProblem { text: "x/node.toml: bad".into(), file: "/repo/.vnoc/nodes/ab/X/node.toml".into() });
        index.marker_problems.push(("pkg/.vnoc-id: bad".into(), "pkg/.vnoc-id".into()));
        let problems = index.problems(std::path::Path::new("/repo"));
        assert_eq!(problems.iter().map(|p| p.file.as_deref()).collect::<Vec<_>>(), [Some("/repo/.vnoc/nodes/ab/X/node.toml"), Some("/repo/pkg/.vnoc-id")]);
    }
}
