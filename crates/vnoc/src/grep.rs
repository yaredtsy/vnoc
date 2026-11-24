//! `grep`: a regular expression over the text of the code, answered in nodes (docs/21-agent-cli/03-read-commands.md §6).
//!
//! The files are the index's (the tree's: Python, not git-ignored), each read from disk and matched **line by line**. A hit belongs to the
//! innermost def whose range holds its line, and to the file when no def does. The nodes come in the tree's order (folders, files, defs in source
//! order), never by number of hits, so one question has one answer and a limit cuts the end. Every hit is counted, and only the first
//! [`LINES_PER_NODE`] of a node are kept: what the answer is for is *which* functions, and the lines are the first look at them.
//!
//! By text, not by meaning: a hit in a comment or a string counts, and a call by another name does not. `grep 'parse_order\('` lists the
//! functions that mention the call, a stand-in for callers and not a resolved list.

use std::collections::HashMap;
use std::path::PathBuf;

use regex::Regex;
use serde::Serialize;
use vnoc_domain::RepoPath;

use crate::index::{Index, Kind, Node};
use crate::project::Project;
use crate::source::read_text;
use crate::userdata::after_frontmatter;

/// How many lines of one node are kept; the rest are counted.
pub const LINES_PER_NODE: usize = 5;

/// A line longer than this many characters is cut, with `…`.
pub const LINE_CUT: usize = 200;

/// One line that matched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HitLine {
    /// 1-based, as in the file
    pub line: u32,
    /// as the file has it, cut at [`LINE_CUT`] characters
    pub text: String,
}

/// A node that has hits.
#[derive(Clone, Debug, Serialize)]
pub struct NodeHits {
    pub kind: Kind,
    /// as every answer names a node: relative to the opened folder
    pub location: String,
    pub path: String,
    /// 1-based, both ends included; `None` for a file
    pub lines: Option<[u32; 2]>,
    pub node_id: Option<String>,
    pub docs: usize,
    /// every hit of the node, the ones not kept too
    pub hits: usize,
    /// the first [`LINES_PER_NODE`] of them
    pub shown: Vec<HitLine>,
    /// `hits` less what is shown
    pub more: usize,
    /// a file row: the hits in it that no def holds
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub outside_def: bool,
}

/// What matched in the code.
#[derive(Debug, Default)]
pub struct GrepAnswer {
    /// every node with a hit, in the tree's order
    pub nodes: Vec<NodeHits>,
    /// how many hits there are in all
    pub hits: usize,
}

/// A doc that has hits.
#[derive(Clone, Debug, Serialize)]
pub struct DocHits {
    pub title: String,
    /// the `.md` file
    pub path: PathBuf,
    /// the permanent ID of the node the doc is on
    pub node_id: String,
    /// where that node is, when the tree has it (`None` for a Missing node, and one whose code was not read)
    pub location: Option<String>,
    pub hits: usize,
    /// the first [`LINES_PER_NODE`], numbered as in the `.md` file (the frontmatter counts)
    pub shown: Vec<HitLine>,
    pub more: usize,
}

#[derive(Debug, Default)]
pub struct DocsAnswer {
    /// in the tree's order of their nodes, then by file name
    pub docs: Vec<DocHits>,
    pub hits: usize,
}

/// A line as it is printed: at most [`LINE_CUT`] characters, and `…` where it was cut.
pub fn cut_line(line: &str) -> String {
    match line.char_indices().nth(LINE_CUT) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_string(),
    }
}

/// The matching lines of `lines`, each with its 1-based number (`first` is the number of `lines[0]`), and how many there were.
fn matching<'a>(re: &Regex, lines: impl Iterator<Item = &'a str>, first: u32) -> Vec<HitLine> {
    lines
        .enumerate()
        .filter(|(_, line)| re.is_match(line))
        .map(|(n, line)| HitLine {
            line: first + n as u32,
            text: cut_line(line),
        })
        .collect()
}

