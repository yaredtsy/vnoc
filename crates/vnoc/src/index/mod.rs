//! The index: everything V-NOC knows about the project right now, held in memory (`04-storage/02-index.md`).
//!
//! The fields are private on purpose: the methods below are the whole interface, so a SQLite copy can replace the storage later
//! without touching callers. They speak in domain words, never in storage words.
//!
//! It holds the outline of every file in scope, the records read from `.vnoc/`, and which node each record is (resolved).
//! Files and folders are nodes too: a location with an empty qualname, found again by their `# FileID:` comment or `.vnoc-id` file.

mod build;
mod structure;
pub(crate) mod tree;
mod view;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use vnoc_domain::{LocationKey, NodeId, RepoPath, occurrences};
use vnoc_driver_protocol::{Def, DefKind};

pub use build::{BuildError, Touched, refresh};
pub use structure::structure;
pub use tree::{Kind, Node};
pub use view::{Counts, KindSel, Lookups, Row, TreeOptions, View, tree_view};

use crate::identity::{FoundBy, Markers, Resolved, resolve};
use crate::protocol::{FailedFile, LostMarker, MissingNode, Problem};
use crate::scan::Folder;
use crate::userdata::{DocTexts, LoadProblem, LoadReport, NodeRecord};

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
        (
            Some(self.id.to_string()),
            self.docs,
            Some(if self.by == FoundBy::Marker {
                tree::Link::Marker
            } else {
                tree::Link::Name
            }),
        )
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
    /// anchor paths of records that exist on disk although the scan did not read them (outside the opened folder, git-ignored, past the
    /// cap, a file the driver failed on). Looked up by `refresh` off the lock; see "Missing means looked for" in `docs/11`.
    on_disk: BTreeSet<RepoPath>,
    /// records no code matches. Computed with `attached`.
    missing: Vec<MissingNode>,
    /// an ID written in more than one place: one entry per copy that is **not** the node. Computed with `attached`.
    duplicates: Vec<Duplicate>,
    /// the text of every doc, for search. Its own `generation`, never the revision: an edited body is not a change to the code or the records (Q2).
    doc_texts: Arc<DocTexts>,
}

/// A copy of an ID that is not where the node was found. Nothing is changed for it: the user is told.
struct Duplicate {
    /// where the node is shown
    winner: LocationKey,
    copy: LocationKey,
}

impl Index {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The doc texts as of the last update. The `Arc` is cheap to take: the disk work on it (`refresh`) is done off the index lock.
    pub fn doc_texts(&self) -> Arc<DocTexts> {
        self.doc_texts.clone()
    }

    /// How many docs there are, and on how many distinct nodes. Every doc the records list counts, **also those of a Missing node**: a doc whose
    /// code is gone is still a doc of the project (`missing` says how many nodes are in that state).
    pub fn doc_counts(&self) -> (usize, usize) {
        let texts = self.doc_texts();
        let nodes: BTreeSet<&NodeId> = texts.iter().map(|(id, _, _)| id).collect();
        (texts.len(), nodes.len())
    }

    /// The generation of [`Index::doc_texts`]: +1 when a doc was added, dropped or read again. Not the revision.
    pub fn doc_generation(&self) -> u64 {
        self.doc_texts.generation()
    }

