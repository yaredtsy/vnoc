//! `vnoc agent install | list | status | remove` (docs/21-agent-cli/07-install.md §2): the skill and the rules block, written for the agents the
//! user picked. The files, the table and the decisions are `agents.rs`; this is the pick, the printing and the exit codes. No driver is needed.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use dialoguer::MultiSelect;
use dialoguer::console::Term;
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};
use serde::Serialize;

use super::args::{AgentCommand, InstallArgs, RemoveArgs};
use super::out::{self, Render};
use super::write::{continued, label};
use super::{Failure, Global, style};
use crate::agents::{
    self, AGENTS, Action, Agent, Change, Options, Part, State, Texts, detect, permission_lines,
};

pub fn run(global: &Global, command: &AgentCommand) -> Result<(), Failure> {
    if global.format.is_some() {
        return Err(Failure::Usage(
            "agent prints no list of nodes; --format is for list commands".into(),
        ));
    }
    match command {
        AgentCommand::List => list(global),
        AgentCommand::Install(args) => install(global, args),
        AgentCommand::Status => status(global),
        AgentCommand::Remove(args) => remove(global, args),
    }
}

// ------------------------------------------------------------------------------------------------------------------ where

/// The folder the files go in: the root as every command finds it. Not the git root of a sub-folder: this is where the user is working.
fn root_of(global: &Global) -> Result<PathBuf, Failure> {
    let cwd = std::env::current_dir()
        .map_err(|e| Failure::Environment(format!("the current folder is not known: {e}")))?;
    let root = super::project::find_root(&cwd, global.root.as_deref());
    if root.is_dir() {
        Ok(root)
    } else {
        Err(Failure::Usage(format!(
            "{} is not a folder",
            root.display()
        )))
    }
}

/// The user's home folder, for `--global` (and for `status` to see what is installed there).
fn home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|p| p.is_absolute())
}

fn names() -> String {
    AGENTS
        .iter()
        .map(|a| a.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The agents named on the command line, in the order given, once each.
fn parse_names(given: &[String]) -> Result<Vec<Agent>, Failure> {
    let mut picked = Vec::new();
    for name in given {
        let agent = Agent::parse(name.trim()).ok_or_else(|| {
            Failure::Usage(format!(
                "`{name}` is not an agent V-NOC knows; the names are {}. Any other agent is `other`",
                names()
            ))
        })?;
        if !picked.contains(&agent) {
            picked.push(agent);
        }
    }
    Ok(picked)
}

// -------------------------------------------------------------------------------------------------------------------- the pick

/// What a line typed at the numbered list means: nothing (Enter) is the ticked agents; otherwise numbers, separated by spaces or commas.
pub fn parse_pick(line: &str, ticked: &[Agent]) -> Result<Vec<Agent>, String> {
    let line = line.trim();
    if line.is_empty() {
        return if ticked.is_empty() {
            Err("nothing is ticked: type the numbers of your agents".into())
        } else {
            Ok(ticked.to_vec())
        };
    }
    let mut picked = Vec::new();
    for word in line.split([' ', ',', '\t']).filter(|w| !w.is_empty()) {
        let agent = word
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| AGENTS.get(i).copied())
            .ok_or_else(|| format!("`{word}` is not one of the numbers 1 to {}", AGENTS.len()))?;
        if !picked.contains(&agent) {
            picked.push(agent);
        }
    }
    Ok(picked)
}

/// How the agents are asked for (07 §2): a menu to move in and tick, the numbered list and one typed line when the menu cannot be drawn, or no
/// question at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    Menu,
    TypedLine,
    None,
}

/// The question is asked on standard error (standard output stays the answer), so what has to be a terminal is standard input and standard
/// error. A terminal that cannot move a cursor (`TERM=dumb`) gets the typed line.
pub fn prompt_for(stdin_tty: bool, stderr_tty: bool, term: Option<&str>) -> Prompt {
    if !stdin_tty || !stderr_tty {
        Prompt::None
    } else if term == Some("dumb") {
        Prompt::TypedLine
    } else {
        Prompt::Menu
    }
}

