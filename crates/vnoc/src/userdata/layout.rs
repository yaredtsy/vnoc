//! Where things are inside `.vnoc/`. **The only file that knows the layout** (`04-storage/01-vnoc-folder.md` §2): everything else
//! asks for a place by what it is, never by building a path.

use std::path::{Path, PathBuf};

use vnoc_domain::NodeId;

/// The folder inside the git root.
pub const FOLDER: &str = ".vnoc";

pub fn project_file(root: &Path) -> PathBuf {
    root.join("vnoc.toml")
}

pub fn gitignore_file(root: &Path) -> PathBuf {
    root.join(".gitignore")
}

/// `nodes/<shard>/<id>/`, where the shard is the last two characters of the ID.
pub fn node_dir(root: &Path, id: &NodeId) -> PathBuf {
    nodes_dir(root).join(id.shard()).join(id.to_string())
}

pub fn nodes_dir(root: &Path) -> PathBuf {
    root.join("nodes")
}

pub fn node_file(root: &Path, id: &NodeId) -> PathBuf {
    node_dir(root, id).join("node.toml")
}

/// Where a removed node folder waits to be restored. Not committed (`local/` is ignored).
pub fn trash_dir(root: &Path) -> PathBuf {
    root.join("local").join("trash")
}

/// The trash folder of a removed node holds a copy of the node's path, so "is this ID taken" can look there too.
pub fn trashed(root: &Path, id: &NodeId) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(trash_dir(root)) else { return vec![] };
    let suffix = format!("_{id}");
    entries.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(&suffix)).map(|e| e.path()).collect()
}