    /// Stores the doc texts of an update. Returns whether the generation moved; **that is not part of `changed`** (an update that only changed doc
    /// bodies announces nothing, Q2).
    fn set_doc_texts(&mut self, texts: Arc<DocTexts>) -> bool {
        let moved = texts.generation() != self.doc_texts.generation();
        self.doc_texts = texts;
        moved
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn failed(&self) -> Vec<FailedFile> {
        self.failed
            .iter()
            .map(|(path, error)| FailedFile {
                path: path.to_string(),
                error: error.clone(),
            })
            .collect()
    }

    /// What the driver said about a file it could not outline.
    pub(crate) fn failure_of(&self, path: &RepoPath) -> Option<&String> {
        self.failed.get(path)
    }

    /// Documented nodes the code does not match, sorted by path (then name).
    pub fn missing(&self) -> Vec<MissingNode> {
        self.missing.clone()
    }

    /// The `node.toml` of the record with this ID, as the last update read it.
    pub fn record_toml(&self, id: &NodeId) -> Option<&vnoc_domain::NodeToml> {
        self.records.get(id).map(|r| &r.toml)
    }

    /// The documented node found at this location, if any (by its ID in the code or by its stored place).
    pub fn node_at(&self, location: &LocationKey) -> Option<NodeId> {
        self.attached.get(&location.to_string()).map(|a| a.id)
    }

    /// Whether this node is in the Missing list (looked for and not found).
    pub fn is_missing(&self, id: &NodeId) -> bool {
        let id = id.to_string();
        self.missing.iter().any(|m| m.id == id)
    }

    /// What the user should look at: broken files first, then IDs written twice. `git_root` makes the files absolute.
    pub fn problems(&self, git_root: &std::path::Path) -> Vec<Problem> {
        let mut all = self.broken(git_root);
        all.extend(self.duplicate_ids(git_root));
        all
    }

    /// The `.vnoc/` files that do not parse and the `.vnoc-id` files that cannot be read (what `problems` lists first).
    pub fn broken(&self, git_root: &std::path::Path) -> Vec<Problem> {
        let absolute = |repo_path: &str| git_root.join(repo_path).to_string_lossy().into_owned();
        let broken = self.problems.iter().map(|p| Problem {
            text: p.text.clone(),
            file: Some(p.file.to_string_lossy().into_owned()),
            line: None,
        });
        let markers = self.marker_problems.iter().map(|(text, file)| Problem {
            text: text.clone(),
            file: Some(absolute(file)),
            line: None,
        });
        broken.chain(markers).collect()
    }

    /// An ID written in two places: one problem per copy that is not the node (what `problems` lists last).
    pub fn duplicate_ids(&self, git_root: &std::path::Path) -> Vec<Problem> {
        let absolute = |repo_path: &str| git_root.join(repo_path).to_string_lossy().into_owned();
        self.duplicates
            .iter()
            .map(|d| {
                // a def is shown at its line; a file or a folder at its top (a folder has no file to open, so its marker is)
                let line = self.def_at(&d.copy).map(|def| def.range.start.line);
                let file = match (d.copy.qualname().is_empty(), self.is_folder(d.copy.path())) {
                    (true, true) => format!(
                        "{}/{}",
                        d.copy.path(),
                        crate::identity::folder_marker::FILE_NAME
                    ),
                    _ => d.copy.path().to_string(),
                };
                let text = format!("{}: its ID is also at {}", place(&d.winner), place(&d.copy));
                Problem {
                    text,
                    file: Some(absolute(&file)),
                    line,
                }
            })
            .collect()
    }

    /// Nodes whose `ID:` line was lost: `node.toml` says the ID is in the code (`id_in_code = "yes"`) and the node was found by its anchor, not by a
    /// marker. A record that never carried an ID (`no`, `never`, or no field) is not lost: it was never in the code. Sorted by path, then name.
    pub fn lost_markers(&self) -> Vec<LostMarker> {
        self.found_by_name(vnoc_domain::IdInCode::Yes)
    }

    /// Nodes attached by name **on purpose**: `node.toml` says `id_in_code = "no"` (the project did not write IDs then, or the marker could not be
    /// written) and the node is found by its anchor. What the extension's *Write ID* is for; `vnoc repair --all` offers them. Sorted as
    /// [`Index::lost_markers`].
    pub fn attached_by_name(&self) -> Vec<LostMarker> {
        self.found_by_name(vnoc_domain::IdInCode::No)
    }

    /// The nodes found by their anchor whose record says `id_in_code` is `want`.
    fn found_by_name(&self, want: vnoc_domain::IdInCode) -> Vec<LostMarker> {
        let mut lost: Vec<LostMarker> = self
            .attached
            .iter()
            .filter(|(_, a)| a.by == FoundBy::Anchor)
            .filter(|(_, a)| {
                self.records
                    .get(&a.id)
                    .is_some_and(|r| r.toml.id_in_code().as_ref() == Some(&want))
            })
            .filter_map(|(key, a)| {
                let location: LocationKey = key.parse().ok()?;
                let kind = if location.qualname().is_empty() {
                    if self.is_folder(location.path()) {
                        Kind::Folder
                    } else {
                        Kind::File
                    }
                } else {
                    match self.def_at(&location)?.kind {
                        DefKind::Class => Kind::Class,
                        DefKind::Function => Kind::Function,
                    }
                };
                Some(LostMarker {
                    id: a.id.to_string(),
                    key: place(&location),
                    kind,
                    path: location.path().to_string(),
                    qualname: location.qualname().to_string(),
                })
            })
            .collect();
        lost.sort_by(|a, b| (&a.path, &a.qualname, &a.id).cmp(&(&b.path, &b.qualname, &b.id)));
        lost
    }

    /// The tree the sidebar and the canvas draw. `prefix`: the opened folder's place in the repo.
    pub fn tree(&self, root_name: &str, prefix: &str) -> Node {
        tree::build(root_name, prefix, &self.files, &self.attached)
    }

    /// The def a location names, as outlined from the saved file. `None` when the file or the def is not in the index.
    pub fn def_at(&self, location: &LocationKey) -> Option<Def> {
        let defs = self.files.get(location.path())?;
        let which = occurrences(defs.iter().map(|d| d.qualname.as_str()));
        defs.iter()
            .zip(which)
            .find(|(d, n)| d.qualname == location.qualname() && *n == location.occurrence())
            .map(|(d, _)| d.clone())
    }

    /// The innermost def of `path` that holds the 0-based line `line0`: of the defs whose range holds it, the one that starts last, and on a
    /// tie the one that ends first. Its occurrence is counted as `def_at` counts it, so a redefinition is `#2`. `None` when no def holds the line.
    pub fn innermost_def(&self, path: &RepoPath, line0: u32) -> Option<LocationKey> {
        let defs = self.files.get(path)?;
        let which = occurrences(defs.iter().map(|d| d.qualname.as_str()));
        let (def, occurrence) = defs
            .iter()
            .zip(which)
            .filter(|(d, _)| d.range.start.line <= line0 && line0 <= d.range.end.line)
            .max_by(|(a, _), (b, _)| {
                let (a, b) = (a.range, b.range);
                (a.start.line, a.start.character)
                    .cmp(&(b.start.line, b.start.character))
                    .then((b.end.line, b.end.character).cmp(&(a.end.line, a.end.character)))
            })?;
        LocationKey::new(path.clone(), def.qualname.clone(), occurrence).ok()
    }

    /// Where the index found the node with this permanent ID: a def's location, or a file's or folder's (an empty qualname). `None` when it was
    /// found nowhere (it may be Missing). `attached` is keyed by place, so this scans it: one lookup per query.
    pub fn found_at(&self, id: &NodeId) -> Option<LocationKey> {
        self.attached
            .iter()
            .find(|(_, a)| a.id == *id)
            .and_then(|(key, _)| key.parse().ok())
    }

    /// The paths of the files the driver outlined.
    fn outlined_paths(&self) -> impl Iterator<Item = &RepoPath> {
        self.files.keys()
    }

    /// How many outlined files have one of these extensions (without the dot).
    pub fn files_with_extensions(&self, extensions: &[String]) -> usize {
        self.outlined_paths()
            .filter(|p| {
                crate::driver_host::extension_of(p.as_str())
                    .is_some_and(|e| extensions.iter().any(|x| x == e))
            })
            .count()
    }

    /// Whether this path is a file in scope (outlined or not).
    pub fn is_file(&self, path: &RepoPath) -> bool {
        self.knows(path)
    }

    /// The files in scope whose last path part is `name`, outlined or not.
    pub fn files_named(&self, name: &str) -> Vec<RepoPath> {
        self.files
            .keys()
            .chain(self.failed.keys())
            .filter(|p| p.as_str().rsplit('/').next() == Some(name))
            .cloned()
            .collect()
    }

    /// Whether this path is a folder in scope (the opened folder itself is not one).
    pub fn is_folder(&self, path: &RepoPath) -> bool {
        self.folders.contains_key(path)
    }

    /// The V-NOC ID written in the file's `# FileID:` comment, if it is one.
    pub fn file_id(&self, path: &RepoPath) -> Option<NodeId> {
        self.file_ids
            .get(path)
            .and_then(|text| NodeId::parse(text).ok())
    }

    /// The ID text of the file's `# FileID:` comment exactly as the driver reported it (`file_id` has it read as an ID, which writes it in capitals):
    /// the text a tour leaves its marker line out by (`tours/hash.rs`).
    pub fn file_id_text(&self, path: &RepoPath) -> Option<&str> {
        self.file_ids.get(path).map(String::as_str)
    }

    /// The ID in the folder's `.vnoc-id`, if there is a readable one.
    pub fn folder_id(&self, path: &RepoPath) -> Option<NodeId> {
        self.folders.get(path).copied().flatten()
    }

    /// An index of these files, files the driver failed on (with its message) and folders, for tests that need one without a driver.
    #[cfg(test)]
    pub(crate) fn for_tests(
        files: Vec<(RepoPath, Vec<Def>)>,
        failed: Vec<(RepoPath, &str)>,
        folders: Vec<RepoPath>,
    ) -> Index {
        Index {
            files: files.into_iter().collect(),
            failed: failed
                .into_iter()
                .map(|(p, m)| (p, m.to_string()))
                .collect(),
            folders: folders.into_iter().map(|f| (f, None)).collect(),
            ..Default::default()
        }
    }

    /// Whether a location names something that is there now: a def, a file (empty qualname) or a folder.
    fn exists(&self, location: &LocationKey) -> bool {
        if location.qualname().is_empty() {
            self.knows(location.path()) || self.is_folder(location.path())
        } else {
            self.def_at(location).is_some()
        }
    }

    /// Whether the index already has an answer (an outline or an error) for this file.
    fn knows(&self, path: &RepoPath) -> bool {
        self.files.contains_key(path) || self.failed.contains_key(path)
    }

    /// Stores what the driver said about a file. Returns whether that differs from what was held.
    fn set_outline(
        &mut self,
        path: RepoPath,
        outcome: Result<(Vec<Def>, Option<String>), String>,
    ) -> bool {
        match outcome {
            Ok((defs, file_id)) => {
                let had_error = self.failed.remove(&path).is_some();
                let old_id = match &file_id {
                    Some(id) => self.file_ids.insert(path.clone(), id.clone()),
                    None => self.file_ids.remove(&path),
                };
                let id_changed = old_id != file_id;
                self.files
                    .insert(path, defs.clone())
                    .is_none_or(|old| old != defs)
                    || had_error
                    || id_changed
            }
            Err(error) => {
                let was_outlined = self.files.remove(&path).is_some();
                let had_id = self.file_ids.remove(&path).is_some();
                self.failed
                    .insert(path, error.clone())
                    .is_none_or(|old| old != error)
                    || was_outlined
                    || had_id
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
                        let file =
                            format!("{}/{}", f.path, crate::identity::folder_marker::FILE_NAME);
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
        let folder_ids = self
            .folders
            .iter()
            .filter_map(|(path, id)| Some((path.clone(), (*id)?)))
            .collect();
        let markers = Markers::read(&self.files, &self.file_ids, &folder_ids);
        let mut attached = HashMap::new();
        let scanned: BTreeSet<String> = self
            .files
            .keys()
            .chain(self.folders.keys())
            .map(|p| p.as_str().to_lowercase())
            .collect();
        let mut missing = Vec::new();
        let mut duplicates = Vec::new();
        for (id, record) in &self.records {
            let anchor = record.toml.location().ok();
            match resolve(id, anchor.as_ref(), &markers, |location| {
                self.exists(location)
            }) {
                Resolved::Found { at, by } => {
                    if by == FoundBy::Marker {
                        duplicates.extend(
                            markers
                                .places(id)
                                .iter()
                                .filter(|copy| **copy != at)
                                .map(|copy| Duplicate {
                                    winner: at.clone(),
                                    copy: copy.clone(),
                                }),
                        );
                    }
                    attached.insert(
                        at.to_string(),
                        Attached {
                            id: *id,
                            docs: record.docs.len(),
                            by,
                        },
                    );
                }
                Resolved::Missing => {
                    // an unreadable anchor cannot be here: `NodeToml::parse` checked it
                    let Some(anchor) = anchor else { continue };
                    if !self.looked_for(anchor.path(), &scanned) {
                        continue;
                    }
                    missing.push(MissingNode {
                        id: id.to_string(),
                        kind: record.toml.kind().map(|k| k.as_str().to_string()),
                        path: anchor.path().to_string(),
                        qualname: anchor.qualname().to_string(),
                        docs: record.docs.len(),
                    });
                }
            }
        }
        missing.sort_by(|a, b| (&a.path, &a.qualname, &a.id).cmp(&(&b.path, &b.qualname, &b.id)));
        self.attached = attached;
        self.missing = missing;
        self.duplicates = duplicates;
    }

    /// Whether V-NOC looked for a node with this anchor path: the path was scanned (an outline, or a folder the walk found; a path that
    /// differs only by case counts, as the old spelling of a renamed file still "exists" on macOS and Windows), or it is gone from disk.
    /// `scanned`: the lower-cased paths of `files` and `folders`.
    fn looked_for(&self, path: &RepoPath, scanned: &BTreeSet<String>) -> bool {
        scanned.contains(&path.as_str().to_lowercase()) || !self.on_disk.contains(path)
    }

    /// The anchor paths `refresh` found on disk. Returns whether that differs.
    fn set_on_disk(&mut self, on_disk: BTreeSet<RepoPath>) -> bool {
        let changed = self.on_disk != on_disk;
        self.on_disk = on_disk;
        changed
    }

    fn set_truncated(&mut self, truncated: bool) -> bool {
        std::mem::replace(&mut self.truncated, truncated) != truncated
    }

    fn next_revision(&mut self) {
        self.revision += 1;
    }
}

/// A location for people: `src/orders.py::parse_order` (`#2` on the second of two same-named defs), or the bare path for a file or a folder.
fn place(location: &LocationKey) -> String {
    if location.qualname().is_empty() {
        location.path().to_string()
    } else {
        location.to_string()
    }
}

/// What a record's anchor path is: the paths `refresh` must look up on disk are the anchors of `records` that the scan did not read.
pub fn anchors_to_check(
    records: &[(NodeId, NodeRecord)],
    read: &BTreeSet<RepoPath>,
) -> BTreeSet<RepoPath> {
    records
        .iter()
        .filter_map(|(_, r)| r.toml.location().ok())
        .map(|l| l.path().clone())
        .filter(|p| !read.contains(p))
        .collect()
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
        let (start, end) = (
            Position { line, character: 0 },
            Position {
                line: line + 1,
                character: 0,
            },
        );
        Def {
            qualname: qualname.into(),
            name: qualname.into(),
            kind: DefKind::Function,
            parent: String::new(),
            parent_kind: None,
            range: Range { start, end },
            params: vec![],
            id: marker.map(|m| m.to_string()),
            syntax_error: false,
        }
    }

    fn record(at: &LocationKey, kind: Option<NodeKind>, docs: usize) -> NodeRecord {
        let mut toml = NodeToml::new(at);
        if let Some(kind) = kind {
            toml.set_kind(&kind);
        }
        NodeRecord {
            toml,
            docs: (0..docs).map(|n| format!("{n}.md")).collect(),
        }
    }

    fn index(files: &[(&str, Vec<Def>)], records: Vec<(NodeId, NodeRecord)>) -> Index {
        let mut index = Index {
            files: files.iter().map(|(p, d)| (path(p), d.clone())).collect(),
            records: records.into_iter().collect(),
            ..Default::default()
        };
        index.resolve_all();
        index
    }

    /// A def over lines `start..=end` (0-based), with its qualname.
    fn span(qualname: &str, start: u32, end: u32) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            range: Range {
                start: at(start),
                end: at(end),
            },
            ..def(qualname, 0, None)
        }
    }

