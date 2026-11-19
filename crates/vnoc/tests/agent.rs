//! `vnoc agent install | list | status | remove` as a process (docs/21-agent-cli/07-install.md §2): the pick, what is printed, the exit codes,
//! and that nothing is written outside the folder given (or, with `--global`, the home folder). The files and the table are tested as data in
//! `agents.rs`; here a **temp folder is the project and another temp folder is the home**, and standard input is never a terminal.
//!
//! `vnoc status` (which shows one line about all this) needs the driver built first, as `cli.rs` does.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn driver_path() -> PathBuf {
    std::env::var_os("VNOC_DRIVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            repo_root()
                .join("drivers/python/target/debug")
                .join(format!(
                    "vnoc-driver-python{}",
                    std::env::consts::EXE_SUFFIX
                ))
        })
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-agent-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// A project folder and a home folder, both empty, outside this repository.
fn world(label: &str) -> (PathBuf, PathBuf) {
    (
        scratch(&format!("{label}-root")),
        scratch(&format!("{label}-home")),
    )
}

fn put(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn get(root: &Path, name: &str) -> String {
    std::fs::read_to_string(root.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// `vnoc <args>` in `root`, with `home` as the home folder and no terminal on standard input.
fn vnoc(root: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vnoc"))
        .current_dir(root)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("VNOC_DRIVER", driver_path())
        .env_remove("VNOC_LOG")
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .stdin(Stdio::null())
        .args(args)
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

/// Every file under `dir`, relative and sorted: what a command left behind.
fn files(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, into: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, base, into);
            } else {
                into.push(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

const SKILL: &str = include_str!("../../../skills/vnoc/SKILL.md");
const RULES: &str = include_str!("../../../skills/vnoc/rules.md");

// --------------------------------------------------------------------------------------------------------------------- install

#[test]
fn claude_and_codex_get_both_folders_and_one_block_and_the_lines_for_their_permissions() {
    let (root, home) = world("two");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    let permissions = out(&o)
        .split_once("\ncommit these files so your team's agents get them\n")
        .expect("the commit line")
        .1
        .to_string();
    assert_eq!(
        out(&o).split("\ncommit").next().unwrap(),
        "created      .claude/skills/vnoc/SKILL.md
             the skill, for claude
created      .agents/skills/vnoc/SKILL.md
             the skill, for codex
created      AGENTS.md
             the rules block, for claude, codex
"
    );
    // the lines: Claude Code's and Codex's, nothing about the writes
    assert!(permissions.contains("add to .claude/settings.json:"));
    assert!(permissions.contains("\"Bash(vnoc show *)\""));
    assert!(permissions.contains("add to ~/.codex/rules/default.rules:"));
    assert!(permissions.contains("prefix_rule(pattern = [\"vnoc\", [\"status\""));
    for write in [
        "vnoc init",
        "vnoc doc new",
        "vnoc doc tag",
        "vnoc relink",
        "vnoc repair",
        "vnoc agent",
    ] {
        assert!(!permissions.contains(write), "{write}");
    }
    assert_eq!(
        files(&root),
        [
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc/SKILL.md",
            "AGENTS.md"
        ]
    );
    assert_eq!(get(&root, ".claude/skills/vnoc/SKILL.md"), SKILL);
    assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), SKILL);
    assert_eq!(get(&root, "AGENTS.md"), RULES);
    assert!(files(&home).is_empty(), "nothing in the home folder");
    // twice: nothing changes, nothing is left, and the exit is 0
    let again = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex"],
    );
    assert_eq!(code(&again), 0);
    assert!(out(&again).starts_with("current      .claude/skills/vnoc/SKILL.md\n"));
    assert!(!out(&again).contains("commit these files"));
    assert_eq!(get(&root, "AGENTS.md"), RULES);
}

#[test]
fn an_agent_with_no_known_syntax_gets_no_permission_lines() {
    let (root, home) = world("cursor");
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "cursor"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).ends_with("\ncommit these files so your team's agents get them\n"));
    assert!(!out(&o).contains("settings.json") && !out(&o).contains("prefix_rule"));
    let quiet = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--quiet"],
    );
    assert!(!out(&quiet).contains("commit these files") && !out(&quiet).contains("Bash("));
}

