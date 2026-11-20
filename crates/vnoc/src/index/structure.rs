//! The answer to `vnoc/structure`: the tree, the counts, and what the index holds besides (the failed, the Missing, the problems).
//!
//! A plain function over an index that is already built: it waits for nothing and does no I/O. The server waits for the first build and
//! asks the driver what it can do; a command will open the project and build the index, then call this.

use std::collections::BTreeMap;

use super::Index;
use crate::project::Project;
use crate::protocol::{Stats, StructureResult};

pub fn structure(
    index: &Index,
    project: &Project,
    capabilities: BTreeMap<String, bool>,
) -> StructureResult {
    let root = index.tree(&project.name(), project.prefix());
    let [folders, files, classes, functions] = root.counts();
    StructureResult {
        revision: index.revision(),
        git_root: project.git_root().to_string_lossy().into_owned(),
        capabilities,
        root,
        stats: Stats {
            folders: folders.saturating_sub(1),
            files,
            classes,
            functions,
        }, // the opened folder is not a folder under itself
        truncated: index.truncated(),
        failed: index.failed(),
        missing: index.missing(),
        problems: index.problems(project.git_root()),
        lost_markers: index.lost_markers(),
    }
}
