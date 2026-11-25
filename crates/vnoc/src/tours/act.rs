//! What the user's actions do, for the commands and for the requests (`docs/26-tours/03-core.md` §8, `02-commands.md` §5, §8): pin (with or without a commit
//! of the tour's files), write the IDs of a tour's stops, edit a tour's title and question, the two settings. `vnoc tour pin | ids | edit | settings` and
//! `vnoc/tourPin | tourIds | tourEdit | tourSettings` call these and nothing else: the rules and the words are here, and the caller owns what it needs of
//! its own (the index and the drivers, which locks it holds, the printing).
//!
//! **Locks.** A function that changes source or `node.toml` ([`write_ids`], [`pin_finish`] with a commit) is called with the project's write lock held by
//! the caller, and takes the tours lock itself, after: never the other way round.

use std::path::Path;
use std::sync::RwLock;

use serde::Serialize;

use super::check::{self, Code};
use super::fold::Tour;
use super::ids::{self, Outcome, Pick};
use super::pin::{self, Changed, Pinnable, Reason};
use super::record::{Commit, Detail, IdsRecord, MetaRecord, Record, StopId, TourId};
use super::serve::short_commit;
use super::settings::{self, Choice, Key, Origin};
use super::store::{self, Located, Measured, StoreError};
use super::{add, finish as finishing};
use crate::identity::{AttachError, Env, MarkerWritten, marker_words};
use crate::index::Index;
use crate::project::Project;
use crate::source;
use crate::userdata::ToursLock;

/// Why an action was not done. The commands say it with an exit code, the requests with an error code.
#[derive(Debug)]
pub enum ActError {
    Store(StoreError),
    /// the request cannot be met as it was made: the sentence
    Usage(String),
    /// a stop the tour does not have (or has planned only): the sentence, and the stops near the one asked for
    NoStop {
        said: String,
        near: Vec<String>,
    },
    /// a rule of the marker edit
    Attach(AttachError),
    /// the disk, git, a driver, a file that cannot be read: not the request's fault
    Environment(String),
}

impl From<StoreError> for ActError {
    fn from(e: StoreError) -> ActError {
        ActError::Store(e)
    }
}

impl std::fmt::Display for ActError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActError::Store(e) => e.fmt(f),
            ActError::Usage(s) | ActError::Environment(s) => f.write_str(s),
            ActError::NoStop { said, .. } => f.write_str(said),
            ActError::Attach(e) => e.fmt(f),
        }
    }
}

/// The tours lock, waited for off the runtime's thread (it sleeps between tries).
pub async fn take_lock(vnoc_root: &Path) -> Result<ToursLock, ActError> {
    let root = vnoc_root.to_path_buf();
    tokio::task::spawn_blocking(move || ToursLock::take(&root))
        .await
        .map_err(|e| ActError::Environment(e.to_string()))?
        .map_err(|e| ActError::Store(e.into()))
}

/// The tour as it is now, under the lock the caller holds. A tour that cannot be read cannot be written to.
pub fn read_tour(vnoc_root: &Path, id: &TourId) -> Result<Tour, ActError> {
    let (_, folded) = store::read(vnoc_root, id)?;
    folded.map_err(|e| {
        let said = format!("the tour {id} cannot be read: {e}");
        // a tour of a newer schema is for a newer V-NOC (the environment); a broken one is the request's
        if e.too_new {
            ActError::Environment(said)
        } else {
            ActError::Usage(said)
        }
    })
}

/// A tour that is in both folders is nobody's to write to.
pub fn not_in_both(vnoc_root: &Path, id: &TourId) -> Result<(), ActError> {
    match store::locate(vnoc_root, id) {
        Located::Both { .. } => Err(StoreError::InBoth(id.clone()).into()),
        _ => Ok(()),
    }
}

/// The project writes no ID lines: `[ids] default = "none"` in `.vnoc/vnoc.toml`.
pub fn project_writes_ids(project: &Project) -> Result<bool, ActError> {
    Ok(crate::userdata::UserData::new(project.git_root())
        .settings()
        .map_err(|e| ActError::Environment(e.to_string()))?
        .markers)
}