fn in_scope(path: &str, scope: &[RepoPath]) -> bool {
    scope.is_empty()
        || scope.iter().any(|s| {
            path == s.as_str()
                || path
                    .strip_prefix(s.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// The nodes of the tree in its order, each with its depth-first successors: the order a tree is read in.
fn depth_first<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(node);
    for child in &node.children {
        depth_first(child, out);
    }
}

/// The hits of `re` in the code of the files under `scope` (repo paths of folders and files; none means every file). Files that cannot be read
/// are left out: the answer is about what is there.
pub fn grep(project: &Project, index: &Index, re: &Regex, scope: &[RepoPath]) -> GrepAnswer {
    let tree = index.tree(&project.name(), project.prefix());
    let mut order = Vec::new();
    depth_first(&tree, &mut order);
    let mut answer = GrepAnswer::default();
    let mut at = 0;
    while at < order.len() {
        let file = order[at];
        at += 1;
        if file.kind != Kind::File || !in_scope(&file.id, scope) {
            continue;
        }
        // the file's own subtree is the next `1 + descendants` entries of the order
        let mut subtree = Vec::new();
        depth_first(file, &mut subtree);
        let Ok(repo_path) = RepoPath::new(file.id.clone()) else {
            continue;
        };
        let Ok(text) = read_text(&project.open_dir().join(&file.path)) else {
            continue;
        };
        let mut owned: HashMap<String, (usize, Vec<HitLine>)> = HashMap::new();
        for hit in matching(re, text.lines(), 1) {
            let owner = index
                .innermost_def(&repo_path, hit.line - 1)
                .map_or_else(|| file.id.clone(), |key| key.to_string());
            let (total, shown) = owned.entry(owner).or_default();
            *total += 1;
            if shown.len() < LINES_PER_NODE {
                shown.push(hit);
            }
        }
        for node in subtree {
            let Some((hits, shown)) = owned.remove(&node.id) else {
                continue;
            };
            answer.hits += hits;
            answer.nodes.push(NodeHits {
                kind: node.kind,
                location: node.location(),
                path: node.path.clone(),
                lines: node.range.map(|r| [r.start.line + 1, r.end.line + 1]),
                node_id: node.node_id.clone(),
                docs: node.docs,
                hits,
                more: hits - shown.len(),
                shown,
                outside_def: node.kind == Kind::File,
            });
        }
    }
    answer
}

/// The hits of `re` in the docs: each `.md` file read from disk, the lines after its frontmatter matched, **numbered as in the file** so that a
/// reader can open the note at that line. Docs come in the tree's order of their nodes (those whose node is not in the tree last), then by file.
pub fn grep_docs(project: &Project, index: &Index, re: &Regex) -> DocsAnswer {
    let tree = index.tree(&project.name(), project.prefix());
    let mut order = Vec::new();
    depth_first(&tree, &mut order);
    let at: HashMap<&str, (usize, &Node)> = order
        .iter()
        .enumerate()
        .filter_map(|(n, node)| node.node_id.as_deref().map(|id| (id, (n, *node))))
        .collect();
    let texts = index.doc_texts();
    let mut found: Vec<(usize, String, String, DocHits)> = Vec::new();
    for (id, file, doc) in texts.iter() {
        let Ok(text) = read_text(&doc.path) else {
            continue;
        };
        let (first, lines) = body_lines(&text);
        let hits = matching(re, lines, first);
        if hits.is_empty() {
            continue;
        }
        let key = id.to_string();
        let node = at.get(key.as_str());
        found.push((
            node.map_or(usize::MAX, |(n, _)| *n),
            key.clone(),
            file.to_string(),
            DocHits {
                title: doc.title.clone(),
                path: doc.path.clone(),
                node_id: key,
                location: node.map(|(_, node)| node.location()),
                hits: hits.len(),
                more: hits.len().saturating_sub(LINES_PER_NODE),
                shown: hits.into_iter().take(LINES_PER_NODE).collect(),
            },
        ));
    }
    found.sort_by(|a, b| (a.0, &a.1, &a.2).cmp(&(b.0, &b.1, &b.2)));
    let docs: Vec<DocHits> = found.into_iter().map(|(_, _, _, doc)| doc).collect();
    DocsAnswer {
        hits: docs.iter().map(|d| d.hits).sum(),
        docs,
    }
}

/// The lines of a doc after its frontmatter, and the 1-based number of the first of them in the file.
fn body_lines(text: &str) -> (u32, std::str::Lines<'_>) {
    let body = after_frontmatter(text).unwrap_or(text);
    let before = text.len() - body.len();
    (
        text[..before].matches('\n').count() as u32 + 1,
        body.lines(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    fn def(qualname: &str, parent: &str, kind: DefKind, from: u32, to: u32) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            qualname: qualname.into(),
            name: qualname.rsplit('.').next().unwrap().into(),
            kind,
            parent: parent.into(),
            parent_kind: None,
            range: Range {
                start: at(from),
                end: at(to),
            },
            params: vec![],
            id: None,
            syntax_error: false,
        }
    }

    /// A project on disk with two files, and the index a driver would make of it. Lines (1-based) of `a.py`:
    ///  1 `# retry`, 2 blank, 3 `def outer():`, 4 `    retry = 0`, 5 `    def inner():`, 6 `        retry += 1`, 7 `    return retry`,
    ///  8 blank, 9 `class K:`, 10 `    def m(self):`, 11 `        return "retry"`
    fn fixture(label: &str) -> (Project, Index) {
        let root = std::env::temp_dir().join(format!("vnoc-grep-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        crate::testdir::remove_at_end(&root);
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        std::fs::write(
            root.join("a.py"),
            "# retry\n\ndef outer():\n    retry = 0\n    def inner():\n        retry += 1\n    return retry\n\nclass K:\n    def m(self):\n        return \"retry\"\n",
        )
        .unwrap();
        std::fs::write(root.join("pkg/b.py"), "x = 'retry'\ny = 2\n").unwrap();
        let path = |p: &str| RepoPath::new(p).unwrap();
        let index = Index::for_tests(
            vec![
                (
                    path("a.py"),
                    vec![
                        def("outer", "", DefKind::Function, 2, 6),
                        def("outer.<locals>.inner", "outer", DefKind::Function, 4, 5),
                        def("K", "", DefKind::Class, 8, 10),
                        def("K.m", "K", DefKind::Function, 9, 10),
                    ],
                ),
                (path("pkg/b.py"), vec![]),
            ],
            vec![],
            vec![path("pkg")],
        );
        (Project::open(&root).unwrap(), index)
    }

    fn locations(answer: &GrepAnswer) -> Vec<(String, usize, bool)> {
        answer
            .nodes
            .iter()
            .map(|n| (n.location.clone(), n.hits, n.outside_def))
            .collect()
    }

    #[test]
    fn a_hit_goes_to_the_innermost_def_and_one_in_no_def_to_the_file() {
        let (project, index) = fixture("owner");
        let answer = grep(&project, &index, &Regex::new("retry").unwrap(), &[]);
        // the tree's order: the folder's files come before... the root's own files? folders first, then files: `pkg/b.py` before `a.py`
        assert_eq!(
            locations(&answer),
            [
                ("pkg/b.py".to_string(), 1, true),
                ("a.py".to_string(), 1, true),
                ("a.py::outer".to_string(), 2, false),
                ("a.py::outer.<locals>.inner".to_string(), 1, false),
                ("a.py::K.m".to_string(), 1, false),
            ]
        );
        assert_eq!(answer.hits, 6);
        let inner = &answer.nodes[3];
        assert_eq!(
            inner.shown,
            [HitLine {
                line: 6,
                text: "        retry += 1".into()
            }]
        );
        assert_eq!(inner.lines, Some([5, 6]));
    }

    #[test]
    fn the_scope_is_a_folder_or_a_file_and_a_prefix_of_a_name_is_not_a_folder() {
        let (project, index) = fixture("scope");
        let re = Regex::new("retry").unwrap();
        let folder = grep(&project, &index, &re, &[RepoPath::new("pkg").unwrap()]);
        assert_eq!(locations(&folder), [("pkg/b.py".to_string(), 1, true)]);
        let file = grep(&project, &index, &re, &[RepoPath::new("a.py").unwrap()]);
        assert_eq!(file.nodes.len(), 4);
        let near = grep(&project, &index, &re, &[RepoPath::new("pk").unwrap()]);
        assert!(near.nodes.is_empty(), "`pk` is not `pkg`");
    }

    #[test]
    fn case_is_the_patterns_to_decide() {
        let (project, index) = fixture("case");
        let loud = grep(&project, &index, &Regex::new("RETRY").unwrap(), &[]);
        assert!(loud.nodes.is_empty());
        let loud = regex::RegexBuilder::new("RETRY")
            .case_insensitive(true)
            .build()
            .unwrap();
        assert_eq!(grep(&project, &index, &loud, &[]).hits, 6);
    }

    #[test]
    fn only_the_first_five_lines_are_kept_and_every_hit_is_counted() {
        let root = std::env::temp_dir().join(format!("vnoc-grep-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        crate::testdir::remove_at_end(&root);
        std::fs::create_dir_all(&root).unwrap();
        let body: String = (0..12).map(|n| format!("    hit {n}\n")).collect();
        std::fs::write(root.join("c.py"), format!("def f():\n{body}")).unwrap();
        let index = Index::for_tests(
            vec![(
                RepoPath::new("c.py").unwrap(),
                vec![def("f", "", DefKind::Function, 0, 12)],
            )],
            vec![],
            vec![],
        );
        let project = Project::open(&root).unwrap();
        let answer = grep(&project, &index, &Regex::new("hit").unwrap(), &[]);
        let f = &answer.nodes[0];
        assert_eq!((f.hits, f.shown.len(), f.more), (12, 5, 7));
        assert_eq!(
            f.shown.last().unwrap().line,
            6,
            "lines 2 to 6 are the first five"
        );
        assert_eq!(answer.hits, 12);
    }

    #[test]
    fn a_long_line_is_cut_with_an_ellipsis() {
        let long = "x".repeat(250);
        let cut = cut_line(&long);
        assert_eq!(cut.chars().count(), LINE_CUT + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(
            cut_line(&"y".repeat(200)),
            "y".repeat(200),
            "200 is not cut"
        );
        // by characters, never inside one
        let wide = "é".repeat(300);
        assert_eq!(cut_line(&wide).chars().count(), LINE_CUT + 1);
    }

    #[test]
    fn a_doc_is_numbered_as_in_its_file_with_the_frontmatter_counted() {
        let text = "---\nnode: X\nhint: a::b\n---\n# Title\n\nline one\nneedle here\n";
        let (first, lines) = body_lines(text);
        assert_eq!(first, 5, "the body starts on line 5 of the file");
        let hits = matching(&Regex::new("needle").unwrap(), lines, first);
        assert_eq!(
            hits,
            [HitLine {
                line: 8,
                text: "needle here".into()
            }]
        );
        // no frontmatter: line 1 is line 1; CRLF does not move a number
        let (first, lines) = body_lines("a\r\nneedle\r\n");
        assert_eq!(first, 1);
        assert_eq!(
            matching(&Regex::new("needle").unwrap(), lines, first)[0].line,
            2
        );
        let (first, _) = body_lines("---\r\nnode: X\r\n---\r\ntext\r\n");
        assert_eq!(first, 4);
    }
}
