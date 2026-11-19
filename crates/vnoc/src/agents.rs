//! Setting coding agents up (docs/21-agent-cli/07-install.md §2): the skill and the rules block, written for the agents the user picked.
//!
//! **A name is only a label on one of two skill folders and one rules file** (07 §2): `.claude/skills/vnoc/` or `.agents/skills/vnoc/`, and
//! `AGENTS.md`, `CLAUDE.md` or `GEMINI.md`. The table is data here ([`Agent`]); a new agent, or one that moves, is a row and never a new mechanism.
//!
//! `plan` looks at the disk and decides, file by file, and writes nothing; `apply` writes what a plan says, each file atomically. Nothing is
//! written outside the root, or outside the home folder for a `--global` install, and no agent's settings file is ever written.
//! Nothing is stored about the pick: the files are the record (`status` reads them).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::userdata::atomic::write_atomic;

/// The version of this binary, which is the version a skill it writes says (a test holds `SKILL.md` to it).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The marker that opens the rules block, and the one that closes it (06 §4). Only what is between them (and they themselves) is ever replaced.
pub const BLOCK_START: &str = "<!-- vnoc:start";
pub const BLOCK_END: &str = "<!-- vnoc:end -->";

/// The two texts that get written: the skill and the rules block. [`Texts::built_in`] is what this binary carries; a test hands in others.
#[derive(Clone, Copy)]
pub struct Texts<'a> {
    pub skill: &'a str,
    pub rules: &'a str,
}

impl Texts<'static> {
    pub fn built_in() -> Self {
        Texts {
            skill: crate::cli::guide::SKILL,
            rules: crate::cli::guide::RULES,
        }
    }
}

// ----------------------------------------------------------------------------------------------------------------------- the table

/// The agents V-NOC knows by name (07 §2). `other` is any agent that is not here: it gets what the open format says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Cline,
    Codex,
    Cursor,
    Pi,
    Opencode,
    Copilot,
    Amp,
    Goose,
    Windsurf,
    Gemini,
    Other,
}

pub const AGENTS: [Agent; 12] = [
    Agent::Claude,
    Agent::Cline,
    Agent::Codex,
    Agent::Cursor,
    Agent::Pi,
    Agent::Opencode,
    Agent::Copilot,
    Agent::Amp,
    Agent::Goose,
    Agent::Windsurf,
    Agent::Gemini,
    Agent::Other,
];

/// The folders a skill goes to: Claude Code and Cline read the first, everyone else the second (07 §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkillFolder {
    Claude,
    Agents,
}

pub const SKILL_FOLDERS: [SkillFolder; 2] = [SkillFolder::Claude, SkillFolder::Agents];

impl SkillFolder {
    /// The skill's own folder, relative to the root (or the home folder).
    pub fn dir(self) -> &'static str {
        match self {
            SkillFolder::Claude => ".claude/skills/vnoc",
            SkillFolder::Agents => ".agents/skills/vnoc",
        }
    }

    pub fn file(self) -> String {
        format!("{}/SKILL.md", self.dir())
    }

    /// The names that map to this folder.
    pub fn agents(self) -> Vec<Agent> {
        AGENTS
            .into_iter()
            .filter(|a| a.skill_folder() == self)
            .collect()
    }
}

/// Which rules file an agent reads (07 §2). `Claude` is the one that depends on the project's files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rules {
    Claude,
    Gemini,
    Agents,
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Cline => "cline",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
            Agent::Pi => "pi",
            Agent::Opencode => "opencode",
            Agent::Copilot => "copilot",
            Agent::Amp => "amp",
            Agent::Goose => "goose",
            Agent::Windsurf => "windsurf",
            Agent::Gemini => "gemini",
            Agent::Other => "other",
        }
    }

    pub fn parse(name: &str) -> Option<Agent> {
        AGENTS.into_iter().find(|a| a.name() == name)
    }

    pub fn skill_folder(self) -> SkillFolder {
        match self {
            Agent::Claude | Agent::Cline => SkillFolder::Claude,
            _ => SkillFolder::Agents,
        }
    }

    fn rules(self) -> Rules {
        match self {
            Agent::Claude => Rules::Claude,
            Agent::Gemini => Rules::Gemini,
            _ => Rules::Agents,
        }
    }
}

/// Every name that reads the skill in `folder`.
fn skill_readers(folder: SkillFolder) -> Vec<Agent> {
    folder.agents()
}

/// Every name that is given the rules file `name` in this project.
fn rules_readers(root: &Path, name: &str) -> Vec<Agent> {
    AGENTS
        .into_iter()
        .filter(|a| rules_file(root, *a, true) == Some(name))
        .collect()
}

/// `CLAUDE.md` that hands over to `AGENTS.md` with an `@AGENTS.md` import (Claude Code's own idiom): the rules then belong in `AGENTS.md`.
fn imports_agents_md(text: &str) -> bool {
    text.lines().any(|line| {
        line.split_whitespace()
            .any(|word| word == "@AGENTS.md" || word == "@./AGENTS.md")
    })
}

/// The one line of the `CLAUDE.md` that V-NOC creates for `claude` when the project has none (07 §2, "The import file for `claude`").
const IMPORT_FILE: &str = "CLAUDE.md";
const IMPORT_TEXT: &str = "@AGENTS.md\n";

/// Whether `text` is exactly the import file V-NOC writes (a Windows checkout may have turned the line break into `\r\n`).
fn is_import_file(text: &str) -> bool {
    text.replace("\r\n", "\n") == IMPORT_TEXT
}

/// Whether `AGENTS.md` in the project holds V-NOC's block: what makes a `CLAUDE.md` that imports it a part of V-NOC's setup, and not only a person's own.
fn agents_md_has_block(root: &Path) -> bool {
    matches!(
        read_text(&root.join("AGENTS.md")),
        Ok(Some(text)) if matches!(find_block(&text), Block::At(..))
    )
}

/// The rules file `agent` is given in this project, relative to the root, or `None` when it would be created only by a name and `all` is not
/// asking for that (07 §2: `--all` writes the files that exist, and `AGENTS.md`).
fn rules_file(root: &Path, agent: Agent, all: bool) -> Option<&'static str> {
    match agent.rules() {
        Rules::Agents => Some("AGENTS.md"),
        Rules::Gemini => (!all || root.join("GEMINI.md").is_file()).then_some("GEMINI.md"),
        Rules::Claude => {
            // Claude Code reads `CLAUDE.md` (or `.claude/CLAUDE.md`) and `AGENTS.md` only when it has no CLAUDE.md, unless the first imports it
            for own in ["CLAUDE.md", ".claude/CLAUDE.md"] {
                if let Ok(text) = fs::read_to_string(root.join(own)) {
                    return Some(if imports_agents_md(&text) {
                        "AGENTS.md"
                    } else {
                        own
                    });
                }
            }
            Some("AGENTS.md")
        }
    }
}

