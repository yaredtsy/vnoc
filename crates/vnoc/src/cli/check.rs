//! `vnoc missing` and `vnoc check [--changed]` (03 §8, §9): the library's `check`, printed. `check` is the only command whose answer can be a
//! failure: it exits `1` when a finding fails, after it printed them.

use std::collections::HashSet;
use std::process::Command;

use serde::Serialize;
use vnoc_domain::{NodeId, RepoPath};

use super::nodes::{Doc, Lookup};
use super::out::{self, Render};
use super::project::{open_from, open_without_driver};
use super::{Failure, Format, Global};
use crate::check::{Finding, check, summary};
use crate::project::Project;

pub async fn missing(global: &Global) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 0);
    let list = opened.index.read().unwrap().missing();
    let rows: Vec<MissingRow> = list
        .iter()
        .filter_map(|node| NodeId::parse(&node.id).ok())
        .map(|id| {
            let gone = lookup.describe_missing(id);
            MissingRow {
                was: gone.was,
                node_id: gone.id.to_string(),
                docs: gone.docs,
                path: gone.path,
            }
        })
        .collect();
    let answer = MissingAnswer {
        schema: 1,
        command: "missing",
        missing: rows,
    };
    match global.format {
        Some(Format::Id) => out::text(
            &answer
                .missing
                .iter()
                .map(|row| format!("{}\n", row.node_id))
                .collect::<String>(),
        ),
        Some(Format::Path) => {
            let mut seen = HashSet::new();
            out::text(
                &answer
                    .missing
                    .iter()
                    .filter(|row| !row.path.is_empty() && seen.insert(row.path.clone()))
                    .map(|row| format!("{}\n", row.path))
                    .collect::<String>(),
            )
        }
        None => out::render(&answer, global),
    }
}

/// A documented node whose code is not found: where it was, and its notes.
#[derive(Serialize)]
struct MissingRow {
    was: String,
    node_id: String,
    docs: Vec<Doc>,
    #[serde(skip)]
    path: String,
}

#[derive(Serialize)]
struct MissingAnswer {
    schema: u32,
    command: &'static str,
    missing: Vec<MissingRow>,
}

impl Render for MissingAnswer {
    fn text(&self, quiet: bool) -> String {
        if self.missing.is_empty() {
            return "nothing is Missing\n".to_string();
        }
        let mut text = String::new();
        for row in &self.missing {
            text.push_str(&format!(
                "MISSING  was {}  ID {}  docs {}\n",
                row.was,
                row.node_id,
                row.docs.len()
            ));
            if !quiet {
                text.push_str(&format!("  → vnoc relink {} <node>\n", row.node_id));
                text.push_str("  or remove it in VS Code if the code is gone\n");
            }
        }
        text
    }
}

pub async fn run(global: &Global, args: &super::args::CheckArgs) -> Result<(), Failure> {
    if global.format.is_some() {
        return Err(Failure::Usage(
            "check prints findings, not a list of nodes; --format is for list commands".into(),
        ));
    }
    // a project with no `.vnoc/` has nothing to check, and no driver is needed to say so
    let project = open_without_driver(global)?;
    if !project.git_root().join(".vnoc").is_dir() {
        return out::render(&CheckAnswer::of(false, Vec::new()), global);
    }
    let only = if args.changed {
        match changed_files(&project) {
            Ok(files) => Some(files),
            Err(why) => {
                out::note(&format!("{why}: checking everything"));
                None
            }
        }
    } else {
        None
    };
    drop(project);
    let opened = open_from(global).await?;
    let findings = check(
        &opened.index.read().unwrap(),
        &opened.project,
        only.as_deref(),
    );
    let answer = CheckAnswer::of(true, findings);
    out::render(&answer, global)?;
    if answer.failing > 0 {
        Err(Failure::CheckFailed)
    } else {
        Ok(())
    }
}

/// The files `git status` says differ from HEAD, as repo paths (git's paths are relative to the git root, as the index's are): staged, unstaged,
/// untracked, and **both** names of a rename (a Missing node is anchored at the old one). Why it could not be asked, as a sentence, when it cannot.
fn changed_files(project: &Project) -> Result<Vec<RepoPath>, String> {
    let root = project.git_root();
    if !root.join(".git").exists() {
        return Err(format!("{} is not a git repository", root.display()));
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "-z", "--untracked-files=all"])
        .output()
        .map_err(|e| format!("git could not be run ({e})"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git status failed ({})",
            said.lines().next().unwrap_or("no message").trim()
        ));
    }
    Ok(parse_status(&String::from_utf8_lossy(&output.stdout)))
}

/// `git status --porcelain -z`: entries `XY path`, and for a rename or a copy the old path as the next entry.
fn parse_status(text: &str) -> Vec<RepoPath> {
    let mut paths = Vec::new();
    let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
    while let Some(entry) = entries.next() {
        let (Some(status), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        paths.extend(RepoPath::new(path).ok());
        if status.contains(['R', 'C'])
            && let Some(old) = entries.next()
        {
            paths.extend(RepoPath::new(old).ok());
        }
    }
    paths
}

/// What `check` answers.
#[derive(Serialize)]
struct CheckAnswer {
    schema: u32,
    command: &'static str,
    /// whether the project has a `.vnoc/` folder: without one there is nothing to check
    initialized: bool,
    findings: Vec<Finding>,
    /// how many findings fail the check
    failing: usize,
    warnings: usize,
}

impl CheckAnswer {
    fn of(initialized: bool, findings: Vec<Finding>) -> CheckAnswer {
        let failing = findings.iter().filter(|f| f.fails).count();
        CheckAnswer {
            schema: 1,
            command: "check",
            initialized,
            warnings: findings.len() - failing,
            failing,
            findings,
        }
    }
}

impl Render for CheckAnswer {
    fn text(&self, quiet: bool) -> String {
        if !self.initialized {
            return if quiet {
                String::new()
            } else {
                "nothing to check (no .vnoc/ here)\n".to_string()
            };
        }
        let mut text = String::new();
        for finding in &self.findings {
            text.push_str(&format!(
                "{}  {}  {}\n",
                finding.kind.word(),
                finding.path,
                finding.message
            ));
            if !quiet {
                text.push_str(&format!("  → {}\n", finding.fix));
            }
        }
        if !quiet {
            text.push_str(&format!("{}\n", summary(&self.findings)));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str) -> Vec<String> {
        parse_status(text)
            .iter()
            .map(|p| p.as_str().to_string())
            .collect()
    }

    #[test]
    fn status_entries_are_staged_unstaged_untracked_and_both_names_of_a_rename() {
        let text = " M billing/tax.py\0A  new.py\0?? notes/todo.py\0R  orders/b.py\0orders/a.py\0 D gone.py\0";
        assert_eq!(
            paths(text),
            [
                "billing/tax.py",
                "new.py",
                "notes/todo.py",
                "orders/b.py",
                "orders/a.py",
                "gone.py"
            ]
        );
        assert!(paths("").is_empty());
        // a name with a space is not quoted under -z
        assert_eq!(paths("?? my docs/a b.py\0"), ["my docs/a b.py"]);
    }
}