    #[test]
    fn the_innermost_def_holds_the_line_and_starts_last() {
        // outer 0..=9, inner 2..=5, deepest 3..=4, then a second `outer` 12..=13 (a redefinition), and `same` twice on one range
        let index = index(
            &[(
                "a.py",
                vec![
                    span("outer", 0, 9),
                    span("outer.inner", 2, 5),
                    span("outer.inner.deepest", 3, 4),
                    span("outer", 12, 13),
                    span("long", 20, 30),
                    span("long.short", 20, 22),
                ],
            )],
            vec![],
        );
        let at = |line| {
            index
                .innermost_def(&path("a.py"), line)
                .map(|l| l.to_string())
        };
        assert_eq!(at(0).as_deref(), Some("a.py::outer"));
        assert_eq!(at(2).as_deref(), Some("a.py::outer.inner"));
        assert_eq!(
            at(4).as_deref(),
            Some("a.py::outer.inner.deepest"),
            "the inner def wins"
        );
        assert_eq!(at(5).as_deref(), Some("a.py::outer.inner"));
        assert_eq!(at(9).as_deref(), Some("a.py::outer"));
        assert_eq!(at(10), None, "a line between defs");
        assert_eq!(at(11), None);
        assert_eq!(
            at(12).as_deref(),
            Some("a.py::outer#2"),
            "a redefinition is #2"
        );
        assert_eq!(at(13).as_deref(), Some("a.py::outer#2"));
        assert_eq!(at(14), None);
        assert_eq!(at(999), None, "past the end");
        // two defs start on the same line: the one that starts later in it, else the one that ends first
        assert_eq!(
            at(20).as_deref(),
            Some("a.py::long.short"),
            "same start: the one that ends first"
        );
        assert_eq!(at(25).as_deref(), Some("a.py::long"));
        assert_eq!(
            index.innermost_def(&path("other.py"), 0),
            None,
            "a file the index does not have"
        );
    }

