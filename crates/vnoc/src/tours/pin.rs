//! Pinning a tour to a commit (`docs/26-tours/01-on-disk.md` §8, `03-core.md` §5): **every call of git in the tours module is here**, with its arguments
//! built here and nowhere else (`06-build-order.md` §1).
//!
//! *Pinned* means one thing: there is a commit in which every stop of the tour is exactly the code the tour was written about. Three questions, in
//! order, and no old file is read to answer them:
//!
//! 1. is every written stop fresh against the files as they are now? (the check of `check.rs`, which also says where each stop and block is now)
//! 2. are all the tour's files committed, and in HEAD? **Two calls**, because `git status -- <path>` says nothing for a clean file, an ignored file and a
//!    path that does not exist alike (TR0, check 6): `status` must say nothing, and `ls-tree HEAD` must name every file.
//! 3. both yes: HEAD is what is on disk is what the tour explains. The `pin` record carries HEAD and where every stop and block is now.
//!
//! What git is given is checked first (`01` §10): a commit is 40 lower-case hex characters ([`Commit`]) and a path a `RepoPath` before git sees either, so a
//! tour that came by `git pull` cannot hand git an option. `git show` gets `<commit>:<path>` as its one argument, **with no `--` before it** (after `--`
//! git reads it as a path and prints nothing, with exit 0: TR0). The paths of `status`, `ls-tree`, `add` and `commit` go after `--`, and are read as
//! literal names (`--literal-pathspecs` for the calls that run no hook; `:(literal)` for the commit, whose hooks must not inherit it): a file called
//! `a*.py` is that file.
//!
//! Nothing here checks anything out: the code of a pinned commit is read with `git show`, and kept in memory by commit and path.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Serialize, Serializer};
use vnoc_domain::RepoPath;

use super::check::{Placed, Recorded, StopState};
use super::fold::{State, Tour};
use super::record::{Commit, PinRecord, StopId};
use super::store;
use super::time;
use crate::project::Project;
use crate::source;
use crate::userdata::layout;

// ---------------------------------------------------------------------------------------------------------------------------------------- git

/// Why git could not be run at all.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Unrun {
    /// `git` is not on the `PATH`
    NotFound,
    /// it could not be started for another reason
    Other(String),
}

/// What git answered: its exit code (none when a signal ended it), standard output and the standard error as text.
struct Said {
    code: Option<i32>,
    out: Vec<u8>,
    err: String,
}

impl Said {
    fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// The first line git said on standard error, for a sentence.
    fn first_line(&self) -> String {
        self.err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("no message")
            .to_string()
    }
}