#[test]
fn with_no_terminal_and_no_flag_it_takes_the_agents_found_here_or_asks_for_a_name() {
    // nothing found: exit 2, the names, and the command to type; nothing written
    let (root, home) = world("none");
    let o = vnoc(&root, &home, &["agent", "install"]);
    assert_eq!((code(&o), out(&o).as_str()), (2, ""));
    let e = err(&o);
    assert!(
        e.starts_with("error: no coding agent is found in this project"),
        "{e}"
    );
    assert!(e.contains("vnoc agent install --agent <name>"));
    for name in [
        "claude", "cline", "codex", "cursor", "pi", "opencode", "copilot", "amp", "goose",
        "windsurf", "gemini", "other",
    ] {
        assert!(e.contains(name), "{name} in {e}");
    }
    assert!(files(&root).is_empty());
    // a `.claude/` folder: claude
    put(&root, ".claude/settings.json", "{}");
    let o = vnoc(&root, &home, &["agent", "install"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(root.join(".claude/skills/vnoc/SKILL.md").is_file());
    assert!(!root.join(".agents").exists(), "only the agent found");
    assert!(root.join("AGENTS.md").is_file());
}

#[test]
fn all_names_unknown_agents_and_conflicting_flags_are_refused() {
    let (root, home) = world("all");
    let o = vnoc(&root, &home, &["agent", "install", "--all"]);
    assert_eq!(code(&o), 0);
    assert_eq!(
        files(&root),
        [
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc/SKILL.md",
            "AGENTS.md"
        ]
    );
    let (root, home) = world("bad");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,vscode"],
    );
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("`vscode` is not an agent V-NOC knows"));
    assert!(err(&o).contains("Any other agent is `other`"));
    assert!(
        files(&root).is_empty(),
        "nothing is written for a pick that is refused"
    );
    for args in [
        &["agent", "install", "--all", "--agent", "claude"][..],
        &[
            "agent",
            "install",
            "--skill-only",
            "--rules-only",
            "--agent",
            "claude",
        ],
    ] {
        assert_eq!(code(&vnoc(&root, &home, args)), 2, "{args:?}");
    }
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--global",
            "--rules-only",
            "--agent",
            "claude",
        ],
    );
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("--global writes no rules"));
    assert_eq!(
        code(&vnoc(
            &root,
            &home,
            &["agent", "install", "--format", "id", "--agent", "claude"]
        )),
        2
    );
    assert!(files(&root).is_empty() && files(&home).is_empty());
}

#[test]
fn a_hand_written_skill_is_left_with_exit_2_and_force_replaces_it() {
    let (root, home) = world("hand");
    let mine = "---\nname: vnoc\ndescription: mine\n---\nhand-written\n";
    put(&root, ".agents/skills/vnoc/SKILL.md", mine);
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    assert_eq!(code(&o), 2);
    assert_eq!(
        err(&o),
        "",
        "what was left is said in its place, on standard output"
    );
    assert!(
        out(&o).starts_with("left         .agents/skills/vnoc/SKILL.md\n"),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("it was not written by V-NOC"));
    assert!(out(&o).contains("--force replaces it"));
    assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), mine);
    assert_eq!(
        get(&root, "AGENTS.md"),
        RULES,
        "the rules were written all the same"
    );
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--force"],
    );
    assert_eq!(code(&o), 0);
    assert!(out(&o).starts_with("replaced     .agents/skills/vnoc/SKILL.md\n"));
    assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), SKILL);
    // an AGENTS.md of the user's: the block is added and the rest is as it was
    let (root, home) = world("hand-rules");
    put(&root, "AGENTS.md", "# Mine\n\nUse tabs.\n");
    assert_eq!(
        code(&vnoc(&root, &home, &["agent", "install", "--agent", "pi"])),
        0
    );
    assert_eq!(
        get(&root, "AGENTS.md"),
        format!("# Mine\n\nUse tabs.\n\n{RULES}")
    );
}

#[test]
fn dry_run_writes_nothing_and_json_lists_the_actions() {
    let (root, home) = world("dry");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,gemini", "--dry-run"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        out(&o).contains("will create  .claude/skills/vnoc/SKILL.md\n"),
        "{}",
        out(&o)
    );
    assert!(!out(&o).contains("commit these files"));
    assert!(files(&root).is_empty() && files(&home).is_empty());
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude,gemini",
            "--dry-run",
            "--json",
        ],
    );
    assert_eq!(code(&o), 0);
    let json: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(json["schema"], 1);
    assert_eq!(json["command"], "agent install");
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["agents"], json!(["claude", "gemini"]));
    let actions: Vec<(String, String, String)> = json["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["part"].as_str().unwrap().into(),
                a["path"].as_str().unwrap().into(),
                a["change"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        actions,
        [
            (
                "skill".to_string(),
                ".claude/skills/vnoc/SKILL.md".to_string(),
                "create".to_string()
            ),
            (
                "skill".to_string(),
                ".agents/skills/vnoc/SKILL.md".to_string(),
                "create".to_string()
            ),
            (
                "rules".to_string(),
                "AGENTS.md".to_string(),
                "create".to_string()
            ),
            (
                "rules".to_string(),
                "GEMINI.md".to_string(),
                "create".to_string()
            ),
        ]
    );
    assert_eq!(json["permissions"][0]["agent"], "claude");
    assert!(files(&root).is_empty());
}

