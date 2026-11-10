//! The opened folder, and where it sits in its git repository.
//!
//! IDs hash the path **relative to the git root**, so the same function gets the same key whether the user opened `repo` or
//! `repo/services/api` (review C1). The UI shows paths relative to the opened folder; this type converts between the two.

use std::io;
use std::path::{Path, PathBuf};

use vnoc_domain::{RepoPath, RepoPathError};

#[derive(Clone, Debug)]
pub struct Project {
    open_dir: PathBuf,
    /// the nearest folder at or above `open_dir` that holds a `.git` entry; `open_dir` itself when there is none
    git_root: PathBuf,
    /// `open_dir` relative to `git_root`, `/`-separated, no trailing slash; empty when they are the same folder
    prefix: String,
}

impl Project {
    pub fn open(dir: &Path) -> io::Result<Project> {
        if !dir.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} is not a folder", dir.display())));
        }
        let git_root = dir.ancestors().find(|d| d.join(".git").exists()).unwrap_or(dir);
        let relative = dir.strip_prefix(git_root).expect("an ancestor is a prefix");
        let prefix = relative.to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the folder path is not UTF-8"))?.replace('\\', "/");
        Ok(Project { open_dir: dir.to_path_buf(), git_root: git_root.to_path_buf(), prefix })
    }

    pub fn open_dir(&self) -> &Path {
        &self.open_dir
    }

    pub fn git_root(&self) -> &Path {
        &self.git_root
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The opened folder's name, for the root of the tree.
    pub fn name(&self) -> String {
        self.open_dir.file_name().map_or_else(|| self.open_dir.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
    }

    /// The repo path of a file given relative to the opened folder.
    pub fn repo_path(&self, scoped: &str) -> Result<RepoPath, RepoPathError> {
        if self.prefix.is_empty() { RepoPath::new(scoped) } else { RepoPath::new(format!("{}/{scoped}", self.prefix)) }
    }

    /// The repo path of an absolute path, or `None` when it is not under the opened folder.
    pub fn repo_path_of(&self, absolute: &Path) -> Option<RepoPath> {
        let scoped = absolute.strip_prefix(&self.open_dir).ok()?.to_str()?.replace('\\', "/");
        self.repo_path(&scoped).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-project-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn without_a_git_folder_the_opened_folder_is_the_root() {
        let d = fresh("nogit");
        let p = Project::open(&d).unwrap();
        assert_eq!((p.prefix(), p.git_root()), ("", d.as_path()));
        assert_eq!(p.repo_path("a/b.py").unwrap().as_str(), "a/b.py");
    }

    #[test]
    fn a_subfolder_of_a_repo_keeps_repo_relative_paths() {
        let d = fresh("sub");
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::create_dir_all(d.join("services/api")).unwrap();
        let p = Project::open(&d.join("services/api")).unwrap();
        assert_eq!((p.prefix(), p.git_root()), ("services/api", d.as_path()));
        assert_eq!(p.repo_path("main.py").unwrap().as_str(), "services/api/main.py");
        assert_eq!(p.repo_path_of(&d.join("services/api/x/y.py")).unwrap().as_str(), "services/api/x/y.py");
        assert!(p.repo_path_of(&d.join("services/other.py")).is_none(), "outside the opened folder");
    }

    #[test]
    fn a_path_that_cannot_be_a_repo_path_is_refused() {
        let p = Project::open(&fresh("bad")).unwrap();
        assert!(p.repo_path("../escape.py").is_err());
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(Project::open(Path::new("/definitely/not/here")).unwrap_err().to_string().contains("not a folder"));
    }
}
