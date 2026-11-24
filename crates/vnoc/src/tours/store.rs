//! The tours on disk: where a tour's file is, how one is started, added to, read and listed, and the state file of each
//! (`docs/26-tours/01-on-disk.md` §1, §2, §5, §9, §10).
//!
//! A tour is one `.jsonl` file, in `.vnoc/local/tours/` (private: every tour starts there) or in `.vnoc/tours/` (the project's). A write takes
//! `local/tours.lock` (a [`ToursLock`]) for as long as it takes to write, and reading never takes it: the file only grows, a line that is not
//! finished is not read, and the state file is replaced whole. The records are not checked here beyond being printed: what a record must say
//! is the job of the code that makes it.
//!
//! Every path comes from `userdata/layout.rs`; this module is the only one that knows what is in the files.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fold::{Problem, Tour, Unreadable, fold};
use super::record::{Detail, Record, StepId, StopId, TourId, TourRecord, print_line};
use super::time;
use crate::userdata::layout;
use crate::userdata::{ToursLock, ToursLockError, UserData, UserDataError};

/// A tour's file: at most 1 MB. A write refuses past it; a reader stops reading there (01 §10).
pub const MAX_BYTES: u64 = 1024 * 1024;

/// How many ids `start` tries before it gives up (an id is taken only by a one-in-a-million chance).
const START_ATTEMPTS: u32 = 10;

/// The characters after the slug of a tour's id: `0-9` and `a-z` without `i`, `l`, `o`, `u` (§5). Thirty-two of them, so five bits each.
const SUFFIX_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// The longest slug of an id.
const MAX_SLUG: usize = 40;

/// Which folder a tour is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Where {
    /// `.vnoc/local/tours/`: this machine's, not committed
    Private,
    /// `.vnoc/tours/`: shared, committed
    Project,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Lock(#[from] ToursLockError),
    #[error("{what}: {source}")]
    Io { what: String, source: io::Error },
    #[error(transparent)]
    Init(#[from] UserDataError),
    #[error("there is no tour {0}")]
    NotFound(TourId),
    #[error("the tour {0} is in .vnoc/tours/ and in .vnoc/local/tours/: keep one")]
    InBoth(TourId),
    #[error("the tour {0} would be over 1 MB: finish it and start a second tour")]
    TooBig(TourId),
    #[error("could not find a free id for a new tour in {START_ATTEMPTS} tries")]
    NoFreeId,
}

fn io_error(what: impl Into<String>) -> impl FnOnce(io::Error) -> StoreError {
    let what = what.into();
    move |source| StoreError::Io { what, source }
}

// ---------------------------------------------------------------------------------------------------------------- where

/// Where a tour's file is, by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    None,
    Private(PathBuf),
    Project(PathBuf),
    /// The same id in both folders (someone pulled a tour they also have privately). Nothing is merged (01 §1).
    Both {
        private: PathBuf,
        project: PathBuf,
    },
}

fn file_in(dir: &Path, id: &TourId) -> PathBuf {
    dir.join(format!("{id}.jsonl"))
}

/// The files `<id>.jsonl` in the two folders.
pub fn locate(vnoc_root: &Path, id: &TourId) -> Located {
    let private = file_in(&layout::local_tours_dir(vnoc_root), id);
    let project = file_in(&layout::tours_dir(vnoc_root), id);
    match (private.is_file(), project.is_file()) {
        (false, false) => Located::None,
        (true, false) => Located::Private(private),
        (false, true) => Located::Project(project),
        (true, true) => Located::Both { private, project },
    }
}

// ------------------------------------------------------------------------------------------------------------------ ids

static MADE: AtomicU32 = AtomicU32::new(0);

/// The slug of a title (01 §5): ASCII letters and digits, lower-case; a space, an underscore or any other ASCII punctuation is a `-` between
/// words (`checkout_order` is `checkout-order`); a character that is not ASCII is dropped; one `-` at most between words and none at the ends;
/// 40 characters at most; `tour` when nothing is left.
fn slug(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if (c.is_ascii() || c.is_whitespace()) && !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(MAX_SLUG);
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "tour".to_string()
    } else {
        slug.to_string()
    }
}

/// The id for `title` on this `attempt`: the slug, `-`, and four characters taken from a BLAKE3 hash of the time in nanoseconds, the process id,
/// the title, the attempt and a number that counts up in this process (two calls in one clock tick differ by it).
fn id_for(title: &str, attempt: u32) -> TourId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut hasher = blake3::Hasher::new();
    hasher.update(&nanos.to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    hasher.update(&MADE.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    hasher.update(&attempt.to_le_bytes());
    hasher.update(title.as_bytes());
    let hash = hasher.finalize();
    let suffix: String = hash.as_bytes()[..4]
        .iter()
        .map(|b| SUFFIX_ALPHABET[usize::from(b & 31)] as char)
        .collect();
    TourId::parse(&format!("{}-{suffix}", slug(title)))
        .expect("a slug and four characters of the alphabet are an id")
}

/// A new tour id for a title: `checkout-order-7k2q`. Two calls with the same title give two ids.
pub fn new_id(title: &str) -> TourId {
    id_for(title, 0)
}

/// One more than the highest stop number ever used in the file, dropped ones too (01 §5). Called under the lock, after reading the file. `None`
/// when the numbers are used up (a file edited by hand to hold the largest one): an id is never reused, so the caller refuses.
pub fn next_stop_id(tour: &Tour) -> Option<StopId> {
    StopId::new(tour.highest_stop().checked_add(1)?)
}

/// One more than the highest block number ever used under the stop, at any depth (01 §5). `None` when the numbers are used up.
pub fn next_block_id(tour: &Tour, stop: StopId) -> Option<StepId> {
    StepId::block(stop, tour.highest_block(stop).checked_add(1)?)
}

/// The `.vnoc/` folder of a project, given the project's root (the git root): where every function of this module that takes `vnoc_root` looks.
pub fn vnoc_root(project_root: &Path) -> PathBuf {
    project_root.join(layout::FOLDER)
}

// ------------------------------------------------------------------------------------------------------------------ pick

/// What a typed tour id names (01 §5: an id, or the start of one when only one tour starts so).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    One(TourId),
    /// the text starts several ids
    Several(Vec<TourId>),
    /// no tour has it: the nearest ids, at most three, for a "did you mean"
    Nothing {
        near: Vec<TourId>,
    },
}

