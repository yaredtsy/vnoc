//! `vnoc agent install | list | status | remove` as a process (docs/21-agent-cli/07-install.md §2): the pick, what is printed, the exit codes,
//! and that nothing is written outside the folder given (or, with `--global`, the home folder). The files and the table are tested as data in
//! `agents.rs`; here a **temp folder is the project and another temp folder is the home**, and standard input is never a terminal.
//!
//! `vnoc status` (which shows one line about all this) needs the driver built first, as `cli.rs` does.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
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
    dunce::canonicalize(dir).unwrap()
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
                let parts: Vec<_> = path
                    .strip_prefix(base)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                into.push(parts.join("/"));
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
const TOUR_SKILL: &str = include_str!("../../../skills/vnoc-tour/SKILL.md");

/// The commands of the writing loop that the allow rules cover (04 §5), and the ones they leave to the user.
const TOUR_ALLOWED: [&str; 12] = [
    "start", "add", "finish", "plan", "drop", "resume", "list", "show", "check", "where", "route",
    "export",
];
const TOUR_ASKED: [&str; 9] = [
    "go", "pin", "ids", "edit", "share", "delete", "import", "settings", "halt",
];
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
created      .claude/skills/vnoc-tour/SKILL.md
             the vnoc-tour skill, for claude
created      .agents/skills/vnoc/SKILL.md
             the skill, for codex
created      .agents/skills/vnoc-tour/SKILL.md
             the vnoc-tour skill, for codex
created      AGENTS.md
             the rules block, for claude, codex
created      CLAUDE.md
             imports AGENTS.md, so every Claude Code reads the rules
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
            ".agents/skills/vnoc-tour/SKILL.md",
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc-tour/SKILL.md",
            ".claude/skills/vnoc/SKILL.md",
            "AGENTS.md",
            "CLAUDE.md"
        ]
    );
    assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n");
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
            ".agents/skills/vnoc-tour/SKILL.md",
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc-tour/SKILL.md",
            ".claude/skills/vnoc/SKILL.md",
            "AGENTS.md",
            "CLAUDE.md"
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
                "tour_skill".to_string(),
                ".claude/skills/vnoc-tour/SKILL.md".to_string(),
                "create".to_string()
            ),
            (
                "skill".to_string(),
                ".agents/skills/vnoc/SKILL.md".to_string(),
                "create".to_string()
            ),
            (
                "tour_skill".to_string(),
                ".agents/skills/vnoc-tour/SKILL.md".to_string(),
                "create".to_string()
            ),
            (
                "rules".to_string(),
                "AGENTS.md".to_string(),
                "create".to_string()
            ),
            (
                "import".to_string(),
                "CLAUDE.md".to_string(),
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
created      ~/.claude/skills/vnoc-tour/SKILL.md
             the vnoc-tour skill, for claude
created      ~/.agents/skills/vnoc/SKILL.md
             the skill, for codex
created      ~/.agents/skills/vnoc-tour/SKILL.md
             the vnoc-tour skill, for codex
"
    );
    assert!(files(&root).is_empty(), "nothing in the project");
    assert_eq!(
        files(&home),
        [
            ".agents/skills/vnoc-tour/SKILL.md",
            ".agents/skills/vnoc/SKILL.md",
            ".claude/skills/vnoc-tour/SKILL.md",
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
    assert_eq!(json["agents"]["files"].as_array().unwrap().len(), 6);
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

// ------------------------------------------------------------------------------------------------------ the import file for `claude`

const IMPORT_LINES: &str = "             imports AGENTS.md, so every Claude Code reads the rules\n";

/// A project with only an `AGENTS.md` of somebody's own text.
fn only_agents_md(label: &str) -> (PathBuf, PathBuf, &'static str) {
    let (root, home) = world(label);
    let original = "# Mine\n\nSome rules.\n";
    put(&root, "AGENTS.md", original);
    (root, home, original)
}

#[test]
fn claude_with_no_claude_md_gets_a_one_line_one_that_imports_agents_md() {
    let (root, home, original) = only_agents_md("import");
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        out(&o).contains(&format!("created      CLAUDE.md\n{IMPORT_LINES}")),
        "{}",
        out(&o)
    );
    assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n");
    let rules = get(&root, "AGENTS.md");
    assert!(rules.starts_with(original) && rules.contains(RULES.trim_end()));
    // the block is in one file: the import file holds nothing else
    assert_eq!(rules.matches("<!-- vnoc:start").count(), 1);

    // status names it, and it is current
    let status = out(&vnoc(&root, &home, &["agent", "status"]));
    assert!(
        status.contains(
            "current      CLAUDE.md\n             the import of AGENTS.md, read by claude\n"
        ),
        "{status}"
    );
    // twice: nothing changes, and it says so
    let again = vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    assert_eq!(code(&again), 0);
    assert!(
        out(&again).contains(&format!("current      CLAUDE.md\n{IMPORT_LINES}")),
        "{}",
        out(&again)
    );
    assert!(!out(&again).contains("commit these files"));
    assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n");
    assert_eq!(get(&root, "AGENTS.md"), rules);

    // remove takes it back, and AGENTS.md is what it was
    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "claude"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        out(&o).contains(&format!("deleted      CLAUDE.md\n{IMPORT_LINES}")),
        "{}",
        out(&o)
    );
    assert!(!root.join("CLAUDE.md").exists());
    assert_eq!(get(&root, "AGENTS.md"), original);
}