    #[test]
    fn the_innermost_def_of_two_that_start_on_one_line_is_the_one_that_starts_later_in_it() {
        let at = |line, character| Position { line, character };
        let mut a = span("a", 0, 0);
        a.range = Range {
            start: at(0, 0),
            end: at(0, 30),
        };
        let mut b = span("b", 0, 0);
        b.range = Range {
            start: at(0, 10),
            end: at(0, 20),
        };
        let index = index(&[("a.py", vec![a, b])], vec![]);
        assert_eq!(
            index.innermost_def(&path("a.py"), 0).unwrap().qualname(),
            "b"
        );
    }

    #[test]
    fn found_at_gives_where_an_id_was_found_and_nothing_for_an_id_not_found() {
        let index = index(
            &[("a.py", vec![def("f", 0, None), def("g", 5, None)])],
            vec![
                (id(1), record(&at("a.py", "g"), None, 1)),
                (id(2), record(&at("gone.py", "h"), None, 1)),
            ],
        );
        assert_eq!(index.found_at(&id(1)), Some(at("a.py", "g")));
        assert_eq!(
            index.found_at(&id(2)),
            None,
            "a Missing node was found nowhere"
        );
        assert!(index.is_missing(&id(2)));
        assert_eq!(
            index.found_at(&id(3)),
            None,
            "an ID the index does not know"
        );
    }