/// `git -C <root> <args>`. `literal` reads every path as a name and not as a pattern; it is not used for the commit, whose hooks would inherit it.
fn git(root: &Path, literal: bool, args: &[&str]) -> Result<Said, Unrun> {
    let mut command = Command::new("git");
    if literal {
        command.arg("--literal-pathspecs");
    }
    let output = command
        .arg("-C")
        .arg(root)
        .args(args)
        // a read of the state is not a reason to write the index
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => Unrun::NotFound,
            _ => Unrun::Other(e.to_string()),
        })?;
    Ok(Said {
        code: output.status.code(),
        out: output.stdout,
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn unrun_reason(unrun: Unrun) -> Reason {
    match unrun {
        Unrun::NotFound => Reason::GitNotFound,
        Unrun::Other(why) => Reason::GitFailed {
            why: format!("git could not be run ({why})"),
        },
    }
}

/// A path as git is given it.
fn names(paths: &[RepoPath]) -> Vec<&str> {
    paths.iter().map(RepoPath::as_str).collect()
}

// ------------------------------------------------------------------------------------------------------------------------------------ reasons

/// A file of the tour with something in it that is not committed: git's two-letter status and the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changed {
    /// as `git status --porcelain` says it: ` M`, `M `, `A `, `??`, …
    pub status: String,
    #[serde(serialize_with = "path_text")]
    pub path: RepoPath,
}

fn path_text<S: Serializer>(path: &RepoPath, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(path.as_str())
}

fn stops_text<S: Serializer>(ids: &[StopId], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(ids.iter().map(ToString::to_string))
}

impl Changed {
    /// `M  app/inventory.py`, `?? app/gateway.py`: git's short form.
    pub fn line(&self) -> String {
        format!("{} {}", self.status, self.path)
    }

    fn untracked(&self) -> bool {
        self.status == "??"
    }
}

/// Why a tour is not pinned, or cannot be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Reason {
    /// the tour is being written (or stopped): a pin is for a finished tour
    Unfinished,
    /// no stop of the tour is written: there is no code to pin
    NoStops,
    NotARepository,
    GitNotFound,
    /// `rev-parse --verify -q HEAD` says there is none
    NoCommit,
    /// a merge, a rebase, a cherry-pick or a revert is in the middle: HEAD is not a state anyone chose
    InProgress {
        what: &'static str,
    },
    /// the commit was made, and a hook (a formatter, `lint-staged`) changed `path` while it did: the commit holds code the check never read, so the
    /// tour is not pinned to it. `tour` is for the sentence: `pin` again checks the new code (01 §8)
    HookChanged {
        #[serde(serialize_with = "path_text")]
        path: RepoPath,
        tour: String,
    },
    /// a stop changed or is gone since it was written: replace it, then pin
    Stale {
        #[serde(serialize_with = "stops_text")]
        changed: Vec<StopId>,
        #[serde(serialize_with = "stops_text")]
        gone: Vec<StopId>,
    },
    /// files of the tour with changes that are not committed
    Uncommitted {
        files: Vec<Changed>,
    },
    /// a file that HEAD does not hold: ignored by git, or never committed
    NotHeld {
        #[serde(serialize_with = "path_text")]
        path: RepoPath,
        ignored: bool,
    },
    /// a file that is in a submodule: its commit is another repository's
    Submodule {
        #[serde(serialize_with = "path_text")]
        path: RepoPath,
        module: String,
    },
    /// git said something this version does not know what to do with
    GitFailed {
        why: String,
    },
    /// the code could not be read to check the tour against it (no driver): so nothing is known of its stops
    NotChecked {
        why: String,
    },
}

/// `s2`, `s2 and s3`, `s1, s2 and s3`.
fn stops_list(ids: &[StopId]) -> String {
    let words: Vec<String> = ids.iter().map(ToString::to_string).collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

impl Reason {
    /// One sentence, no full stop: what follows `not pinned: `.
    pub fn words(&self) -> String {
        match self {
            Reason::Unfinished => "the tour is still being written: finish it first".to_string(),
            Reason::NoStops => "no stop is written: there is no code to pin".to_string(),
            Reason::NotARepository => "this is not a git repository".to_string(),
            Reason::GitNotFound => "git was not found: pinning needs it".to_string(),
            Reason::NoCommit => "this repository has no commit yet".to_string(),
            Reason::InProgress { what } => format!("{what} is in progress: finish it first"),
            Reason::HookChanged { path, tour } => {
                format!("a hook changed {path} while committing it: run vnoc tour pin {tour} again")
            }
            Reason::Stale { changed, gone } => {
                let mut parts = Vec::new();
                match changed.as_slice() {
                    [] => {}
                    [one] => parts.push(format!("{one} changed since it was written")),
                    many => parts.push(format!(
                        "{} changed since they were written",
                        stops_list(many)
                    )),
                }
                match gone.as_slice() {
                    [] => {}
                    [one] => parts.push(format!("{one} is gone")),
                    many => parts.push(format!("{} are gone", stops_list(many))),
                }
                format!("{}: replace what changed, then pin", parts.join(" · "))
            }
            Reason::Uncommitted { files } => match files.len() {
                1 => "1 file has changes that are not committed".to_string(),
                n => format!("{n} files have changes that are not committed"),
            },
            Reason::NotHeld {
                path,
                ignored: true,
            } => {
                format!("{path} is ignored by git: this tour cannot be pinned")
            }
            Reason::NotHeld {
                path,
                ignored: false,
            } => {
                format!("{path} is not in the last commit")
            }
            Reason::Submodule { path, module } => format!(
                "{path} is in the submodule {module}: its commit is another repository's, so this tour cannot be pinned"
            ),
            Reason::GitFailed { why } => why.clone(),
            Reason::NotChecked { why } => {
                format!("the tour could not be checked against the code: {why}")
            }
        }
    }

    /// The lines of `not pinned: …`: the sentence, and under it (indented) the files it is about. `of_this_tour` is `vnoc tour pin`'s wording
    /// of the files (02 §8: "2 files of this tour have …"), which names a tour the command was asked about.
    pub fn lines(&self, of_this_tour: bool) -> Vec<String> {
        let words = match self {
            Reason::Uncommitted { files } if of_this_tour => match files.len() {
                1 => "1 file of this tour has changes that are not committed".to_string(),
                n => format!("{n} files of this tour have changes that are not committed"),
            },
            _ => self.words(),
        };
        let mut lines = vec![format!("not pinned: {words}")];
        if let Reason::Uncommitted { files } = self {
            lines.extend(files.iter().map(|f| format!("  {}", f.line())));
        }
        lines
    }
}

/// The answer of [`can_pin`]: HEAD, when a pin can be made now, or every reason it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pinnable {
    Yes { head: Commit },
    No(Vec<Reason>),
}