/// Every id of a tour in either folder, once, sorted. Only the file names are read.
fn every_id(vnoc_root: &Path) -> Vec<TourId> {
    let mut ids: Vec<TourId> = [
        layout::tours_dir(vnoc_root),
        layout::local_tours_dir(vnoc_root),
    ]
    .iter()
    .filter_map(|dir| fs::read_dir(dir).ok())
    .flat_map(|entries| entries.flatten())
    .filter(|entry| !entry.file_type().is_ok_and(|t| t.is_dir()))
    .filter_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        name.strip_suffix(".jsonl")
            .and_then(|stem| TourId::parse(stem).ok())
    })
    .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The number of single-character edits between two texts.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != *cb))
                .min(above + 1)
                .min(row[j] + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The tour a typed id names: an id as it is, else the one id that starts with the text. Several that start with it are `Several`; none is
/// `Nothing`, with the ids nearest to the text (by edit distance to the id or to its slug, and only those not far off).
pub fn pick(vnoc_root: &Path, text: &str) -> Pick {
    let ids = every_id(vnoc_root);
    let text = text.trim();
    if let Some(exact) = ids.iter().find(|id| id.as_str() == text) {
        return Pick::One(exact.clone());
    }
    if !text.is_empty() {
        let mut starts: Vec<TourId> = ids
            .iter()
            .filter(|id| id.as_str().starts_with(text))
            .cloned()
            .collect();
        match starts.len() {
            0 => {}
            1 => return Pick::One(starts.remove(0)),
            _ => return Pick::Several(starts),
        }
    }
    let far = (text.chars().count() / 3).max(3);
    let mut near: Vec<(usize, TourId)> = ids
        .into_iter()
        .map(|id| {
            let distance = edit_distance(text, id.as_str()).min(edit_distance(text, id.slug()));
            (distance, id)
        })
        .filter(|(distance, _)| *distance <= far)
        .collect();
    near.sort();
    near.truncate(3);
    Pick::Nothing {
        near: near.into_iter().map(|(_, id)| id).collect(),
    }
}

// ---------------------------------------------------------------------------------------------------------------- start

/// What is wrong with the arguments of `start`, one sentence each: a title is one line of 1 to 60 characters, a question one line of at most
/// 200, an author one line (02 §3).
pub fn check_start(params: &StartParams) -> Vec<String> {
    fn one_line(text: &str) -> bool {
        !text.contains(['\n', '\r'])
    }
    let mut problems = Vec::new();
    let title = params.title.trim();
    if title.is_empty() {
        problems.push("--title is empty: say what the tour is of, in a few words".to_string());
    } else if !one_line(title) {
        problems.push("--title is not one line".to_string());
    } else if title.chars().count() > 60 {
        problems.push(format!(
            "--title is {} characters: the most it takes is 60",
            title.chars().count()
        ));
    }
    if let Some(question) = &params.question {
        let question = question.trim();
        if !one_line(question) {
            problems.push("--question is not one line".to_string());
        } else if question.chars().count() > 200 {
            problems.push(format!(
                "--question is {} characters: the most it takes is 200",
                question.chars().count()
            ));
        }
    }
    if let Some(author) = &params.author
        && (!one_line(author) || author.trim().is_empty())
    {
        problems.push("--author is not one line of text".to_string());
    }
    problems
}

// ---------------------------------------------------------------------------------------------------------------- start

pub struct StartParams {
    pub title: String,
    pub question: Option<String>,
    pub author: Option<String>,
    pub detail: Detail,
}

#[derive(Debug)]
pub struct Started {
    pub id: TourId,
    /// the new file, in `.vnoc/local/tours/`
    pub path: PathBuf,
    /// the project had no `.vnoc/vnoc.toml`, and `start` made `.vnoc/` (the way `vnoc init` does)
    pub created_vnoc: bool,
}

/// Starts a tour: its first line, in a new file in `.vnoc/local/tours/`. In a project with no `.vnoc/vnoc.toml` it makes `.vnoc/` first, as the
/// first attach does (`UserData::initialize`). The id is new in both folders; a taken one (a clash of four characters) is tried again, ten times.
pub fn start(project_root: &Path, params: StartParams) -> Result<Started, StoreError> {
    let vnoc_root = project_root.join(layout::FOLDER);
    let created_vnoc = !layout::project_file(&vnoc_root).exists();
    UserData::new(project_root).initialize(true)?;
    let _lock = ToursLock::take(&vnoc_root)?;
    for attempt in 0..START_ATTEMPTS {
        let id = id_for(&params.title, attempt);
        if locate(&vnoc_root, &id) != Located::None {
            continue;
        }
        let first = Record::Tour(TourRecord {
            schema: 1,
            id: id.clone(),
            title: params.title.clone(),
            question: params.question.clone(),
            author: params.author.clone(),
            at: time::now(),
            detail: params.detail,
            next: None,
        });
        let path = file_in(&layout::local_tours_dir(&vnoc_root), &id);
        match crate::userdata::atomic::create_new(&path, print_line(&first).as_bytes()) {
            Ok(()) => {
                return Ok(Started {
                    id,
                    path,
                    created_vnoc,
                });
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error(format!("could not write {}", path.display()))(e)),
        }
    }
    Err(StoreError::NoFreeId)
}

// ---------------------------------------------------------------------------------------------------------------- append

/// The file a write goes to: the tour must be there, in one folder only.
fn writable(vnoc_root: &Path, id: &TourId) -> Result<PathBuf, StoreError> {
    match locate(vnoc_root, id) {
        Located::None => Err(StoreError::NotFound(id.clone())),
        Located::Both { .. } => Err(StoreError::InBoth(id.clone())),
        Located::Private(path) | Located::Project(path) => Ok(path),
    }
}

/// Adds records to the end of a tour, under the tours lock: every record's line in **one** `write_all` on a file opened for append. A file that
/// does not end in `\n` (a crash tore its last line) gets one first, in the same write, so the torn piece becomes one broken line and the
/// records are whole lines after it. Refuses when the file would pass 1 MB.
pub fn append(vnoc_root: &Path, id: &TourId, records: &[Record]) -> Result<(), StoreError> {
    let lock = ToursLock::take(vnoc_root)?;
    append_locked(&lock, vnoc_root, id, records)
}

/// [`append`] for a caller that holds the lock already: one that reads the file, makes ids from it ([`next_stop_id`]) and writes under the same
/// lock takes a [`ToursLock`] itself, [`read`]s, then calls this. (The lock is not re-entrant: `append` inside the lock would wait for itself.)
pub fn append_locked(
    _lock: &ToursLock,
    vnoc_root: &Path,
    id: &TourId,
    records: &[Record],
) -> Result<(), StoreError> {
    let path = writable(vnoc_root, id)?;
    let what = || format!("could not append to {}", path.display());
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .open(&path)
        .map_err(io_error(what()))?;
    let length = file.metadata().map_err(io_error(what()))?.len();
    let mut bytes = Vec::new();
    if length > 0 && !ends_in_newline(&mut file, length).map_err(io_error(what()))? {
        bytes.push(b'\n');
    }
    for record in records {
        bytes.extend_from_slice(print_line(record).as_bytes());
    }
    if length + bytes.len() as u64 > MAX_BYTES {
        return Err(StoreError::TooBig(id.clone()));
    }
    if bytes.is_empty() {
        return Ok(());
    }
    file.write_all(&bytes).map_err(io_error(what()))
}

fn ends_in_newline(file: &mut File, length: u64) -> io::Result<bool> {
    let mut last = [0u8; 1];
    file.seek(SeekFrom::Start(length - 1))?;
    file.read_exact(&mut last)?;
    Ok(last[0] == b'\n')
}

// ------------------------------------------------------------------------------------------------------------------ read

/// What a file folds to: the tour, or why it cannot be read at all.
pub type Folded = Result<Tour, Unreadable>;

/// A tour and where it was found.
pub type Loaded = (Where, Folded);

/// Folds the file at `path`. A file over 1 MB is folded up to its last `\n` within the first 1 MB, and one problem says the rest was not read.
/// Bytes that are not UTF-8 are replaced, so one broken line breaks only itself.
fn load(path: &Path) -> io::Result<Folded> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let over = bytes.len() as u64 > MAX_BYTES;
    if over {
        bytes.truncate(MAX_BYTES as usize);
        match bytes.iter().rposition(|b| *b == b'\n') {
            Some(last) => bytes.truncate(last + 1),
            None => bytes.clear(),
        }
    }
    let lines_read = bytes.iter().filter(|b| **b == b'\n').count();
    let mut folded = fold(&String::from_utf8_lossy(&bytes));
    if over && let Ok(tour) = &mut folded {
        tour.problems.push(Problem {
            line: lines_read + 1,
            why: "the file is over 1 MB: the rest is not read".to_string(),
        });
    }
    Ok(folded)
}