/// Whether a dialoguer prompt is on the screen: only then does Ctrl-C have a cursor to give back.
static PROMPT_SHOWN: AtomicBool = AtomicBool::new(false);

/// Ctrl-C at a prompt: `console` hides the cursor while it draws and, on Ctrl-C, raises SIGINT without showing it again, so the shell would be left
/// with no cursor. Show it (when a prompt was on the screen), and end the way an interrupted program does (130).
fn interrupted(prompt_shown: bool) -> ! {
    if prompt_shown {
        let mut err = std::io::stderr();
        let _ = err.write_all(b"\x1b[?25h");
        let _ = err.flush();
    }
    std::process::exit(130)
}

/// Runs a dialoguer prompt with Ctrl-C handled (see `interrupted`). The handler is installed the first time a prompt is shown, not at start-up, so
/// the rest of the command line keeps the default Ctrl-C; it can only be set once, and it ends the process the same way the default does.
fn at_a_prompt<T>(ask: impl FnOnce() -> T) -> T {
    static HANDLER: Once = Once::new();
    HANDLER.call_once(|| {
        let _ = ctrlc::set_handler(|| interrupted(PROMPT_SHOWN.load(Ordering::SeqCst)));
    });
    PROMPT_SHOWN.store(true, Ordering::SeqCst);
    let answer = ask();
    PROMPT_SHOWN.store(false, Ordering::SeqCst);
    answer
}

/// What the user did at the menu.
enum Menu {
    Picked(Vec<Agent>),
    Cancelled,
    /// the terminal could not draw it: ask the other way
    Unavailable,
}

fn menu(found: &[Agent]) -> Menu {
    let width = AGENTS.iter().map(|a| a.name().len()).max().unwrap_or(0);
    let items: Vec<(String, bool)> = AGENTS
        .iter()
        .map(|agent| {
            let here = found.contains(agent);
            let label = if here {
                format!(
                    "{:<width$}  {}",
                    agent.name(),
                    style::paint_err(style::dim(), "found here")
                )
            } else {
                agent.name().to_string()
            };
            (label, here)
        })
        .collect();
    // the title and the keys stay in the scroll-back as the question's header; the menu below clears itself when the user is done
    let _ = writeln!(
        std::io::stderr(),
        "{}  {}",
        style::paint_err(style::bold(), "Which coding agents does this project use?"),
        style::paint_err(
            style::dim(),
            "(↑/↓ move, Space ticks, Enter accepts, Esc cancels)"
        )
    );
    let colorful = ColorfulTheme::default();
    let plain = SimpleTheme;
    let theme: &dyn Theme = if style::on(style::Stream::Err) {
        &colorful
    } else {
        &plain
    };
    let asked = at_a_prompt(|| {
        MultiSelect::with_theme(theme)
            .items_checked(items)
            .report(false)
            .interact_on_opt(&Term::stderr())
    });
    match asked {
        Ok(Some(chosen)) => Menu::Picked(
            chosen
                .into_iter()
                .filter_map(|i| AGENTS.get(i).copied())
                .collect(),
        ),
        // Esc or q
        Ok(None) => Menu::Cancelled,
        // Ctrl-C: `console` has raised SIGINT, and the handler may be ending the process already; do the same here so the answer is the same
        Err(dialoguer::Error::IO(e)) if e.kind() == std::io::ErrorKind::Interrupted => {
            interrupted(true)
        }
        Err(_) => Menu::Unavailable,
    }
}

/// The numbered list, on standard error, and one line read from standard input.
fn ask(ticked: &[Agent]) -> Result<Vec<Agent>, Failure> {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(
        err,
        "Which coding agents does this project use? ({} are the ones found here)",
        style::paint_err(style::bold(), "[x]")
    );
    for (i, agent) in AGENTS.iter().enumerate() {
        let mark = if ticked.contains(agent) { "[x]" } else { "[ ]" };
        let _ = writeln!(err, "  {:>2}. {mark} {}", i + 1, agent.name());
    }
    let _ = write!(
        err,
        "Press Enter for the [x] ones, or type numbers like \"1 3\" and press Enter: "
    );
    let _ = err.flush();
    drop(err);
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| Failure::Environment(format!("could not read your answer: {e}")))?;
    parse_pick(&line, ticked).map_err(Failure::Usage)
}