pub fn count_files(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
}

// ---------------------------------------------------------------------------------------------------------------------------------- pin

/// How a pin is asked for.
pub enum Ask<'a> {
    /// `finish` and `import` (02 §5): files that are not committed are dealt with by `[tours] commit`, or by the flags of this call
    Finish {
        commit: Choice,
        message: Option<&'a str>,
    },
    /// `vnoc tour pin` (02 §8): files are committed only with `--commit`
    Pin {
        commit: bool,
        message: Option<&'a str>,
    },
    /// `vnoc/tourPin { dry: true }`: the answer of a pin that commits nothing, and writes nothing (no pin record, no check record, no commit): `pinned` says it would be
    /// pinned now, and the reasons say why not. The cover asks it to say why a tour is not pinned
    Look,
}

/// What a commit made to pin did.
#[derive(Serialize)]
pub struct CommittedOut {
    pub commit: String,
    pub message: String,
    /// the tour's code files
    pub files: Vec<String>,
    /// the `node.toml` of the tour's IDs that were committed with them (TR9b), under the code files in the text
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub node_files: Vec<String>,
}

impl CommittedOut {
    /// `committed 2 files as 3c9e1f7   <message>` and the files under it, code first, then the node files of the IDs.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "committed {} as {}   {}",
            count_files(self.files.len() + self.node_files.len()),
            &self.commit[..7],
            self.message
        )];
        lines.extend(
            self.files
                .iter()
                .chain(&self.node_files)
                .map(|f| format!("  {f}")),
        );
        lines
    }
}

/// What became of the pin of one tour: the answer of `vnoc tour pin --json` and of `vnoc/tourPin`.
#[derive(Serialize)]
pub struct PinOut {
    /// `pinned`, `ask` (the user is to be asked: `[tours] commit` is `ask`) or `not_pinned`
    pub state: &'static str,
    /// the pinned commit, all 40 characters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed: Option<CommittedOut>,
    /// why it is not pinned: the stale stops, the files that are not committed, git's own words
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<Reason>,
    /// for `ask`: the command that would do it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// for `ask`: the `node.toml` files of the tour's IDs that `--commit` would commit with the code files (which are in `reasons`)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub node_files: Vec<String>,
    /// `--commit` was asked and was refused: nothing was committed
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub refused: bool,
}

impl PinOut {
    /// A pin that holds: the commit, and `committed` when files were committed to make it.
    pub fn pinned(commit: &Commit, committed: Option<CommittedOut>) -> PinOut {
        PinOut {
            state: "pinned",
            commit: Some(commit.as_str().to_string()),
            committed,
            reasons: Vec::new(),
            command: None,
            node_files: Vec::new(),
            refused: false,
        }
    }
}

/// What [`pin_finish`] did: the answer, the lines to print under it (no end of line), and the 7 characters of the commit when the tour is pinned now.
pub struct Pinned {
    pub out: PinOut,
    pub lines: Vec<String>,
    pub at: Option<String>,
}