/// A tour read, with the file's length from **before** it was read, and where the file is: what a recorded check is made true of (a file that
/// grows while it is read makes the record not valid, never wrong).
#[derive(Debug)]
pub struct Measured {
    pub place: Where,
    pub tour: Folded,
    pub len: u64,
    pub path: PathBuf,
}

/// [`read`], with the length and the path.
pub fn read_measured(vnoc_root: &Path, id: &TourId) -> Result<Measured, StoreError> {
    let (place, path) = match locate(vnoc_root, id) {
        Located::None => return Err(StoreError::NotFound(id.clone())),
        Located::Private(path) => (Where::Private, path),
        Located::Project(path) | Located::Both { project: path, .. } => (Where::Project, path),
    };
    let what = format!("could not read {}", path.display());
    let len = fs::metadata(&path).map_err(io_error(what.clone()))?.len();
    let tour = load(&path).map_err(io_error(what))?;
    Ok(Measured {
        place,
        tour,
        len,
        path,
    })
}

/// Reads a tour: the fold of its file, and the folder it is in (the project's one when the id is in both). A tour that cannot be read comes
/// back as `Err(Unreadable)` inside `Ok`, not as an error of this function. Never takes the lock and never writes.
pub fn read(vnoc_root: &Path, id: &TourId) -> Result<Loaded, StoreError> {
    let (place, path) = match locate(vnoc_root, id) {
        Located::None => return Err(StoreError::NotFound(id.clone())),
        Located::Private(path) => (Where::Private, path),
        Located::Project(path) | Located::Both { project: path, .. } => (Where::Project, path),
    };
    let folded = load(&path).map_err(io_error(format!("could not read {}", path.display())))?;
    Ok((place, folded))
}

// ------------------------------------------------------------------------------------------------------------------ list

/// One tour of a folder, as `list` found it.
#[derive(Debug)]
pub struct Row {
    pub id: TourId,
    pub place: Where,
    pub path: PathBuf,
    pub tour: Folded,
    /// the file's length from before it was read
    pub len: u64,
}

#[derive(Debug, Default)]
pub struct Listed {
    /// newest first by the tour record's `at`, then by id; what cannot be read is last. The same id in both folders is two rows (the project's first).
    pub tours: Vec<Row>,
    /// a file that is not a tour's, a folder that could not be read: each one said once
    pub problems: Vec<String>,
}

/// Every tour of both folders (a folder that is not there is empty), the rules of 01 §10: a file whose name is not `<valid tour id>.jsonl` is
/// skipped and named once. Never writes.
pub fn list(vnoc_root: &Path) -> Listed {
    let mut listed = Listed::default();
    for (place, dir, shown) in [
        (Where::Project, layout::tours_dir(vnoc_root), ".vnoc/tours/"),
        (
            Where::Private,
            layout::local_tours_dir(vnoc_root),
            ".vnoc/local/tours/",
        ),
    ] {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                listed.problems.push(format!("could not read {shown}: {e}"));
                continue;
            }
        };
        let mut names = HashSet::new();
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name
                .strip_suffix(".jsonl")
                .and_then(|stem| TourId::parse(stem).ok());
            let Some(id) = id else {
                if names.insert(name.clone()) {
                    listed.problems.push(format!(
                        "{shown}{name} is not a tour: a tour's file is named <id>.jsonl"
                    ));
                }
                continue;
            };
            let path = entry.path();
            let len = fs::metadata(&path).map_or(0, |m| m.len());
            let tour = load(&path).unwrap_or_else(|e| {
                Err(Unreadable {
                    why: format!("could not read the file: {e}"),
                    too_new: false,
                })
            });
            listed.tours.push(Row {
                id,
                place,
                path,
                tour,
                len,
            });
        }
    }
    listed.tours.sort_by(|a, b| {
        let at = |row: &Row| row.tour.as_ref().map_or(String::new(), |t| t.at.clone());
        at(b)
            .cmp(&at(a))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.place.cmp(&b.place).reverse())
    });
    listed.problems.sort();
    listed
}

// ------------------------------------------------------------------------------------------------------------ the state file

/// This machine's state of one tour (01 §9): where the reader is, and what the last check found. The two parts are typed later (the check in
/// TR5, the place in TR10); here they are the JSON they are written as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TourState {
    pub schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Value>,
}