/// The agents to write for, or `None` when the user chose to write nothing (said already, exit `0`).
fn pick(args: &InstallArgs, root: &Path) -> Result<Option<Vec<Agent>>, Failure> {
    if args.all {
        return Ok(Some(AGENTS.to_vec()));
    }
    if !args.agent.is_empty() {
        return parse_names(&args.agent).map(Some);
    }
    let found = detect(root);
    let term = std::env::var("TERM").ok();
    let mut mode = prompt_for(
        std::io::stdin().is_terminal(),
        std::io::stderr().is_terminal(),
        term.as_deref(),
    );
    if mode == Prompt::Menu {
        match menu(&found) {
            Menu::Picked(agents) if agents.is_empty() => {
                let _ = writeln!(std::io::stderr(), "no agent picked: nothing written");
                return Ok(None);
            }
            Menu::Picked(agents) => return Ok(Some(agents)),
            Menu::Cancelled => {
                let _ = writeln!(std::io::stderr(), "cancelled: nothing written");
                return Ok(None);
            }
            Menu::Unavailable => mode = Prompt::TypedLine,
        }
    }
    if mode == Prompt::TypedLine {
        return ask(&found).map(Some);
    }
    // an agent or a script runs this: what the project shows, and when it shows nothing the agent has to say what it is
    if found.is_empty() {
        return Err(Failure::Usage(format!(
            "no coding agent is found in this project. Name yours: vnoc agent install --agent <name>\n  the names are {}",
            names()
        )));
    }
    Ok(Some(found))
}

// ---------------------------------------------------------------------------------------------------------------------- answer

#[derive(Serialize)]
struct Permission {
    agent: Agent,
    text: &'static str,
}

#[derive(Serialize)]
struct Changes {
    schema: u32,
    command: &'static str,
    dry_run: bool,
    root: String,
    agents: Vec<Agent>,
    actions: Vec<Action>,
    /// the lines to add to an agent's own settings: only for an agent whose syntax was read (07 §4)
    permissions: Vec<Permission>,
}

fn verb(change: Change, dry_run: bool) -> &'static str {
    match (change, dry_run) {
        (Change::Create, false) => "created",
        (Change::Create, true) => "will create",
        (Change::Replace, false) | (Change::ReplaceBlock, false) => "replaced",
        (Change::Replace, true) | (Change::ReplaceBlock, true) => "will replace",
        (Change::AddBlock, false) => "added",
        (Change::AddBlock, true) => "will add",
        (Change::Same, _) => "current",
        (Change::Leave, false) => "left",
        (Change::Leave, true) => "will leave",
        (Change::Delete, false) => "deleted",
        (Change::Delete, true) => "will delete",
        (Change::RemoveBlock, false) => "removed",
        (Change::RemoveBlock, true) => "will remove",
    }
}