/// The rules file `agent` is given in this project (what `list` shows): relative to the root.
pub fn rules_for(root: &Path, agent: Agent) -> &'static str {
    rules_file(root, agent, false).unwrap_or("AGENTS.md")
}

// ------------------------------------------------------------------------------------------------------------------------- detect

/// The agents this project shows signs of (07 §2). It only ticks boxes: nothing is written on its account.
pub fn detect(root: &Path) -> Vec<Agent> {
    let has = |p: &str| root.join(p).exists();
    let markers: [(Agent, &[&str]); 8] = [
        (Agent::Claude, &[".claude", "CLAUDE.md"]),
        (Agent::Codex, &[".codex"]),
        (Agent::Cursor, &[".cursor"]),
        (Agent::Pi, &[".pi"]),
        (Agent::Opencode, &[".opencode", "opencode.json"]),
        (Agent::Gemini, &[".gemini", "GEMINI.md"]),
        (Agent::Copilot, &[".github/copilot-instructions.md"]),
        (Agent::Cline, &[".clinerules", ".cline"]),
    ];
    let mut found: Vec<Agent> = markers
        .iter()
        .filter(|(_, files)| files.iter().any(|f| has(f)))
        .map(|(agent, _)| *agent)
        .collect();
    if found.is_empty() && (has("AGENTS.md") || has(".agents")) {
        found.push(Agent::Other);
    }
    found.sort();
    found
}

// -------------------------------------------------------------------------------------------------------------------------- plan

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    Skill,
    Rules,
    /// the one-line `CLAUDE.md` that imports `AGENTS.md`
    Import,
}

/// What happens to one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// the file is not there: it is written
    Create,
    /// a skill file of V-NOC's (or one `--force` replaces) is written over
    Replace,
    /// a rules file without the block: the block is appended after a blank line
    AddBlock,
    /// a rules file with the block: only what is between the markers is replaced
    ReplaceBlock,
    /// the file is what would be written already
    Same,
    /// not touched, and why (`Action::why`)
    Leave,
    /// `remove`: V-NOC's skill file is deleted (and its folder, if that is empty then)
    Delete,
    /// `remove`: the block is taken out of the file, and the rest is as it was
    RemoveBlock,
}

impl Change {
    /// Whether applying it writes or deletes anything.
    pub fn touches(self) -> bool {
        !matches!(self, Change::Same | Change::Leave)
    }
}

/// One file, and what is done to it.
#[derive(Clone, Debug, Serialize)]
pub struct Action {
    pub part: Part,
    /// as the user reads it: relative to the root, or `~/…` in the home folder
    pub path: String,
    pub change: Change,
    /// the names this action is for: the ones picked that map to this file
    pub agents: Vec<Agent>,
    /// every name that reads this file (a folder or a file is shared by all that map to it)
    pub readers: Vec<Agent>,
    /// why a file is left (or is replaced though it is not V-NOC's)
    pub why: Option<String>,
    #[serde(skip)]
    full: PathBuf,
    #[serde(skip)]
    content: Option<String>,
}

impl Action {
    /// Whether the user has to be told it was refused: a file V-NOC did not write, or markers it cannot trust.
    pub fn refused(&self) -> bool {
        self.change == Change::Leave
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// the skill goes to the picked agents' user folders under `home`, and no rules are written
    pub global: bool,
    pub skill: bool,
    pub rules: bool,
    /// replace a `SKILL.md` that V-NOC did not write
    pub force: bool,
    /// `--all`: every row of the table, so a rules file that is not `AGENTS.md` only when it exists
    pub all: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            global: false,
            skill: true,
            rules: true,
            force: false,
            all: false,
        }
    }
}

/// The version a `SKILL.md` says it is (`metadata.vnoc-version` in its header), which is also how V-NOC knows a file is its own.
pub fn skill_version(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    for line in lines {
        if line.trim_end() == "---" {
            return None;
        }
        if line.starts_with([' ', '\t'])
            && let Some(value) = line.trim().strip_prefix("vnoc-version:")
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The block's place in a text: from the start of its first line to the end of the closing marker.
enum Block {
    None,
    At(usize, usize),
    /// an opening marker with no closing one after it
    Broken,
}

fn find_block(text: &str) -> Block {
    let mut start = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if start.is_none() && line.starts_with(BLOCK_START) {
            start = Some(offset);
        }
        if let Some(s) = start
            && let Some(i) = line.find(BLOCK_END)
        {
            return Block::At(s, offset + i + BLOCK_END.len());
        }
        offset += line.len();
    }
    if start.is_some() {
        Block::Broken
    } else {
        Block::None
    }
}

/// `text` with the block set: replaced where it is, or appended after one blank line. `None` when the file already holds exactly it.
fn with_block(text: &str, block: &str) -> Result<Option<(Change, String)>, String> {
    let block = block.trim_end_matches('\n');
    match find_block(text) {
        Block::Broken => Err(format!(
            "it has a `{BLOCK_START}` line and no `{BLOCK_END}` after it: fix that by hand, then run this again"
        )),
        Block::At(start, end) => {
            if same_text(&text[start..end], block) {
                return Ok(None);
            }
            Ok(Some((
                Change::ReplaceBlock,
                format!("{}{block}{}", &text[..start], &text[end..]),
            )))
        }
        Block::None => {
            let gap = if text.is_empty() || text.ends_with("\n\n") {
                ""
            } else if text.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            Ok(Some((Change::AddBlock, format!("{text}{gap}{block}\n"))))
        }
    }
}

/// Equal but for line endings: a file saved on Windows holds the same block.
fn same_text(a: &str, b: &str) -> bool {
    a.replace('\r', "").trim_end() == b.replace('\r', "").trim_end()
}

/// What is read of a path: its text; `None` when it is not there; an error text when it is there and cannot be read as text.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "it is not UTF-8 text".to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn shown(base: &Path, home: bool, path: &Path) -> String {
    let rel = path
        .strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if home { format!("~/{rel}") } else { rel }
}

struct Plan<'a> {
    root: &'a Path,
    home: Option<&'a Path>,
    actions: Vec<Action>,
}

impl Plan<'_> {
    /// Adds the action for `full`, or, when the same file is already there (two agents that share it), the agent to it.
    fn push(&mut self, mut action: Action) {
        match self.actions.iter_mut().find(|a| a.full == action.full) {
            Some(existing) => {
                for agent in action.agents.drain(..) {
                    if !existing.agents.contains(&agent) {
                        existing.agents.push(agent);
                    }
                }
                existing.agents.sort();
            }
            None => self.actions.push(action),
        }
    }
}

