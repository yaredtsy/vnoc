//! `vnoc status` (03 §2): is V-NOC working here, and what does it know. A handler over `index::structure`: it builds nothing of its own.

use std::collections::BTreeMap;

use serde::Serialize;

use super::project::{Opened, open_from};
use super::{Failure, Global, out, style};
use crate::driver_host::LanguageState;
use crate::index;
use crate::protocol::{FailedFile, LostMarker, MissingNode, Problem, Stats};

#[derive(Serialize)]
struct Status {
    schema: u32,
    command: &'static str,
    version: &'static str,
    root: String,
    initialized: bool,
    /// the drivers that ran, or failed (a failed one is a row with its error, not the whole command's exit 3); a driver that read no file is not
    /// listed, so the list is empty when no file had to be read
    drivers: Vec<Driver>,
    stats: Stats,
    docs: Docs,
    missing: Vec<MissingNode>,
    /// nodes found by their name only, whose `ID:` line was lost (`check` fails on them)
    lost_markers: Vec<LostMarker>,
    problems: Vec<Problem>,
    failed: Vec<FailedFile>,
    truncated: bool,
    /// the skill and the rules block `vnoc agent install` wrote here (07 §2): the files are the record
    agents: crate::agents::Status,
}

#[derive(Serialize)]
struct Driver {
    name: String,
    version: String,
    /// the files this driver outlined
    files: usize,
    /// what it said it reads, without the dot (empty for a driver that never answered its handshake)
    extensions: Vec<String>,
    /// `running`, or `failed` (then `error` says why)
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// why part of what it said is not used (an extension another driver owns)
    warnings: Vec<String>,
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
    let languages = host.languages().await;
    let index = index.read().unwrap();
    let docs = index.doc_counts();
    let answer = index::structure(&index, project, BTreeMap::new());
    // one driver outlined every file the tree has; with several, a driver's files are the ones with an extension it owns
    let several = languages.len() > 1;
    let drivers = languages
        .into_iter()
        .map(|l| {
            let (state, error) = match l.state {
                LanguageState::Running => ("running", None),
                LanguageState::Failed(reason) => ("failed", Some(reason)),
            };
            Driver {
                files: if several {
                    index.files_with_extensions(&l.extensions)
                } else {
                    answer.stats.files
                },
                name: l.name,
                version: l.version,
                extensions: l.extensions,
                state,
                error,
                warnings: l.warnings,
            }
        })
        // a driver with no file to read is "not started": every driver is started now, so what says it is the count. A failed one is always shown.
        .filter(|d| d.files > 0 || d.error.is_some())
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
        agents: crate::agents::status(project.open_dir(), None, &crate::agents::Texts::built_in()),
    }
}

/// `1 file`, `3 files`: the number in bold, grouped by thousands.
fn count(n: usize, one: &str, many: &str) -> String {
    style::count(n, one, many)
}

/// One labelled line: `  code     …`. The label is dim and the same width in every line.
fn line(label: &str, rest: &str) -> String {
    format!(
        "  {}   {rest}\n",
        style::paint(style::dim(), format!("{label:<6}"))
    )
}

