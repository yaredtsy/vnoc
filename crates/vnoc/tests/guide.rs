//! The guides (`guide/*.md`), the skill (`skills/vnoc/SKILL.md`) and the rules block (`skills/vnoc/rules.md`) against the binary
//! (docs/21-agent-cli/06-skill.md §6): **a guide cannot mention a flag the binary does not have, and an output it shows is an output it prints.**
//!
//! A line is a command line when it starts with `vnoc ` inside a code block, or when a span in backticks does (in prose). Then:
//! - with a `<placeholder>` it is parsed by clap, in this process, and not run (the subcommand and every flag must exist);
//! - a bare command name (`vnoc doc tag`, in prose) is the same: clap only, since it lacks the arguments the prose is not about;
//! - `vnoc agent …` is the same too: it writes files (into the home folder with `--global`), and `tests/agent.rs` runs it;
//! - any other is run, through `sh`, on a copy of the sample: exit `0` or `1` passes, and `2` only when the line ends in a `# exit 2` comment
//!   (a refusal shown on purpose), which must then be true;
//! - the lines under a command in a code block are the output it printed: they must equal a run, once the copy's path and the version are normalized.
//!
//! One code block is one session: its commands run in order on one fresh copy, so a write is seen by the lines after it. Needs the driver built
//! first, as `cli.rs` does, and `sh` (Unix).
#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use clap::Parser;
use clap::error::ErrorKind;
use regex::Regex;
use vnoc::cli::Cli;

// ------------------------------------------------------------------------------------------------------------ the texts

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    dunce::canonicalize(crate_dir().join("../..")).unwrap()
}

fn skill_dir() -> PathBuf {
    repo_root().join("skills/vnoc")
}

