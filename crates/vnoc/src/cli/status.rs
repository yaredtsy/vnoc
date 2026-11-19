//! `vnoc status` (03 §2): is V-NOC working here, and what does it know. A handler over `index::structure`: it builds nothing of its own.

use std::collections::BTreeMap;

use serde::Serialize;

use super::project::{Opened, open_from};
use super::{Failure, Global, out};
use crate::index;
use crate::protocol::{FailedFile, LostMarker, MissingNode, Problem, Stats};

#[derive(Serialize)]
struct Status {
    schema: u32,
    command: &'static str,
    version: &'static str,
    root: String,
    initialized: bool,
    /// the drivers that ran: one today, and none when no file had to be read (a folder with no Python files starts no driver)
    drivers: Vec<Driver>,
    stats: Stats,
    docs: Docs,
    missing: Vec<MissingNode>,
    /// nodes found by their name only, whose `ID:` line was lost (`check` fails on them)
    lost_markers: Vec<LostMarker>,
    problems: Vec<Problem>,
    failed: Vec<FailedFile>,
    truncated: bool,
}

#[derive(Serialize)]
struct Driver {
    name: String,
    version: String,
    /// the files this driver outlined
    files: usize,
}

#[derive(Serialize)]
struct Docs {
    docs: usize,
    nodes: usize,
}

pub async fn run(global: &Global) -> Result<(), Failure> {
    if global.format.is_some() {
        return Err(Failure::Usage(
            "status prints no list; --format is for list commands".into(),
        ));
    }
    let opened = open_from(global).await?;
    out::render(&status_of(&opened).await, global)
}

impl out::Render for Status {
    fn text(&self, quiet: bool) -> String {
        text(self, quiet)
    }
}

async fn status_of(opened: &Opened) -> Status {
    let Opened {
        project,
        host,
        index,
    } = opened;
    let info = host.info().await;
    let index = index.read().unwrap();
    let docs = index.doc_counts();
    let answer = index::structure(&index, project, BTreeMap::new());
    // there is one driver, and it outlined every file the tree has
    let drivers = info
        .map(|(name, version)| Driver {
            name,
            version,
            files: answer.stats.files,
        })
        .into_iter()
        .collect();
    Status {
        schema: 1,
        command: "status",
        version: env!("CARGO_PKG_VERSION"),
        // the one absolute path a command prints: it is the root itself
        root: project.open_dir().to_string_lossy().into_owned(),
        initialized: project.git_root().join(".vnoc").is_dir(),
        drivers,
        stats: answer.stats,
        docs: Docs {
            docs: docs.0,
            nodes: docs.1,
        },
        missing: answer.missing,
        lost_markers: index.lost_markers(),
        problems: answer.problems,
        failed: answer.failed,
        truncated: answer.truncated,
    }
}

/// `1 file`, `3 files`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn text(s: &Status, quiet: bool) -> String {
    let driver = if s.drivers.is_empty() {
        "driver not started (no Python files to read)".to_string()
    } else {
        let each: Vec<String> = s
            .drivers
            .iter()
            .map(|d| {
                format!(
                    "{} {} ({})",
                    d.name,
                    d.version,
                    count(d.files, "file", "files")
                )
            })
            .collect();
        format!("drivers {}", each.join(", "))
    };
    let docs = if s.initialized && s.docs.docs > 0 {
        format!(
            "{} on {}",
            count(s.docs.docs, "doc", "docs"),
            count(s.docs.nodes, "node", "nodes")
        )
    } else {
        "no docs".to_string()
    };
    let mut out = format!(
        "V-NOC {}   root {}   {driver}\n{} · {} · {} · {} · {docs}\n",
        s.version,
        s.root,
        count(s.stats.folders, "folder", "folders"),
        count(s.stats.files, "file", "files"),
        count(s.stats.classes, "class", "classes"),
        count(s.stats.functions, "function", "functions"),
    );
    let mut third = Vec::new();
    if s.initialized {
        third.push(format!("missing {}", s.missing.len()));
        third.push(format!("lost markers {}", s.lost_markers.len()));
        third.push(format!("problems {}", s.problems.len()));
    }
    if !s.failed.is_empty() {
        third.push(format!("failed {}", s.failed.len()));
    }
    if s.truncated {
        third.push("stopped at 20,000 files".to_string());
    }
    if !third.is_empty() {
        out.push_str(&third.join(" · "));
        if s.missing.len() + s.lost_markers.len() + s.problems.len() > 0 && !quiet {
            out.push_str("      → vnoc check");
        }
        out.push('\n');
    }
    if !s.initialized {
        out.push_str(
            "no .vnoc/ here yet: nothing is documented (vnoc init starts it; ask the user first)\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(initialized: bool) -> Status {
        Status {
            schema: 1,
            command: "status",
            version: "0.2.0",
            root: "/r".into(),
            initialized,
            drivers: vec![Driver {
                name: "python".into(),
                version: "0.2.0".into(),
                files: 10,
            }],
            stats: Stats {
                folders: 3,
                files: 10,
                classes: 1,
                functions: 21,
            },
            docs: Docs { docs: 7, nodes: 7 },
            missing: vec![],
            lost_markers: vec![],
            problems: vec![],
            failed: vec![],
            truncated: false,
        }
    }

    #[test]
    fn the_lines_of_a_project_with_docs() {
        assert_eq!(
            text(&status(true), false),
            "V-NOC 0.2.0   root /r   drivers python 0.2.0 (10 files)\n\
             3 folders · 10 files · 1 class · 21 functions · 7 docs on 7 nodes\n\
             missing 0 · lost markers 0 · problems 0\n"
        );
    }

    #[test]
    fn a_project_without_a_vnoc_folder_says_so_and_has_no_third_line() {
        assert_eq!(
            text(&status(false), false),
            "V-NOC 0.2.0   root /r   drivers python 0.2.0 (10 files)\n\
             3 folders · 10 files · 1 class · 21 functions · no docs\n\
             no .vnoc/ here yet: nothing is documented (vnoc init starts it; ask the user first)\n"
        );
    }

    #[test]
    fn the_hint_is_for_a_problem_and_not_for_quiet() {
        let mut s = status(true);
        s.missing.push(MissingNode {
            id: "X".into(),
            kind: None,
            path: "a.py".into(),
            qualname: "f".into(),
            docs: 1,
        });
        s.truncated = true;
        assert!(text(&s, false).contains(
            "missing 1 · lost markers 0 · problems 0 · stopped at 20,000 files      → vnoc check\n"
        ));
        assert!(!text(&s, true).contains("vnoc check"));
    }
}