impl Default for TourState {
    fn default() -> Self {
        TourState {
            schema: 1,
            place: None,
            check: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Place,
    Check,
}

fn state_file(vnoc_root: &Path, id: &TourId) -> PathBuf {
    layout::tour_state_dir(vnoc_root).join(format!("{id}.json"))
}

/// The state of a tour on this machine. A file that is not there, or does not parse (or is a schema this version does not know), is the empty
/// state: it is this machine's and it can be made again (01 §10). Never takes the lock: the file is replaced whole.
pub fn read_state(vnoc_root: &Path, id: &TourId) -> TourState {
    fs::read(state_file(vnoc_root, id))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<TourState>(&bytes).ok())
        .filter(|state| state.schema == 1)
        .unwrap_or_default()
}

/// Replaces one part of the state and keeps the other: under the tours lock, reads the file, sets the part, and writes the whole file again
/// (`write_atomic`). A file that did not parse is replaced.
pub fn write_state_part(
    vnoc_root: &Path,
    id: &TourId,
    part: Part,
    value: Value,
) -> Result<(), StoreError> {
    let _lock = ToursLock::take(vnoc_root)?;
    let mut state = read_state(vnoc_root, id);
    match part {
        Part::Place => state.place = Some(value),
        Part::Check => state.check = Some(value),
    }
    let mut bytes = serde_json::to_vec(&state).expect("JSON is JSON");
    bytes.push(b'\n');
    let path = state_file(vnoc_root, id);
    crate::userdata::atomic::write_atomic(&path, &bytes)
        .map_err(io_error(format!("could not write {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::record::{DropRecord, MetaRecord, parse_line};
    use serde_json::json;
    use std::collections::BTreeSet;

    const AT: &str = "2026-11-23T18:40:02Z";

    /// An empty project folder (no `.vnoc/`), fresh for this test.
    fn project(label: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-tours-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn vnoc_of(project: &Path) -> PathBuf {
        project.join(".vnoc")
    }

    fn params(title: &str) -> StartParams {
        StartParams {
            title: title.to_string(),
            question: Some("why?".to_string()),
            author: Some("claude-code".to_string()),
            detail: Detail::Normal,
        }
    }

    fn id(text: &str) -> TourId {
        TourId::parse(text).unwrap()
    }

    /// The first line of a tour, as `start` writes it.
    fn head(id: &str, at: &str) -> String {
        format!(
            r#"{{"type":"tour","schema":1,"id":"{id}","title":"T {id}","at":"{at}","detail":"normal"}}"#
        )
    }

    /// Puts a file in one of the two folders by hand.
    fn put(vnoc_root: &Path, place: Where, id: &str, text: &str) -> PathBuf {
        let dir = match place {
            Where::Private => layout::local_tours_dir(vnoc_root),
            Where::Project => layout::tours_dir(vnoc_root),
        };
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        fs::write(&path, text).unwrap();
        path
    }

    fn meta(title: &str) -> Record {
        Record::Meta(MetaRecord {
            title: Some(title.to_string()),
            question: None,
            detail: None,
        })
    }

    fn halt() -> Record {
        Record::Halt(crate::tours::record::Bare {})
    }

    /// Every file under `dir`, as paths relative to it with `/`.
    fn files_under(dir: &Path) -> BTreeSet<String> {
        fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else {
                    let relative = path.strip_prefix(base).unwrap();
                    out.insert(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = BTreeSet::new();
        walk(dir, dir, &mut out);
        out
    }

    // ---- ids

    #[test]
    fn ids_for_five_titles() {
        let long =
            "How the order service decides what to do when the payment provider times out halfway";
        let cases = [
            ("checkout_order", "checkout-order"),
            (long, "how-the-order-service-decides-what-to-do"),
            ("!!! ??? ...", "tour"),
            ("Über café", "ber-caf"),
            ("  --Hello,   World!--  ", "hello-world"),
            ("", "tour"),
            ("日本語", "tour"),
        ];
        for (title, slug) in cases {
            let made = new_id(title);
            let text = made.as_str();
            assert!(text.starts_with(&format!("{slug}-")), "{title:?}: {text}");
            assert_eq!(made.slug(), slug, "{title:?}");
            assert!(made.slug().len() <= 40, "{text}");
            assert!(text.is_ascii(), "{text}");
            assert_eq!(TourId::parse(text).unwrap(), made, "it is a valid id");
        }
        // the long title is cut at 40 characters and the cut never leaves a `-` at the end
        assert!(new_id(long).slug().len() <= 40);
        let cut_on_a_dash = format!("{}-b", "a".repeat(39));
        assert_eq!(new_id(&cut_on_a_dash).slug(), "a".repeat(39));
        assert_eq!(new_id(&"x".repeat(100)).slug(), "x".repeat(40));
    }

    #[test]
    fn two_equal_titles_give_two_ids() {
        let ids: BTreeSet<TourId> = (0..200).map(|_| new_id("same title")).collect();
        assert_eq!(ids.len(), 200, "the four characters differ every time");
        for id in &ids {
            assert!(id.as_str().starts_with("same-title-"));
        }
    }

    // ---- start

    #[test]
    fn start_twice_makes_two_files_each_with_a_valid_first_line() {
        let root = project("start-twice");
        let a = start(&root, params("checkout_order")).unwrap();
        let b = start(&root, params("checkout_order")).unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(a.path, b.path);
        assert!(a.created_vnoc && !b.created_vnoc);
        let vnoc_root = vnoc_of(&root);
        for started in [&a, &b] {
            assert_eq!(
                started.path,
                layout::local_tours_dir(&vnoc_root).join(format!("{}.jsonl", started.id))
            );
            let text = fs::read_to_string(&started.path).unwrap();
            assert_eq!(text.matches('\n').count(), 1, "{text}");
            let (place, tour) = read(&vnoc_root, &started.id).unwrap();
            let tour = tour.unwrap();
            assert_eq!(place, Where::Private);
            assert_eq!(tour.id, started.id);
            assert_eq!(tour.title, "checkout_order");
            assert_eq!(tour.question.as_deref(), Some("why?"));
            assert_eq!(tour.author.as_deref(), Some("claude-code"));
            assert_eq!(tour.detail, Detail::Normal);
            assert!(
                tour.at.starts_with("20") && tour.at.ends_with('Z'),
                "{}",
                tour.at
            );
            assert!(tour.problems.is_empty() && tour.stops.is_empty());
        }
    }

    #[test]
    fn start_in_a_project_with_no_vnoc_folder_makes_exactly_the_files_init_makes_and_the_lock_and_the_tour()
     {
        let root = project("start-fresh");
        let started = start(&root, params("T")).unwrap();
        assert!(started.created_vnoc);
        let want: BTreeSet<String> = [
            ".vnoc/vnoc.toml".to_string(),
            ".vnoc/.gitignore".to_string(),
            ".vnoc/local/tours.lock".to_string(),
            format!(".vnoc/local/tours/{}.jsonl", started.id),
        ]
        .into();
        assert_eq!(files_under(&root), want);
        assert_eq!(
            fs::read_to_string(root.join(".vnoc/.gitignore")).unwrap(),
            "local/\n"
        );
    }

    #[test]
    fn start_in_a_project_that_has_a_vnoc_folder_leaves_vnoc_toml_byte_for_byte() {
        let root = project("start-existing");
        let toml = "format = 1\n# my own comment\n[ids]\ndefault = \"none\"\n";
        fs::create_dir_all(root.join(".vnoc")).unwrap();
        fs::write(root.join(".vnoc/vnoc.toml"), toml).unwrap();
        let started = start(&root, params("T")).unwrap();
        assert!(!started.created_vnoc);
        assert_eq!(
            fs::read_to_string(root.join(".vnoc/vnoc.toml")).unwrap(),
            toml
        );
        assert!(
            !root.join(".vnoc/.gitignore").exists(),
            "nothing else is made"
        );
    }

    #[test]
    fn start_does_not_take_an_id_the_project_already_has() {
        // an id that is taken in the project's folder is skipped: with ten tries, one of them is free
        let root = project("start-taken");
        let vnoc_root = vnoc_of(&root);
        let started = start(&root, params("T")).unwrap();
        let (private, project_path) = (
            started.path.clone(),
            put(&vnoc_root, Where::Project, started.id.as_str(), "x\n"),
        );
        assert!(private.is_file() && project_path.is_file());
        assert!(matches!(
            locate(&vnoc_root, &started.id),
            Located::Both { .. }
        ));
    }

    // ---- append

    #[test]
    fn append_adds_every_line_in_order_and_nothing_else() {
        let root = project("append");
        let started = start(&root, params("T")).unwrap();
        let vnoc_root = vnoc_of(&root);
        let before = fs::read_to_string(&started.path).unwrap();
        append(&vnoc_root, &started.id, &[meta("one"), halt(), meta("two")]).unwrap();
        append(&vnoc_root, &started.id, &[]).unwrap();
        let after = fs::read_to_string(&started.path).unwrap();
        assert_eq!(
            after,
            format!(
                "{before}{{\"type\":\"meta\",\"title\":\"one\"}}\n{{\"type\":\"halt\"}}\n{{\"type\":\"meta\",\"title\":\"two\"}}\n"
            )
        );
        let (_, tour) = read(&vnoc_root, &started.id).unwrap();
        assert_eq!(tour.unwrap().title, "two");
    }

    #[test]
    fn append_to_a_tour_that_is_not_there_says_so() {
        let root = project("append-none");
        let vnoc_root = vnoc_of(&root);
        let err = append(&vnoc_root, &id("nope-0000"), &[halt()]).unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
        assert_eq!(err.to_string(), "there is no tour nope-0000");
        assert!(matches!(
            read(&vnoc_root, &id("nope-0000")).unwrap_err(),
            StoreError::NotFound(_)
        ));
    }

    #[test]
    fn append_to_a_torn_file_turns_the_torn_piece_into_one_problem_and_the_records_are_read() {
        let root = project("torn");
        let vnoc_root = vnoc_of(&root);
        let text = format!(
            "{}\n{{\"type\":\"meta\",\"title\":\"half",
            head("t-0000", AT)
        );
        let path = put(&vnoc_root, Where::Private, "t-0000", &text);
        // as it is, the torn last line is not read and is not a problem
        let (_, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        assert!(tour.unwrap().problems.is_empty());

        append(&vnoc_root, &id("t-0000"), &[meta("whole")]).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            after,
            format!("{text}\n{{\"type\":\"meta\",\"title\":\"whole\"}}\n")
        );
        let (_, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        let tour = tour.unwrap();
        assert_eq!(tour.problems.len(), 1, "{:?}", tour.problems);
        assert_eq!(tour.problems[0].line, 2);
        assert_eq!(tour.title, "whole", "the appended record is read");
    }

    #[test]
    fn append_to_a_file_with_crlf_ends_keeps_working() {
        let root = project("crlf");
        let vnoc_root = vnoc_of(&root);
        put(
            &vnoc_root,
            Where::Project,
            "t-0000",
            &format!("{}\r\n", head("t-0000", AT)),
        );
        append(&vnoc_root, &id("t-0000"), &[meta("x")]).unwrap();
        let (place, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        assert_eq!(place, Where::Project);
        let tour = tour.unwrap();
        assert!(tour.problems.is_empty());
        assert_eq!(tour.title, "x");
    }

    #[test]
    fn two_writers_and_a_reader_one_hundred_appends_each() {
        // TR0's check 3, as a test, with a File handle of its own for every append and every lock (as two processes would have). CI runs it on
        // Windows and Linux: there it is the answer.
        let root = project("race");
        let started = start(&root, params("race")).unwrap();
        let vnoc_root = vnoc_of(&root);
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let reader = {
            let (vnoc_root, id, done) = (vnoc_root.clone(), started.id.clone(), done.clone());
            std::thread::spawn(move || {
                let mut reads = 0;
                loop {
                    let finished = done.load(Ordering::SeqCst);
                    let (_, tour) = read(&vnoc_root, &id).expect("a read never fails");
                    let tour = tour.expect("a read is never Unreadable");
                    assert!(tour.problems.is_empty(), "{:?}", tour.problems);
                    reads += 1;
                    if finished {
                        return reads;
                    }
                }
            })
        };
        let writers: Vec<_> = (0..2)
            .map(|w| {
                let (vnoc_root, id) = (vnoc_root.clone(), started.id.clone());
                std::thread::spawn(move || {
                    for i in 0..100 {
                        append(&vnoc_root, &id, &[meta(&format!("w{w}-{i}"))]).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        done.store(true, Ordering::SeqCst);
        assert!(reader.join().unwrap() >= 1);

        let text = fs::read_to_string(&started.path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 201, "the first line and 200 new ones");
        let mut titles = BTreeSet::new();
        for line in &lines[1..] {
            let Record::Meta(m) = parse_line(line).unwrap() else {
                panic!("{line}")
            };
            titles.insert(m.title.unwrap());
        }
        assert_eq!(titles.len(), 200, "each one once, none torn, none lost");
    }

    #[test]
    fn append_refuses_when_the_file_would_pass_1_mb() {
        let root = project("big-append");
        let vnoc_root = vnoc_of(&root);
        let halt_len = print_line(&halt()).len() as u64;
        // a file of exactly 1 MB less one halt: a halt fits, and then nothing does
        let first = head("t-0000", AT);
        let (open, close) = (r#"{"type":"x","pad":""#, r#""}"#);
        let filler_len = MAX_BYTES as usize - halt_len as usize - first.len() - 2;
        let filler = format!(
            "{open}{}{close}",
            "a".repeat(filler_len - open.len() - close.len())
        );
        let text = format!("{first}\n{filler}\n");
        assert_eq!(text.len() as u64, MAX_BYTES - halt_len);
        let path = put(&vnoc_root, Where::Private, "t-0000", &text);
        append(&vnoc_root, &id("t-0000"), &[halt()]).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            MAX_BYTES,
            "exactly 1 MB is allowed"
        );
        let err = append(&vnoc_root, &id("t-0000"), &[halt()]).unwrap_err();
        assert!(matches!(err, StoreError::TooBig(_)));
        assert_eq!(
            err.to_string(),
            "the tour t-0000 would be over 1 MB: finish it and start a second tour"
        );
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            MAX_BYTES,
            "nothing was written"
        );
    }

    #[test]
    fn a_tour_in_both_folders_is_read_from_the_project_and_every_write_refuses() {
        let root = project("both");
        let vnoc_root = vnoc_of(&root);
        let private = put(
            &vnoc_root,
            Where::Private,
            "t-0000",
            &format!("{}\n", head("t-0000", AT)),
        );
        let shared = put(
            &vnoc_root,
            Where::Project,
            "t-0000",
            &format!(
                "{}\n{{\"type\":\"meta\",\"title\":\"from the project\"}}\n",
                head("t-0000", AT)
            ),
        );
        let (place, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        assert_eq!(place, Where::Project);
        assert_eq!(tour.unwrap().title, "from the project");

        let err = append(&vnoc_root, &id("t-0000"), &[halt()]).unwrap_err();
        assert!(matches!(err, StoreError::InBoth(_)));
        assert_eq!(
            err.to_string(),
            "the tour t-0000 is in .vnoc/tours/ and in .vnoc/local/tours/: keep one"
        );
        assert!(!fs::read_to_string(&private).unwrap().contains("halt"));
        assert!(!fs::read_to_string(&shared).unwrap().contains("halt"));

        let listed = list(&vnoc_root);
        let rows: Vec<(String, Where)> = listed
            .tours
            .iter()
            .map(|r| (r.id.to_string(), r.place))
            .collect();
        assert_eq!(
            rows,
            [
                ("t-0000".to_string(), Where::Project),
                ("t-0000".to_string(), Where::Private)
            ]
        );
    }

    // ---- read

    #[test]
    fn a_tour_that_cannot_be_read_comes_back_as_such_and_not_as_an_error() {
        let root = project("unreadable");
        let vnoc_root = vnoc_of(&root);
        put(&vnoc_root, Where::Private, "t-0000", "this is not JSON\n");
        let (place, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        assert_eq!(place, Where::Private);
        assert!(tour.unwrap_err().why.contains("first line"));
    }

    #[test]
    fn a_file_over_1_mb_reads_its_first_mb_and_says_so_once() {
        let root = project("big-read");
        let vnoc_root = vnoc_of(&root);
        let mut lines = vec![head("t-0000", AT)];
        let mut total = lines[0].len() + 1;
        let mut i = 0;
        while total < 1_200_000 {
            i += 1;
            let line = format!(r#"{{"type":"meta","title":"line {i}"}}"#);
            total += line.len() + 1;
            lines.push(line);
        }
        let mut text = lines.join("\n");
        text.push('\n');
        put(&vnoc_root, Where::Private, "t-0000", &text);

        // the lines that fit in the first MB, counted here by hand
        let (mut size, mut fit) = (0usize, 0usize);
        for line in &lines {
            if size + line.len() + 1 > MAX_BYTES as usize {
                break;
            }
            size += line.len() + 1;
            fit += 1;
        }
        assert!(fit < lines.len());

        let (_, tour) = read(&vnoc_root, &id("t-0000")).unwrap();
        let tour = tour.unwrap();
        assert_eq!(
            tour.title,
            format!("line {}", fit - 1),
            "the last line of the first MB"
        );
        assert_eq!(
            tour.problems,
            [Problem {
                line: fit + 1,
                why: "the file is over 1 MB: the rest is not read".to_string()
            }]
        );
        // append refuses on it, too
        assert!(matches!(
            append(&vnoc_root, &id("t-0000"), &[halt()]).unwrap_err(),
            StoreError::TooBig(_)
        ));
        // and `list` reads it the same way
        let listed = list(&vnoc_root);
        assert_eq!(listed.tours[0].tour.as_ref().unwrap().problems.len(), 1);
    }

    // ---- list

    #[test]
    fn list_has_both_folders_skips_a_wrong_name_once_and_sorts_newest_first() {
        let root = project("list");
        let vnoc_root = vnoc_of(&root);
        let one = |id: &str, at: &str| format!("{}\n", head(id, at));
        put(&vnoc_root, Where::Private, "aa-0000", ""); // an empty file
        put(&vnoc_root, Where::Private, "bb-0000", "garbage\n"); // an unreadable first line
        put(
            &vnoc_root,
            Where::Private,
            "cc-0000",
            &one("cc-0000", "2026-11-23T10:00:00Z"),
        );
        put(
            &vnoc_root,
            Where::Private,
            "dd-0000",
            &one("dd-0000", "2026-11-24T10:00:00Z"),
        );
        put(
            &vnoc_root,
            Where::Project,
            "cc-0000",
            &one("cc-0000", "2026-11-23T10:00:00Z"),
        );
        put(
            &vnoc_root,
            Where::Project,
            "ee-0000",
            &one("ee-0000", "2026-11-25T10:00:00Z"),
        );
        put(
            &vnoc_root,
            Where::Project,
            "ff-0000",
            &one("ff-0000", "2026-11-23T10:00:00Z"),
        );
        fs::write(layout::local_tours_dir(&vnoc_root).join("notes.txt"), "x").unwrap();
        fs::write(
            layout::local_tours_dir(&vnoc_root).join("Bad-Name.jsonl"),
            "x",
        )
        .unwrap();
        fs::write(
            layout::local_tours_dir(&vnoc_root).join("nohyphen.jsonl"),
            "x",
        )
        .unwrap();
        fs::create_dir_all(layout::local_tours_dir(&vnoc_root).join("a-folder-0000.jsonl"))
            .unwrap();

        let before = files_under(&root);
        let listed = list(&vnoc_root);
        assert_eq!(files_under(&root), before, "list writes nothing");

        let rows: Vec<String> = listed
            .tours
            .iter()
            .map(|r| {
                format!(
                    "{} {} {}",
                    r.id,
                    match r.place {
                        Where::Project => "project",
                        Where::Private => "private",
                    },
                    if r.tour.is_ok() { "ok" } else { "unreadable" }
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                "ee-0000 project ok",
                "dd-0000 private ok",
                "cc-0000 project ok",
                "cc-0000 private ok",
                "ff-0000 project ok",
                "aa-0000 private unreadable",
                "bb-0000 private unreadable",
            ],
            "newest first, the same time by id, the project's copy first, what cannot be read last"
        );
        assert!(
            listed.tours[5]
                .tour
                .as_ref()
                .unwrap_err()
                .why
                .contains("no complete first line")
        );
        assert_eq!(listed.problems.len(), 3, "{:?}", listed.problems);
        for name in ["Bad-Name.jsonl", "nohyphen.jsonl", "notes.txt"] {
            assert_eq!(
                listed.problems.iter().filter(|p| p.contains(name)).count(),
                1,
                "{name} is named once: {:?}",
                listed.problems
            );
        }
    }

    #[test]
    fn list_with_no_folders_is_empty_and_makes_none() {
        let root = project("list-none");
        let listed = list(&vnoc_of(&root));
        assert!(listed.tours.is_empty() && listed.problems.is_empty());
        assert!(files_under(&root).is_empty());
        assert!(!root.join(".vnoc").exists());
    }

    // ---- the highest numbers ever used

    fn stop_line(n: u32) -> String {
        format!(
            r#"{{"type":"stop","id":"s{n}","path":"a.py","qualname":"f{n}","kind":"function","text":"t"}}"#
        )
    }

    fn step_line(n: u32, m: u32) -> String {
        format!(
            r#"{{"type":"step","id":"s{n}.b{m}","stop":"s{n}","lines":[1,2],"hash":"0000000000000000","title":"t","text":"t"}}"#
        )
    }

    fn drop_line(id: &str) -> String {
        format!(r#"{{"type":"drop","id":"{id}"}}"#)
    }

    fn tour_of(lines: &[String]) -> Tour {
        let mut text = format!("{}\n", head("t-0000", AT));
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        fold(&text).unwrap()
    }

    fn stop_id(n: u32) -> StopId {
        StopId::new(n).unwrap()
    }

    #[test]
    fn the_next_stop_is_one_above_the_highest_ever_used_a_dropped_one_too() {
        let plan = r#"{"type":"plan","stops":[{"id":"s1","path":"a.py","qualname":"a"},{"id":"s2","path":"a.py","qualname":"b"},{"id":"s3","path":"a.py","qualname":"c"}]}"#;
        // a plan of s1..s3, a write of s2, a drop of s3
        let tour = tour_of(&[plan.to_string(), stop_line(2), drop_line("s3")]);
        assert!(tour.stop(stop_id(3)).is_none(), "s3 is gone from the route");
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s4");
        // a drop of a stop that was never there still used its number
        let tour = tour_of(&[stop_line(1), drop_line("s7")]);
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s8");
        // a stop written twice is one number
        let tour = tour_of(&[stop_line(1), stop_line(1)]);
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s2");
        // nothing yet
        assert_eq!(next_stop_id(&tour_of(&[])).unwrap().to_string(), "s1");
        // a block of a stop that is not there is a problem line, and the number is still used
        let tour = tour_of(&[step_line(5, 4)]);
        assert_eq!(tour.problems.len(), 1);
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s6");
        assert_eq!(
            next_block_id(&tour, stop_id(5)).unwrap().to_string(),
            "s5.b5"
        );
    }

    #[test]
    fn the_next_block_is_one_above_the_highest_ever_used_under_that_stop() {
        let tour = tour_of(&[
            stop_line(1),
            step_line(1, 1),
            step_line(1, 2),
            drop_line("s1.b2"),
        ]);
        assert_eq!(
            steps_of(&tour),
            ["s1.b1"],
            "s1.b2 was dropped, and its number stays used"
        );
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().to_string(),
            "s1.b3"
        );
        assert_eq!(
            next_block_id(&tour, stop_id(2)).unwrap().to_string(),
            "s2.b1"
        );
        // a block replaced is one number
        let tour = tour_of(&[stop_line(1), step_line(1, 1), step_line(1, 1)]);
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().to_string(),
            "s1.b2"
        );
        // a stop written again clears its blocks, and the numbers they used stay used
        let tour = tour_of(&[stop_line(1), step_line(1, 3), stop_line(1)]);
        assert_eq!(steps_of(&tour), Vec::<String>::new());
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().to_string(),
            "s1.b4"
        );
    }

    fn steps_of(tour: &Tour) -> Vec<String> {
        tour.stops
            .iter()
            .flat_map(|s| s.blocks.iter().map(|b| b.id.to_string()))
            .collect()
    }

    #[test]
    fn the_next_numbers_start_above_what_an_import_recorded() {
        let first = r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"quick","next":{"stop":9,"blocks":{"s1":5}}}"#;
        let tour = fold(&format!("{first}\n{}\n", stop_line(2))).unwrap();
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s10");
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().to_string(),
            "s1.b6"
        );
        assert_eq!(
            next_block_id(&tour, stop_id(2)).unwrap().to_string(),
            "s2.b1"
        );
        // a line above `next` wins over it
        let tour = fold(&format!(
            "{first}\n{}\n{}\n",
            stop_line(12),
            step_line(1, 8)
        ))
        .unwrap();
        assert_eq!(next_stop_id(&tour).unwrap().to_string(), "s13");
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().to_string(),
            "s1.b9"
        );
    }

    #[test]
    fn the_highest_number_does_not_wrap_or_panic() {
        let tour = tour_of(&[drop_line("s4294967295"), drop_line("s1.b4294967295")]);
        assert_eq!(next_stop_id(&tour), None, "used up: no number is reused");
        assert_eq!(next_block_id(&tour, stop_id(1)), None);
        // one below the largest still has the largest to give
        let tour = tour_of(&[drop_line("s4294967294"), drop_line("s1.b4294967294")]);
        assert_eq!(next_stop_id(&tour).unwrap().number(), u32::MAX);
        assert_eq!(
            next_block_id(&tour, stop_id(1)).unwrap().block_number(),
            Some(u32::MAX)
        );
    }

    // ---- the state file

    #[test]
    fn writing_the_place_keeps_the_check_and_the_other_way_round() {
        let root = project("state");
        let vnoc_root = vnoc_of(&root);
        let tour = id("t-0000");
        assert_eq!(
            read_state(&vnoc_root, &tour),
            TourState::default(),
            "no file: empty"
        );
        assert!(!vnoc_root.exists(), "reading makes nothing");

        let place = json!({"at": "s3.b2", "seen": ["s1"], "open": true});
        let check = json!({"tour_len": 4182, "stops": {"s3": "changed"}});
        write_state_part(&vnoc_root, &tour, Part::Place, place.clone()).unwrap();
        let state = read_state(&vnoc_root, &tour);
        assert_eq!(
            (state.schema, state.place.as_ref(), state.check.as_ref()),
            (1, Some(&place), None)
        );
        write_state_part(&vnoc_root, &tour, Part::Check, check.clone()).unwrap();
        let state = read_state(&vnoc_root, &tour);
        assert_eq!(
            (state.place.as_ref(), state.check.as_ref()),
            (Some(&place), Some(&check)),
            "both"
        );

        let place2 = json!({"at": "end"});
        write_state_part(&vnoc_root, &tour, Part::Place, place2.clone()).unwrap();
        let state = read_state(&vnoc_root, &tour);
        assert_eq!(
            (state.place.as_ref(), state.check.as_ref()),
            (Some(&place2), Some(&check))
        );

        // another tour has its own file
        assert_eq!(read_state(&vnoc_root, &id("u-0000")), TourState::default());
        let dir = layout::tour_state_dir(&vnoc_root);
        let names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["t-0000.json"], "no temporary file is left");
    }

    #[test]
    fn a_state_file_that_does_not_parse_reads_as_empty_and_the_next_write_replaces_it() {
        let root = project("state-broken");
        let vnoc_root = vnoc_of(&root);
        let tour = id("t-0000");
        let dir = layout::tour_state_dir(&vnoc_root);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t-0000.json");
        for broken in [
            "{ not json",
            "",
            "[]",
            r#"{"schema":2,"place":{"at":"s1"}}"#,
            r#"{"place":{"at":"s1"}}"#,
            r#"{"schema":1,"place":"#,
        ] {
            fs::write(&file, broken).unwrap();
            assert_eq!(
                read_state(&vnoc_root, &tour),
                TourState::default(),
                "{broken:?}"
            );
        }
        fs::write(&file, "{ not json").unwrap();
        write_state_part(&vnoc_root, &tour, Part::Check, json!({"tour_len": 1})).unwrap();
        let state = read_state(&vnoc_root, &tour);
        assert_eq!(state.check, Some(json!({"tour_len": 1})));
        assert_eq!(state.place, None);
        assert!(serde_json::from_str::<Value>(&fs::read_to_string(&file).unwrap()).is_ok());
    }

    #[test]
    fn the_state_of_a_tour_is_one_file_whatever_folder_the_tour_is_in() {
        // sharing a tour (a move between folders) keeps its place and its check: the state file is named by the id alone
        let root = project("state-move");
        let vnoc_root = vnoc_of(&root);
        let started = start(&root, params("T")).unwrap();
        write_state_part(&vnoc_root, &started.id, Part::Place, json!({"at": "cover"})).unwrap();
        fs::create_dir_all(layout::tours_dir(&vnoc_root)).unwrap();
        fs::rename(
            &started.path,
            layout::tours_dir(&vnoc_root).join(format!("{}.jsonl", started.id)),
        )
        .unwrap();
        assert_eq!(read(&vnoc_root, &started.id).unwrap().0, Where::Project);
        assert_eq!(
            read_state(&vnoc_root, &started.id).place,
            Some(json!({"at": "cover"}))
        );
    }

    // ---- picking a tour by what was typed

    /// A tour file named `id` in `.vnoc/local/tours/`, with only its first line.
    fn tour_named(vnoc_root: &Path, id: &str) {
        let dir = layout::local_tours_dir(vnoc_root);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{id}.jsonl")), "{}\n").unwrap();
    }

    fn ids(list: &[TourId]) -> Vec<&str> {
        list.iter().map(TourId::as_str).collect()
    }

    #[test]
    fn a_typed_id_is_the_id_or_the_one_id_that_starts_with_it() {
        let root = project("pick");
        let vnoc_root = vnoc_of(&root);
        for id in [
            "checkout-order-7k2q",
            "reserve-stock-m3xd",
            "reserve-tax-0aaa",
        ] {
            tour_named(&vnoc_root, id);
        }
        let one = |text: &str| match pick(&vnoc_root, text) {
            Pick::One(id) => id.to_string(),
            other => panic!("{text}: {other:?}"),
        };
        assert_eq!(one("checkout-order-7k2q"), "checkout-order-7k2q");
        assert_eq!(
            one("check"),
            "checkout-order-7k2q",
            "the start of the only one that starts so"
        );
        assert_eq!(one("reserve-s"), "reserve-stock-m3xd");
        match pick(&vnoc_root, "reserve") {
            Pick::Several(found) => {
                assert_eq!(ids(&found), ["reserve-stock-m3xd", "reserve-tax-0aaa"])
            }
            other => panic!("{other:?}"),
        }
        // the same id in both folders is one tour to name
        let shared = layout::tours_dir(&vnoc_root);
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("checkout-order-7k2q.jsonl"), "{}\n").unwrap();
        assert_eq!(one("checkout"), "checkout-order-7k2q");
    }

    #[test]
    fn an_id_that_starts_nothing_gets_the_nearest_ids_and_no_folder_means_none() {
        let root = project("pick-near");
        let vnoc_root = vnoc_of(&root);
        // no folder at all
        assert_eq!(pick(&vnoc_root, "anything"), Pick::Nothing { near: vec![] });
        for id in ["checkout-order-7k2q", "reserve-stock-m3xd", "zebra-0000"] {
            tour_named(&vnoc_root, id);
        }
        fs::write(layout::local_tours_dir(&vnoc_root).join("notes.txt"), "x").unwrap();
        match pick(&vnoc_root, "chekout-order-7k2q") {
            Pick::Nothing { near } => assert_eq!(ids(&near), ["checkout-order-7k2q"]),
            other => panic!("{other:?}"),
        }
        // a slug typed with a slip is near its id too
        match pick(&vnoc_root, "reserv-stock") {
            Pick::Nothing { near } => assert_eq!(ids(&near), ["reserve-stock-m3xd"]),
            other => panic!("{other:?}"),
        }
        // nothing like it
        assert_eq!(
            pick(&vnoc_root, "qqqqqqqqqqqqqqqq"),
            Pick::Nothing { near: vec![] }
        );
        // an empty text starts every id and names none
        match pick(&vnoc_root, "") {
            Pick::Nothing { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_arguments_of_start_are_one_line_of_the_right_size() {
        let ok = |title: &str| check_start(&params(title));
        assert!(ok("checkout_order").is_empty());
        assert!(
            ok(&"é".repeat(60)).is_empty(),
            "60 characters, not 60 bytes"
        );
        assert_eq!(
            ok(&"x".repeat(61)),
            ["--title is 61 characters: the most it takes is 60"]
        );
        assert_eq!(
            ok("  \t "),
            ["--title is empty: say what the tour is of, in a few words"]
        );
        assert_eq!(ok("two\nlines"), ["--title is not one line"]);
        let mut p = params("t");
        p.question = Some("q".repeat(201));
        p.author = Some("a\nb".to_string());
        assert_eq!(
            check_start(&p),
            [
                "--question is 201 characters: the most it takes is 200",
                "--author is not one line of text"
            ]
        );
        p.question = Some("q".repeat(200));
        p.author = Some("  ".to_string());
        assert_eq!(check_start(&p), ["--author is not one line of text"]);
    }

    #[test]
    fn drop_record_is_used_by_the_tests_of_the_numbers() {
        // keeps the import honest: the drop line the tests print is the line `print_line` writes
        let line = print_line(&Record::Drop(DropRecord {
            id: StepId::parse("s3.b1").unwrap(),
        }));
        assert_eq!(line.trim_end(), drop_line("s3.b1"));
    }
}