fn text(s: &Status, quiet: bool) -> String {
    let mut out = format!(
        "{}   {}\n\n",
        style::paint(style::bold(), format!("V-NOC {}", s.version)),
        style::paint(style::location(), &s.root)
    );
    out.push_str(&line(
        "code",
        &[
            count(s.stats.folders, "folder", "folders"),
            count(s.stats.files, "file", "files"),
            count(s.stats.classes, "class", "classes"),
            count(s.stats.functions, "function", "functions"),
        ]
        .join(" · "),
    ));
    if s.initialized {
        let docs = if s.docs.docs > 0 {
            format!(
                "{} on {}",
                count(s.docs.docs, "doc", "docs"),
                count(s.docs.nodes, "node", "nodes")
            )
        } else {
            "no docs".to_string()
        };
        out.push_str(&line("docs", &docs));
    } else {
        out.push_str(&line(
            "docs",
            &format!(
                "{}: no .vnoc/ in this project",
                style::paint(style::warn(), "none yet")
            ),
        ));
        // the second line of the cell, under the first
        out.push_str(&format!(
            "{}vnoc init starts it (ask the user first)\n",
            " ".repeat(11)
        ));
    }
    // health: what is wrong, or `all good`. Files that failed and a scan that stopped are on it too, with or without a `.vnoc/`.
    let mut health = Vec::new();
    if s.initialized {
        health.push(format!("{} missing", style::failing(s.missing.len())));
        let lost = s.lost_markers.len();
        health.push(format!(
            "{} lost {}",
            style::failing(lost),
            if lost == 1 { "marker" } else { "markers" }
        ));
        let problems = s.problems.len();
        health.push(format!(
            "{} {}",
            style::failing(problems),
            if problems == 1 { "problem" } else { "problems" }
        ));
    }
    if !s.failed.is_empty() {
        health.push(format!("failed {}", style::failing(s.failed.len())));
    }
    if s.truncated {
        health.push(style::paint(style::warn(), "stopped at 20,000 files"));
    }
    if !health.is_empty() {
        let wrong = s.missing.len() + s.lost_markers.len() + s.problems.len() + s.failed.len();
        let mut rest = if s.initialized && wrong == 0 && !s.truncated {
            style::paint(style::good(), "all good")
        } else {
            health.join(" · ")
        };
        if s.missing.len() + s.lost_markers.len() + s.problems.len() > 0 && !quiet {
            rest.push_str(&format!(
                "     {}",
                style::paint(style::dim(), "→ vnoc check")
            ));
        }
        out.push_str(&line("health", &rest));
    }
    if s.drivers.is_empty() {
        out.push_str(&line("driver", "not started (no Python files to read)"));
    }
    for d in &s.drivers {
        let shown = match &d.error {
            // the first line of the reason: the rest is in `--json`
            Some(reason) => format!(
                "{} · {} {}",
                d.name,
                style::paint(style::warn(), "failed:"),
                reason.lines().next().unwrap_or_default()
            ),
            None => format!(
                "{} {} · read {}",
                d.name,
                d.version,
                count(d.files, "file", "files")
            ),
        };
        out.push_str(&line("driver", &shown));
        for warning in &d.warnings {
            out.push_str(&line(
                "driver",
                &format!(
                    "{} · {} {warning}",
                    d.name,
                    style::paint(style::warn(), "warning:")
                ),
            ));
        }
    }
    out.push_str(&line("agents", &super::agent::summary(&s.agents)));
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
                extensions: vec!["py".into()],
                state: "running",
                error: None,
                warnings: vec![],
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
            agents: crate::agents::Status::default(),
        }
    }

    fn lost(n: usize) -> Vec<LostMarker> {
        (0..n)
            .map(|i| LostMarker {
                id: format!("ID{i}"),
                key: format!("a.py::f{i}"),
                kind: crate::index::Kind::Function,
                path: "a.py".into(),
                qualname: format!("f{i}"),
            })
            .collect()
    }

    fn missing(n: usize) -> Vec<MissingNode> {
        (0..n)
            .map(|i| MissingNode {
                id: format!("X{i}"),
                kind: None,
                path: "a.py".into(),
                qualname: "f".into(),
                docs: 1,
            })
            .collect()
    }

    #[test]
    fn a_project_with_nothing_wrong_is_all_good_and_has_no_hint() {
        assert_eq!(
            text(&status(true), false),
            "V-NOC 0.2.0   /r\n\n\
             \x20 code     3 folders · 10 files · 1 class · 21 functions\n\
             \x20 docs     7 docs on 7 nodes\n\
             \x20 health   all good\n\
             \x20 driver   python 0.2.0 · read 10 files\n\
             \x20 agents   not set up (vnoc agent install)\n"
        );
    }

    #[test]
    fn health_counts_each_thing_and_the_hint_is_for_a_problem_and_not_for_quiet() {
        let mut s = status(true);
        s.missing = missing(1);
        s.lost_markers = lost(1);
        let shown = text(&s, false);
        assert!(
            shown.contains("  health   1 missing · 1 lost marker · 0 problems     → vnoc check\n"),
            "{shown}"
        );
        s.lost_markers = lost(2);
        assert!(text(&s, true).contains("  health   1 missing · 2 lost markers · 0 problems\n"));
        assert!(!text(&s, true).contains("vnoc check"));
    }

    #[test]
    fn a_failed_file_and_a_stopped_scan_are_on_the_health_line_and_are_not_all_good() {
        let mut s = status(true);
        s.truncated = true;
        s.failed = vec![FailedFile {
            path: "b.py".into(),
            error: "x".into(),
        }];
        let shown = text(&s, false);
        assert!(
            shown.contains("  health   0 missing · 0 lost markers · 0 problems · failed 1 · stopped at 20,000 files\n"),
            "{shown}"
        );
        assert!(
            !shown.contains("vnoc check"),
            "no hint: nothing to check yet"
        );
    }

    #[test]
    fn a_project_without_a_vnoc_folder_says_none_yet_and_has_no_health_line() {
        assert_eq!(
            text(&status(false), false),
            "V-NOC 0.2.0   /r\n\n\
             \x20 code     3 folders · 10 files · 1 class · 21 functions\n\
             \x20 docs     none yet: no .vnoc/ in this project\n\
             \x20          vnoc init starts it (ask the user first)\n\
             \x20 driver   python 0.2.0 · read 10 files\n\
             \x20 agents   not set up (vnoc agent install)\n"
        );
        // but a file that failed is a health line, with or without `.vnoc/`
        let mut s = status(false);
        s.failed = vec![FailedFile {
            path: "b.py".into(),
            error: "x".into(),
        }];
        assert!(text(&s, false).contains("  health   failed 1\n"));
    }

    #[test]
    fn no_driver_started_says_so() {
        let mut s = status(true);
        s.drivers.clear();
        assert!(text(&s, false).contains("  driver   not started (no Python files to read)\n"));
        // the agents line is the last one
        assert!(text(&s, false).ends_with("  agents   not set up (vnoc agent install)\n"));
    }

    #[test]
    fn each_driver_has_its_own_line_a_failed_one_says_why_and_a_warning_is_a_line_of_its_own() {
        let mut s = status(true);
        s.drivers.insert(
            0,
            Driver {
                name: "bend".into(),
                version: "0.3.0".into(),
                files: 2,
                extensions: vec!["bend".into()],
                state: "running",
                error: None,
                warnings: vec![
                    "`.py` files go to `python`, which is first by name: `bend` claims them too and does not get them".into(),
                ],
            },
        );
        s.drivers.push(Driver {
            name: "ts".into(),
            version: String::new(),
            files: 0,
            extensions: vec![],
            state: "failed",
            error: Some("could not start /x/vnoc-driver-ts: not found\nsecond line".into()),
            warnings: vec![],
        });
        let shown = text(&s, false);
        assert!(
            shown.contains(
                "  health   all good\n\
                 \x20 driver   bend 0.3.0 · read 2 files\n\
                 \x20 driver   bend · warning: `.py` files go to `python`, which is first by name: `bend` claims them too and does not get them\n\
                 \x20 driver   python 0.2.0 · read 10 files\n\
                 \x20 driver   ts · failed: could not start /x/vnoc-driver-ts: not found\n\
                 \x20 agents   "
            ),
            "{shown}"
        );
        // JSON: every field is there, `error` only when there is one
        let json = serde_json::to_value(&s.drivers).unwrap();
        assert_eq!(json[0]["state"], "running");
        assert!(json[0].get("error").is_none());
        assert_eq!(json[0]["extensions"], serde_json::json!(["bend"]));
        assert_eq!(json[2]["state"], "failed");
        assert!(json[2]["error"].as_str().unwrap().ends_with("second line"));
    }

    #[test]
    fn counts_have_thousands_separators() {
        let mut s = status(true);
        s.stats.files = 1805;
        s.stats.functions = 12_345;
        s.docs.docs = 1000;
        let shown = text(&s, false);
        assert!(
            shown.contains("1,805 files · 1 class · 12,345 functions"),
            "{shown}"
        );
        assert!(shown.contains("1,000 docs on 7 nodes"), "{shown}");
        assert!(shown.contains("read 10 files"));
    }

    #[test]
    fn in_a_terminal_the_palette_marks_the_parts_and_the_words_are_the_same() {
        let mut s = status(true);
        s.missing = missing(1);
        let plain = text(&s, false);
        let painted = style::with_color(true, || text(&s, false));
        assert_ne!(plain, painted);
        // taking the codes out gives the plain text back, to the byte
        let stripped =
            String::from_utf8(anstream::adapter::strip_bytes(painted.as_bytes()).into_vec())
                .unwrap();
        assert_eq!(stripped, plain);
        assert!(
            painted.starts_with("\x1b[1mV-NOC 0.2.0\x1b[0m   \x1b[36m/r\x1b[0m\n\n"),
            "{painted:?}"
        );
        assert!(
            painted.contains("\x1b[2mcode  \x1b[0m"),
            "labels are dim: {painted:?}"
        );
        assert!(
            painted.contains("\x1b[31m1\x1b[0m missing"),
            "a failing count is red: {painted:?}"
        );
        assert!(
            painted.contains("\x1b[1m0\x1b[0m problems"),
            "a count of 0 is bold, not red: {painted:?}"
        );
        assert!(
            painted.contains("\x1b[2m→ vnoc check\x1b[0m"),
            "the hint is dim"
        );
    }
}