#[test]
fn remove_keeps_an_import_file_a_person_added_a_line_to_and_says_so() {
    let (root, home, original) = only_agents_md("import-edited");
    vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    put(&root, "CLAUDE.md", "@AGENTS.md\nAnd always say please.\n");
    // status: no longer the line V-NOC wrote
    let status = out(&vnoc(&root, &home, &["agent", "status"]));
    assert!(status.contains("changed      CLAUDE.md\n"), "{status}");
    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "claude"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        out(&o).contains("left         CLAUDE.md\n")
            && out(&o)
                .contains("it has more than the one import line V-NOC wrote, so it is not removed"),
        "{}",
        out(&o)
    );
    assert_eq!(
        get(&root, "CLAUDE.md"),
        "@AGENTS.md\nAnd always say please.\n"
    );
    assert_eq!(get(&root, "AGENTS.md"), original);
    // an identical file that no block of V-NOC's goes with is a person's own: it is not removed either
    let (root, home) = world("import-theirs");
    put(&root, "CLAUDE.md", "@AGENTS.md\n");
    put(&root, "AGENTS.md", "mine\n");
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n");
}

#[test]
fn a_claude_md_that_is_there_or_one_in_dot_claude_gets_no_import_file() {
    // an existing CLAUDE.md takes the block, as before
    let (root, home) = world("import-existing");
    put(&root, "CLAUDE.md", "# Claude's own\n");
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(get(&root, "CLAUDE.md").starts_with("# Claude's own\n"));
    assert!(get(&root, "CLAUDE.md").contains(RULES.trim_end()));
    assert!(!root.join("AGENTS.md").exists());
    assert!(!out(&o).contains("imports AGENTS.md"), "{}", out(&o));
    // .claude/CLAUDE.md is Claude Code's too: the block goes there, and no CLAUDE.md is made
    let (root, home) = world("import-dot-claude");
    put(&root, ".claude/CLAUDE.md", "# in the folder\n");
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(!root.join("CLAUDE.md").exists());
    assert!(get(&root, ".claude/CLAUDE.md").contains(RULES.trim_end()));
    // another agent alone never makes one
    let (root, home, _) = only_agents_md("import-codex");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    assert!(!root.join("CLAUDE.md").exists());
    assert!(root.join("AGENTS.md").is_file());
}