impl Pinnable {
    /// The files that are not committed, when that is the **only** thing in the way: what `--commit` commits.
    pub fn only_uncommitted(&self) -> Option<&[Changed]> {
        match self {
            Pinnable::No(reasons) => match reasons.as_slice() {
                [Reason::Uncommitted { files }] => Some(files),
                _ => None,
            },
            Pinnable::Yes { .. } => None,
        }
    }
}

// --------------------------------------------------------------------------------------------------------------------------------- can_pin

/// What needs no look at the code: the tour is finished and has something to pin, git runs, this is a repository with a commit, and no merge or
/// rebase is in the middle. HEAD when all of it holds; else why not. Asked first, so a project with no git is told so without a check.
pub fn head(project: &Project, tour: &Tour) -> Result<Commit, Vec<Reason>> {
    if tour.state != State::Finished {
        return Err(vec![Reason::Unfinished]);
    }
    if !tour.stops.iter().any(|s| s.written) {
        return Err(vec![Reason::NoStops]);
    }
    let root = project.git_root();
    let said = git(root, false, &["rev-parse", "--verify", "-q", "HEAD"])
        .map_err(|u| vec![unrun_reason(u)])?;
    let hex = String::from_utf8_lossy(&said.out).trim().to_string();
    let head = match said.code {
        Some(0) => Commit::parse(&hex).map_err(|_| {
            vec![Reason::GitFailed {
                why: format!("HEAD is `{hex}`, which is not a 40-character commit id"),
            }]
        })?,
        // `-q` makes "there is no such commit" exit 1 and say nothing (TR0)
        Some(1) => return Err(vec![Reason::NoCommit]),
        _ if said.err.contains("not a git repository") => {
            return Err(vec![Reason::NotARepository]);
        }
        _ => {
            return Err(vec![Reason::GitFailed {
                why: format!("git rev-parse failed ({})", said.first_line()),
            }]);
        }
    };
    if let Some(what) = in_progress(root) {
        return Err(vec![Reason::InProgress { what }]);
    }
    Ok(head)
}