fn guide_dir() -> PathBuf {
    crate_dir().join("guide")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every text that holds command lines: (a name to point at, its text).
fn texts() -> Vec<(String, String)> {
    let mut texts = vec![
        ("SKILL.md".to_string(), read(&skill_dir().join("SKILL.md"))),
        ("rules.md".to_string(), read(&skill_dir().join("rules.md"))),
        // the two READMEs are read by people who type what they find there (A13r)
        (
            "README.md".to_string(),
            read(&repo_root().join("README.md")),
        ),
        (
            "extension/README.md".to_string(),
            read(&repo_root().join("extension/README.md")),
        ),
    ];
    for topic in TOPICS {
        texts.push((
            format!("guide/{topic}.md"),
            read(&guide_dir().join(format!("{topic}.md"))),
        ));
    }
    texts
}

const TOPICS: [&str; 5] = ["read", "tree", "find", "write", "check"];

// ------------------------------------------------------------------------------------------------------------ the lines

/// A command line, with the output pasted under it (none when nothing is).
struct Cmd {
    at: String,
    line: String,
    pasted: Option<Vec<String>>,
}

/// A session: the commands that run in order on one copy.
struct Session {
    cmds: Vec<Cmd>,
}

fn trim_blank_ends(mut lines: Vec<String>) -> Vec<String> {
    for l in &mut lines {
        *l = l.trim_end().to_string();
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// The sessions of a text: each code block is one, each command in backticks in prose is one of its own.
fn sessions(name: &str, text: &str) -> Vec<Session> {
    let mut sessions = Vec::new();
    let mut block: Option<Session> = None;
    for (n, raw) in text.lines().enumerate() {
        let at = format!("{name}:{}", n + 1);
        if raw.trim_start().starts_with("```") {
            match block.take() {
                Some(mut done) => {
                    for cmd in &mut done.cmds {
                        cmd.pasted = cmd
                            .pasted
                            .take()
                            .map(trim_blank_ends)
                            .filter(|l| !l.is_empty());
                    }
                    if !done.cmds.is_empty() {
                        sessions.push(done);
                    }
                }
                None => block = Some(Session { cmds: Vec::new() }),
            }
            continue;
        }
        match &mut block {
            Some(session) => {
                if raw.starts_with("vnoc ") {
                    session.cmds.push(Cmd {
                        at,
                        line: raw.to_string(),
                        pasted: None,
                    });
                } else if let Some(cmd) = session.cmds.last_mut() {
                    cmd.pasted
                        .get_or_insert_with(Vec::new)
                        .push(raw.to_string());
                }
            }
            None => {
                // prose: the odd pieces between backticks
                for span in raw.split('`').skip(1).step_by(2) {
                    if span.starts_with("vnoc ") {
                        sessions.push(Session {
                            cmds: vec![Cmd {
                                at: at.clone(),
                                line: span.to_string(),
                                pasted: None,
                            }],
                        });
                    }
                }
            }
        }
    }
    assert!(block.is_none(), "{name}: a code block is not closed");
    sessions
}

/// What to do with a command line.
enum Plan {
    /// clap has accepted it; it is not run (a placeholder, a bare name, `--help`)
    ParsedOnly,
    /// run it; `refusal` when the line says `# exit 2`
    Run { refusal: bool },
}

/// The shell's words of `line` (quotes, backslashes, a `#` comment, `|` between commands): the pieces clap is given. `Err` for an unclosed quote.
fn pipeline(line: &str) -> Result<Vec<Vec<String>>, String> {
    let mut commands = vec![Vec::new()];
    let mut word: Option<String> = None;
    let mut chars = line.chars().peekable();
    let flush = |word: &mut Option<String>, commands: &mut Vec<Vec<String>>| {
        if let Some(w) = word.take() {
            commands.last_mut().unwrap().push(w);
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => w.push(c),
                        None => return Err("a single quote is not closed".into()),
                    }
                }
            }
            '"' => {
                let w = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\' | '$' | '`')) => w.push(c),
                            Some(c) => {
                                w.push('\\');
                                w.push(c);
                            }
                            None => return Err("a double quote is not closed".into()),
                        },
                        Some(c) => w.push(c),
                        None => return Err("a double quote is not closed".into()),
                    }
                }
            }
            '\\' => {
                if let Some(c) = chars.next() {
                    word.get_or_insert_with(String::new).push(c);
                }
            }
            '#' if word.is_none() => break,
            '|' => {
                flush(&mut word, &mut commands);
                commands.push(Vec::new());
            }
            c if c.is_whitespace() => flush(&mut word, &mut commands),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    flush(&mut word, &mut commands);
    Ok(commands)
}

fn placeholder() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^<>\n]+>").unwrap())
}

/// Has clap accept every `vnoc` of the line, and says whether the line is to be run.
fn plan(line: &str) -> Result<Plan, String> {
    let has_placeholder = placeholder().is_match(line);
    // `vnoc <command> --help`: any command, so a command-name placeholder stands for `status`, and only `--help` may follow it
    let subcommand_placeholder = Regex::new(r"^vnoc <[^<>]+>( |$)").unwrap().is_match(line);
    let line_for_clap = placeholder().replace_all(line, "x");
    let segments = pipeline(&line_for_clap)?;
    let mut run = !has_placeholder;
    for words in &segments {
        if words.first().map(String::as_str) != Some("vnoc") {
            // the other end of a pipe (`| head`) is the shell's; only `vnoc` has to be understood
            continue;
        }
        let mut args: Vec<String> = words.clone();
        if subcommand_placeholder {
            if args[2..].iter().any(|a| a != "--help" && a != "-h") {
                return Err("a placeholder for the command may only be followed by --help".into());
            }
            args[1] = "status".into();
        }
        match Cli::try_parse_from(&args) {
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
                run = false;
            }
            Err(e)
                if e.kind() == ErrorKind::MissingRequiredArgument
                    && args[1..].iter().all(|a| !a.starts_with('-')) =>
            {
                // a command's name, in prose: its flags are the prose's business only when it names them
                run = false;
            }
            Err(e) => {
                return Err(format!(
                    "clap refuses `{}`: {}",
                    line.trim(),
                    e.to_string().lines().next().unwrap_or("")
                ));
            }
        }
        if subcommand_placeholder {
            run = false;
        }
    }
    // `vnoc agent …` writes skill and rules files (and `--global` into the home folder): clap understands it here, and `tests/agent.rs` runs it
    if segments.iter().any(|w| {
        w.first().map(String::as_str) == Some("vnoc")
            && w.get(1).map(String::as_str) == Some("agent")
    }) {
        run = false;
    }
    let refusal = comment_of(line).is_some_and(|c| c.trim() == "exit 2");
    if refusal && !run {
        return Err("`# exit 2` on a line that is not run".into());
    }
    Ok(if run {
        Plan::Run { refusal }
    } else {
        Plan::ParsedOnly
    })
}

/// The `# …` comment at the end of a command line (outside quotes).
fn comment_of(line: &str) -> Option<&str> {
    let mut quote: Option<char> = None;
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '#') if prev_space => return Some(&line[i + 1..]),
            _ => {}
        }
        prev_space = c.is_whitespace();
    }
    None
}

// ------------------------------------------------------------------------------------------------------------ running

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