/// Not pinned, and why, worded for the way it was asked.
pub fn not_pinned(
    ask: &Ask<'_>,
    id: &TourId,
    pinnable: &Pinnable,
    committed: Option<CommittedOut>,
    node_files: &[Changed],
) -> Pinned {
    let Pinnable::No(reasons) = pinnable else {
        unreachable!("not_pinned is for a pin that cannot be made")
    };
    let of_this_tour = matches!(ask, Ask::Pin { .. } | Ask::Look);
    // a commit that was made stays made: it is said first, and the reason under it
    let mut lines: Vec<String> = committed
        .as_ref()
        .map(CommittedOut::lines)
        .unwrap_or_default();
    for reason in reasons {
        lines.extend(reason.lines(of_this_tour));
    }
    let mut out = PinOut {
        state: "not_pinned",
        commit: None,
        committed,
        reasons: reasons.clone(),
        command: None,
        node_files: Vec::new(),
        refused: false,
    };
    match ask {
        Ask::Finish { commit, .. } => {
            if let (Choice::Ask, Some(files)) = (commit, pinnable.only_uncommitted()) {
                let why = reasons[0].words();
                lines = finishing::ask_commit_lines(id, &why, files, node_files);
                out.state = "ask";
                out.command = Some(format!("vnoc tour pin {id} --commit"));
                out.node_files = node_files.iter().map(|f| f.path.to_string()).collect();
            }
        }
        Ask::Look => {
            // what a commit of the files would take along: the dialog lists them
            out.node_files = node_files.iter().map(|f| f.path.to_string()).collect();
        }
        Ask::Pin { commit, .. } => {
            if *commit {
                // `--commit` is for files that are the one thing in the way; nothing was committed unless a commit was made
                out.refused = pinnable.only_uncommitted().is_none() && out.committed.is_none();
                if out.refused {
                    lines.push("nothing was committed: --commit is for files that are the one thing in the way".to_string());
                }
            } else if pinnable.only_uncommitted().is_some() {
                lines.push(format!(
                    "commit them, then run this again. Or: vnoc tour pin {id} --commit"
                ));
            }
        }
    }
    Pinned {
        out,
        lines,
        at: None,
    }
}

/// The commit the tour is pinned at, when it is and HEAD has that commit behind it: there is nothing to do (`vnoc tour pin` says "pinned … already").
pub fn pinned_already(project: &Project, tour: &Tour) -> Option<Commit> {
    let pin = tour.pin.as_ref().filter(|_| tour.pinned())?;
    pin::reachable(project, &pin.commit).then(|| pin.commit.clone())
}

/// How [`pin_begin`] went.
pub enum Begun {
    /// what needs no look at the code already decided it: not pinned, and why
    Done(Pinned),
    /// the code is to be looked at: give the index to [`pin_finish`] with these
    Check { measured: Measured, head: Commit },
}

/// The first half of the pin of one tour (01 §8, 02 §5 and §8): the tour is read, and what needs no look at the code is asked (finished, git runs, a repository
/// with a commit, no merge in the middle). A project with no git is told so without an index.
pub fn pin_begin(
    project: &Project,
    vnoc_root: &Path,
    id: &TourId,
    ask: &Ask<'_>,
) -> Result<Begun, ActError> {
    not_in_both(vnoc_root, id)?;
    let measured = store::read_measured(vnoc_root, id)?;
    let tour = match &measured.tour {
        Ok(tour) => tour,
        Err(e) => {
            return Err(ActError::Usage(format!(
                "the tour {id} cannot be read: {e}"
            )));
        }
    };
    match pin::head(project, tour) {
        Ok(head) => Ok(Begun::Check { measured, head }),
        Err(reasons) => Ok(Begun::Done(not_pinned(
            ask,
            id,
            &Pinnable::No(reasons),
            None,
            &[],
        ))),
    }
}

/// The code could not be read to check the tour (no driver, no index): not pinned, and why.
pub fn not_checked(ask: &Ask<'_>, id: &TourId, why: String) -> Pinned {
    not_pinned(
        ask,
        id,
        &Pinnable::No(vec![Reason::NotChecked { why }]),
        None,
        &[],
    )
}