#[test]
fn global_writes_the_skill_in_the_home_folder_and_nothing_in_the_project() {
    let (root, home) = world("global");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex", "--global"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert_eq!(
        out(&o),
        "created      ~/.claude/skills/vnoc/SKILL.md
             the skill, for claude
created      ~/.agents/skills/vnoc/SKILL.md
             the skill, for codex
"
    );
    assert!(files(&root).is_empty(), "nothing in the project");
    assert_eq!(
        files(&home),
        [
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc/SKILL.md"
        ]
    );
    // `status` sees them, and `remove --global` takes them back
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert!(
        out(&o).contains("~/.claude/skills/vnoc/SKILL.md"),
        "{}",
        out(&o)
    );
    let o = vnoc(&root, &home, &["agent", "remove", "--global"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).starts_with("deleted      ~/.claude/skills/vnoc/SKILL.md\n"));
    assert!(files(&home).is_empty());
}

// ------------------------------------------------------------------------------------------------------ list, status, remove

#[test]
fn list_names_the_agents_which_are_found_and_what_each_would_get() {
    let (root, home) = world("list");
    put(&root, ".cursor/rules/a.mdc", "x");
    put(&root, "CLAUDE.md", "x");
    let o = vnoc(&root, &home, &["agent", "list"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    let text = out(&o);
    let line = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap()
            .to_string()
    };
    assert_eq!(
        line("claude "),
        "claude    found here  .claude/skills/vnoc/SKILL.md  CLAUDE.md"
    );
    assert_eq!(
        line("cursor"),
        "cursor    found here  .agents/skills/vnoc/SKILL.md  AGENTS.md"
    );
    assert_eq!(
        line("gemini"),
        "gemini                .agents/skills/vnoc/SKILL.md  GEMINI.md"
    );
    assert_eq!(
        text.lines()
            .filter(|l| !l.is_empty() && !l.starts_with("vnoc "))
            .count(),
        12
    );
    let json: Value =
        serde_json::from_slice(&vnoc(&root, &home, &["agent", "list", "--json"]).stdout).unwrap();
    assert_eq!(json["command"], "agent list");
    let found: Vec<&str> = json["agents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["detected"] == true)
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(found, ["claude", "cursor"]);
    assert_eq!(json["agents"][0]["rules"], "CLAUDE.md");
    assert!(files(&root).len() == 2, "list writes nothing");
}

#[test]
fn status_says_current_older_changed_and_missing_and_remove_takes_back_only_what_was_written() {
    let (root, home) = world("status");
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert_eq!(
        (code(&o), out(&o).as_str()),
        (0, "not set up: vnoc agent install\n")
    );
    put(&root, "AGENTS.md", "# Mine\n\nUse tabs.\n");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert_eq!(code(&o), 0);
    assert!(
        out(&o).starts_with(
            "current      .agents/skills/vnoc/SKILL.md\n             the skill, read by "
        ),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("current      AGENTS.md\n             the rules block, read by "));
    // older: the version of the installed copy edited
    let version = env!("CARGO_PKG_VERSION");
    put(
        &root,
        ".agents/skills/vnoc/SKILL.md",
        &SKILL.replace(
            &format!("vnoc-version: \"{version}\""),
            "vnoc-version: \"0.0.1\"",
        ),
    );
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert!(
        out(&o).contains(&format!("older        .agents/skills/vnoc/SKILL.md\n             the skill, it is 0.0.1, this vnoc is {version}: vnoc agent install\n")),
        "{}",
        out(&o)
    );
    let json: Value =
        serde_json::from_slice(&vnoc(&root, &home, &["agent", "status", "--json"]).stdout).unwrap();
    assert_eq!(json["set_up"], true);
    assert_eq!(json["files"][0]["state"], "older");
    assert_eq!(json["files"][0]["version"], "0.0.1");
    // reinstalling brings it up, and a hand edit is `changed`
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    put(
        &root,
        ".agents/skills/vnoc/SKILL.md",
        &format!("{SKILL}\nand say please\n"),
    );
    assert!(out(&vnoc(&root, &home, &["agent", "status"])).contains("changed      .agents/skills/vnoc/SKILL.md\n             the skill, by hand: vnoc agent install replaces it\n"));
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    // the skill with no rules, and the rules with no skill
    vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!(
        get(&root, "AGENTS.md"),
        "# Mine\n\nUse tabs.\n",
        "the user's text is as it was"
    );
    assert!(!root.join(".agents/skills/vnoc").exists());
    assert_eq!(
        out(&vnoc(&root, &home, &["agent", "status"])),
        "not set up: vnoc agent install\n"
    );
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--skill-only"],
    );
    assert!(out(&vnoc(&root, &home, &["agent", "status"])).contains("missing      the rules block\n             nothing carries it: vnoc agent install --rules-only\n"));
}

#[test]
fn remove_deletes_nothing_it_did_not_write() {
    let (root, home) = world("remove");
    let mine = "---\nname: vnoc\n---\nhand-written\n";
    put(&root, ".claude/skills/vnoc/SKILL.md", mine);
    put(&root, "CLAUDE.md", "# Mine\n");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex,gemini"],
    );
    assert!(root.join("GEMINI.md").is_file());
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    let text = out(&o);
    assert!(
        text.contains("deleted      .agents/skills/vnoc/SKILL.md\n"),
        "{text}"
    );
    assert!(
        text.contains("left         .claude/skills/vnoc/SKILL.md\n"),
        "{text}"
    );
    assert!(text.contains("removed      AGENTS.md\n") || text.contains("deleted      AGENTS.md\n"));
    assert_eq!(get(&root, ".claude/skills/vnoc/SKILL.md"), mine);
    assert_eq!(get(&root, "CLAUDE.md"), "# Mine\n");
    assert!(!root.join("GEMINI.md").exists());
    assert!(
        !root.join("AGENTS.md").exists(),
        "it held nothing but the block"
    );
    // nothing more to remove
    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "codex"]);
    assert_eq!((code(&o), out(&o).as_str()), (0, "nothing to remove\n"));
    assert_eq!(
        code(&vnoc(
            &root,
            &home,
            &["agent", "remove", "--agent", "vscode"]
        )),
        2
    );
}

