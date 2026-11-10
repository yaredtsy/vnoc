//! Where a node is right now: a repo path, a qualified name, and which of several same-named definitions it is.
//! None of this is an identity: it changes on every rename and move. The identity is the [`NodeId`](crate::NodeId).

use std::fmt;
use std::num::NonZeroU32;

/// A repo-relative path, `/` separators only, **exactly as git's index records it**.
///
/// The type refuses what can never be a repo-relative path (a backslash, a leading `/`, a drive letter, a `..` or `.`
/// segment, an empty segment). It does **not** normalise Unicode: on macOS the file system hands back decomposed names
/// (NFD) while git records precomposed ones (NFC), and the ID is a hash of these bytes, so take the path from git
/// (`git ls-files`, a tree), never from a directory listing.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct RepoPath(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoPathError {
    #[error("a repo path is not empty")]
    Empty,
    #[error("a repo path uses `/`, never a backslash: `{0}`")]
    Backslash(String),
    #[error("a repo path is relative to the git root, not absolute: `{0}`")]
    Absolute(String),
    #[error("a repo path has no empty, `.` or `..` segment: `{0}`")]
    BadSegment(String),
    #[error("a repo path has no NUL byte")]
    Nul,
}

impl RepoPath {
    pub fn new(path: impl Into<String>) -> Result<RepoPath, RepoPathError> {
        let path = path.into();
        if path.is_empty() {
            return Err(RepoPathError::Empty);
        }
        if path.contains('\0') {
            return Err(RepoPathError::Nul);
        }
        if path.contains('\\') {
            return Err(RepoPathError::Backslash(path));
        }
        let drive = path.len() >= 2 && path.as_bytes()[1] == b':' && path.as_bytes()[0].is_ascii_alphabetic();
        if path.starts_with('/') || drive {
            return Err(RepoPathError::Absolute(path));
        }
        if path.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
            return Err(RepoPathError::BadSegment(path));
        }
        Ok(RepoPath(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocationKeyError {
    #[error("an occurrence is 1-based; 0 is not a valid occurrence")]
    ZeroOccurrence,
}

/// A node that has no permanent ID (yet): `path::qualname`, and which same-named definition of that file it is.
///
/// `qualname` is the language's own qualified name (Python: `__qualname__`, **including `<locals>`**; empty for a file).
/// `occurrence` is **1-based**, counted among definitions with the same `qualname` in the same file, in source order.
/// A location is what a node is called until it is persisted; nothing may save a reference to one.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LocationKey {
    path: RepoPath,
    qualname: String,
    occurrence: NonZeroU32,
}

impl LocationKey {
    pub fn new(path: RepoPath, qualname: impl Into<String>, occurrence: u32) -> Result<LocationKey, LocationKeyError> {
        let occurrence = NonZeroU32::new(occurrence).ok_or(LocationKeyError::ZeroOccurrence)?;
        Ok(LocationKey { path, qualname: qualname.into(), occurrence })
    }
    pub fn path(&self) -> &RepoPath {
        &self.path
    }
    pub fn qualname(&self) -> &str {
        &self.qualname
    }
    pub fn occurrence(&self) -> u32 {
        self.occurrence.get()
    }
}

/// Shown as `path::qualname`, with `#n` appended from the second occurrence on.
impl fmt::Display for LocationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.path, self.qualname)?;
        if self.occurrence.get() > 1 {
            write!(f, "#{}", self.occurrence)?;
        }
        Ok(())
    }
}

/// The occurrence of each definition: **1-based, counted among the same `qualname`, in the order given** (source order).
///
/// ```
/// assert_eq!(vnoc_domain::occurrences(["f", "g", "f", "f"]), vec![1, 1, 2, 3]);
/// ```
pub fn occurrences<'a>(qualnames: impl IntoIterator<Item = &'a str>) -> Vec<u32> {
    let mut seen: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    qualnames
        .into_iter()
        .map(|q| {
            let n = seen.entry(q).or_insert(0);
            *n += 1;
            *n
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_paths_that_cannot_be_repo_relative_are_refused() {
        for bad in ["", "/etc/x.py", "C:/x.py", "c:\\x.py", "a\\b.py", "a//b.py", "a/./b.py", "../x.py", "a/../b.py", "a/b/", "a\0b"] {
            assert!(RepoPath::new(bad).is_err(), "{bad:?} should be refused");
        }
        for good in ["a.py", "src/orders.py", "my dir/mod.py", "caf\u{e9}/\u{e9}.py", ".vnoc/nodes/ab/x/node.toml"] {
            assert!(RepoPath::new(good).is_ok(), "{good:?} should be accepted");
        }
    }

    #[test]
    fn unicode_is_kept_exactly_as_given() {
        let nfc = "caf\u{e9}.py";
        let nfd = "cafe\u{301}.py";
        assert_ne!(RepoPath::new(nfc).unwrap(), RepoPath::new(nfd).unwrap());
        assert_eq!(RepoPath::new(nfd).unwrap().as_str(), nfd);
    }

    #[test]
    fn a_location_shows_path_qualname_and_the_occurrence_after_the_first() {
        let p = RepoPath::new("src/a.py").unwrap();
        assert_eq!(LocationKey::new(p.clone(), "C.f", 1).unwrap().to_string(), "src/a.py::C.f");
        assert_eq!(LocationKey::new(p.clone(), "C.f", 2).unwrap().to_string(), "src/a.py::C.f#2");
        assert_eq!(LocationKey::new(p, "C.f", 0), Err(LocationKeyError::ZeroOccurrence));
    }

    #[test]
    fn occurrences_count_per_qualname_in_order() {
        assert_eq!(occurrences([]), Vec::<u32>::new());
        assert_eq!(occurrences(["a", "b", "a", "a", "b"]), vec![1, 1, 2, 3, 2]);
    }
}