/// The second half: the check against the code as it is now, then HEAD or the reasons; files that are not committed are committed first when `ask` says so
/// (the caller then holds the project's write lock). The tours lock is taken here, only to write the pin.
pub fn pin_finish(
    project: &Project,
    index: &RwLock<Index>,
    vnoc_root: &Path,
    id: &TourId,
    ask: &Ask<'_>,
    measured: &Measured,
    head: Commit,
) -> Result<Pinned, ActError> {
    let tour = measured
        .tour
        .as_ref()
        .map_err(|e| ActError::Usage(format!("the tour {id} cannot be read: {e}")))?;
    let (recorded, placed) = placed_check(project, index, tour, measured.len);
    // what was just found is the record of this tour: the next look costs `stat`s (a look writes nothing)
    if !matches!(ask, Ask::Look) {
        check::record(vnoc_root, id, &recorded)?;
    }

    let mut pinnable = pin::can_pin(project, tour, head, &recorded, &placed);
    let wants_commit = match ask {
        Ask::Finish { commit, .. } => *commit == Choice::Always,
        Ask::Pin { commit, .. } => *commit,
        Ask::Look => false,
    };
    let mut committed = None;
    // the node files of the tour's IDs go with the code files, and only when code is being committed: they do not decide the pin
    let node_files = if pinnable.only_uncommitted().is_some() {
        pin::node_files(project, tour)
    } else {
        Vec::new()
    };
    if wants_commit && let Some(files) = pinnable.only_uncommitted() {
        let message = match ask {
            Ask::Finish { message, .. } | Ask::Pin { message, .. } => message.map(String::from),
            Ask::Look => None,
        }
        .unwrap_or_else(|| finishing::default_message(&tour.title));
        let code_files: Vec<Changed> = files.to_vec();
        let all: Vec<Changed> = code_files.iter().chain(&node_files).cloned().collect();
        let done = pin::commit_files(project, &all, &message)
            .map_err(|e| ActError::Environment(e.to_string()))?;
        committed = Some(CommittedOut {
            commit: done.commit.as_str().to_string(),
            message,
            files: code_files.iter().map(|f| f.path.to_string()).collect(),
            node_files: node_files.iter().map(|f| f.path.to_string()).collect(),
        });
        // a hook that changed a file while the commit was made: the commit holds code the check never read. It stays; it is not what was checked
        let code_paths: Vec<_> = code_files.iter().map(|f| f.path.clone()).collect();
        let hooked = pin::changed_since_read(project, &placed.read, &code_paths);
        pinnable = match hooked.first() {
            Some(path) => {
                // git left the index with those files as they were before the hook: HEAD's, so the next `pin` finds them committed
                pin::settle_index(project, &hooked);
                Pinnable::No(vec![Reason::HookChanged {
                    path: path.clone(),
                    tour: id.to_string(),
                }])
            }
            // what is on disk is what HEAD holds now: asked again, with the new HEAD
            None => pin::can_pin(project, tour, done.commit, &recorded, &placed),
        };
    }
    match pinnable {
        Pinnable::No(_) => Ok(not_pinned(ask, id, &pinnable, committed, &node_files)),
        // a look says it would be pinned, and leaves the file as it is
        Pinnable::Yes { head } if matches!(ask, Ask::Look) => Ok(Pinned {
            out: PinOut::pinned(&head, None),
            lines: Vec::new(),
            at: Some(short_commit(&head)),
        }),
        Pinnable::Yes { head } => {
            // the tour is as it was when it was read: a write in between would be a stop the pin never saw
            let lock = ToursLock::take(vnoc_root).map_err(StoreError::from)?;
            let len = std::fs::metadata(&measured.path).map(|m| m.len()).ok();
            if len != Some(measured.len) {
                return Err(StoreError::Refused(format!(
                    "the tour {id} changed while it was being pinned: run the command again"
                ))
                .into());
            }
            let record = pin::make(head.clone(), &placed);
            store::append_locked(&lock, vnoc_root, id, &[Record::Pin(record)])?;
            drop(lock);
            let at = short_commit(&head);
            let mut lines = committed
                .as_ref()
                .map(CommittedOut::lines)
                .unwrap_or_default();
            if matches!(ask, Ask::Pin { .. }) {
                lines.push(format!("pinned {id} at {at}"));
            }
            Ok(Pinned {
                out: PinOut::pinned(&head, committed),
                lines,
                at: Some(at),
            })
        }
    }
}

/// A real check of the tour against the code as it is now, and where its stops and blocks are (what a pin records).
pub fn placed_check(
    project: &Project,
    index: &RwLock<Index>,
    tour: &Tour,
    len: u64,
) -> (check::Recorded, check::Placed) {
    let index = index.read().unwrap_or_else(|e| e.into_inner());
    let tree = index.tree(&project.name(), project.prefix());
    let code = Code {
        index: &index,
        project,
        tree: &tree,
    };
    let mut read = |path: &Path| source::read_text(path);
    check::check_placed(&code, &mut read, tour, len)
}

