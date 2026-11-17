//! A reference to a node, as a person pastes it (`docs/20-search/04-core.md` §4): `src/orders.py:142`, `src/orders.py::OrderService.parse_order`,
//! `0Q8V5W4T2F7KX9M3B6D1RZ4H7K`, or a path.
//!
//! [`parse`] reads the text and only the text; [`Index::resolve_ref`] says which node it names, from what the index holds. The search box tries a
//! query of one token as a reference, and the CLI (doc 21) tries every `<node>` argument, so both go through these two functions.
//!
//! A path in a reference is relative to the opened folder, which is what a person sees, or absolute; failing that it is read as a repo path, or with
//! the opened folder's name in front, or (a file name with no `/`) as the one file of that name. A pasted line is 1-based, as an editor and a
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

/// `path:line`, `path:line:column`, `path:L<line>`, `path#L<line>` or `path#L<line>-L<line>`, as the path and the (first) line.
fn file_line(text: &str) -> Option<(&str, u32)> {
    if let Some((path, fragment)) = text.rsplit_once('#')
        && !path.is_empty()
        && let Some(line) = hash_line(fragment)
    {
        return Some((path, line));
    }
    let (rest, last) = text.rsplit_once(':')?;
    if number(last).is_some()
        && let Some((path, line)) = rest.rsplit_once(':')
        && let Some(line) = l_number(line)
        && !path.is_empty()
    {
        return Some((path, line));
    }
    let last = l_number(last)?;
    (!rest.is_empty()).then_some((rest, last))
}

/// What follows a `#`: `L<n>` or `L<n>-L<m>`, `L` in either case; the line is `n`.
fn hash_line(fragment: &str) -> Option<u32> {
    let (first, second) = match fragment.split_once('-') {
        Some((first, second)) => (first, Some(second)),
        None => (fragment, None),
    };
    let line = l_number(first).filter(|_| first.starts_with(['L', 'l']))?;
    if let Some(second) = second {
        l_number(second).filter(|_| second.starts_with(['L', 'l']))?;
    }
    Some(line)
}

/// A number, with an `L` before it or not.
fn l_number(text: &str) -> Option<u32> {
    number(text.strip_prefix(['L', 'l']).unwrap_or(text))
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
                let found = self.first_path(project, path, |p| {
                    let location = format!("{p}::{rest}").parse::<LocationKey>().ok()?;
                    if location.qualname().is_empty() {
                        (self.is_file(p) || self.is_folder(p)).then_some(location)
                    } else {
                        self.def_at(&location).map(|_| location)
                    }
                });
                match found {
                    Some(location) if location.qualname().is_empty() => {
                        node(location.path().to_string())
                    }
                    Some(location) => node(location.to_string()),
                    None => Found::Nothing,
                }
            }
            NodeRef::FileLine { path, line } => {
                let Some(path) =
                    self.first_path(project, path, |p| self.is_file(p).then(|| p.clone()))
                else {
                    return Found::Nothing;
                };
                // line 0 is no line
                let def = line
                    .checked_sub(1)
                    .and_then(|l| self.innermost_def(&path, l));
                match def {
                    Some(location) => node(location.to_string()),
                    None => node(path.to_string()),
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
            NodeRef::Path(text) => {
                match self.first_path(project, text, |p| {
                    (self.is_file(p) || self.is_folder(p)).then(|| p.clone())
                }) {
                    Some(path) => node(path.to_string()),
                    None => Found::Nothing,
                }
            }
        }
    }

    /// What `has` makes of the first repo path the pasted `text` can be, and the index has: relative to the opened folder, as a repo path (the
    /// tree's own ids), with the opened folder's name dropped from the front; then, for a text with no `/`, the one file that has that name.
    /// An absolute path is only itself.
    fn first_path<T>(
        &self,
        project: &Project,
        text: &str,
        has: impl Fn(&RepoPath) -> Option<T>,
    ) -> Option<T> {
        if is_absolute(text) {
            return has(&project.repo_path_of(Path::new(text))?);
        }
        let text = text.replace('\\', "/");
        let text = text.strip_prefix("./").unwrap_or(&text);
        let text = text.trim_end_matches('/');
        let name = project.name();
        let candidates = [
            project.repo_path(text).ok(),
            RepoPath::new(text).ok(),
            text.strip_prefix(name.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(|rest| project.repo_path(rest).ok()),
        ];
        if let Some(found) = candidates.iter().flatten().find_map(&has) {
            return Some(found);
        }
        if text.contains('/') {
            return None;
        }
        // a file name alone: only when one file has it
        match &self.files_named(text)[..] {
            [only] => has(only),
            _ => None,
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
            ("nested.py:L25", line("nested.py", 25)),
            ("nested.py:l25", line("nested.py", 25)),
            ("nested.py:L25:9", line("nested.py", 25)),
            ("nested.py#L25", line("nested.py", 25)),
            ("nested.py#l25", line("nested.py", 25)),
            ("src/orders.py#L25-L30", line("src/orders.py", 25)),
            ("src/orders.py#L25-l30", line("src/orders.py", 25)),
            ("nested.py#L0", line("nested.py", 0)),
            ("nested.py#L99999999999", line("nested.py", u32::MAX)),
            ("C:\\repo\\a.py#L12", line("C:\\repo\\a.py", 12)),
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
            ("nested.py:Lx", path("nested.py:Lx")),
            ("nested.py#Lx", path("nested.py#Lx")),
            ("nested.py#L", path("nested.py#L")),
            ("nested.py#25", path("nested.py#25")),
            ("nested.py#L25-", path("nested.py#L25-")),
            ("nested.py#L25-30", path("nested.py#L25-30")),
            ("#L25", path("#L25")),
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
