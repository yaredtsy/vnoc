//! `check`: what is wrong with the project's documentation, as findings a command prints and CI fails on (docs/21-agent-cli/03-read-commands.md §9).
//!
//! The findings are the index's own answers, each given a path, a sentence and the fix it names: Missing nodes (`Index::missing`), lost `ID:` lines
//! (`Index::lost_markers`), an ID written twice and broken `.vnoc/` files (`Index::problems`, as the NEEDS ATTENTION view lists them), and files the
//! driver could not read (`Index::failed`). So CI and the editor agree. A file that does not parse is a **warning**: a syntax error in the middle of
//! an edit is normal, and it fails nothing.

use serde::Serialize;
use vnoc_domain::RepoPath;

use crate::index::{Index, Kind};
use crate::project::Project;
use crate::protocol::Problem;

/// What a finding is about. The words are stable: they are the `kind` of `--json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// a documented node whose code is not found
    Missing,
    /// `node.toml` says the ID is in the code and the code has none
    LostMarker,
    /// one ID written in two places
    DuplicateId,
    /// a `node.toml` that does not parse, a folder in `nodes/` that is not a node, a `.vnoc-id` that cannot be read
    Broken,
    /// a file the driver could not outline
    Unread,
}

impl FindingKind {
    /// The word a row starts with, the same as in `--json`.
    pub fn word(self) -> &'static str {
        match self {
            FindingKind::Missing => "missing",
            FindingKind::LostMarker => "lost_marker",
            FindingKind::DuplicateId => "duplicate_id",
            FindingKind::Broken => "broken",
            FindingKind::Unread => "unread",
        }
    }
}

/// One thing `check` found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    /// whether it fails the check: all but a file that could not be read
    pub fails: bool,
    /// relative to the opened folder; absolute when the file is not under it (`.vnoc/` above the root)
    pub path: String,
    /// what is wrong, in a sentence
    pub message: String,
    /// what to do about it, as a line to run or to follow
    pub fix: String,
    /// the node's permanent ID, when the finding is about a node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// the repo path of the file it is about, which `only` is matched against
    #[serde(skip)]
    pub repo_path: Option<RepoPath>,
}

/// `1 problem`, `2 problems`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A repo path as it is shown: below the opened folder.
fn below(prefix: &str, repo_path: &str) -> String {
    if prefix.is_empty() {
        return repo_path.to_string();
    }
    repo_path
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(repo_path)
        .to_string()
}

/// The first line of what the driver said, which is the part about the file.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text).trim()
}

/// What the line a marker is written as is called, by the kind of node: a def has an `ID:` line in its docstring, a file a `# FileID:` comment, a
/// folder a `.vnoc-id` file.
fn marker_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Function | Kind::Class => "`ID:` line",
        Kind::File => "`# FileID:` comment",
        Kind::Folder => "`.vnoc-id` file",
    }
}

/// A problem of the index as a finding of `kind`: its file is made relative to the opened folder, and the line (0-based in the problem) is added to
/// the fix when there is one.
fn from_problem(project: &Project, problem: &Problem, kind: FindingKind) -> Finding {
    let absolute = problem.file.as_deref().map(std::path::Path::new);
    let repo_path = absolute
        .and_then(|file| file.strip_prefix(project.git_root()).ok())
        .and_then(|rel| RepoPath::new(rel.to_string_lossy().replace('\\', "/")).ok());
    // below the opened folder, as a doc file is printed; absolute when `.vnoc/` is not under it (the root is below the git root)
    let path = absolute.map_or_else(String::new, |file| {
        file.strip_prefix(project.open_dir())
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
    });
    let at = match problem.line {
        Some(line) => format!("{path}:{}", line + 1),
        None => path.clone(),
    };
    let fix = match kind {
        FindingKind::DuplicateId => {
            format!(
                "open {at} and delete the `ID:` line of the copy (the node keeps its ID where it was found)"
            )
        }
        // the tags' descriptions are not a node: V-NOC reads that file and never writes it, so the line is the team's to fix
        _ if path.ends_with("tags.toml") => {
            format!("open {at} and fix the line named (V-NOC reads this file and never writes it)")
        }
        _ => format!("open {at}: fix the file, or remove it if it is not a node"),
    };
    // one line: a TOML error runs over several, and the text starts with the file it is about, which the row already says
    let said = first_line(&problem.text);
    let message = [
        Some(path.as_str()),
        repo_path.as_ref().map(RepoPath::as_str),
    ]
    .into_iter()
    .flatten()
    .find_map(|file| {
        said.strip_prefix(file)
            .and_then(|rest| rest.strip_prefix(": "))
    })
    .unwrap_or(said)
    .to_string();
    Finding {
        kind,
        fails: true,
        path,
        message,
        fix,
        id: None,
        repo_path,
    }
}

/// Every finding, in the order of the table of 03 §9: Missing, lost markers, IDs twice, broken files, files that could not be read; each group in
/// the order the index gives it (by path). `only` keeps the findings in those files (a Missing node counts for the last file it was known in).
pub fn check(index: &Index, project: &Project, only: Option<&[RepoPath]>) -> Vec<Finding> {
    let prefix = project.prefix();
    let mut found: Vec<Finding> = Vec::new();

    for node in index.missing() {
        let was = if node.qualname.is_empty() {
            below(prefix, &node.path)
        } else {
            format!("{}::{}", below(prefix, &node.path), node.qualname)
        };
        let docs = count(node.docs, "doc", "docs");
        found.push(Finding {
            kind: FindingKind::Missing,
            fails: true,
            path: below(prefix, &node.path),
            message: format!(
                "documented code not found: was {was} · ID {} · {docs}",
                node.id
            ),
            fix: format!(
                "vnoc relink {} <node> if the code moved; remove it in VS Code if the code is gone",
                node.id
            ),
            repo_path: RepoPath::new(node.path.clone()).ok(),
            id: Some(node.id),
        });
    }

    for lost in index.lost_markers() {
        let location = if lost.qualname.is_empty() {
            below(prefix, &lost.path)
        } else {
            format!("{}::{}", below(prefix, &lost.path), lost.qualname)
        };
        found.push(Finding {
            kind: FindingKind::LostMarker,
            fails: true,
            path: below(prefix, &lost.path),
            message: format!(
                "the {} of {location} is gone (ID {}): the node was found by its name only",
                marker_of(lost.kind),
                lost.id
            ),
            fix: format!("vnoc repair {location}"),
            repo_path: RepoPath::new(lost.path.clone()).ok(),
            id: Some(lost.id),
        });
    }

    let git_root = project.git_root();
    for problem in index.duplicate_ids(git_root) {
        found.push(from_problem(project, &problem, FindingKind::DuplicateId));
    }
    for problem in index.broken(git_root) {
        found.push(from_problem(project, &problem, FindingKind::Broken));
    }

    for failed in index.failed() {
        found.push(Finding {
            kind: FindingKind::Unread,
            fails: false,
            path: below(prefix, &failed.path),
            message: format!("the driver could not read it: {}", first_line(&failed.error)),
            fix: "fix the file; V-NOC reads it again when it parses (a warning: it does not fail the check)".to_string(),
            repo_path: RepoPath::new(failed.path.clone()).ok(),
            id: None,
        });
    }

    if let Some(only) = only {
        found.retain(|f| {
            f.repo_path
                .as_ref()
                .is_some_and(|path| only.iter().any(|keep| keep == path))
        });
    }
    found
}
