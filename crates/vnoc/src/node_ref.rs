//! A reference to a node, as a person pastes it (`docs/20-search/04-core.md` §4): `src/orders.py:142`, `src/orders.py::OrderService.parse_order`,
//! `0Q8V5W4T2F7KX9M3B6D1RZ4H7K`, or a path.
//!
//! [`parse`] reads the text and only the text; [`Index::resolve_ref`] says which node it names, from what the index holds. The search box tries a
//! query of one token as a reference, and the CLI (doc 21) tries every `<node>` argument, so both go through these two functions.
//!
//! A path in a reference is relative to the opened folder, which is what a person sees, or absolute. A pasted line is 1-based, as an editor and a
//! stack trace show it, and a def's range is 0-based.

use std::path::Path;

use vnoc_domain::{LocationKey, NodeId, RepoPath};

use crate::index::Index;
use crate::project::Project;

/// What a pasted text says, before the index is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeRef {
    /// `path::qualname`, with `#2` for a second def of the name. The path is not yet a repo path, so the text is kept.
    Location(String),
    /// `path:line` or `path:line:column`; the column is dropped. The line is as pasted, 1-based.
    FileLine {
        path: String,
        line: u32,
    },
    Id(NodeId),
    /// Anything else that is not empty: a file or a folder.
    Path(String),
}

/// Which node a [`NodeRef`] names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The node's id in the tree: a def's location key as text, the repo path for a file or a folder.
    Node(String),
    /// The ID is a node nothing in the code matches any more.
    Missing(NodeId),
    Nothing,
}

/// Reads `text` as a reference. Pure; `None` only for text with nothing in it.
///
/// A `::` makes a location. A text that ends in `:<digits>` (and again `:<digits>` for a column) is a file and a line: the colon of a Windows
/// drive (`C:\a.py`) is never one, as only what follows the last colon is read. A 26-character ID, in any case, is an ID. The rest is a path.
pub fn parse(text: &str) -> Option<NodeRef> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("::") {
        return Some(NodeRef::Location(text.to_string()));
    }
    if let Some((path, line)) = file_line(text) {
        return Some(NodeRef::FileLine {
            path: path.to_string(),
            line,
        });
    }
    Some(match NodeId::parse(text) {
        Ok(id) => NodeRef::Id(id),
        Err(_) => NodeRef::Path(text.to_string()),
    })
}

/// `path:line` or `path:line:column`, as the path and the line.
fn file_line(text: &str) -> Option<(&str, u32)> {
    let (rest, last) = text.rsplit_once(':')?;
    let last = number(last)?;
    if let Some((path, line)) = rest.rsplit_once(':')
        && let Some(line) = number(line)
        && !path.is_empty()
    {
        return Some((path, line));
    }
    (!rest.is_empty()).then_some((rest, last))
}

/// Digits only, and at least one; a number too big for a line is the biggest line, which no file has.
fn number(text: &str) -> Option<u32> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().unwrap_or(u32::MAX))
}

/// Whether a pasted path is absolute: `/…`, or a Windows drive and a separator (`C:\…`, `C:/…`).
fn is_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with('/')
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/'))
}

/// The repo path of a pasted path: relative to the opened folder, or absolute under it. `None` when it is neither.
fn repo_path(project: &Project, text: &str) -> Option<RepoPath> {
    if is_absolute(text) {
        return project.repo_path_of(Path::new(text));
    }
    let text = text.replace('\\', "/");
    let text = text.strip_prefix("./").unwrap_or(&text);
    project.repo_path(text.trim_end_matches('/')).ok()
}