    #[test]
    fn found_at_gives_a_second_occurrence_and_a_file() {
        let second = LocationKey::new(path("a.py"), "f", 2).unwrap();
        let index = index(
            &[("a.py", vec![def("f", 0, None), def("f", 5, None)])],
            vec![
                (id(1), record(&second, None, 1)),
                (id(2), record(&at("a.py", ""), Some(NodeKind::File), 1)),
            ],
        );
        assert_eq!(index.found_at(&id(1)), Some(second));
        assert_eq!(index.found_at(&id(2)), Some(at("a.py", "")));
    }

    #[test]
    fn a_record_nothing_matches_is_missing_with_its_stored_anchor_and_sorted_by_path() {
        let index = index(
            &[("a.py", vec![def("f", 0, None)])],
            vec![
                (
                    id(1),
                    record(&at("z.py", "gone"), Some(NodeKind::Function), 2),
                ),
                (id(2), record(&at("a.py", "f"), None, 1)),
                (id(3), record(&at("b/old.py", "g"), None, 1)),
            ],
        );
        let missing = index.missing();
        assert_eq!(
            missing
                .iter()
                .map(|m| (
                    m.path.as_str(),
                    m.qualname.as_str(),
                    m.kind.as_deref(),
                    m.docs
                ))
                .collect::<Vec<_>>(),
            [
                ("b/old.py", "g", None, 1),
                ("z.py", "gone", Some("function"), 2)
            ]
        );
        assert_eq!(missing[1].id, id(1).to_string());
        assert_eq!(
            index.tree("r", "").counts()[3],
            1,
            "the found one is in the tree, the missing ones are not"
        );
    }