#[test]
fn dry_run_lists_the_import_file_and_writes_nothing() {
    let (root, home, original) = only_agents_md("import-dry");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--dry-run"],
    );
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(
        out(&o).contains(&format!("will create  CLAUDE.md\n{IMPORT_LINES}")),
        "{}",
        out(&o)
    );
    assert!(!root.join("CLAUDE.md").exists());
    assert!(!root.join(".claude").exists());
    assert_eq!(get(&root, "AGENTS.md"), original);
}

#[test]
fn all_writes_the_import_file_too_and_the_json_names_it() {
    let (root, home, _) = only_agents_md("import-all");
    let o = vnoc(&root, &home, &["agent", "install", "--all", "--json"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let json: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(
        json["actions"].as_array().unwrap().iter().any(|a| {
            a["part"] == "import" && a["path"] == "CLAUDE.md" && a["change"] == "create"
        }),
        "{json}"
    );
    assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n");
}

// ------------------------------------------------------------------------------------------ remove leaves no empty folder it made (A11f)

fn exists(root: &Path, name: &str) -> bool {
    root.join(name).exists()
}

#[test]
fn remove_on_a_fresh_install_leaves_no_folder_it_made() {
    let (root, home) = world("remove-tidy");
    let o = vnoc(&root, &home, &["agent", "install", "--all"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        exists(&root, ".claude/skills/vnoc/SKILL.md")
            && exists(&root, ".agents/skills/vnoc/SKILL.md")
    );
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(!exists(&root, ".claude"), "{:?}", files(&root));
    assert!(!exists(&root, ".agents"), "{:?}", files(&root));
    assert!(files(&root).is_empty(), "{:?}", files(&root));
    // `--global` goes the same way in the home folder
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex", "--global"],
    );
    assert_eq!(code(&o), 0, "{}", err(&o));
    let o = vnoc(&root, &home, &["agent", "remove", "--global"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(files(&home).is_empty(), "{:?}", files(&home));
    assert!(!exists(&home, ".claude") && !exists(&home, ".agents"));
}

#[test]
fn remove_never_removes_a_folder_with_anything_left_in_it() {
    let (root, home) = world("remove-keeps");
    put(&root, ".claude/settings.json", "{}\n");
    put(&root, ".agents/notes.txt", "mine\n");
    put(
        &root,
        ".claude/skills/other/SKILL.md",
        "---\nname: other\n---\n",
    );
    vnoc(&root, &home, &["agent", "install", "--all"]);
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    // the skill folders V-NOC made are gone, and everything that was there stays, with the folders that hold it
    assert!(!exists(&root, ".claude/skills/vnoc") && !exists(&root, ".agents/skills"));
    assert_eq!(get(&root, ".claude/settings.json"), "{}\n");
    assert!(exists(&root, ".claude/skills/other/SKILL.md"));
    assert_eq!(get(&root, ".agents/notes.txt"), "mine\n");
    // with only a settings file beside it: `.claude/` stays, and `skills/` goes
    let (root, home) = world("remove-settings");
    put(&root, ".claude/settings.json", "{}\n");
    vnoc(&root, &home, &["agent", "install", "--agent", "claude"]);
    vnoc(&root, &home, &["agent", "remove", "--agent", "claude"]);
    assert_eq!(files(&root), [".claude/settings.json"]);
}

// ------------------------------------------------------------------------------------------------ the permissions (07 §4, A12c)
// The question itself needs a terminal and is checked by hand; here the flags, and what follows from there being no terminal.

const SETTINGS_LOCAL: &str = ".claude/settings.local.json";

fn allow_in(root: &Path, name: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(&get(root, name)).unwrap();
    v["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn permissions_writes_the_entries_to_the_local_settings_file_and_says_so() {
    let (root, home) = world("perm-local");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert_eq!(
        out(&o),
        "created      .claude/skills/vnoc/SKILL.md
             the skill, for claude
created      .claude/skills/vnoc-tour/SKILL.md
             the vnoc-tour skill, for claude
created      AGENTS.md
             the rules block, for claude
created      CLAUDE.md
             imports AGENTS.md, so every Claude Code reads the rules
allowed      V-NOC's read commands and the tour commands for Claude Code
in           .claude/settings.local.json

commit these files so your team's agents get them
"
    );
    let entries = allow_in(&root, SETTINGS_LOCAL);
    assert_eq!(entries.len(), 27);
    assert!(entries.contains(&"Bash(vnoc show *)".to_string()));
    assert!(entries.iter().all(|e| e.starts_with("Bash(vnoc ")));
    assert!(
        !entries
            .iter()
            .any(|e| e.contains("agent") || e.contains("doc new"))
    );
    assert!(!exists(&root, ".claude/settings.json"));
    // the lines are not printed when they were written; twice changes nothing
    assert!(!out(&o).contains("to let Claude Code run"));
    let again = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    assert_eq!(code(&again), 0);
    assert!(
        out(&again)
            .contains("current      V-NOC's read commands and the tour commands for Claude Code\n"),
        "{}",
        out(&again)
    );
    assert_eq!(allow_in(&root, SETTINGS_LOCAL), entries);
    // only the settings file of this user written: nothing for the team to commit
    let only = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--permissions",
            "--skill-only",
            "--dry-run",
        ],
    );
    assert_eq!(code(&only), 0);
    let (root2, home2) = world("perm-local-only");
    let o = vnoc(
        &root2,
        &home2,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--rules-only",
            "--permissions",
        ],
    );
    assert!(
        out(&o)
            .contains("allowed      V-NOC's read commands and the tour commands for Claude Code")
    );
    // the rules were written, so the team has a file; the settings alone would not say it
    assert!(out(&o).contains("commit these files"));
    std::fs::remove_file(root2.join("AGENTS.md")).unwrap();
}

#[test]
fn permissions_project_writes_the_shared_file_and_not_the_local_one() {
    let (root, home) = world("perm-project");
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--permissions=project",
        ],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(out(&o).contains("allowed      V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.json\n"), "{}", out(&o));
    assert_eq!(allow_in(&root, ".claude/settings.json").len(), 27);
    assert!(!exists(&root, SETTINGS_LOCAL));
    // an unknown scope is a usage error
    let bad = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--permissions=world",
        ],
    );
    assert_eq!(code(&bad), 2);
    // the flag takes its value with `=`: `--permissions project` would be a folder-less argument, not the scope
    let both = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--permissions",
            "--no-permissions",
        ],
    );
    assert_eq!(code(&both), 2);
    let global = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--global",
            "--permissions",
        ],
    );
    assert_eq!(code(&global), 2);
}