// ---------------------------------------------------------------------------------------------------------------------------------- ids

/// What `ids` says went with a stop.
#[derive(Serialize)]
pub struct IdStop {
    pub stop: String,
    /// `written`, `had`, `by_name` or `gone`
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// the file the ID line is in, or the `.vnoc-id` file of the folder
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// What `ids` did: the answer of `vnoc tour ids --json`, and of `vnoc/tourIds` (`changed` is the files that were changed).
#[derive(Serialize)]
pub struct IdsWritten {
    pub schema: u32,
    pub command: &'static str,
    pub tour: String,
    pub stops: Vec<IdStop>,
    /// every file written or made, once, in the order they were
    pub changed: Vec<String>,
    /// an `ids` record was written
    pub recorded: bool,
    /// one line for each stop, for the text
    #[serde(skip)]
    pub words: Vec<String>,
}

/// The stops of the tour with what there is to do for each, from the index as it is now. No lock is held: it is for counting.
pub fn picks_of(
    project: &Project,
    index: &RwLock<Index>,
    tour: &Tour,
    named: &[StopId],
) -> Vec<(StopId, Pick)> {
    let index = index.read().unwrap_or_else(|e| e.into_inner());
    let tree = index.tree(&project.name(), project.prefix());
    let code = Code {
        index: &index,
        project,
        tree: &tree,
    };
    ids::pick(&code, tour, named)
}

/// Gives the stops' nodes an ID (`vnoc tour ids`, `vnoc/tourIds`): `ensure_node` for each node that has none, each marker's edit telling the tours where their
/// lines went (`apply_edits` runs `rebase`), then the `ids` record under the tours lock. The caller holds the project's write lock and **not** the tours lock;
/// `env` says where the edits go (the disk, or the editor) and which drivers are asked.
pub async fn write_ids(
    env: &mut Env<'_>,
    vnoc_root: &Path,
    id: &TourId,
    named: &[StopId],
) -> Result<IdsWritten, ActError> {
    not_in_both(vnoc_root, id)?;
    if !project_writes_ids(env.project)? {
        return Err(ActError::Usage(
            "this project writes no ID lines ([ids] default = \"none\" in .vnoc/vnoc.toml)".into(),
        ));
    }
    let tour = read_tour(vnoc_root, id)?;
    for wanted in named {
        if !tour.stop(*wanted).is_some_and(|s| s.written) {
            let near = add::did_you_mean_stops(&tour, *wanted);
            return Err(ActError::NoStop {
                said: format!("no written stop {wanted} in this tour"),
                near: near.iter().map(ToString::to_string).collect(),
            });
        }
    }
    let picks = picks_of(env.project, env.index, &tour, named);
    let done = ids::write(env, &picks).await.map_err(ActError::Attach)?;

    // the tour as the edits left it (each marker's edit wrote a `rebase`), then the `ids` record, under the tours lock
    let lock = take_lock(vnoc_root).await?;
    let tour = read_tour(vnoc_root, id)?;
    let record: Option<IdsRecord> = ids::record(&tour, &done);
    let recorded = record.is_some();
    if let Some(record) = record {
        store::append_locked(&lock, vnoc_root, id, &[Record::Ids(record)])?;
    }
    drop(lock);

    let mut stops = Vec::new();
    let mut words = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    for ((stop, outcome), (_, pick)) in done.iter().zip(&picks) {
        let (state, node, file, said) = match outcome {
            Outcome::Had(node) => ("had", Some(node.to_string()), None, String::new()),
            Outcome::Written {
                id: node,
                path,
                kind,
                marker,
            } => {
                let said = match marker {
                    MarkerWritten::Line(line) => marker_words(*kind, *line),
                    MarkerWritten::FolderFile => "a .vnoc-id file was made".to_string(),
                };
                ("written", Some(node.to_string()), Some(path.clone()), said)
            }
            Outcome::ByName { id: node, notice } => (
                "by_name",
                Some(node.to_string()),
                None,
                notice.clone().unwrap_or_else(|| {
                    "the ID is known but is not written in the code".to_string()
                }),
            ),
            Outcome::Gone => (
                "gone",
                None,
                None,
                "its node is not found in the code now: skipped".to_string(),
            ),
        };
        let what = match pick {
            Pick::Needs { path, .. } => path.clone(),
            _ => String::new(),
        };
        if let Some(file) = &file
            && !changed.contains(file)
        {
            changed.push(file.clone());
        }
        words.push(match (state, what.is_empty()) {
            ("written", false) => format!("{what}   {said}"),
            ("had", _) => String::new(),
            _ => said.clone(),
        });
        stops.push(IdStop {
            stop: stop.to_string(),
            state,
            id: node,
            changed: file,
            said: (!said.is_empty()).then_some(said),
        });
    }
    Ok(IdsWritten {
        schema: 1,
        command: "tour ids",
        tour: id.to_string(),
        stops,
        changed,
        recorded,
        words,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------- edit

/// `vnoc tour edit`, `vnoc/tourEdit`: a `meta` record with the title, the question, the detail that were given, under the tours lock. The words are the store's
/// own (`check_edit`).
pub fn edit(
    vnoc_root: &Path,
    id: &TourId,
    title: Option<&str>,
    question: Option<&str>,
    detail: Option<Detail>,
) -> Result<(), ActError> {
    let (title, question) = (title.map(str::trim), question.map(str::trim));
    let problems = store::check_edit(title, question);
    if !problems.is_empty() {
        return Err(ActError::Usage(problems.join("\n  ")));
    }
    not_in_both(vnoc_root, id)?;
    let lock = ToursLock::take(vnoc_root).map_err(StoreError::from)?;
    read_tour(vnoc_root, id)?;
    let meta = MetaRecord {
        title: title.map(String::from),
        question: question.map(String::from),
        detail,
    };
    store::append_locked(&lock, vnoc_root, id, &[Record::Meta(meta)])?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------------------- settings

#[derive(Serialize)]
pub struct SettingShown {
    pub value: Choice,
    /// `file` (`[tours]` in `.vnoc/vnoc.toml` says it) or `default`
    pub from: Origin,
}

/// The two settings of `[tours]` (01 §12): the answer of `vnoc tour settings --json` and of `vnoc/tourSettings`.
#[derive(Serialize)]
pub struct SettingsShown {
    pub schema: u32,
    pub command: &'static str,
    pub ids: SettingShown,
    pub commit: SettingShown,
    /// the keys this call changed
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// `ids = "always"` in a project that writes no ID lines: it does nothing there
    #[serde(skip_serializing_if = "Option::is_none")]
    pub does_nothing: Option<String>,
}

fn settings_error(e: settings::SettingsError) -> ActError {
    match e {
        settings::SettingsError::NoFile => ActError::Usage(e.to_string()),
        _ => ActError::Environment(e.to_string()),
    }
}

/// `vnoc tour settings`, `vnoc/tourSettings`: each key of `set` is changed first (`settings::set`: nothing else in `vnoc.toml` moves), then both are read.
/// The caller holds the project's write lock when `set` is not empty: it is the project's file.
pub fn settings(project: &Project, set: &[(Key, Choice)]) -> Result<SettingsShown, ActError> {
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut changed = Vec::new();
    for (key, choice) in set {
        settings::set(&vnoc_root, *key, *choice).map_err(settings_error)?;
        changed.push(key.as_str());
    }
    let read = settings::read(&vnoc_root).map_err(settings_error)?;
    let shown = |s: settings::Setting| SettingShown {
        value: s.value,
        from: s.from,
    };
    Ok(SettingsShown {
        schema: 1,
        command: "tour settings",
        ids: shown(read.ids),
        commit: shown(read.commit),
        changed,
        problems: read.problems,
        does_nothing: settings::does_nothing_here(read.ids.value, project_writes_ids(project)?),
    })
}