/// `a merge`, `a rebase`, `a cherry-pick` or `a revert`, when one is in the middle: `MERGE_HEAD`, `rebase-merge` or `rebase-apply`, `CHERRY_PICK_HEAD`,
/// `REVERT_HEAD` exist in the git folder (`--git-path` finds it for a worktree too).
fn in_progress(root: &Path) -> Option<&'static str> {
    let exists = |name: &str| {
        let said = git(root, false, &["rev-parse", "--git-path", name]).ok()?;
        if !said.ok() {
            return None;
        }
        let path = String::from_utf8_lossy(&said.out).trim().to_string();
        Some(root.join(path).exists())
    };
    if exists("MERGE_HEAD") == Some(true) {
        return Some("a merge");
    }
    if exists("rebase-merge") == Some(true) || exists("rebase-apply") == Some(true) {
        return Some("a rebase");
    }
    if exists("CHERRY_PICK_HEAD") == Some(true) {
        return Some("a cherry-pick");
    }
    if exists("REVERT_HEAD") == Some(true) {
        return Some("a revert");
    }
    None
}

/// `git status --porcelain -z --untracked-files=all -- <paths>`: what has something in it that is not committed, by path. A rename's old name (the entry
/// after it) is skipped: it is not a file of the tour.
fn status_of(root: &Path, paths: &[RepoPath]) -> Result<BTreeMap<String, String>, Reason> {
    let mut args = vec!["status", "--porcelain", "-z", "--untracked-files=all", "--"];
    args.extend(names(paths));
    let said = git(root, true, &args).map_err(unrun_reason)?;
    if !said.ok() {
        return Err(Reason::GitFailed {
            why: format!("git status failed ({})", said.first_line()),
        });
    }
    let text = String::from_utf8_lossy(&said.out).into_owned();
    let mut found = BTreeMap::new();
    let mut entries = text.split('\0').filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        let (Some(status), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        if status.contains(['R', 'C']) {
            entries.next();
        }
        found.insert(path.to_string(), status.to_string());
    }
    Ok(found)
}

/// `git ls-tree --name-only -z HEAD -- <paths>`: the paths HEAD holds as files.
fn held_by_head(root: &Path, paths: &[RepoPath]) -> Result<BTreeSet<String>, Reason> {
    let mut args = vec!["ls-tree", "--name-only", "-z", "HEAD", "--"];
    args.extend(names(paths));
    let said = git(root, true, &args).map_err(unrun_reason)?;
    if !said.ok() {
        return Err(Reason::GitFailed {
            why: format!("git ls-tree failed ({})", said.first_line()),
        });
    }
    Ok(String::from_utf8_lossy(&said.out)
        .split('\0')
        .filter(|n| !n.is_empty())
        .map(String::from)
        .collect())
}

/// The submodule a path is in, if one of the folders above it is a commit entry of HEAD (mode `160000`).
fn submodule_of(root: &Path, path: &RepoPath) -> Option<String> {
    let mut above = Vec::new();
    let mut at = 0;
    while let Some(slash) = path.as_str()[at..].find('/') {
        at += slash;
        above.push(&path.as_str()[..at]);
        at += 1;
    }
    if above.is_empty() {
        return None;
    }
    let mut args = vec!["ls-tree", "-z", "HEAD", "--"];
    args.extend(above);
    let said = git(root, true, &args).ok()?;
    if !said.ok() {
        return None;
    }
    String::from_utf8_lossy(&said.out)
        .split('\0')
        .filter_map(|entry| entry.split_once('\t'))
        .find(|(info, _)| info.starts_with("160000 "))
        .map(|(_, name)| name.to_string())
}

/// Whether git ignores a path: `git check-ignore -q`, exit 0. Only to word a reason. (`check-ignore` refuses `--literal-pathspecs`: "pathspec magic not
/// supported by this command"; a file called `a*.py` is only worded wrongly, never pinned wrongly.)
fn ignored(root: &Path, path: &RepoPath) -> bool {
    git(root, false, &["check-ignore", "-q", "--", path.as_str()]).is_ok_and(|said| said.ok())
}