#[test]
fn the_user_s_own_entries_and_keys_stay_and_nothing_is_added_twice() {
    let (root, home) = world("perm-merge");
    let before = "{\n\t\"model\": \"opus\",\n\t\"permissions\": {\n\t\t\"allow\": [\"Bash(ls *)\", \"Bash(vnoc show *)\"],\n\t\t\"deny\": [\"Bash(rm *)\"]\n\t}\n}\n";
    put(&root, SETTINGS_LOCAL, before);
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(out(&o).contains("allowed      V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.local.json\n             only V-NOC's entries; the rest of the file is as it was\n"), "{}", out(&o));
    let after = get(&root, SETTINGS_LOCAL);
    // tabs stayed tabs; the key order stayed
    assert!(after.starts_with("{\n\t\"model\": \"opus\",\n\t\"permissions\": {\n\t\t\"allow\": [\n\t\t\t\"Bash(ls *)\","), "{after}");
    assert!(after.find("\"allow\"").unwrap() < after.find("\"deny\"").unwrap());
    let entries = allow_in(&root, SETTINGS_LOCAL);
    assert_eq!(&entries[..2], ["Bash(ls *)", "Bash(vnoc show *)"]);
    assert_eq!(
        entries.len(),
        28,
        "one user entry and twenty-seven of ours, `vnoc show` once"
    );
    let v: Value = serde_json::from_str(&after).unwrap();
    assert_eq!(v["model"], "opus");
    assert_eq!(v["permissions"]["deny"], json!(["Bash(rm *)"]));
}