/// What to write for `picked`, file by file, by 07 §2. Reads the disk and writes nothing. Several agents that share a folder or a file make one
/// action. `home` is only read for `--global` (it is `None` when the home folder is not known: then a global plan is empty).
pub fn plan(
    root: &Path,
    home: Option<&Path>,
    picked: &[Agent],
    options: &Options,
    texts: &Texts,
) -> Vec<Action> {
    let mut plan = Plan {
        root,
        home,
        actions: Vec::new(),
    };
    if options.skill {
        for &agent in picked {
            plan.skill(agent, options, texts);
        }
    }
    if options.rules && !options.global {
        for &agent in picked {
            plan.rules(agent, options, texts);
        }
    }
    plan.actions
}

impl Plan<'_> {
    fn skill(&mut self, agent: Agent, options: &Options, texts: &Texts) {
        let (base, is_home) = if options.global {
            match self.home {
                Some(home) => (home, true),
                None => return,
            }
        } else {
            (self.root, false)
        };
        let full = base.join(agent.skill_folder().file());
        let mut action = Action {
            part: Part::Skill,
            path: shown(base, is_home, &full),
            change: Change::Create,
            agents: vec![agent],
            readers: skill_readers(agent.skill_folder()),
            why: None,
            content: Some(texts.skill.to_string()),
            full: full.clone(),
        };
        match read_text(&full) {
            Ok(None) => {}
            Ok(Some(existing)) => {
                if skill_version(&existing).is_some() {
                    if existing == texts.skill {
                        action.change = Change::Same;
                        action.content = None;
                    } else {
                        action.change = Change::Replace;
                    }
                } else if options.force {
                    action.change = Change::Replace;
                    action.why = Some("it was not written by V-NOC (its header has no metadata.vnoc-version): replaced because of --force".into());
                } else {
                    action.change = Change::Leave;
                    action.content = None;
                    action.why = Some("it was not written by V-NOC (its header has no metadata.vnoc-version); --force replaces it".into());
                }
            }
            Err(why) => {
                action.change = Change::Leave;
                action.content = None;
                action.why = Some(why);
            }
        }
        self.push(action);
    }

    fn rules(&mut self, agent: Agent, options: &Options, texts: &Texts) {
        let Some(name) = rules_file(self.root, agent, options.all) else {
            return;
        };
        let full = self.root.join(name);
        let mut action = Action {
            part: Part::Rules,
            path: name.to_string(),
            change: Change::Create,
            agents: vec![agent],
            readers: rules_readers(self.root, name),
            why: None,
            content: None,
            full: full.clone(),
        };
        match read_text(&full) {
            Ok(None) => {
                action.content = Some(format!("{}\n", texts.rules.trim_end_matches('\n')));
            }
            Ok(Some(existing)) => match with_block(&existing, texts.rules) {
                Ok(None) => action.change = Change::Same,
                Ok(Some((change, text))) => {
                    action.change = change;
                    action.content = Some(text);
                }
                Err(why) => {
                    action.change = Change::Leave;
                    action.why = Some(why);
                }
            },
            Err(why) => {
                action.change = Change::Leave;
                action.why = Some(why);
            }
        }
        self.push(action);
        if agent == Agent::Claude && name == "AGENTS.md" {
            self.import_file();
        }
    }

    /// `claude` with no `CLAUDE.md` (and no `.claude/CLAUDE.md`): the block went to `AGENTS.md`, which Claude Code reads only from v2.1.277, so a
    /// `CLAUDE.md` of exactly one import line is created, and every version reads the block (07 §2). One that is already exactly that line is `Same`.
    /// Any other `CLAUDE.md` is not this rule's: it is not touched here.
    fn import_file(&mut self) {
        let full = self.root.join(IMPORT_FILE);
        let (change, content) = match read_text(&full) {
            Ok(Some(text)) if is_import_file(&text) => (Change::Same, None),
            Ok(None) if !self.root.join(".claude/CLAUDE.md").exists() => {
                (Change::Create, Some(IMPORT_TEXT.to_string()))
            }
            _ => return,
        };
        self.push(Action {
            part: Part::Import,
            path: IMPORT_FILE.to_string(),
            change,
            agents: vec![Agent::Claude],
            readers: vec![Agent::Claude],
            why: None,
            content,
            full,
        });
    }
}