impl Index {
    /// The node `r` names, by the tree's id. Reads only what the index holds (no disk). A reference to a path that is not in the opened folder, to
    /// a def that is not there, or to an ID found nowhere, is [`Found::Nothing`]; an ID the index knows as Missing is [`Found::Missing`].
    pub fn resolve_ref(&self, project: &Project, r: &NodeRef) -> Found {
        let node = |id: String| Found::Node(id);
        match r {
            NodeRef::Location(text) => {
                let Some((path, rest)) = text.split_once("::") else {
                    return Found::Nothing;
                };
                let Some(path) = repo_path(project, path) else {
                    return Found::Nothing;
                };
                let Ok(location) = format!("{path}::{rest}").parse::<LocationKey>() else {
                    return Found::Nothing;
                };
                if location.qualname().is_empty() {
                    self.file_or_folder(&path)
                } else if self.def_at(&location).is_some() {
                    node(location.to_string())
                } else {
                    Found::Nothing
                }
            }
            NodeRef::FileLine { path, line } => {
                let Some(path) = repo_path(project, path) else {
                    return Found::Nothing;
                };
                // line 0 is no line
                let def = line
                    .checked_sub(1)
                    .and_then(|l| self.innermost_def(&path, l));
                match def {
                    Some(location) => node(location.to_string()),
                    None if self.is_file(&path) => node(path.to_string()),
                    None => Found::Nothing,
                }
            }
            NodeRef::Id(id) => match self.found_at(id) {
                Some(location) if location.qualname().is_empty() => {
                    node(location.path().to_string())
                }
                Some(location) => node(location.to_string()),
                None if self.is_missing(id) => Found::Missing(*id),
                None => Found::Nothing,
            },
            NodeRef::Path(text) => match repo_path(project, text) {
                Some(path) => self.file_or_folder(&path),
                None => Found::Nothing,
            },
        }
    }

    fn file_or_folder(&self, path: &RepoPath) -> Found {
        if self.is_file(path) || self.is_folder(path) {
            Found::Node(path.to_string())
        } else {
            Found::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(path: &str, line: u32) -> Option<NodeRef> {
        Some(NodeRef::FileLine {
            path: path.to_string(),
            line,
        })
    }

    fn path(text: &str) -> Option<NodeRef> {
        Some(NodeRef::Path(text.to_string()))
    }

    fn location(text: &str) -> Option<NodeRef> {
        Some(NodeRef::Location(text.to_string()))
    }

    #[test]
    fn the_parse_table() {
        const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
        let lower = ID.to_lowercase();
        let id = Some(NodeRef::Id(NodeId::parse(ID).unwrap()));
        let rows: Vec<(&str, Option<NodeRef>)> = vec![
            ("", None),
            ("   ", None),
            // a location
            (
                "nested.py::Outer.method",
                location("nested.py::Outer.method"),
            ),
            (
                "nested.py::Outer.method#2",
                location("nested.py::Outer.method#2"),
            ),
            ("src/orders.py::", location("src/orders.py::")),
            ("/x/a.py::f", location("/x/a.py::f")),
            // a file and a line
            ("nested.py:25", line("nested.py", 25)),
            ("nested.py:25:9", line("nested.py", 25)),
            ("src/orders.py:142", line("src/orders.py", 142)),
            ("nested.py:0", line("nested.py", 0)),
            ("nested.py:99999999999", line("nested.py", u32::MAX)),
            ("C:\\repo\\a.py:12", line("C:\\repo\\a.py", 12)),
            ("C:/repo/a.py:12", line("C:/repo/a.py", 12)),
            ("C:\\repo\\a.py:12:3", line("C:\\repo\\a.py", 12)),
            ("/x/a.py:3", line("/x/a.py", 3)),
            // an ID, in any case
            (ID, id.clone()),
            (&lower, id),
            // a path: what is neither
            ("src/orders.py", path("src/orders.py")),
            ("src/billing/", path("src/billing/")),
            ("C:\\repo\\a.py", path("C:\\repo\\a.py")),
            ("nested.py:", path("nested.py:")),
            ("nested.py:x", path("nested.py:x")),
            (":12", path(":12")),
            // 26 characters `NodeId::parse` refuses: a `U`, a first character over 7, 25 and 27 characters
            (
                "0Q8V5W4T2F7KX9M3B6D1RZ4H7U",
                path("0Q8V5W4T2F7KX9M3B6D1RZ4H7U"),
            ),
            (
                "8Q8V5W4T2F7KX9M3B6D1RZ4H7K",
                path("8Q8V5W4T2F7KX9M3B6D1RZ4H7K"),
            ),
            (
                "0Q8V5W4T2F7KX9M3B6D1RZ4H7",
                path("0Q8V5W4T2F7KX9M3B6D1RZ4H7"),
            ),
        ];
        for (text, expect) in rows {
            assert_eq!(parse(text), expect, "{text:?}");
        }
    }

    #[test]
    fn text_is_trimmed() {
        assert_eq!(parse("  nested.py:25 "), line("nested.py", 25));
    }
}