#[test]
fn a_settings_file_that_is_not_json_is_left_exit_2_and_the_lines_are_printed() {
    let (root, home) = world("perm-invalid");
    put(&root, SETTINGS_LOCAL, "{ \"permissions\": ");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    assert_eq!(code(&o), 2);
    assert_eq!(get(&root, SETTINGS_LOCAL), "{ \"permissions\": ");
    let text = out(&o);
    assert!(text.contains("left         V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.local.json\n             it is not valid JSON"), "{text}");
    assert!(text.contains("to let Claude Code run V-NOC's read commands and the commands that write a tour without asking, add to .claude/settings.json:"), "{text}");
    // the rest was written all the same
    assert!(exists(&root, ".claude/skills/vnoc/SKILL.md"));
}

#[test]
fn no_permissions_writes_none_and_prints_the_lines_and_so_does_having_no_terminal() {
    for flags in [&["--no-permissions"][..], &[][..]] {
        let (root, home) = world("perm-none");
        let mut args = vec!["agent", "install", "--agent", "claude,codex"];
        args.extend_from_slice(flags);
        let o = vnoc(&root, &home, &args);
        assert_eq!((code(&o), err(&o).as_str()), (0, ""), "{flags:?}");
        assert!(
            !exists(&root, SETTINGS_LOCAL) && !exists(&root, ".claude/settings.json"),
            "{flags:?}"
        );
        assert!(files(&home).is_empty(), "{flags:?}");
        assert!(
            out(&o).contains("add to .claude/settings.json:")
                && out(&o).contains("add to ~/.codex/rules/default.rules:"),
            "{flags:?}"
        );
        assert!(!out(&o).contains("allowed"), "{flags:?}");
    }
}

#[test]
fn codex_gets_a_rules_file_in_the_home_folder_only_and_a_file_that_is_not_ours_is_left() {
    let (root, home) = world("perm-codex");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--permissions"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(
        out(&o).contains(
            "allowed      V-NOC's read commands and the tour commands for Codex\nin           ~/.codex/rules/vnoc.rules\n"
        ),
        "{}",
        out(&o)
    );
    let rules = get(&home, ".codex/rules/vnoc.rules");
    assert!(rules.starts_with("# V-NOC: written by `vnoc agent install`"));
    assert!(rules.contains("prefix_rule(pattern = [\"vnoc\", \"doc\", [\"list\", \"read\", \"path\"]], decision = \"allow\")"));
    assert!(
        !exists(&root, ".codex"),
        "nothing of Codex's in the project"
    );
    // someone's own vnoc.rules: left, exit 2, the lines printed
    put(&home, ".codex/rules/vnoc.rules", "# mine\n");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--permissions"],
    );
    assert_eq!(code(&o), 2);
    assert_eq!(get(&home, ".codex/rules/vnoc.rules"), "# mine\n");
    assert!(
        out(&o).contains("left         V-NOC's read commands and the tour commands for Codex"),
        "{}",
        out(&o)
    );
    assert!(out(&o).contains("add to ~/.codex/rules/default.rules:"));
}

