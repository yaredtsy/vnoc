//! Reading `.vnoc/nodes/*/*/` back: every node folder becomes a record, and a broken one becomes a problem, never a failure
//! (one bad file from a merge must not stop the other two hundred docs from loading).

use std::fs;
use std::path::Path;

use vnoc_domain::{NodeId, NodeToml};

use super::layout;

/// What is in one node folder.
pub struct NodeRecord {
    pub toml: NodeToml,
    /// file names of the `*.md` files, sorted
    pub docs: Vec<String>,
}

#[derive(Default)]
pub struct LoadReport {
    pub records: Vec<(NodeId, NodeRecord)>,
    /// one line each, for people: which file, what is wrong
    pub problems: Vec<String>,
}

pub fn load_all(root: &Path) -> LoadReport {
    let mut report = LoadReport::default();
    for shard in dirs_in(&layout::nodes_dir(root)) {
        for folder in dirs_in(&shard) {
            let name = folder.file_name().unwrap_or_default().to_string_lossy().into_owned();
            // folder names are IDs in canonical form: 26 characters, upper case
            let Some(id) = NodeId::parse(&name).ok().filter(|id| id.to_string() == name) else {
                report.problems.push(format!("{}: the folder name is not a node ID", folder.display()));
                continue;
            };
            let file = folder.join("node.toml");
            match fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|text| NodeToml::parse(&text).map_err(|e| e.to_string())) {
                Ok(toml) => report.records.push((id, NodeRecord { toml, docs: markdown_files(&folder) })),
                Err(e) => report.problems.push(format!("{}: {e}", file.display())),
            }
        }
    }
    report
}

/// The sub-folders of `dir` that are real folders (a symlink is not followed out of `.vnoc/`); none when `dir` is missing.
fn dirs_in(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut dirs: Vec<_> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    dirs.sort();
    dirs
}

fn markdown_files(folder: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(folder) else { return vec![] };
    let mut names: Vec<String> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".md")).collect();
    names.sort();
    names
}
