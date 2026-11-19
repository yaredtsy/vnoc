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
    /// `path::qualname`, with `#2` for a second def of the name: the text before the first `::` and the text after it. The path is not yet a repo
    /// path, so it is kept as written; `rest` may be empty (`src/orders.py::`), which names the file.
    Location {
        path: String,
        rest: String,
    },
    /// `path:line` or `path:line:column`; the column is dropped. The line is as pasted, 1-based.
    FileLine {
        path: String,
        line: u32,
    },
    Id(NodeId),
    /// Anything else that is not empty: a file or a folder.
    Path(String),
}

impl NodeRef {
    /// The path the reference was written with, as written; `None` for an ID, which has none.
    pub fn path(&self) -> Option<&str> {
        match self {
            NodeRef::Location { path, .. }
            | NodeRef::FileLine { path, .. }
            | NodeRef::Path(path) => Some(path),
            NodeRef::Id(_) => None,
        }
    }

    /// The same reference at another path (a command line makes it relative to the root before it asks). An ID has no path and is returned as it is.
    pub fn with_path(self, path: String) -> NodeRef {
        match self {
            NodeRef::Location { rest, .. } => NodeRef::Location { path, rest },
            NodeRef::FileLine { line, .. } => NodeRef::FileLine { path, line },
            NodeRef::Path(_) => NodeRef::Path(path),
            id @ NodeRef::Id(_) => id,
        }
    }
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
    if let Some((path, rest)) = text.split_once("::") {
        return Some(NodeRef::Location {
            path: path.to_string(),
            rest: rest.to_string(),
        });
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
            NodeRef::Location { path, rest } => {
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

/// Why a reference found nothing (or found a file the driver could not read, or a line the file does not have): which case of `02-naming-a-node.md` §4
/// it is. The sentence is the command line's. Paths are as the tree shows them, relative to the opened folder, except where a case is about what was
/// typed (`NoFile`, `NotRead`, `Outside`), which keep the text as it was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// the file is in the index and has no def of that name; `name` is what followed `::`
    NoDef { path: String, name: String },
    /// nothing on disk has this path (also what a qualified name with no file, `OrderService.parse_order`, is: it parses as a path)
    NoFile(String),
    /// on disk, and not in the index: not Python, or ignored by git, or past the scan's cap
    NotRead(String),
    /// the file is in scope and the driver could not outline it
    Failed { path: String, message: String },
    /// a file name with no folder, that several files have; `files` are sorted
    Ambiguous { name: String, files: Vec<String> },
    /// the file has `lines` lines and the reference named a later one
    PastEnd { path: String, lines: usize },
    /// no node has this ID, and no Missing record does either
    NoId(NodeId),
    /// the path is not under the root
    Outside(String),
}

impl Index {
    /// Which case a reference that found nothing is. Asked **after** a miss (`resolve_ref` stays as it is, with no reason), so it may read the disk:
    /// the file's line count for `PastEnd`, and whether a path exists for `NotRead` and `NoFile`.
    ///
    /// It also answers for the two things `resolve_ref` calls a node and the tree does not have: a file the driver failed on (`Failed`), and a line
    /// past the end of a file (`PastEnd`: `resolve_ref` gives the file). For a reference that names something fine it has no answer to give, and
    /// returns `NoDef` with an empty name.
    pub fn why_not(&self, project: &Project, r: &NodeRef) -> Why {
        let NodeRef::Id(id) = r else {
            return self.why_not_path(project, r);
        };
        Why::NoId(*id)
    }

    fn why_not_path(&self, project: &Project, r: &NodeRef) -> Why {
        let given = r.path().unwrap_or_default();
        // the file or folder the path names, by the rules `resolve_ref` uses
        let found = self.first_path(project, given, |p| {
            (self.is_file(p) || self.is_folder(p)).then(|| p.clone())
        });
        if let Some(path) = found {
            let shown = shown(project, &path);
            if let Some(message) = self.failure_of(&path) {
                return Why::Failed {
                    path: shown,
                    message: message.clone(),
                };
            }
            if let NodeRef::FileLine { line, .. } = r
                && let Some(lines) = line_count(project, &path)
                && *line as usize > lines
            {
                return Why::PastEnd { path: shown, lines };
            }
            let name = match r {
                NodeRef::Location { rest, .. } => rest.clone(),
                _ => String::new(),
            };
            return Why::NoDef { path: shown, name };
        }
        let text = given.replace('\\', "/");
        if given.is_empty() {
            return Why::NoFile(String::new());
        }
        let outside = if is_absolute(given) {
            project.repo_path_of(Path::new(given)).is_none()
        } else {
            project.repo_path(&text).is_err()
        };
        if outside {
            return Why::Outside(given.to_string());
        }
        if !text.contains('/') {
            let files = self.files_named(&text);
            if files.len() > 1 {
                let mut files: Vec<String> = files.iter().map(|f| shown(project, f)).collect();
                files.sort();
                return Why::Ambiguous { name: text, files };
            }
        }
        let on_disk = if is_absolute(given) {
            Path::new(given).exists()
        } else {
            project.open_dir().join(&text).exists() || project.git_root().join(&text).exists()
        };
        if on_disk {
            Why::NotRead(given.to_string())
        } else {
            Why::NoFile(given.to_string())
        }
    }
}

/// A repo path as the tree shows it: relative to the opened folder.
fn shown(project: &Project, path: &RepoPath) -> String {
    let prefix = project.prefix();
    path.as_str()
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|_| !prefix.is_empty())
        .unwrap_or(path.as_str())
        .to_string()
}

/// How many lines the file has now, `None` when it cannot be read.
fn line_count(project: &Project, path: &RepoPath) -> Option<usize> {
    let bytes = std::fs::read(project.git_root().join(path.as_str())).ok()?;
    Some(String::from_utf8_lossy(&bytes).lines().count())
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
        let (path, rest) = text.split_once("::").unwrap();
        Some(NodeRef::Location {
            path: path.to_string(),
            rest: rest.to_string(),
        })
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

    // ---- why_not

    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    fn def(qualname: &str) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            qualname: qualname.into(),
            name: qualname.rsplit('.').next().unwrap().into(),
            kind: DefKind::Function,
            parent: String::new(),
            parent_kind: None,
            range: Range {
                start: at(0),
                end: at(1),
            },
            params: vec![],
            id: None,
            syntax_error: false,
        }
    }

    /// A project on disk and the index a driver would have made of it: `src/orders.py` and `src/a.py` read, `src/bad.py` failed, `src/app.ts` on
    /// disk and not read, two `metering.py`, one `core/` folder.
    fn fixture(label: &str) -> (Project, Index) {
        let root = std::env::temp_dir().join(format!("vnoc-whynot-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (file, text) in [
            ("src/orders.py", "def parse_order():\n    pass\n"),
            ("src/a.py", "1\n2\n3\n4\n5\n"),
            ("src/bad.py", "def (:\n"),
            ("src/app.ts", "export {}\n"),
            ("core/metering.py", "x = 1\n"),
            ("billing/metering.py", "x = 2\n"),
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let path = |p: &str| RepoPath::new(p).unwrap();
        let index = Index::for_tests(
            vec![
                (path("src/orders.py"), vec![def("parse_order")]),
                (path("src/a.py"), vec![]),
                (path("core/metering.py"), vec![]),
                (path("billing/metering.py"), vec![]),
            ],
            vec![(path("src/bad.py"), "invalid syntax at line 1")],
            vec![path("src"), path("core"), path("billing")],
        );
        (Project::open(&root).unwrap(), index)
    }

    fn why(index: &Index, project: &Project, text: &str) -> Why {
        index.why_not(project, &parse(text).unwrap())
    }

    #[test]
    fn why_a_reference_found_nothing_one_row_per_case() {
        let (project, index) = fixture("table");
        let id = NodeId::parse("0Q8V5W4T2F7KX9M3B6D1RZ4H7K").unwrap();
        let s = |t: &str| t.to_string();
        let rows: Vec<(&str, Why)> = vec![
            (
                "src/orders.py::parse_ordr",
                Why::NoDef {
                    path: s("src/orders.py"),
                    name: s("parse_ordr"),
                },
            ),
            (
                "src/orders.py::Outer.parse_order#2",
                Why::NoDef {
                    path: s("src/orders.py"),
                    name: s("Outer.parse_order#2"),
                },
            ),
            // a qualified name with no file parses as a path, and no such path is on disk
            (
                "OrderService.parse_order",
                Why::NoFile(s("OrderService.parse_order")),
            ),
            ("src/ordrs.py", Why::NoFile(s("src/ordrs.py"))),
            ("src/ordrs.py:12", Why::NoFile(s("src/ordrs.py"))),
            ("::f", Why::NoFile(s(""))),
            ("src/app.ts", Why::NotRead(s("src/app.ts"))),
            ("src/app.ts:3", Why::NotRead(s("src/app.ts"))),
            (
                "src/bad.py",
                Why::Failed {
                    path: s("src/bad.py"),
                    message: s("invalid syntax at line 1"),
                },
            ),
            (
                "src/bad.py:1",
                Why::Failed {
                    path: s("src/bad.py"),
                    message: s("invalid syntax at line 1"),
                },
            ),
            (
                "src/bad.py::f",
                Why::Failed {
                    path: s("src/bad.py"),
                    message: s("invalid syntax at line 1"),
                },
            ),
            (
                "metering.py",
                Why::Ambiguous {
                    name: s("metering.py"),
                    files: vec![s("billing/metering.py"), s("core/metering.py")],
                },
            ),
            (
                "metering.py:3",
                Why::Ambiguous {
                    name: s("metering.py"),
                    files: vec![s("billing/metering.py"), s("core/metering.py")],
                },
            ),
            (
                "src/a.py:99",
                Why::PastEnd {
                    path: s("src/a.py"),
                    lines: 5,
                },
            ),
            ("0Q8V5W4T2F7KX9M3B6D1RZ4H7K", Why::NoId(id)),
            (
                "/somewhere/else/x.py",
                Why::Outside(s("/somewhere/else/x.py")),
            ),
            ("../x.py", Why::Outside(s("../x.py"))),
            ("src/../../x.py::f", Why::Outside(s("src/../../x.py"))),
        ];
        for (text, expect) in rows {
            assert_eq!(why(&index, &project, text), expect, "{text:?}");
        }
    }

    #[test]
    fn a_line_inside_the_file_is_not_past_the_end() {
        let (project, index) = fixture("lines");
        // not a miss at all: the answer is the "no reason" one, an empty name
        assert_eq!(
            why(&index, &project, "src/a.py:5"),
            Why::NoDef {
                path: "src/a.py".into(),
                name: String::new()
            }
        );
    }

    #[test]
    fn a_path_is_shown_relative_to_the_opened_folder() {
        let root = std::env::temp_dir().join(format!("vnoc-whynot-sub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("services/api")).unwrap();
        std::fs::write(root.join("services/api/a.py"), "1\n2\n").unwrap();
        let project = Project::open(&root.join("services/api")).unwrap();
        let index = Index::for_tests(
            vec![(RepoPath::new("services/api/a.py").unwrap(), vec![])],
            vec![],
            vec![],
        );
        assert_eq!(
            why(&index, &project, "a.py::missing"),
            Why::NoDef {
                path: "a.py".into(),
                name: "missing".into()
            }
        );
        assert_eq!(
            why(&index, &project, "a.py:9"),
            Why::PastEnd {
                path: "a.py".into(),
                lines: 2
            }
        );
    }

    #[test]
    fn with_path_changes_only_the_path() {
        let moved = |text: &str| parse(text).unwrap().with_path("b/x.py".into());
        assert_eq!(moved("a.py::f#2"), location("b/x.py::f#2").unwrap());
        assert_eq!(moved("a.py:12"), line("b/x.py", 12).unwrap());
        assert_eq!(moved("a.py"), path("b/x.py").unwrap());
        let id = parse("0Q8V5W4T2F7KX9M3B6D1RZ4H7K").unwrap();
        assert_eq!(id.clone().with_path("b".into()), id);
        assert_eq!(id.path(), None);
        assert_eq!(parse("a.py::f").unwrap().path(), Some("a.py"));
        assert_eq!(parse("a.py:3").unwrap().path(), Some("a.py"));
    }
}