#[test]
fn a_dry_run_lists_the_permissions_too_and_writes_nothing() {
    let (root, home) = world("perm-dry");
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude,codex",
            "--permissions",
            "--dry-run",
        ],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    let text = out(&o);
    assert!(text.contains("will allow   V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.local.json\n"), "{text}");
    assert!(
        text.contains(
            "will allow   V-NOC's read commands and the tour commands for Codex\nin           ~/.codex/rules/vnoc.rules\n"
        ),
        "{text}"
    );
    assert!(files(&root).is_empty() && files(&home).is_empty());
    // in JSON: an action with part `permissions`
    let j = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude",
            "--permissions",
            "--dry-run",
            "--json",
        ],
    );
    let v: Value = serde_json::from_str(&out(&j)).unwrap();
    let action = v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["part"] == "permissions")
        .unwrap();
    assert_eq!(action["path"], ".claude/settings.local.json");
    assert_eq!(action["change"], "create");
    assert_eq!(
        v["permissions"],
        json!([]),
        "written, so no lines to add by hand"
    );
    // with --json and no flag, nothing is asked and nothing is written, as with no terminal
    let j = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--json"],
    );
    assert!(!exists(&root, SETTINGS_LOCAL));
    let v: Value = serde_json::from_str(&out(&j)).unwrap();
    assert_eq!(v["permissions"].as_array().unwrap().len(), 1);
}

#[test]
fn remove_takes_back_only_what_install_added_and_status_says_where_they_are() {
    let (root, home) = world("perm-remove");
    put(
        &root,
        SETTINGS_LOCAL,
        "{\n  \"permissions\": {\n    \"allow\": [\"Bash(ls *)\"]\n  }\n}\n",
    );
    vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude,codex",
            "--permissions",
        ],
    );
    assert_eq!(allow_in(&root, SETTINGS_LOCAL).len(), 28);
    let s = vnoc(&root, &home, &["agent", "status"]);
    assert_eq!(code(&s), 0);
    assert!(out(&s).contains("allowed      V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.local.json\n"), "{}", out(&s));
    assert!(
        out(&s).contains(
            "allowed      V-NOC's read commands and the tour commands for Codex\nin           ~/.codex/rules/vnoc.rules\n"
        ),
        "{}",
        out(&s)
    );
    let sj: Value =
        serde_json::from_str(&out(&vnoc(&root, &home, &["agent", "status", "--json"]))).unwrap();
    assert_eq!(
        sj["permissions"][0],
        json!({"agent": "claude", "state": "in_place", "path": ".claude/settings.local.json"})
    );

    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "claude"]);
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    assert!(out(&o).contains("removed      V-NOC's read commands and the tour commands for Claude Code\nin           .claude/settings.local.json\n"), "{}", out(&o));
    assert_eq!(
        allow_in(&root, SETTINGS_LOCAL),
        ["Bash(ls *)"],
        "the user's entry stays, so does the list"
    );
    assert!(
        exists(&home, ".codex/rules/vnoc.rules"),
        "codex was not named"
    );
    // for everything: the rules file goes too
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!(code(&o), 0);
    assert!(
        out(&o).contains(
            "deleted      V-NOC's read commands and the tour commands for Codex\nin           ~/.codex/rules/vnoc.rules\n"
        ),
        "{}",
        out(&o)
    );
    assert!(!exists(&home, ".codex/rules/vnoc.rules"));
    let s = vnoc(&root, &home, &["agent", "status"]);
    assert_eq!(out(&s), "not set up: vnoc agent install\n");
}

#[test]
fn a_settings_file_v_noc_made_goes_when_it_is_taken_back_to_nothing() {
    let (root, home) = world("perm-made");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    assert!(exists(&root, SETTINGS_LOCAL));
    let o = vnoc(&root, &home, &["agent", "remove", "--agent", "claude"]);
    assert_eq!(code(&o), 0);
    assert!(
        out(&o)
            .contains("deleted      V-NOC's read commands and the tour commands for Claude Code"),
        "{}",
        out(&o)
    );
    assert!(!exists(&root, SETTINGS_LOCAL));
}

// --------------------------------------------------------------------------------------------------------------------- the tours' skill

