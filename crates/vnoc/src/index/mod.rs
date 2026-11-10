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
use crate::protocol::FailedFile;
use crate::scan::Folder;
use crate::userdata::{LoadReport, NodeRecord};

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
    /// `.vnoc-id` files that could not be read: one line each
    marker_problems: Vec<String>,
    /// files in scope the driver could not outline, with its message
    failed: BTreeMap<RepoPath, String>,
    /// the scope had more files than the cap; the rest are not read
    truncated: bool,
    /// every node folder in `.vnoc/nodes/`
    records: BTreeMap<NodeId, NodeRecord>,
    /// what was wrong in `.vnoc/` (a `node.toml` that does not parse…): one line each
    problems: Vec<String>,
    /// where each record was found, by location key text. Computed from `files` and `records` at the end of every update.
    attached: HashMap<String, Attached>,
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

    pub fn problems(&self) -> Vec<String> {
        self.problems.iter().chain(&self.marker_problems).cloned().collect()
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
                        problems.push(format!("{}/{}: {e}", f.path, crate::identity::folder_marker::FILE_NAME));
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
        for (id, record) in &self.records {
            let anchor = record.toml.location().ok();
            let found = resolve(id, anchor.as_ref(), &markers, |location| self.exists(location));
            if let Resolved::Found { at, by } = found {
                attached.insert(at.to_string(), Attached { id: *id, docs: record.docs.len(), by });
            }
        }
        self.attached = attached;
    }

    fn set_truncated(&mut self, truncated: bool) -> bool {
        std::mem::replace(&mut self.truncated, truncated) != truncated
    }

    fn next_revision(&mut self) {
        self.revision += 1;
    }
}