fn who(agents: &[Agent]) -> String {
    agents
        .iter()
        .map(|a| a.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The line under a file: what it is, who it was written for (or, when taking it back for some agents, who else reads it), and why it was left.
fn about(action: &Action, command: &str, picked: &[Agent]) -> String {
    let what = match (action.part, action.change) {
        (Part::Skill, _) => "the skill",
        (Part::Rules, Change::AddBlock) => "the V-NOC block, after what was there",
        (Part::Rules, Change::RemoveBlock) => "the V-NOC block; the rest of the file is as it was",
        (Part::Rules, Change::Delete) => "it held nothing but the V-NOC block",
        (Part::Rules, _) => "the rules block",
        (Part::Import, _) => "imports AGENTS.md, so every Claude Code reads the rules",
    };
    let mut line = what.to_string();
    if action.part == Part::Import {
        // the sentence says who it is for
    } else if command == "agent install" {
        line.push_str(&format!(", for {}", who(&action.agents)));
    } else if !picked.is_empty() {
        // a folder or a file is shared: taking it back for one agent takes it from every agent that reads it
        let also: Vec<Agent> = action
            .readers
            .iter()
            .copied()
            .filter(|a| !picked.contains(a))
            .collect();
        if !also.is_empty() {
            line.push_str(&format!("; also read by {}", who(&also)));
        }
    }
    match &action.why {
        Some(why) => format!("{line}: {why}"),
        None => line,
    }
}

impl Render for Changes {
    fn text(&self, quiet: bool) -> String {
        if self.actions.is_empty() {
            return "nothing to remove\n".to_string();
        }
        let mut text = String::new();
        for action in &self.actions {
            let word = verb(action.change, self.dry_run);
            let word = if action.change == Change::Leave {
                style::paint(style::warn(), format!("{word:<13}"))
            } else {
                label(word)
            };
            text.push_str(&format!("{word}{}\n", action.path));
            text.push_str(&continued(&about(action, self.command, &self.agents)));
        }
        let wrote = !self.dry_run
            && self
                .actions
                .iter()
                .any(|a| a.change.touches() && !a.path.starts_with("~/"));
        if wrote && self.command == "agent install" && !quiet {
            text.push_str("\ncommit these files so your team's agents get them\n");
        }
        if !quiet {
            for line in &self.permissions {
                text.push_str(&format!("\n{}\n", line.text));
            }
        }
        text
    }
}

fn finish(global: &Global, changes: Changes) -> Result<(), Failure> {
    // a file `install` could not write is a request not met; a `SKILL.md` that `remove` leaves was never V-NOC's to take back
    let refused = changes.command == "agent install" && changes.actions.iter().any(Action::refused);
    out::render(&changes, global)?;
    // each file left was printed in its place: nothing more to say, and the request was not met
    if refused {
        Err(Failure::Reported)
    } else {
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------------------------- install

fn install(global: &Global, args: &InstallArgs) -> Result<(), Failure> {
    if args.global && args.rules_only {
        return Err(Failure::Usage(
            "--global writes no rules (a rule about .vnoc/ does not belong in every project's context); drop --rules-only".into(),
        ));
    }
    let root = root_of(global)?;
    let home = home();
    if args.global && home.is_none() {
        return Err(Failure::Environment(
            "the home folder is not known (HOME is not set): --global has nowhere to write".into(),
        ));
    }
    let Some(picked) = pick(args, &root)? else {
        return Ok(());
    };
    let options = Options {
        global: args.global,
        skill: !args.rules_only,
        rules: !args.skill_only,
        force: args.force,
        all: args.all,
    };
    let actions = agents::plan(
        &root,
        home.as_deref(),
        &picked,
        &options,
        &Texts::built_in(),
    );
    if !args.dry_run {
        agents::apply(&actions, &root, home.as_deref())
            .map_err(|e| Failure::Environment(format!("could not write {e}")))?;
    }
    finish(
        global,
        Changes {
            schema: 1,
            command: "agent install",
            dry_run: args.dry_run,
            root: root.to_string_lossy().into_owned(),
            permissions: if args.global {
                Vec::new()
            } else {
                permission_lines(&picked)
                    .into_iter()
                    .map(|(agent, text)| Permission { agent, text })
                    .collect()
            },
            agents: picked,
            actions,
        },
    )
}

// --------------------------------------------------------------------------------------------------------------------- remove

fn remove(global: &Global, args: &RemoveArgs) -> Result<(), Failure> {
    let root = root_of(global)?;
    let home = home();
    if args.global && home.is_none() {
        return Err(Failure::Environment(
            "the home folder is not known (HOME is not set): --global has nowhere to look".into(),
        ));
    }
    let picked = if args.agent.is_empty() {
        None
    } else {
        Some(parse_names(&args.agent)?)
    };
    let actions = agents::plan_remove(&root, home.as_deref(), picked.as_deref(), args.global);
    agents::apply(&actions, &root, home.as_deref())
        .map_err(|e| Failure::Environment(format!("could not remove {e}")))?;
    finish(
        global,
        Changes {
            schema: 1,
            command: "agent remove",
            dry_run: false,
            root: root.to_string_lossy().into_owned(),
            agents: picked.unwrap_or_default(),
            actions,
            permissions: Vec::new(),
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------------- list

#[derive(Serialize)]
struct Row {
    name: &'static str,
    detected: bool,
    skill: String,
    rules: &'static str,
}

#[derive(Serialize)]
struct List {
    schema: u32,
    command: &'static str,
    agents: Vec<Row>,
}

impl Render for List {
    fn text(&self, quiet: bool) -> String {
        let width = self.agents.iter().map(|r| r.name.len()).max().unwrap_or(0);
        let skill = self.agents.iter().map(|r| r.skill.len()).max().unwrap_or(0);
        let mut text = String::new();
        for row in &self.agents {
            let found = if row.detected { "found here" } else { "" };
            text.push_str(&format!(
                "{:<width$}  {:<10}  {:<skill$}  {}\n",
                style::paint(style::bold(), row.name),
                style::paint(style::good(), found),
                row.skill,
                row.rules
            ));
        }
        if !quiet {
            text.push_str("\nvnoc agent install --agent <name>[,<name>…] writes the skill and the rules; `other` is any agent not listed\n");
        }
        text
    }
}

fn list(global: &Global) -> Result<(), Failure> {
    let root = root_of(global)?;
    let found = detect(&root);
    let agents = AGENTS
        .iter()
        .map(|&a| Row {
            name: a.name(),
            detected: found.contains(&a),
            skill: a.skill_folder().file(),
            rules: agents::rules_for(&root, a),
        })
        .collect();
    out::render(
        &List {
            schema: 1,
            command: "agent list",
            agents,
        },
        global,
    )
}

// -------------------------------------------------------------------------------------------------------------------- status

#[derive(Serialize)]
struct StatusAnswer {
    schema: u32,
    command: &'static str,
    set_up: bool,
    #[serde(flatten)]
    status: agents::Status,
}

/// What a state says in a line, and whether it is a good one.
fn describe(state: &State, part: Part) -> (&'static str, String, bool) {
    match state {
        State::Current => ("current", String::new(), true),
        State::Older { version } => (
            "older",
            format!(
                "it is {version}, this vnoc is {}: vnoc agent install",
                agents::VERSION
            ),
            false,
        ),
        State::Newer { version } => (
            "newer",
            format!("it is {version}, this vnoc is {}", agents::VERSION),
            false,
        ),
        State::Changed => (
            "changed",
            match part {
                Part::Skill => "by hand: vnoc agent install replaces it".to_string(),
                Part::Rules => "it differs from this vnoc's block (edited, or from another version): vnoc agent install replaces it".to_string(),
                Part::Import => "it has more than the one import line V-NOC wrote: vnoc agent remove leaves it".to_string(),
            },
            false,
        ),
    }
}

impl Render for StatusAnswer {
    fn text(&self, _quiet: bool) -> String {
        if self.status.files.is_empty() {
            return "not set up: vnoc agent install\n".to_string();
        }
        let mut text = String::new();
        for file in &self.status.files {
            let (word, detail, good) = describe(&file.state, file.part);
            let word = if good {
                label(word)
            } else {
                style::paint(style::warn(), format!("{word:<13}"))
            };
            text.push_str(&format!("{word}{}\n", file.path));
            let part = match file.part {
                Part::Skill => "the skill",
                Part::Rules => "the rules block",
                Part::Import => "the import of AGENTS.md",
            };
            let line = if detail.is_empty() {
                format!("{part}, read by {}", who(&file.serves))
            } else {
                format!("{part}, {detail}")
            };
            text.push_str(&continued(&line));
        }
        for part in &self.status.missing {
            let (what, fix) = match part {
                Part::Skill => ("the skill", "--skill-only"),
                Part::Rules | Part::Import => ("the rules block", "--rules-only"),
            };
            text.push_str(&format!(
                "{}{what}\n{}",
                style::paint(style::warn(), format!("{:<13}", "missing")),
                continued(&format!("nothing carries it: vnoc agent install {fix}"))
            ));
        }
        text
    }
}

fn status(global: &Global) -> Result<(), Failure> {
    let root = root_of(global)?;
    let home = home();
    let status = agents::status(&root, home.as_deref(), &Texts::built_in());
    out::render(
        &StatusAnswer {
            schema: 1,
            command: "agent status",
            set_up: !status.files.is_empty(),
            status,
        },
        global,
    )
}

/// The one line `vnoc status` gives (03 §2): `skill current (.claude/skills) · rules in AGENTS.md`, or that nothing is set up, or what to run.
pub fn summary(status: &agents::Status) -> String {
    if status.files.is_empty() {
        return "not set up (vnoc agent install)".to_string();
    }
    let mut parts = Vec::new();
    for part in [Part::Skill, Part::Rules] {
        let files: Vec<_> = status.files.iter().filter(|f| f.part == part).collect();
        if files.is_empty() {
            continue;
        }
        let name = if part == Part::Skill {
            "skill"
        } else {
            "rules"
        };
        // the worst state decides what is said
        let worst = files
            .iter()
            .find(|f| matches!(f.state, State::Older { .. }))
            .or_else(|| files.iter().find(|f| matches!(f.state, State::Changed)))
            .or_else(|| {
                files
                    .iter()
                    .find(|f| matches!(f.state, State::Newer { .. }))
            });
        let places = files
            .iter()
            .map(|f| match part {
                Part::Skill => f.path.trim_end_matches("/vnoc/SKILL.md").to_string(),
                Part::Rules | Part::Import => f.path.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(match worst.map(|f| &f.state) {
            None => match part {
                Part::Skill => format!("skill current ({places})"),
                Part::Rules | Part::Import => format!("rules in {places}"),
            },
            Some(State::Older { version }) => style::paint(
                style::warn(),
                format!(
                    "{name} older ({version} → {}): vnoc agent install",
                    agents::VERSION
                ),
            ),
            Some(State::Newer { version }) => style::paint(
                style::warn(),
                format!("{name} newer ({version}; this vnoc is {})", agents::VERSION),
            ),
            Some(State::Changed) => style::paint(
                style::warn(),
                format!("{name} changed by hand: vnoc agent install"),
            ),
            Some(State::Current) => unreachable!("a current file is not the worst"),
        });
    }
    for part in &status.missing {
        let (name, flag) = match part {
            Part::Skill => ("skill", "--skill-only"),
            Part::Rules | Part::Import => ("rules", "--rules-only"),
        };
        parts.push(style::paint(
            style::warn(),
            format!("{name} missing: vnoc agent install {flag}"),
        ));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbered_list_takes_enter_numbers_and_refuses_anything_else() {
        let ticked = [Agent::Claude, Agent::Codex];
        // Enter: the ticked ones
        assert_eq!(parse_pick("", &ticked).unwrap(), ticked);
        assert_eq!(parse_pick("  \n", &ticked).unwrap(), ticked);
        // numbers, by spaces or commas, once each
        assert_eq!(
            parse_pick("1 3", &ticked).unwrap(),
            [Agent::Claude, Agent::Codex]
        );
        assert_eq!(
            parse_pick("3,1,3", &ticked).unwrap(),
            [Agent::Codex, Agent::Claude]
        );
        assert_eq!(parse_pick("12", &[]).unwrap(), [Agent::Other]);
        // anything else, and a number off the list
        assert!(parse_pick("x", &ticked).unwrap_err().contains("`x`"));
        assert!(parse_pick("0", &ticked).is_err());
        assert!(parse_pick("13", &ticked).is_err());
        assert!(parse_pick("1 x", &ticked).is_err());
        // Enter with nothing ticked is not a pick
        assert!(parse_pick("", &[]).is_err());
    }

    #[test]
    fn the_menu_needs_a_terminal_on_input_and_on_standard_error_that_can_draw_it() {
        // standard output does not matter: the question is on standard error
        assert_eq!(prompt_for(true, true, Some("xterm-256color")), Prompt::Menu);
        assert_eq!(prompt_for(true, true, None), Prompt::Menu);
        // a terminal that cannot move a cursor: the typed line
        assert_eq!(prompt_for(true, true, Some("dumb")), Prompt::TypedLine);
        // an agent or a script: no question, whatever TERM says
        assert_eq!(prompt_for(false, true, Some("xterm")), Prompt::None);
        assert_eq!(prompt_for(true, false, Some("xterm")), Prompt::None);
        assert_eq!(prompt_for(false, false, Some("dumb")), Prompt::None);
    }
}