/// The questions 1 and 2 of 01 §8, for a tour whose [`head`] held. `recorded` and `placed` are of one check made just now (`check::check_placed`).
pub fn can_pin(
    project: &Project,
    tour: &Tour,
    head: Commit,
    recorded: &Recorded,
    placed: &Placed,
) -> Pinnable {
    // 1. every written stop fresh
    let of = |wanted: StopState| -> Vec<StopId> {
        tour.stops
            .iter()
            .filter(|s| s.written && recorded.stop(s.id) == wanted)
            .map(|s| s.id)
            .collect()
    };
    let (changed, gone) = (of(StopState::Changed), of(StopState::Gone));
    if !changed.is_empty() || !gone.is_empty() {
        return Pinnable::No(vec![Reason::Stale { changed, gone }]);
    }
    // 2. every file committed, and in HEAD
    let files = placed.files();
    if files.is_empty() {
        return Pinnable::Yes { head };
    }
    let root = project.git_root();
    let status = match status_of(root, &files) {
        Ok(status) => status,
        Err(reason) => return Pinnable::No(vec![reason]),
    };
    let held = match held_by_head(root, &files) {
        Ok(held) => held,
        Err(reason) => return Pinnable::No(vec![reason]),
    };
    let mut reasons = Vec::new();
    let uncommitted: Vec<Changed> = files
        .iter()
        .filter_map(|path| {
            status.get(path.as_str()).map(|status| Changed {
                status: status.clone(),
                path: path.clone(),
            })
        })
        .collect();
    // a file git does not hold, that status did not name (a name it did is already said as uncommitted): in a submodule, ignored, or never committed
    for path in &files {
        if held.contains(path.as_str()) || status.contains_key(path.as_str()) {
            continue;
        }
        reasons.push(match submodule_of(root, path) {
            Some(module) => Reason::Submodule {
                path: path.clone(),
                module,
            },
            None => Reason::NotHeld {
                path: path.clone(),
                ignored: ignored(root, path),
            },
        });
    }
    if !uncommitted.is_empty() {
        reasons.insert(0, Reason::Uncommitted { files: uncommitted });
    }
    if reasons.is_empty() {
        Pinnable::Yes { head }
    } else {
        Pinnable::No(reasons)
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------- make

/// The `pin` record: HEAD, now, and where every stop and block is now (01 §4). Only for a [`Pinnable::Yes`]'s `head` and the `placed` of the check
/// that answered it.
pub fn make(head: Commit, placed: &Placed) -> PinRecord {
    PinRecord {
        commit: head,
        at: time::now(),
        places: placed.places(),
    }
}

// ------------------------------------------------------------------------------------------------------------------------------ node files

/// The `node.toml` of each stop's own ID that git shows as not committed (untracked or modified): what `ids` just wrote, and what `--commit` takes
/// with the code so the ID lines and the node files they name go in one commit (01 §8). **Only `node.toml`**, never another file of that folder (a doc
/// the user wrote there is theirs). The folder is found by [`layout::node_file`], the one place that knows the layout. They do not decide the pin: that
/// is about code. An empty list when git cannot say.
pub fn node_files(project: &Project, tour: &Tour) -> Vec<Changed> {
    let root = project.git_root();
    let vnoc_root = store::vnoc_root(root);
    let mut paths: Vec<RepoPath> = Vec::new();
    for stop in tour.stops.iter().filter(|s| s.written) {
        let Some(node) = stop.node else { continue };
        let file = layout::node_file(&vnoc_root, &node);
        let Ok(relative) = file.strip_prefix(root) else {
            continue;
        };
        let Ok(path) = RepoPath::new(relative.to_string_lossy().replace('\\', "/")) else {
            continue;
        };
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Vec::new();
    }
    let Ok(status) = status_of(root, &paths) else {
        return Vec::new();
    };
    paths
        .into_iter()
        .filter_map(|path| {
            status.get(path.as_str()).map(|status| Changed {
                status: status.clone(),
                path,
            })
        })
        .collect()
}

/// After a commit: the files among `files` whose text now is not the text the check read (`read`, [`Placed::read`]). A hook that formats or adds to a
/// file while the commit is made leaves a commit with code the check never saw: it must not be pinned (01 §8). A file the check did not read (a folder
/// stop has none) is not compared. A file that cannot be read now counts as changed.
pub fn changed_since_read(
    project: &Project,
    read: &BTreeMap<RepoPath, String>,
    files: &[RepoPath],
) -> Vec<RepoPath> {
    let root = project.git_root();
    files
        .iter()
        .filter(|path| {
            read.get(*path).is_some_and(|was| {
                source::read_text(&root.join(path.as_str()))
                    .map_or(true, |now| &super::check::digest(&now) != was)
            })
        })
        .cloned()
        .collect()
}

/// Puts the index of these files back to HEAD's: `git reset -q -- <files>`. For files a hook changed while `commit --only` made the commit: the hook's
/// `git add` goes into the commit, but git leaves the real index with the file as it was **before** the hook, so `git status` would say `MM` for a file
/// the commit holds whole, and the next `pin` would refuse it as not committed (TR9b; seen with plain git, not a thing of ours). Only files the commit
/// just took, so no other staged change is touched. A failure is not told: the next `pin` says what it sees.
pub fn settle_index(project: &Project, files: &[RepoPath]) {
    if files.is_empty() {
        return;
    }
    let mut args = vec!["reset", "-q", "--"];
    args.extend(names(files));
    let _ = git(project.git_root(), true, &args);
}

// ----------------------------------------------------------------------------------------------------------------------------- commit_files

/// What a commit of the tour's files did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    pub commit: Commit,
    pub files: Vec<RepoPath>,
}

/// Why the files were not committed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    /// git could not be run, or answered something unexpected
    #[error("{0}")]
    Git(String),
    /// git refused the commit (a hook that failed, an identity that is not set): what it printed
    #[error("git did not make the commit: {0}")]
    Refused(String),
}