/// A folder with the one `vnoc` in it, for the `PATH` of the shell that runs a line.
fn bin_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("vnoc-guide-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_vnoc"), dir.join("vnoc")).unwrap();
        dir
    })
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

/// A fresh copy of the sample **outside this repository**: a `.git` above it would move the root (05 §2).
fn fresh_sample(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-guide-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dunce::canonicalize(dir).unwrap();
    copy_dir(&crate_dir().join("tests/sample"), &dir);
    dir
}

/// What the line printed (standard output and error together, in the order they came), and its exit code.
fn run_line(root: &Path, line: &str) -> (i32, String) {
    let path = format!(
        "{}:{}",
        bin_dir().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{{\n{line}\n}} 2>&1"))
        .current_dir(root)
        .env("PATH", path)
        .env("VNOC_DRIVER", driver_path())
        // whatever a line does with the home folder, it is not the real one
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env_remove("VNOC_LOG")
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        out.status.code().expect("exited, not killed by a signal"),
        String::from_utf8(out.stdout).unwrap(),
    )
}

/// What a run printed, as a guide shows it: the copy's path and the version as stand-ins, no trailing space, no trailing blank line.
fn normalized(printed: &str, root: &Path) -> Vec<String> {
    let text = printed
        .replace(root.to_str().unwrap(), "<root>")
        .replace(env!("CARGO_PKG_VERSION"), "<version>");
    trim_blank_ends(text.lines().map(String::from).collect())
}

#[derive(Default)]
struct Findings {
    /// a command line the binary does not understand or that failed
    commands: Vec<String>,
    /// a pasted output that is not what the command prints
    outputs: Vec<String>,
    /// how many command lines were looked at, per text
    seen: Vec<(String, usize)>,
}

fn check_session(label: &str, session: &Session, found: &mut Findings) {
    let root = fresh_sample(label);
    for cmd in &session.cmds {
        let plan = match plan(&cmd.line) {
            Ok(plan) => plan,
            Err(why) => {
                found.commands.push(format!("{}: {why}", cmd.at));
                continue;
            }
        };
        let Plan::Run { refusal } = plan else {
            if cmd.pasted.is_some() {
                found.outputs.push(format!(
                    "{}: an output is pasted under a line that is not run: {}",
                    cmd.at, cmd.line
                ));
            }
            continue;
        };
        let (exit, printed) = run_line(&root, &cmd.line);
        let ok = match exit {
            0 | 1 => !refusal,
            2 => refusal,
            _ => false,
        };
        if !ok {
            found.commands.push(format!(
                "{}: `{}` exited {exit}{}\n{printed}",
                cmd.at,
                cmd.line,
                if refusal { " (it says `# exit 2`)" } else { "" }
            ));
        }
        if let Some(pasted) = &cmd.pasted {
            let real = normalized(&printed, &root);
            if &real != pasted {
                found.outputs.push(format!(
                    "{}: `{}`\n--- pasted\n{}\n--- printed\n{}",
                    cmd.at,
                    cmd.line,
                    pasted.join("\n"),
                    real.join("\n")
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Every session of every text, run once for the two tests that read the findings.
fn findings() -> &'static Findings {
    static FOUND: OnceLock<Findings> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut all = Findings::default();
        let mut work = Vec::new();
        for (name, text) in texts() {
            let list = sessions(&name, &text);
            all.seen.push((
                name.clone(),
                list.iter().map(|s| s.cmds.len()).sum::<usize>(),
            ));
            for (i, session) in list.into_iter().enumerate() {
                work.push((format!("{}-{i}", name.replace(['/', '.'], "-")), session));
            }
        }
        // each session is its own copy and its own processes: they run side by side, a few at a time
        let queue = std::sync::Mutex::new(work);
        let found = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    loop {
                        let Some((label, session)) = queue.lock().unwrap().pop() else {
                            break;
                        };
                        let mut part = Findings::default();
                        check_session(&label, &session, &mut part);
                        found.lock().unwrap().push(part);
                    }
                });
            }
        });
        for part in found.into_inner().unwrap() {
            all.commands.extend(part.commands);
            all.outputs.extend(part.outputs);
        }
        all.commands.sort();
        all.outputs.sort();
        all
    })
}

// ------------------------------------------------------------------------------------------------------------ the tests

#[test]
fn the_guides_have_command_lines_to_check() {
    let seen = &findings().seen;
    assert_eq!(seen.len(), 2 + 2 + TOPICS.len());
    for (name, count) in seen {
        let least = match name.as_str() {
            "rules.md" => 1,
            "SKILL.md" => 15,
            // the repository README has some 57 (24 run, the rest parsed against placeholders); the Marketplace page has one
            "README.md" => 40,
            "extension/README.md" => 1,
            _ => 3,
        };
        assert!(
            *count >= least,
            "{name}: {count} command lines found, at least {least} expected: did the way lines are found break?"
        );
    }
}