#[test]
fn removing_for_one_agent_says_who_else_reads_what_it_takes_back() {
    let (root, home) = world("remove-shared");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex,cursor"],
    );
    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "codex"]);
    assert_eq!(code(&o), 0);
    let text = out(&o);
    assert!(
        text.starts_with(
            "deleted      .agents/skills/vnoc/SKILL.md\n             the skill; also read by "
        ),
        "{text}"
    );
    assert!(text.contains("cursor"), "{text}");
    assert!(!text.contains("also read by codex"), "{text}");
    assert!(
        files(&root).is_empty(),
        "the shared skill and the block were taken"
    );
}

// ---------------------------------------------------------------------------------------------------------------- vnoc status

fn sample(label: &str) -> PathBuf {
    let root = scratch(label);
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample"),
        &root,
    );
    root
}

#[test]
fn vnoc_status_has_a_line_for_the_agents() {
    let root = sample("status-line");
    let home = scratch("status-line-home");
    let line = |args: &[&str]| -> String {
        let o = vnoc(&root, &home, args);
        assert_eq!(code(&o), 0, "{}", err(&o));
        out(&o)
            .lines()
            .find(|l| l.trim_start().starts_with("agents"))
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(
        line(&["status"]),
        "  agents   not set up (vnoc agent install)"
    );
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex"],
    );
    assert_eq!(
        line(&["status"]),
        "  agents   skill current (.claude/skills, .agents/skills) · rules in AGENTS.md"
    );
    let json: Value =
        serde_json::from_slice(&vnoc(&root, &home, &["status", "--json"]).stdout).unwrap();
    assert_eq!(json["agents"]["files"].as_array().unwrap().len(), 3);
    assert_eq!(json["agents"]["files"][0]["state"], "current");
    assert_eq!(json["agents"]["missing"], json!([]));
    // an older skill says what to run
    let version = env!("CARGO_PKG_VERSION");
    put(
        &root,
        ".agents/skills/vnoc/SKILL.md",
        &SKILL.replace(
            &format!("vnoc-version: \"{version}\""),
            "vnoc-version: \"0.0.1\"",
        ),
    );
    assert_eq!(
        line(&["status"]),
        format!(
            "  agents   skill older (0.0.1 → {version}): vnoc agent install · rules in AGENTS.md"
        )
    );
    // rules and no skill
    vnoc(&root, &home, &["agent", "remove"]);
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--rules-only"],
    );
    assert_eq!(
        line(&["status"]),
        "  agents   rules in AGENTS.md · skill missing: vnoc agent install --skill-only"
    );
}