/// Commits **exactly** these files, the whole of each, and nothing else: `git add -- <the untracked among them>` (`commit --only` does not know a file
/// git has never seen: TR0), then `git commit --only -m <message> -- <files>`. Other staged files stay staged and out of the commit. A hook that fails
/// fails the commit, with its output, and the files `add` took in are taken out again: nothing happened. The caller has checked that nothing else is in
/// the way ([`Pinnable::only_uncommitted`]).
pub fn commit_files(
    project: &Project,
    files: &[Changed],
    message: &str,
) -> Result<Committed, CommitError> {
    let root = project.git_root();
    let run = |literal: bool, args: &[&str]| {
        git(root, literal, args).map_err(|u| match u {
            Unrun::NotFound => CommitError::Git("git was not found".to_string()),
            Unrun::Other(why) => CommitError::Git(format!("git could not be run ({why})")),
        })
    };
    let untracked: Vec<&str> = files
        .iter()
        .filter(|f| f.untracked())
        .map(|f| f.path.as_str())
        .collect();
    if !untracked.is_empty() {
        let mut args = vec!["add", "--"];
        args.extend(&untracked);
        let said = run(true, &args)?;
        if !said.ok() {
            return Err(CommitError::Git(format!(
                "git add failed ({})",
                said.first_line()
            )));
        }
    }
    // `:(literal)` is the same promise as `--literal-pathspecs`, without putting it in the environment of the hooks
    let literal: Vec<String> = files
        .iter()
        .map(|f| format!(":(literal){}", f.path))
        .collect();
    let mut args = vec!["commit", "--only", "-m", message, "--"];
    args.extend(literal.iter().map(String::as_str));
    let said = run(false, &args)?;
    if !said.ok() {
        if !untracked.is_empty() {
            // taken in by `add` a moment ago, and not committed: put back as they were
            let mut undo = vec!["reset", "-q", "--"];
            undo.extend(&untracked);
            let _ = git(root, true, &undo);
        }
        let mut printed = String::from_utf8_lossy(&said.out).trim().to_string();
        if !said.err.trim().is_empty() {
            if !printed.is_empty() {
                printed.push('\n');
            }
            printed.push_str(said.err.trim());
        }
        return Err(CommitError::Refused(printed));
    }
    let after = run(false, &["rev-parse", "--verify", "-q", "HEAD"])?;
    let hex = String::from_utf8_lossy(&after.out).trim().to_string();
    let commit = Commit::parse(&hex).map_err(|e| CommitError::Git(e.to_string()))?;
    Ok(Committed {
        commit,
        files: files.iter().map(|f| f.path.clone()).collect(),
    })
}