/// Writes (or deletes, for `remove`) what the actions say, one file at a time, atomically. `Same` and `Leave` are skipped. Stops at the first
/// failure and says which file.
pub fn apply(actions: &[Action], root: &Path, home: Option<&Path>) -> io::Result<()> {
    for action in actions.iter().filter(|a| a.change.touches()) {
        // nothing is written outside the root, or the home folder for a `--global` install
        let inside =
            action.full.starts_with(root) || home.is_some_and(|h| action.full.starts_with(h));
        if !inside {
            return Err(io::Error::other(format!(
                "{}: outside the project",
                action.full.display()
            )));
        }
        let at = |e: io::Error| io::Error::new(e.kind(), format!("{}: {e}", action.path));
        match action.change {
            Change::Delete => {
                fs::remove_file(&action.full).map_err(at)?;
                // the skill's own folder (`…/skills/vnoc/`) goes with its file, and `skills/` and then `.claude/` or `.agents/` after it, each only while
                // it is empty (`remove_dir` refuses a folder with anything in it); a rules file's folder is the project
                if action.part == Part::Skill {
                    let mut dir = action.full.parent();
                    for _ in 0..3 {
                        match dir {
                            Some(d) if fs::remove_dir(d).is_ok() => dir = d.parent(),
                            _ => break,
                        }
                    }
                }
            }
            Change::RemoveBlock
            | Change::Create
            | Change::Replace
            | Change::AddBlock
            | Change::ReplaceBlock => {
                let content = action.content.as_deref().unwrap_or_default();
                write_atomic(&action.full, content.as_bytes()).map_err(at)?;
            }
            Change::Same | Change::Leave => {}
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------------------ status

/// The state of one file V-NOC wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    Current,
    /// a skill that says an older version than this binary's
    Older {
        version: String,
    },
    /// a skill that says a newer one: a teammate's V-NOC is ahead of this one
    Newer {
        version: String,
    },
    /// the skill's version is this one's and its text is not; or a rules block that is not this version's text (edited, or from another version)
    Changed,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileStatus {
    pub part: Part,
    pub path: String,
    #[serde(flatten)]
    pub state: State,
    /// the names that read this file
    pub serves: Vec<Agent>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    /// every file V-NOC wrote that is still there
    pub files: Vec<FileStatus>,
    /// a half that is absent while the other is there: the skill with no rules, or the rules with no skill
    pub missing: Vec<Part>,
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a).cmp(&parts(b))
}

/// The rules files V-NOC could have written, relative to the root.
const RULES_FILES: [&str; 4] = ["AGENTS.md", "CLAUDE.md", ".claude/CLAUDE.md", "GEMINI.md"];

/// What is on disk, read as what V-NOC wrote: each skill file with a `metadata.vnoc-version`, each rules file with the block. With `home`, the
/// user-level skill files too. Nothing is stored about which agents were picked: this is the record.
pub fn status(root: &Path, home: Option<&Path>, texts: &Texts) -> Status {
    let mut status = Status::default();
    let mut bases: Vec<(&Path, bool)> = vec![(root, false)];
    if let Some(home) = home {
        bases.push((home, true));
    }
    for (base, is_home) in bases {
        for folder in SKILL_FOLDERS {
            let full = base.join(folder.file());
            let Ok(Some(text)) = read_text(&full) else {
                continue;
            };
            let Some(version) = skill_version(&text) else {
                continue;
            };
            let state = match compare_versions(&version, VERSION) {
                std::cmp::Ordering::Less => State::Older { version },
                std::cmp::Ordering::Greater => State::Newer { version },
                std::cmp::Ordering::Equal if text == texts.skill => State::Current,
                std::cmp::Ordering::Equal => State::Changed,
            };
            status.files.push(FileStatus {
                part: Part::Skill,
                path: shown(base, is_home, &full),
                state,
                serves: folder.agents(),
            });
        }
    }
    for name in RULES_FILES {
        let full = root.join(name);
        let Ok(Some(text)) = read_text(&full) else {
            continue;
        };
        let Block::At(start, end) = find_block(&text) else {
            continue;
        };
        let state = if same_text(&text[start..end], texts.rules) {
            State::Current
        } else {
            State::Changed
        };
        let serves = rules_readers(root, name);
        status.files.push(FileStatus {
            part: Part::Rules,
            path: name.to_string(),
            state,
            serves,
        });
    }
    // the import file: V-NOC's while it is exactly the line; one that imports `AGENTS.md` and has more is said to be changed, but only where V-NOC's
    // block is in `AGENTS.md` (a person's own `CLAUDE.md` that imports it is not news in a project V-NOC was never set up in)
    if let Ok(Some(text)) = read_text(&root.join(IMPORT_FILE)) {
        let state = if is_import_file(&text) {
            Some(State::Current)
        } else if imports_agents_md(&text) && agents_md_has_block(root) {
            Some(State::Changed)
        } else {
            None
        };
        if let Some(state) = state {
            status.files.push(FileStatus {
                part: Part::Import,
                path: IMPORT_FILE.to_string(),
                state,
                serves: vec![Agent::Claude],
            });
        }
    }
    let has = |part| status.files.iter().any(|f| f.part == part);
    match (has(Part::Skill), has(Part::Rules)) {
        (true, false) => status.missing.push(Part::Rules),
        (false, true) => status.missing.push(Part::Skill),
        _ => {}
    }
    status
}

// ----------------------------------------------------------------------------------------------------------------------- remove

/// What `remove` would do for `picked` (`None`: every agent), or for the user-level skills with `global`: V-NOC's skill files are deleted, the
/// block is taken out of each rules file, a rules file that held nothing else goes, and **nothing else is touched**. A `SKILL.md` that V-NOC did
/// not write is left, and said.
///
/// A skill folder and a rules file are shared by every name that maps to them: removing for `codex` takes the skill every `.agents/skills`
/// reader uses. Each action says who reads the file.
pub fn plan_remove(
    root: &Path,
    home: Option<&Path>,
    picked: Option<&[Agent]>,
    global: bool,
) -> Vec<Action> {
    let agents: Vec<Agent> = picked
        .map(<[Agent]>::to_vec)
        .unwrap_or_else(|| AGENTS.to_vec());
    let mut plan = Plan {
        root,
        home,
        actions: Vec::new(),
    };
    for &agent in &agents {
        let (base, is_home) = if global {
            match home {
                Some(home) => (home, true),
                None => continue,
            }
        } else {
            (root, false)
        };
        let full = base.join(agent.skill_folder().file());
        if let Ok(Some(text)) = read_text(&full) {
            let ours = skill_version(&text).is_some();
            plan.push(Action {
                part: Part::Skill,
                path: shown(base, is_home, &full),
                change: if ours { Change::Delete } else { Change::Leave },
                agents: vec![agent],
                readers: skill_readers(agent.skill_folder()),
                why: (!ours).then(|| {
                    "it was not written by V-NOC (its header has no metadata.vnoc-version), so it is not removed".to_string()
                }),
                full,
                content: None,
            });
        }
        if global {
            continue;
        }
        // the files this agent's block could be in: for Claude Code its own files, and `AGENTS.md` only while that is where it would be written now
        // (so taking back `claude` does not take the block the other agents read)
        let candidates: Vec<&str> = match agent.rules() {
            Rules::Agents => vec!["AGENTS.md"],
            Rules::Gemini => vec!["GEMINI.md"],
            Rules::Claude => {
                let mut own = vec!["CLAUDE.md", ".claude/CLAUDE.md"];
                if rules_file(root, agent, false) == Some("AGENTS.md") {
                    own.push("AGENTS.md");
                }
                own
            }
        };
        for name in candidates {
            let full = root.join(name);
            let Ok(Some(text)) = read_text(&full) else {
                continue;
            };
            let Block::At(start, end) = find_block(&text) else {
                continue;
            };
            let (change, content) = without_block(&text, start, end);
            plan.push(Action {
                part: Part::Rules,
                path: name.to_string(),
                change,
                agents: vec![agent],
                readers: rules_readers(root, name),
                why: None,
                full,
                content,
            });
        }
        if agent == Agent::Claude {
            plan.remove_import_file();
        }
    }
    // the skills first, then the rules files (a stable sort: the order inside each is the table's)
    plan.actions.sort_by_key(|a| a.part != Part::Skill);
    plan.actions
}

impl Plan<'_> {
    /// The import file V-NOC wrote for `claude`: deleted while it is exactly its one line, and only where V-NOC's block is in `AGENTS.md` (an
    /// identical file a person made is theirs). A `CLAUDE.md` that imports `AGENTS.md` and has more in it is left, and said.
    fn remove_import_file(&mut self) {
        let full = self.root.join(IMPORT_FILE);
        let Ok(Some(text)) = read_text(&full) else {
            return;
        };
        if !agents_md_has_block(self.root) {
            return;
        }
        let (change, why) = if is_import_file(&text) {
            (Change::Delete, None)
        } else if imports_agents_md(&text) {
            (
                Change::Leave,
                Some(
                    "it has more than the one import line V-NOC wrote, so it is not removed"
                        .to_string(),
                ),
            )
        } else {
            return;
        };
        self.push(Action {
            part: Part::Import,
            path: IMPORT_FILE.to_string(),
            change,
            agents: vec![Agent::Claude],
            readers: vec![Agent::Claude],
            why,
            content: None,
            full,
        });
    }
}

/// `text` without the block at `start..end`: the line break after it goes with it, and so does the blank line that was put before it, when it
/// closes the file. A file that held nothing else is deleted.
fn without_block(text: &str, start: usize, end: usize) -> (Change, Option<String>) {
    let mut before = &text[..start];
    let mut after = &text[end..];
    after = after
        .strip_prefix("\r\n")
        .or_else(|| after.strip_prefix('\n'))
        .unwrap_or(after);
    // a block set apart by a blank line on each side leaves one blank line, not two
    if before.ends_with("\n\n") {
        after = after
            .strip_prefix("\r\n")
            .or_else(|| after.strip_prefix('\n'))
            .unwrap_or(after);
    }
    if after.trim().is_empty() && before.ends_with("\n\n") {
        before = &before[..before.len() - 1];
        after = "";
    }
    if before.trim().is_empty() && after.trim().is_empty() {
        return (Change::Delete, None);
    }
    (Change::RemoveBlock, Some(format!("{before}{after}")))
}

// ------------------------------------------------------------------------------------------------------------------- permissions

/// The lines that let an agent run V-NOC's read commands without asking (07 §4), for the agents whose syntax was read in the docs on 2026-11-19:
/// Claude Code and Codex. The name of the agent, and the text. Nothing for any other. `vnoc agent` is never in them: setting up agents is
/// the user's step, and a write is never allowed by default.
pub fn permission_lines(picked: &[Agent]) -> Vec<(Agent, &'static str)> {
    const CLAUDE: &str = "to let Claude Code run V-NOC's read commands without asking, add to .claude/settings.json:
  \"permissions\": { \"allow\": [\"Bash(vnoc status *)\", \"Bash(vnoc show *)\", \"Bash(vnoc code *)\", \"Bash(vnoc doc list *)\", \"Bash(vnoc doc read *)\",
                             \"Bash(vnoc doc path *)\", \"Bash(vnoc tree *)\", \"Bash(vnoc find *)\", \"Bash(vnoc grep *)\", \"Bash(vnoc calls *)\",
                             \"Bash(vnoc missing *)\", \"Bash(vnoc check *)\", \"Bash(vnoc tags *)\", \"Bash(vnoc guide *)\",
                             \"Bash(vnoc skill *)\"] }";
    const CODEX: &str = "to let Codex run them without asking, add to ~/.codex/rules/default.rules:
  prefix_rule(pattern = [\"vnoc\", [\"status\", \"show\", \"code\", \"tree\", \"find\", \"grep\", \"calls\", \"missing\", \"check\", \"tags\", \"guide\", \"skill\"]], decision = \"allow\")
  prefix_rule(pattern = [\"vnoc\", \"doc\", [\"list\", \"read\", \"path\"]], decision = \"allow\")";
    let mut lines = Vec::new();
    if picked.contains(&Agent::Claude) {
        lines.push((Agent::Claude, CLAUDE));
    }
    if picked.contains(&Agent::Codex) {
        lines.push((Agent::Codex, CODEX));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vnoc-agents-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn put(root: &Path, name: &str, text: &str) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn get(root: &Path, name: &str) -> String {
        fs::read_to_string(root.join(name)).unwrap()
    }

    fn texts() -> Texts<'static> {
        Texts::built_in()
    }

    /// The skill as an older V-NOC wrote it.
    fn old_skill() -> String {
        crate::cli::guide::SKILL.replace(
            &format!("vnoc-version: \"{VERSION}\""),
            "vnoc-version: \"0.0.1\"",
        )
    }

    fn install(
        root: &Path,
        home: Option<&Path>,
        picked: &[Agent],
        options: &Options,
    ) -> Vec<Action> {
        let actions = plan(root, home, picked, options, &texts());
        apply(&actions, root, home).unwrap();
        actions
    }

    fn paths(actions: &[Action]) -> Vec<(&str, Change)> {
        actions
            .iter()
            .map(|a| (a.path.as_str(), a.change))
            .collect()
    }

    fn block_count(text: &str) -> usize {
        text.matches(BLOCK_START).count()
    }

    #[test]
    fn each_row_of_the_table_gives_its_skill_folder_and_its_rules_file() {
        let root = fresh("table");
        let table = [
            ("claude", ".claude/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("cline", ".claude/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("codex", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("cursor", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("pi", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("opencode", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("copilot", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("amp", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("goose", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("windsurf", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
            ("gemini", ".agents/skills/vnoc/SKILL.md", "GEMINI.md"),
            ("other", ".agents/skills/vnoc/SKILL.md", "AGENTS.md"),
        ];
        assert_eq!(table.len(), AGENTS.len());
        for (name, skill, rules) in table {
            let agent = Agent::parse(name).unwrap();
            let actions = plan(&root, None, &[agent], &Options::default(), &texts());
            // `claude` in a project with no CLAUDE.md also gets the import file (07 §2)
            let mut expected = vec![(skill, Change::Create), (rules, Change::Create)];
            if agent == Agent::Claude {
                expected.push(("CLAUDE.md", Change::Create));
            }
            assert_eq!(paths(&actions), expected, "{name}");
            assert_eq!(actions[0].part, Part::Skill);
            assert_eq!(actions[1].part, Part::Rules);
            assert_eq!(actions[0].agents, [agent]);
            assert_eq!(rules_for(&root, agent), rules);
        }
        // nothing was written by planning
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(Agent::parse("nope"), None);
    }

    #[test]
    fn gemini_with_no_gemini_md_creates_it_with_the_block() {
        let root = fresh("gemini");
        let actions = install(&root, None, &[Agent::Gemini], &Options::default());
        assert_eq!(actions[1].change, Change::Create);
        let text = get(&root, "GEMINI.md");
        assert_eq!(text, format!("{}\n", crate::cli::guide::RULES.trim_end()));
        assert!(text.starts_with(BLOCK_START) && text.trim_end().ends_with(BLOCK_END));
        assert!(!root.join("AGENTS.md").exists());
    }

    #[test]
    fn claude_follows_its_claude_md_unless_that_imports_agents_md() {
        // no CLAUDE.md: AGENTS.md (Claude Code reads it then)
        let root = fresh("claude-none");
        assert_eq!(rules_for(&root, Agent::Claude), "AGENTS.md");
        // a CLAUDE.md: it gets the block, and AGENTS.md is not made
        put(&root, "CLAUDE.md", "# Notes\nuse tabs\n");
        let actions = install(&root, None, &[Agent::Claude], &Options::default());
        assert_eq!(actions[1].path, "CLAUDE.md");
        assert_eq!(actions[1].change, Change::AddBlock);
        assert!(get(&root, "CLAUDE.md").starts_with("# Notes\nuse tabs\n\n<!-- vnoc:start"));
        assert!(!root.join("AGENTS.md").exists());
        // a CLAUDE.md that imports AGENTS.md: AGENTS.md gets it and CLAUDE.md is left as it is
        let root = fresh("claude-import");
        put(&root, "CLAUDE.md", "@AGENTS.md\n\n## Claude Code\nx\n");
        let actions = install(&root, None, &[Agent::Claude], &Options::default());
        assert_eq!(actions[1].path, "AGENTS.md");
        assert_eq!(actions[1].change, Change::Create);
        assert_eq!(get(&root, "CLAUDE.md"), "@AGENTS.md\n\n## Claude Code\nx\n");
        // the other place Claude Code reads
        let root = fresh("claude-dot");
        put(&root, ".claude/CLAUDE.md", "mine\n");
        assert_eq!(rules_for(&root, Agent::Claude), ".claude/CLAUDE.md");
        put(&root, ".claude/CLAUDE.md", "@AGENTS.md\n");
        assert_eq!(rules_for(&root, Agent::Claude), "AGENTS.md");
    }

    #[test]
    fn agents_that_share_a_folder_or_a_file_make_one_action() {
        // claude + cursor: both folders
        let root = fresh("share-two");
        let actions = plan(
            &root,
            None,
            &[Agent::Claude, Agent::Cursor],
            &Options::default(),
            &texts(),
        );
        assert_eq!(
            paths(&actions),
            [
                (".claude/skills/vnoc/SKILL.md", Change::Create),
                (".agents/skills/vnoc/SKILL.md", Change::Create),
                ("AGENTS.md", Change::Create),
                ("CLAUDE.md", Change::Create),
            ]
        );
        assert_eq!(actions[2].agents, [Agent::Claude, Agent::Cursor]);
        // codex + pi: one folder, written once, and one AGENTS.md
        let root = fresh("share-one");
        let actions = install(&root, None, &[Agent::Codex, Agent::Pi], &Options::default());
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].agents, [Agent::Codex, Agent::Pi]);
        assert_eq!(block_count(&get(&root, "AGENTS.md")), 1);
    }

    #[test]
    fn all_is_both_folders_and_the_rules_files_that_exist_with_agents_md() {
        let root = fresh("all");
        let all = Options {
            all: true,
            ..Options::default()
        };
        let actions = plan(&root, None, &AGENTS, &all, &texts());
        assert_eq!(
            paths(&actions),
            [
                (".claude/skills/vnoc/SKILL.md", Change::Create),
                (".agents/skills/vnoc/SKILL.md", Change::Create),
                ("AGENTS.md", Change::Create),
                ("CLAUDE.md", Change::Create),
            ]
        );
        // a GEMINI.md and a CLAUDE.md that are there get the block too
        put(&root, "GEMINI.md", "g\n");
        put(&root, "CLAUDE.md", "c\n");
        let actions = plan(&root, None, &AGENTS, &all, &texts());
        let rules: Vec<_> = actions
            .iter()
            .filter(|a| a.part == Part::Rules)
            .map(|a| a.path.as_str())
            .collect();
        assert_eq!(rules, ["CLAUDE.md", "AGENTS.md", "GEMINI.md"]);
    }

    #[test]
    fn detect_finds_what_each_agent_leaves_behind() {
        let marks: [(&str, Agent); 11] = [
            (".claude/x", Agent::Claude),
            ("CLAUDE.md", Agent::Claude),
            (".codex/x", Agent::Codex),
            (".cursor/x", Agent::Cursor),
            (".pi/x", Agent::Pi),
            (".opencode/x", Agent::Opencode),
            ("opencode.json", Agent::Opencode),
            (".gemini/x", Agent::Gemini),
            ("GEMINI.md", Agent::Gemini),
            (".github/copilot-instructions.md", Agent::Copilot),
            (".clinerules/x", Agent::Cline),
        ];
        for (i, (mark, agent)) in marks.into_iter().enumerate() {
            let root = fresh(&format!("detect-{i}"));
            put(&root, mark, "x");
            assert_eq!(detect(&root), [agent], "{mark}");
        }
        let root = fresh("detect-cline-dot");
        put(&root, ".cline/x", "x");
        assert_eq!(detect(&root), [Agent::Cline]);
        // an AGENTS.md or .agents/ alone is `other`; with another sign it is not counted
        let root = fresh("detect-other");
        assert!(detect(&root).is_empty());
        put(&root, "AGENTS.md", "x");
        assert_eq!(detect(&root), [Agent::Other]);
        let root = fresh("detect-other-dir");
        put(&root, ".agents/x", "x");
        assert_eq!(detect(&root), [Agent::Other]);
        put(&root, ".cursor/x", "x");
        assert_eq!(detect(&root), [Agent::Cursor]);
        // several, in the table's order
        put(&root, ".codex/x", "x");
        assert_eq!(detect(&root), [Agent::Codex, Agent::Cursor]);
    }

    #[test]
    fn the_block_is_added_after_one_blank_line_and_the_rest_is_byte_for_byte() {
        let block = crate::cli::guide::RULES.trim_end();
        for (label, before) in [
            ("nl", "# My project\n\nSome rules.\n"),
            ("blank", "# My project\n\n\n"),
            ("no-nl", "# My project"),
            ("crlf", "# My project\r\n\r\ntext\r\n"),
            ("empty", ""),
        ] {
            let root = fresh(&format!("block-{label}"));
            put(&root, "AGENTS.md", before);
            let actions = install(&root, None, &[Agent::Codex], &Options::default());
            let after = get(&root, "AGENTS.md");
            assert!(
                after.starts_with(before),
                "{label}: the text before is untouched"
            );
            let added = &after[before.len()..];
            if before.is_empty() {
                assert_eq!(actions[1].change, Change::AddBlock);
                assert_eq!(added, format!("{block}\n"));
            } else {
                // exactly one blank line between what was there and the block
                let gap = added.len() - added.trim_start_matches('\n').len();
                let before_gap = before.len() - before.trim_end_matches('\n').len();
                // one blank line: what is needed to make two line breaks; none added when the file already ends in a blank line
                assert_eq!(
                    gap,
                    2usize.saturating_sub(before_gap),
                    "{label}: one blank line"
                );
                assert_eq!(added.trim_start_matches('\n'), format!("{block}\n"));
            }
            // installing again changes nothing: one block
            let again = plan(&root, None, &[Agent::Codex], &Options::default(), &texts());
            assert_eq!(
                paths(&again),
                [
                    (".agents/skills/vnoc/SKILL.md", Change::Same),
                    ("AGENTS.md", Change::Same)
                ],
                "{label}"
            );
            apply(&again, &root, None).unwrap();
            assert_eq!(get(&root, "AGENTS.md"), after);
            assert_eq!(block_count(&after), 1);
        }
    }

    #[test]
    fn a_block_from_another_version_is_replaced_in_place_and_nothing_around_it_moves() {
        let root = fresh("replace-block");
        let old = "<!-- vnoc:start (older) -->\nold rules\n<!-- vnoc:end -->";
        put(
            &root,
            "AGENTS.md",
            &format!("# Top\n\nbefore\n\n{old}\n\nafter, with no newline at the end"),
        );
        let actions = install(&root, None, &[Agent::Codex], &Options::default());
        assert_eq!(actions[1].change, Change::ReplaceBlock);
        let block = crate::cli::guide::RULES.trim_end();
        assert_eq!(
            get(&root, "AGENTS.md"),
            format!("# Top\n\nbefore\n\n{block}\n\nafter, with no newline at the end")
        );
    }

    #[test]
    fn markers_that_cannot_be_trusted_leave_the_file_alone() {
        let root = fresh("broken");
        let text = "intro\n<!-- vnoc:start (x) -->\nhalf a block\n";
        put(&root, "AGENTS.md", text);
        let actions = install(&root, None, &[Agent::Codex], &Options::default());
        assert_eq!(actions[1].change, Change::Leave);
        assert!(actions[1].refused());
        assert!(actions[1].why.as_deref().unwrap().contains("vnoc:end"));
        assert_eq!(get(&root, "AGENTS.md"), text);
        // the skill beside it was still written
        assert!(root.join(".agents/skills/vnoc/SKILL.md").is_file());
    }

    #[test]
    fn a_skill_is_ours_when_its_header_has_a_version() {
        assert_eq!(
            skill_version(crate::cli::guide::SKILL).as_deref(),
            Some(VERSION)
        );
        assert_eq!(skill_version("---\nname: vnoc\n---\nbody"), None);
        assert_eq!(skill_version("# no header\n  vnoc-version: \"1\"\n"), None);
        // a version in the body, past the header, is not the header's
        assert_eq!(
            skill_version("---\nname: x\n---\n  vnoc-version: \"1\"\n"),
            None
        );
        assert_eq!(
            skill_version("---\nname: x\nmetadata:\n  vnoc-version: \"0.9.1\"\n---\n").as_deref(),
            Some("0.9.1")
        );
    }

    #[test]
    fn a_skill_md_that_is_not_ours_is_left_and_force_replaces_it() {
        let root = fresh("not-ours");
        let mine = "---\nname: vnoc\ndescription: my own\n---\nhand-written\n";
        put(&root, ".agents/skills/vnoc/SKILL.md", mine);
        let actions = install(&root, None, &[Agent::Codex], &Options::default());
        assert_eq!(actions[0].change, Change::Leave);
        assert!(actions[0].refused());
        assert!(actions[0].why.as_deref().unwrap().contains("--force"));
        assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), mine);
        // the rules, which are separate, are written all the same
        assert!(root.join("AGENTS.md").is_file());
        // --force
        let force = Options {
            force: true,
            ..Options::default()
        };
        let actions = install(&root, None, &[Agent::Codex], &force);
        assert_eq!(actions[0].change, Change::Replace);
        assert!(!actions[0].refused());
        assert_eq!(
            get(&root, ".agents/skills/vnoc/SKILL.md"),
            crate::cli::guide::SKILL
        );
        // one of ours, from an older version, is replaced with no --force
        put(&root, ".agents/skills/vnoc/SKILL.md", &old_skill());
        let actions = install(&root, None, &[Agent::Codex], &Options::default());
        assert_eq!(actions[0].change, Change::Replace);
        assert_eq!(actions[0].why, None);
    }

    #[test]
    fn skill_only_and_rules_only_write_one_half() {
        let root = fresh("halves");
        let skill = Options {
            rules: false,
            ..Options::default()
        };
        let actions = install(&root, None, &[Agent::Codex], &skill);
        assert_eq!(
            paths(&actions),
            [(".agents/skills/vnoc/SKILL.md", Change::Create)]
        );
        assert!(!root.join("AGENTS.md").exists());
        let root = fresh("halves-rules");
        let rules = Options {
            skill: false,
            ..Options::default()
        };
        let actions = install(&root, None, &[Agent::Codex], &rules);
        assert_eq!(paths(&actions), [("AGENTS.md", Change::Create)]);
        assert!(!root.join(".agents").exists());
    }

    #[test]
    fn global_writes_the_skill_under_home_and_no_rules_and_nothing_in_the_root() {
        let root = fresh("global-root");
        let home = fresh("global-home");
        let global = Options {
            global: true,
            ..Options::default()
        };
        let actions = install(&root, Some(&home), &[Agent::Claude, Agent::Codex], &global);
        assert_eq!(
            paths(&actions),
            [
                ("~/.claude/skills/vnoc/SKILL.md", Change::Create),
                ("~/.agents/skills/vnoc/SKILL.md", Change::Create)
            ]
        );
        assert!(actions.iter().all(|a| a.part == Part::Skill));
        assert!(home.join(".claude/skills/vnoc/SKILL.md").is_file());
        assert!(home.join(".agents/skills/vnoc/SKILL.md").is_file());
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            0,
            "nothing in the root"
        );
        assert!(!home.join("AGENTS.md").exists() && !home.join("CLAUDE.md").exists());
        // no home known: nothing is planned
        assert!(plan(&root, None, &[Agent::Codex], &global, &texts()).is_empty());
        // status sees the user-level files when it is given the home
        let seen = status(&root, Some(&home), &texts());
        assert_eq!(seen.files.len(), 2);
        assert!(seen.files.iter().all(|f| f.path.starts_with("~/")));
        // and remove --global takes them back, and only them
        put(&root, ".agents/skills/vnoc/SKILL.md", &old_skill());
        let removal = plan_remove(&root, Some(&home), None, true);
        apply(&removal, &root, Some(&home)).unwrap();
        assert!(!home.join(".claude/skills/vnoc").exists());
        assert!(!home.join(".agents/skills/vnoc").exists());
        assert!(root.join(".agents/skills/vnoc/SKILL.md").is_file());
    }

    #[test]
    fn apply_writes_nothing_outside_the_root() {
        let root = fresh("outside-root");
        let elsewhere = fresh("outside-else");
        let actions = plan(
            &elsewhere,
            None,
            &[Agent::Codex],
            &Options::default(),
            &texts(),
        );
        let err = apply(&actions, &root, None).unwrap_err();
        assert!(err.to_string().contains("outside the project"), "{err}");
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    }

    #[test]
    fn status_says_current_older_newer_changed_and_missing() {
        let root = fresh("status");
        assert!(status(&root, None, &texts()).files.is_empty());
        install(
            &root,
            None,
            &[Agent::Claude, Agent::Codex],
            &Options::default(),
        );
        let seen = status(&root, None, &texts());
        assert!(seen.missing.is_empty());
        let states: Vec<_> = seen
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.state.clone()))
            .collect();
        assert_eq!(
            states,
            [
                (".claude/skills/vnoc/SKILL.md", State::Current),
                (".agents/skills/vnoc/SKILL.md", State::Current),
                ("AGENTS.md", State::Current),
                ("CLAUDE.md", State::Current),
            ]
        );
        assert_eq!(seen.files[0].serves, [Agent::Claude, Agent::Cline]);
        assert!(seen.files[1].serves.contains(&Agent::Codex));
        assert!(
            seen.files[2].serves.contains(&Agent::Claude)
                && seen.files[2].serves.contains(&Agent::Codex)
        );

        // older: the version in the installed copy edited down
        put(&root, ".agents/skills/vnoc/SKILL.md", &old_skill());
        let seen = status(&root, None, &texts());
        assert_eq!(
            seen.files[1].state,
            State::Older {
                version: "0.0.1".into()
            }
        );
        // newer
        let newer = crate::cli::guide::SKILL.replace(
            &format!("vnoc-version: \"{VERSION}\""),
            "vnoc-version: \"99.0.0\"",
        );
        put(&root, ".agents/skills/vnoc/SKILL.md", &newer);
        assert_eq!(
            status(&root, None, &texts()).files[1].state,
            State::Newer {
                version: "99.0.0".into()
            }
        );
        // changed by hand: this version's header, other words
        let edited = format!("{}\nAnd always say please.\n", crate::cli::guide::SKILL);
        put(&root, ".agents/skills/vnoc/SKILL.md", &edited);
        assert_eq!(status(&root, None, &texts()).files[1].state, State::Changed);
        // a rules block that differs
        let text =
            get(&root, "AGENTS.md").replace("Notes are information", "Notes are mere information");
        put(&root, "AGENTS.md", &text);
        assert_eq!(status(&root, None, &texts()).files[2].state, State::Changed);
        // a SKILL.md that is not ours is not a file V-NOC wrote
        put(
            &root,
            ".agents/skills/vnoc/SKILL.md",
            "---\nname: vnoc\n---\nmine\n",
        );
        assert_eq!(status(&root, None, &texts()).files.len(), 3); // AGENTS.md and the import file

        // missing: the skill with no rules, and the rules with no skill
        let root = fresh("status-missing");
        install(
            &root,
            None,
            &[Agent::Codex],
            &Options {
                rules: false,
                ..Options::default()
            },
        );
        assert_eq!(status(&root, None, &texts()).missing, [Part::Rules]);
        let root = fresh("status-missing-2");
        install(
            &root,
            None,
            &[Agent::Codex],
            &Options {
                skill: false,
                ..Options::default()
            },
        );
        assert_eq!(status(&root, None, &texts()).missing, [Part::Skill]);
    }

    #[test]
    fn remove_takes_back_what_was_written_and_nothing_else() {
        // a rules file with other text: the block goes, and the file is as it was
        let root = fresh("remove");
        let original = "# My project\n\nSome rules.\n";
        put(&root, "AGENTS.md", original);
        install(
            &root,
            None,
            &[Agent::Codex, Agent::Gemini],
            &Options::default(),
        );
        assert!(root.join("GEMINI.md").is_file());
        let removal = plan_remove(&root, None, None, false);
        let changes: Vec<_> = paths(&removal);
        assert_eq!(
            changes,
            [
                (".agents/skills/vnoc/SKILL.md", Change::Delete),
                ("AGENTS.md", Change::RemoveBlock),
                ("GEMINI.md", Change::Delete),
            ]
        );
        apply(&removal, &root, None).unwrap();
        assert_eq!(
            get(&root, "AGENTS.md"),
            original,
            "the rest is byte for byte"
        );
        assert!(
            !root.join("GEMINI.md").exists(),
            "a file that held only the block goes"
        );
        assert!(!root.join(".agents/skills/vnoc").exists());
        // the folders V-NOC made around it go too, once they are empty (A11f)
        assert!(!root.join(".agents").exists());
        // and nothing is left to remove
        assert!(plan_remove(&root, None, None, false).is_empty());

        // a skill folder that holds something else keeps it, and a hand-written SKILL.md is left
        let root = fresh("remove-else");
        install(&root, None, &[Agent::Codex], &Options::default());
        put(&root, ".agents/skills/vnoc/notes.txt", "mine");
        apply(&plan_remove(&root, None, None, false), &root, None).unwrap();
        assert!(!root.join(".agents/skills/vnoc/SKILL.md").exists());
        assert_eq!(get(&root, ".agents/skills/vnoc/notes.txt"), "mine");
        let root = fresh("remove-hand");
        let hand = "---\nname: vnoc\n---\nhand-written\n";
        put(&root, ".agents/skills/vnoc/SKILL.md", hand);
        let removal = plan_remove(&root, None, None, false);
        assert_eq!(
            paths(&removal),
            [(".agents/skills/vnoc/SKILL.md", Change::Leave)]
        );
        apply(&removal, &root, None).unwrap();
        assert_eq!(get(&root, ".agents/skills/vnoc/SKILL.md"), hand);

        // by agent: claude's files, not codex's; a block in the middle of a file goes with its own blank lines kept for the rest
        let root = fresh("remove-by-agent");
        put(&root, "CLAUDE.md", "intro\n");
        install(
            &root,
            None,
            &[Agent::Claude, Agent::Codex],
            &Options::default(),
        );
        let removal = plan_remove(&root, None, Some(&[Agent::Claude]), false);
        apply(&removal, &root, None).unwrap();
        assert!(!root.join(".claude/skills/vnoc").exists());
        assert_eq!(get(&root, "CLAUDE.md"), "intro\n");
        assert!(root.join(".agents/skills/vnoc/SKILL.md").is_file());
        assert_eq!(block_count(&get(&root, "AGENTS.md")), 1);
        let middle = format!("top\n\n{}\n\nbottom\n", crate::cli::guide::RULES.trim_end());
        put(&root, "AGENTS.md", &middle);
        apply(&plan_remove(&root, None, None, false), &root, None).unwrap();
        assert_eq!(get(&root, "AGENTS.md"), "top\n\nbottom\n");
    }

    #[test]
    fn the_permission_lines_are_printed_only_for_the_agents_whose_syntax_was_read() {
        assert!(permission_lines(&[Agent::Cursor, Agent::Pi, Agent::Other]).is_empty());
        let both = permission_lines(&[Agent::Codex, Agent::Claude, Agent::Gemini]);
        let agents: Vec<_> = both.iter().map(|(a, _)| *a).collect();
        assert_eq!(agents, [Agent::Claude, Agent::Codex]);
        let claude = both[0].1;
        // the reads are allowed; no write, and not `vnoc agent`
        for read in [
            "status", "show", "code", "doc list", "doc read", "doc path", "tree", "find", "grep",
            "calls", "missing", "check", "tags", "guide", "skill",
        ] {
            assert!(claude.contains(&format!("Bash(vnoc {read} *)")), "{read}");
        }
        for write in ["init", "doc new", "doc tag", "relink", "repair", "agent"] {
            assert!(!claude.contains(&format!("vnoc {write}")), "{write}");
            assert!(!both[1].1.contains(&format!("\"{write}\"")), "{write}");
        }
    }
}