    /// A repo path made absolute the way `problems` does it: on Windows the join puts a `\` after the root (the editor opens either spelling).
    fn under_repo(repo_path: &str) -> String {
        std::path::Path::new("/repo")
            .join(repo_path)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn an_id_in_two_places_is_one_problem_and_the_winner_does_not_change() {
        let files = [
            ("a.py", vec![def("f", 3, Some(id(1)))]),
            (
                "copy.py",
                vec![def("x", 0, None), def("parse_order", 7, Some(id(1)))],
            ),
        ];
        // the anchor says a.py::f, so a.py wins
        let at_anchor = index(&files, vec![(id(1), record(&at("a.py", "f"), None, 1))]);
        let problems = at_anchor.problems(std::path::Path::new("/repo"));
        assert_eq!(
            problems,
            [Problem {
                text: "a.py::f: its ID is also at copy.py::parse_order".into(),
                file: Some(under_repo("copy.py")),
                line: Some(7)
            }]
        );
        assert_eq!(at_anchor.attached.get("a.py::f").map(|a| a.id), Some(id(1)));

        // the anchor is gone: the first by path still wins, and it is the same winner
        let no_anchor = index(&files, vec![(id(1), record(&at("old.py", "f"), None, 1))]);
        assert_eq!(no_anchor.attached.keys().collect::<Vec<_>>(), ["a.py::f"]);
        assert_eq!(no_anchor.problems(std::path::Path::new("/repo")).len(), 1);

        // an ID written once is not a problem
        assert!(
            index(
                &files[..1],
                vec![(id(1), record(&at("a.py", "f"), None, 1))]
            )
            .problems(std::path::Path::new("/repo"))
            .is_empty()
        );
    }

    #[test]
    fn a_copy_on_the_second_of_two_same_named_defs_ends_in_its_number() {
        let files = [
            ("a.py", vec![def("f", 3, Some(id(1)))]),
            ("copy.py", vec![def("g", 0, None), def("g", 5, Some(id(1)))]),
        ];
        let index = index(&files, vec![(id(1), record(&at("a.py", "f"), None, 1))]);
        assert_eq!(
            index.problems(std::path::Path::new("/repo"))[0].text,
            "a.py::f: its ID is also at copy.py::g#2"
        );
    }

    /// `a.py` is outlined; `bad.py` failed in the driver; `out/x.py` is outside the scan (or ignored, or past the cap): the same to the index.
    fn looked_for(anchor: &str, on_disk: &[&str], failed: &[&str]) -> bool {
        let mut index = index(
            &[("a.py", vec![def("f", 0, None)])],
            vec![(
                id(1),
                record(&at(anchor, "gone"), Some(NodeKind::Function), 1),
            )],
        );
        index.failed = failed
            .iter()
            .map(|p| (path(p), "boom".to_string()))
            .collect();
        index.folders.insert(path("pkg"), None);
        index.on_disk = on_disk.iter().map(|p| path(p)).collect();
        index.resolve_all();
        !index.missing().is_empty()
    }

    #[test]
    fn a_record_whose_code_was_not_scanned_is_listed_only_when_its_path_is_gone() {
        // outside the opened folder, git-ignored, past the cap: not read, on disk -> unknown; gone -> listed
        assert!(!looked_for("out/x.py", &["out/x.py"], &[]));
        assert!(looked_for("out/x.py", &[], &[]));
        // a file the driver failed on: nothing was looked for
        assert!(!looked_for("bad.py", &["bad.py"], &["bad.py"]));
        assert!(looked_for("bad.py", &[], &["bad.py"]));
        // a scanned file or folder: the ID and the name were looked for and are not there
        assert!(looked_for("a.py", &[], &[]));
        assert!(looked_for("pkg", &["pkg"], &[]));
    }

    #[test]
    fn a_path_that_differs_only_by_case_counts_as_scanned() {
        // `A.py` was renamed to `a.py`; on macOS the old spelling "exists", and the node must still show as Missing, not turn unknown
        assert!(looked_for("A.py", &["A.py"], &[]));
    }

    #[test]
    fn broken_files_are_problems_with_the_file_to_open() {
        let mut index = Index::default();
        index.problems.push(LoadProblem {
            text: "x/node.toml: bad".into(),
            file: "/repo/.vnoc/nodes/ab/X/node.toml".into(),
        });
        index
            .marker_problems
            .push(("pkg/.vnoc-id: bad".into(), "pkg/.vnoc-id".into()));
        let problems = index.problems(std::path::Path::new("/repo"));
        assert_eq!(
            problems.iter().map(|p| p.file.clone()).collect::<Vec<_>>(),
            [
                Some("/repo/.vnoc/nodes/ab/X/node.toml".to_string()),
                Some(under_repo("pkg/.vnoc-id"))
            ]
        );
    }

    /// The set of lost markers as (key, kind) pairs.
    fn lost(index: &Index) -> Vec<(String, Kind)> {
        index
            .lost_markers()
            .into_iter()
            .map(|l| (l.key, l.kind))
            .collect()
    }

    #[test]
    fn a_node_found_by_its_marker_is_not_lost() {
        let g = at("a.py", "g");
        // f has its ID in the code, though its record says `a.py::g` (it moved): found by its marker. g's record is at g and g has no ID: lost.
        let index = index(
            &[("a.py", vec![def("f", 0, Some(id(1))), def("g", 4, None)])],
            vec![
                (id(1), record(&g, Some(NodeKind::Function), 1)),
                (id(2), record(&g, Some(NodeKind::Function), 1)),
            ],
        );
        // id(1) is found at f by its marker; id(2) is found at g by its anchor, and its record says the ID is in the code
        assert_eq!(lost(&index), [("a.py::g".to_string(), Kind::Function)]);
        assert_eq!(index.lost_markers()[0].id, id(2).to_string());
    }

    #[test]
    fn found_by_name_with_the_id_in_code_is_lost_and_never_or_no_is_not() {
        let (f, g, h) = (at("a.py", "f"), at("a.py", "g"), at("a.py", "h"));
        let mut no = record(&g, Some(NodeKind::Function), 0);
        no.toml.set_id_in_code(vnoc_domain::IdInCode::No);
        let mut never = record(&h, Some(NodeKind::Function), 0);
        never.toml.set_id_in_code(vnoc_domain::IdInCode::Never);
        // a record that does not say (an older file, or a hand-written one)
        let silent = NodeRecord {
            toml: NodeToml::parse("path = \"a.py\"\nqualname = \"k\"\n").unwrap(),
            docs: vec![],
        };
        let index = index(
            &[(
                "a.py",
                vec![
                    def("f", 0, None),
                    def("g", 3, None),
                    def("h", 6, None),
                    def("k", 9, None),
                ],
            )],
            vec![
                (id(1), record(&f, Some(NodeKind::Function), 2)),
                (id(2), no),
                (id(3), never),
                (id(4), silent),
            ],
        );
        // only the record that says `yes`
        assert_eq!(lost(&index), [("a.py::f".to_string(), Kind::Function)]);
    }

    #[test]
    fn a_missing_node_is_not_a_lost_marker_and_a_class_and_a_file_are_named_for_what_they_are() {
        let file = at("a.py", "");
        let class = at("a.py", "K");
        let gone = at("a.py", "gone");
        let mut klass = def("K", 2, None);
        klass.kind = DefKind::Class;
        let index = index(
            &[("a.py", vec![def("f", 0, None), klass])],
            vec![
                (id(1), record(&file, Some(NodeKind::File), 1)),
                (id(2), record(&class, Some(NodeKind::Class), 1)),
                (id(3), record(&gone, Some(NodeKind::Function), 1)),
            ],
        );
        assert!(index.is_missing(&id(3)));
        assert_eq!(
            lost(&index),
            [
                ("a.py".to_string(), Kind::File),
                ("a.py::K".to_string(), Kind::Class)
            ],
            "the file is named by its path, sorted before the def; the Missing node is in `missing` only"
        );
        let first = &index.lost_markers()[0];
        assert_eq!((first.path.as_str(), first.qualname.as_str()), ("a.py", ""));
    }
}