#[test]
fn install_writes_the_tours_skill_beside_the_first_for_each_agent_that_gets_the_first() {
    let (root, home) = world("tour-skill");
    let o = vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex"],
    );
    assert_eq!((code(&o), err(&o).as_str()), (0, ""));
    for folder in [".claude/skills", ".agents/skills"] {
        assert_eq!(get(&root, &format!("{folder}/vnoc/SKILL.md")), SKILL);
        assert_eq!(
            get(&root, &format!("{folder}/vnoc-tour/SKILL.md")),
            TOUR_SKILL
        );
    }
    // the rules block is the one block it was: it names no second skill
    let rules = get(&root, "AGENTS.md");
    assert_eq!(rules.matches("<!-- vnoc:start").count(), 1);
    assert!(
        !rules.contains("vnoc-tour"),
        "the rules block does not change"
    );
    // an agent that is not picked gets neither
    let (root, home) = world("tour-skill-one");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    assert!(!root.join(".claude").exists());
    assert!(root.join(".agents/skills/vnoc-tour/SKILL.md").is_file());
    // twice changes nothing
    let again = vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    assert!(
        out(&again).contains("current      .agents/skills/vnoc-tour/SKILL.md"),
        "{}",
        out(&again)
    );
    // `--skill-only` writes both skills and no rules; `--rules-only` neither skill
    let (root, home) = world("tour-skill-only");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--skill-only"],
    );
    assert_eq!(
        files(&root),
        [
            ".agents/skills/vnoc-tour/SKILL.md",
            ".agents/skills/vnoc/SKILL.md"
        ]
    );
    let (root, home) = world("tour-rules-only");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--rules-only"],
    );
    assert_eq!(files(&root), ["AGENTS.md"]);
}

#[test]
fn an_install_from_before_tours_is_told_the_second_skill_is_not_installed_and_install_adds_it() {
    let (root, home) = world("before-tours");
    // what an older V-NOC left: the first skill and the rules, no vnoc-tour
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    std::fs::remove_dir_all(root.join(".agents/skills/vnoc-tour")).unwrap();
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(out(&o).contains("vnoc-tour: not installed"), "{}", out(&o));
    assert!(
        out(&o).contains("current      .agents/skills/vnoc/SKILL.md"),
        "{}",
        out(&o)
    );
    let json: Value =
        serde_json::from_slice(&vnoc(&root, &home, &["agent", "status", "--json"]).stdout).unwrap();
    assert_eq!(json["missing"], json!(["tour_skill"]));
    // the one line of `vnoc status` says it too
    let status = vnoc(&root, &home, &["status"]);
    assert!(
        out(&status).contains("vnoc-tour: not installed"),
        "{}",
        out(&status)
    );
    // a fresh install has nothing missing, and says both skills
    let (root, home) = world("not-before-tours");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    let o = vnoc(&root, &home, &["agent", "status"]);
    assert!(!out(&o).contains("not installed"), "{}", out(&o));
    assert!(
        out(&o).contains(".agents/skills/vnoc-tour/SKILL.md"),
        "{}",
        out(&o)
    );
    assert!(
        out(&o).contains("the vnoc-tour skill, read by codex"),
        "{}",
        out(&o)
    );
    // installing again over the old install writes the second and changes nothing else
    let (root, home) = world("before-tours-2");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    std::fs::remove_dir_all(root.join(".agents/skills/vnoc-tour")).unwrap();
    let before = get(&root, ".agents/skills/vnoc/SKILL.md");
    let o = vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(get(&root, ".agents/skills/vnoc-tour/SKILL.md"), TOUR_SKILL);
    assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), before);
    assert!(!out(&vnoc(&root, &home, &["agent", "status"])).contains("not installed"));
}