#[test]
fn every_command_line_exists_and_runs_on_the_sample() {
    let bad = &findings().commands;
    assert!(bad.is_empty(), "\n{}\n", bad.join("\n\n"));
}

#[test]
fn every_pasted_output_is_what_the_command_prints() {
    let bad = &findings().outputs;
    assert!(bad.is_empty(), "\n{}\n", bad.join("\n\n"));
}

#[test]
fn a_line_that_names_no_flag_the_binary_has_is_refused() {
    // the test must be able to fail: these are the ways a guide goes wrong
    for line in [
        "vnoc show x --no-such-flag",
        "vnoc no-such-command",
        "vnoc doc tags",
        "vnoc doc new <node> --title \"x\" --nope",
        "vnoc <command> --depth 2",
        "vnoc show 'x",
    ] {
        assert!(plan(line).is_err(), "{line}");
    }
    for line in [
        "vnoc show orders/service.py:12 --no-code",
        "vnoc tags",
        "vnoc tags --authors",
        "vnoc doc tag",
        "vnoc <command> --help",
        "vnoc relink <ID> <node>",
        "vnoc grep 'parse_order\\(' --names",
        "vnoc find 'no:doc kind:class' --format id | vnoc show - --no-code",
        "vnoc show billing/tax.py::rate_fro  # exit 2",
        "vnoc agent install",
        "vnoc agent install --agent claude,codex --global",
        "vnoc agent status",
    ] {
        assert!(plan(line).is_ok(), "{line}: {:?}", plan(line).err());
    }
    // an agent line is understood, never run
    for line in [
        "vnoc agent install",
        "vnoc agent remove --global",
        "vnoc agent list",
    ] {
        assert!(matches!(plan(line), Ok(Plan::ParsedOnly)), "{line}");
    }
    assert!(plan("vnoc agent install --nope").is_err());
    assert!(plan("vnoc agent frob").is_err());
    // a comment is not a flag, and a refusal is only claimed for a line that runs
    assert!(plan("vnoc show <node>  # exit 2").is_err());
    assert!(matches!(
        plan("vnoc show billing/tax.py::rate_fro  # exit 2"),
        Ok(Plan::Run { refusal: true })
    ));
}

// ------------------------------------------------------------------------------------------------------------ the skill's header

/// The header's top-level keys and, for each, the text after its colon and its indented lines.
fn header(text: &str) -> Vec<(String, String, Vec<String>)> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "SKILL.md starts with its header");
    let mut keys: Vec<(String, String, Vec<String>)> = Vec::new();
    loop {
        let line = lines
            .next()
            .expect("the header is closed by a second `---`");
        if line == "---" {
            return keys;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            keys.last_mut()
                .expect("an indented line belongs to a key")
                .2
                .push(line.to_string());
        } else {
            let (key, value) = line
                .split_once(':')
                .unwrap_or_else(|| panic!("a header line is `key: value`: {line}"));
            keys.push((key.to_string(), value.trim().to_string(), Vec::new()));
        }
    }
}

