//! The index: everything V-NOC knows about the project right now, held in memory (`04-storage/02-index.md`).
//!
//! The fields are private on purpose: the methods below are the whole interface, so a SQLite copy can replace the storage later
//! without touching callers. They speak in domain words, never in storage words.
//!
//! Step 1 holds the outline of every file in scope. Folders' markers, `.vnoc/` records and resolution join it in later steps.

mod build;
mod tree;

use std::collections::{BTreeMap, BTreeSet};

use vnoc_domain::RepoPath;
use vnoc_driver_protocol::Def;

pub use build::{BuildError, Touched, refresh};
pub use tree::{Kind, Node};

use crate::protocol::FailedFile;

#[derive(Default)]
pub struct Index {
    /// +1 on every change; the UI uses it to know "my copy is old"
    revision: u64,
    /// every file in scope the driver could outline; sorted, so the tree comes out in order
    files: BTreeMap<RepoPath, Vec<Def>>,
    /// files in scope the driver could not outline, with its message
    failed: BTreeMap<RepoPath, String>,
    /// the scope had more files than the cap; the rest are not read
    truncated: bool,
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

    /// The tree the sidebar and the canvas draw. `prefix`: the opened folder's place in the repo.
    pub fn tree(&self, root_name: &str, prefix: &str) -> Node {
        tree::build(root_name, prefix, &self.files)
    }

    /// Whether the index already has an answer (an outline or an error) for this file.
    fn knows(&self, path: &RepoPath) -> bool {
        self.files.contains_key(path) || self.failed.contains_key(path)
    }

    /// Stores what the driver said about a file. Returns whether that differs from what was held.
    fn set_outline(&mut self, path: RepoPath, outcome: Result<Vec<Def>, String>) -> bool {
        match outcome {
            Ok(defs) => {
                let had_error = self.failed.remove(&path).is_some();
                self.files.insert(path, defs.clone()).is_none_or(|old| old != defs) || had_error
            }
            Err(error) => {
                let was_outlined = self.files.remove(&path).is_some();
                self.failed.insert(path, error.clone()).is_none_or(|old| old != error) || was_outlined
            }
        }
    }

    /// Forgets every file that is not in `keep`. Returns whether any was forgotten.
    fn retain_only(&mut self, keep: &BTreeSet<RepoPath>) -> bool {
        let before = self.files.len() + self.failed.len();
        self.files.retain(|p, _| keep.contains(p));
        self.failed.retain(|p, _| keep.contains(p));
        self.files.len() + self.failed.len() != before
    }

    fn set_truncated(&mut self, truncated: bool) -> bool {
        std::mem::replace(&mut self.truncated, truncated) != truncated
    }

    fn next_revision(&mut self) {
        self.revision += 1;
    }
}