#[test]
fn remove_takes_back_both_skills_and_the_folders_it_made() {
    let (root, home) = world("remove-both");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude,codex"],
    );
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert!(
        out(&o).contains(".claude/skills/vnoc-tour/SKILL.md"),
        "{}",
        out(&o)
    );
    assert!(
        out(&o).contains(".agents/skills/vnoc-tour/SKILL.md"),
        "{}",
        out(&o)
    );
    assert_eq!(files(&root), Vec::<String>::new(), "nothing is left");
    assert!(!root.join(".claude").exists() && !root.join(".agents").exists());
    // a vnoc-tour skill written by hand is left, and said
    let (root, home) = world("remove-hand-tour");
    vnoc(&root, &home, &["agent", "install", "--agent", "codex"]);
    put(
        &root,
        ".agents/skills/vnoc-tour/SKILL.md",
        "---\nname: vnoc-tour\n---\nmine\n",
    );
    let o = vnoc(&root, &home, &["agent", "remove"]);
    assert_eq!(code(&o), 0, "{}", out(&o));
    assert!(
        out(&o).contains("left         .agents/skills/vnoc-tour/SKILL.md"),
        "{}",
        out(&o)
    );
    assert!(get(&root, ".agents/skills/vnoc-tour/SKILL.md").contains("mine"));
    assert!(
        !root.join(".agents/skills/vnoc/SKILL.md").exists(),
        "the first skill still went"
    );
    // --global: both, from the home folder
    let (root, home) = world("remove-global");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "codex", "--global"],
    );
    assert!(home.join(".agents/skills/vnoc-tour/SKILL.md").is_file());
    vnoc(&root, &home, &["agent", "remove", "--global"]);
    assert_eq!(files(&home), Vec::<String>::new());
}

#[test]
fn the_allow_list_has_where_and_the_writing_loop_and_none_of_what_reaches_further() {
    let (root, home) = world("allow-tour");
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude,codex",
            "--permissions",
        ],
    );
    assert_eq!(code(&o), 0, "{}", err(&o));
    // Claude Code: one `Bash(vnoc tour <command> *)` for each
    let entries = allow_in(&root, SETTINGS_LOCAL);
    for command in TOUR_ALLOWED {
        let rule = format!("Bash(vnoc tour {command} *)");
        assert_eq!(entries.iter().filter(|e| **e == rule).count(), 1, "{rule}");
    }
    assert!(entries.contains(&"Bash(vnoc tour where *)".to_string()));
    for command in TOUR_ASKED {
        assert!(
            !entries
                .iter()
                .any(|e| e.contains(&format!("vnoc tour {command}"))),
            "`{command}` asks the user: {entries:?}"
        );
    }
    assert!(
        !entries
            .iter()
            .any(|e| e == "Bash(vnoc tour *)" || e == "Bash(vnoc *)"),
        "{entries:?}"
    );
    // Codex: one rule, the same twelve words, and none of the others
    let rules = get(&home, ".codex/rules/vnoc.rules");
    let line = rules
        .lines()
        .find(|l| l.contains("\"tour\""))
        .unwrap_or_else(|| panic!("a rule for tour in\n{rules}"));
    assert!(line.contains("decision = \"allow\""), "{line}");
    for command in TOUR_ALLOWED {
        assert!(
            line.contains(&format!("\"{command}\"")),
            "{command} in {line}"
        );
    }
    for command in TOUR_ASKED {
        assert!(
            !line.contains(&format!("\"{command}\"")),
            "{command} in {line}"
        );
    }
    // the lines printed for an agent that is not given the rules say the same
    let (root, home) = world("allow-tour-lines");
    let o = vnoc(
        &root,
        &home,
        &[
            "agent",
            "install",
            "--agent",
            "claude,codex",
            "--no-permissions",
        ],
    );
    let text = out(&o);
    assert!(text.contains("\"Bash(vnoc tour where *)\""), "{text}");
    assert!(
        text.contains("\"vnoc\", \"tour\", [\"start\", \"add\", \"finish\""),
        "{text}"
    );
    for command in TOUR_ASKED {
        assert!(!text.contains(&format!("vnoc tour {command}")), "{command}");
    }
    // taking it back takes the tour rules too
    let (root, home) = world("allow-tour-remove");
    vnoc(
        &root,
        &home,
        &["agent", "install", "--agent", "claude", "--permissions"],
    );
    vnoc(&root, &home, &["agent", "remove"]);
    assert!(
        !root.join(SETTINGS_LOCAL).exists(),
        "a settings file of only ours goes"
    );
}