#[test]
fn the_skills_header_is_what_the_format_wants() {
    let text = read(&skill_dir().join("SKILL.md"));
    let keys = header(&text);
    let names: Vec<&str> = keys.iter().map(|(k, _, _)| k.as_str()).collect();
    assert_eq!(
        names,
        ["name", "description", "metadata"],
        "no other top-level key"
    );
    let value = |key: &str| keys.iter().find(|(k, _, _)| k == key).unwrap();

    // name: the folder's name, lower case, digits and hyphens, at most 64
    let name = &value("name").1;
    let folder = skill_dir()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(name, &folder, "the name is the folder's name");
    assert!(name.len() <= 64);
    assert!(
        Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$")
            .unwrap()
            .is_match(name),
        "{name}"
    );

    // description: one line, at most 1,024 characters, and a plain YAML scalar that every parser reads the same way
    let (_, description, children) = value("description");
    assert!(children.is_empty(), "the description is one line");
    assert!(!description.is_empty() && description.chars().count() <= 1024);
    assert!(
        !description.contains(": ") && !description.contains(" #") && !description.ends_with(':'),
        "a `: ` or ` #` in an unquoted description is not a plain YAML scalar: reword it"
    );
    assert!(
        !description.starts_with(|c: char| "\"'[]{}&*!|>%@`#-?,".contains(c)),
        "a YAML indicator at the start of the description"
    );
    assert!(
        description.contains(".vnoc/") && description.contains("ID:"),
        "the description names what a task would contain"
    );

    // metadata: strings only, and the version of the skill
    let (_, inline, children) = value("metadata");
    assert!(inline.is_empty() && !children.is_empty());
    let quoted = Regex::new(r#"^  [a-z][a-z-]*: ".*"$"#).unwrap();
    for child in children {
        assert!(
            quoted.is_match(child),
            "metadata holds quoted strings: {child}"
        );
    }
}

/// The skill carries the version of the binary it is compiled into: a release changes both, and this fails until it has.
#[test]
fn the_skills_version_is_the_crates() {
    let text = read(&skill_dir().join("SKILL.md"));
    let (_, _, children) = header(&text)
        .into_iter()
        .find(|(k, _, _)| k == "metadata")
        .unwrap();
    assert!(
        children.contains(&format!(
            "  vnoc-version: \"{}\"",
            env!("CARGO_PKG_VERSION")
        )),
        "metadata.vnoc-version in skills/vnoc/SKILL.md is not {} (the version of the crate): bump it with the crate",
        env!("CARGO_PKG_VERSION")
    );
}

// ------------------------------------------------------------------------------------------------------------ the binary prints them

fn vnoc(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_vnoc"))
        .args(args)
        .current_dir(std::env::temp_dir())
        .env_remove("VNOC_DRIVER")
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn skill_and_guide_print_the_files_they_were_built_from() {
    let (exit, skill, err) = vnoc(&["skill"]);
    assert_eq!((exit, err.as_str()), (0, ""));
    assert_eq!(skill, read(&skill_dir().join("SKILL.md")));
    for topic in TOPICS {
        let (exit, page, err) = vnoc(&["guide", topic]);
        assert_eq!((exit, err.as_str()), (0, ""), "{topic}");
        assert_eq!(
            page,
            read(&guide_dir().join(format!("{topic}.md"))),
            "{topic}: through a pipe the page is the file, byte for byte"
        );
    }
    assert_eq!(vnoc::cli::guide::RULES, read(&skill_dir().join("rules.md")));
    assert_eq!(vnoc::cli::guide::SKILL, read(&skill_dir().join("SKILL.md")));
}

#[test]
fn guide_lists_the_topics_and_an_unknown_one_is_a_refusal_with_the_list() {
    let (exit, list, _) = vnoc(&["guide"]);
    assert_eq!(exit, 0);
    for topic in TOPICS {
        assert!(
            list.lines().any(|l| l.trim_start().starts_with(topic)),
            "{topic} in\n{list}"
        );
    }
    let (exit, out, err) = vnoc(&["guide", "nope"]);
    assert_eq!((exit, out.as_str()), (2, ""));
    assert!(err.starts_with("error: no guide topic `nope`"), "{err}");
    for topic in TOPICS {
        assert!(err.contains(&format!("  {topic}")), "{err}");
    }
    // they print text: a flag that asks for another form is refused, not ignored
    assert_eq!(vnoc(&["guide", "--json"]).0, 2);
    assert_eq!(vnoc(&["skill", "--json"]).0, 2);
    // and they need no project and no driver (this ran in a folder with neither)
}

#[test]
fn the_topics_are_the_files_of_the_guide_folder_and_each_is_short() {
    let on_disk: BTreeSet<String> = std::fs::read_dir(guide_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    let expected: BTreeSet<String> = TOPICS.iter().map(|t| format!("{t}.md")).collect();
    assert_eq!(
        on_disk, expected,
        "a guide file that is not a topic is never printed"
    );
    for topic in TOPICS {
        let text = read(&guide_dir().join(format!("{topic}.md")));
        let lines = text.lines().count();
        assert!(
            lines <= 60,
            "{topic}.md is {lines} lines: a topic is about 40"
        );
        assert!(text.ends_with('\n') && !text.contains("\n\n\n"));
    }
}

#[test]
fn the_rules_block_has_its_markers_and_the_skill_is_the_size_the_plan_wants() {
    let rules = read(&skill_dir().join("rules.md"));
    assert!(rules.starts_with("<!-- vnoc:start "));
    assert!(rules.trim_end().ends_with("<!-- vnoc:end -->"));
    assert_eq!(rules.matches("vnoc:start").count(), 1);
    let skill = read(&skill_dir().join("SKILL.md"));
    assert!(
        skill.lines().count() < 120,
        "the format advises under 500 lines; this skill is meant to be a tenth of that"
    );
}