// ------------------------------------------------------------------------------------------------------------------------------ as written

/// The code of a pinned commit, by (folder, commit, path): a commit's file never changes. Only what was read is kept.
fn kept() -> &'static Mutex<Kept> {
    static KEPT: OnceLock<Mutex<Kept>> = OnceLock::new();
    KEPT.get_or_init(Mutex::default)
}

/// The files read so far: (git root, commit, path) → the file's lines.
type Kept = HashMap<(String, String, String), Arc<Vec<String>>>;

/// The lines `first..=last` (1-based) of `path` as the pinned `commit` has it, split as `source.rs` splits them (`\r\n` ends a line, a byte order mark
/// is dropped). `None` when git has no such commit, or the commit has no such file, or the commit or the path is not one a pin may name: the stop
/// then falls back to how a tour that is not pinned shows it. Read with `git show`, never checked out.
pub fn as_written(
    project: &Project,
    commit: &str,
    path: &str,
    first: u32,
    last: u32,
) -> Option<Vec<String>> {
    // before git sees either (01 §10)
    let commit = Commit::parse(commit).ok()?;
    let path = RepoPath::new(path).ok()?;
    let key = (
        project.git_root().to_string_lossy().into_owned(),
        commit.as_str().to_string(),
        path.as_str().to_string(),
    );
    let known = kept().lock().ok()?.get(&key).cloned();
    let all = match known {
        Some(all) => all,
        None => {
            // no `--`: after it the argument is a path and git prints nothing and succeeds (TR0). `--no-textconv`: the bytes the commit has
            let object = format!("{}:{}", commit.as_str(), path.as_str());
            let said = git(
                project.git_root(),
                false,
                &["show", "--no-textconv", &object],
            )
            .ok()?;
            if !said.ok() {
                return None;
            }
            let text = String::from_utf8_lossy(&said.out);
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
            let all: Arc<Vec<String>> = Arc::new(
                source::split_lines(text)
                    .into_iter()
                    .map(String::from)
                    .collect(),
            );
            kept().lock().ok()?.insert(key, Arc::clone(&all));
            all
        }
    };
    let from = (first.max(1) - 1) as usize;
    if from >= all.len() || last < first {
        return None;
    }
    let to = (last as usize).min(all.len());
    Some(all[from..to].to_vec())
}

// ------------------------------------------------------------------------------------------------------------------------------ reachable

/// Whether the commit is in this repository at all: `git cat-file -e <commit>^{commit}`.
pub fn has_commit(project: &Project, commit: &Commit) -> bool {
    let object = format!("{}^{{commit}}", commit.as_str());
    git(project.git_root(), false, &["cat-file", "-e", &object]).is_ok_and(|said| said.ok())
}

/// Whether HEAD is the commit or has it behind it: `git merge-base --is-ancestor <commit> HEAD`. False for a commit this repository does not have
/// (a shallow clone, a rewritten history), and for no git. A pin that is not reachable can be made again (01 §8).
pub fn reachable(project: &Project, commit: &Commit) -> bool {
    git(
        project.git_root(),
        false,
        &["merge-base", "--is-ancestor", commit.as_str(), "HEAD"],
    )
    .is_ok_and(|said| said.ok())
}

impl fmt::Display for Changed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.line())
    }
}

#[cfg(test)]
mod tests;
