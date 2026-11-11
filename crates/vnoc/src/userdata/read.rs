//! Reading `.vnoc/nodes/*/*/` back: every node folder becomes a record, and a broken one becomes a problem, never a failure
//! (one bad file from a merge must not stop the other two hundred docs from loading).

use std::fs;
use std::path::Path;

use vnoc_domain::{NodeId, NodeToml};

use super::layout;

/// What is in one node folder.
#[derive(Debug)]
pub struct NodeRecord {
    pub toml: NodeToml,
    /// file names of the `*.md` files, sorted
    pub docs: Vec<String>,
}

/// Two records are the same when their `node.toml` text and their docs are.
impl PartialEq for NodeRecord {
    fn eq(&self, other: &NodeRecord) -> bool {
        self.docs == other.docs && self.toml.to_string() == other.toml.to_string()
    }
}

/// Something wrong in `.vnoc/`: a line for people, and the file to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    pub text: String,
    /// absolute path of the broken file or folder
    pub file: std::path::PathBuf,
}

#[derive(Default)]
pub struct LoadReport {
    pub records: Vec<(NodeId, NodeRecord)>,
    /// one line each, for people: which file, what is wrong
    pub problems: Vec<LoadProblem>,
}

pub fn load_all(root: &Path) -> LoadReport {
    let mut report = LoadReport::default();
    for shard in dirs_in(&layout::nodes_dir(root)) {
        for folder in dirs_in(&shard) {
            let name = folder.file_name().unwrap_or_default().to_string_lossy().into_owned();
            // folder names are IDs in canonical form: 26 characters, upper case
            let Some(id) = NodeId::parse(&name).ok().filter(|id| id.to_string() == name) else {
                report.problems.push(LoadProblem { text: format!("{}: the folder name is not a node ID", shown(root, &folder)), file: folder.clone() });
                continue;
            };
            let file = folder.join("node.toml");
            match fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|text| NodeToml::parse(&text).map_err(|e| e.to_string())) {
                Ok(toml) => report.records.push((id, NodeRecord { toml, docs: markdown_files(&folder) })),
                Err(e) => report.problems.push(LoadProblem { text: format!("{}: {e}", shown(root, &file)), file: file.clone() }),
            }
        }
    }
    report
}

/// A path for people: relative to the repo (`.vnoc/nodes/ab/ID/node.toml`), never the absolute one.
fn shown(root: &Path, path: &Path) -> String {
    let inside = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
    format!("{}/{inside}", layout::FOLDER)
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

/// One doc of a node, as the UI lists it.
pub struct DocInfo {
    /// the file name inside the node folder
    pub file: String,
    /// the first `# heading`, else the file name
    pub title: String,
    pub path: std::path::PathBuf,
}

pub fn docs_of(root: &Path, id: &NodeId) -> Vec<DocInfo> {
    let folder = layout::node_dir(root, id);
    markdown_files(&folder)
        .into_iter()
        .map(|file| {
            let path = folder.join(&file);
            let title = fs::read_to_string(&path).ok().and_then(|text| title_of(&text)).unwrap_or_else(|| file.trim_end_matches(".md").to_string());
            DocInfo { file, title, path }
        })
        .collect()
}

/// The first `# heading` after the frontmatter.
fn title_of(text: &str) -> Option<String> {
    let body = match text.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")) {
        Some((_, body)) => body,
        None => text,
    };
    body.lines().find_map(|line| line.strip_prefix("# ")).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::title_of;

    #[test]
    fn the_title_is_the_first_heading_after_the_frontmatter() {
        assert_eq!(title_of("---\nnode: X\nhint: a::b\n---\n# Why we parse twice\n\ntext").as_deref(), Some("Why we parse twice"));
        assert_eq!(title_of("no frontmatter\n# Here").as_deref(), Some("Here"));
        assert_eq!(title_of("---\nhint: # not a title\n---\ntext only"), None);
        assert_eq!(title_of("## second level only"), None);
    }
}
