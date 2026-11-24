//! Is a tour still about the code it was written about? (`docs/26-tours/03-core.md` §2, §3; `01-on-disk.md` §9.)
//!
//! [`compare`] is one stop against its node as it is now: the stop's hash, then each block where it was, else slid inside its parent's lines.
//! [`check`] finds every written stop ([`super::find`]), reads each file once and compares; its answer is a [`Recorded`], which is also what the
//! state file keeps. [`recall`] gives the recorded answer back when it still holds, with `stat`s and no read of source, **and without an index**:
//! a command asks it first for every tour and builds the index only for the tours that need a real check.
//!
//! Left out of every hash, here as in `hash.rs`: a line that holds an ID marker. A block may therefore be *moved* by one line (an `ID:` line was
//! added above it) and be fresh.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use vnoc_domain::RepoPath;

use super::add::marker_ids;
use super::find::{find, place_of};
use super::fold::{Tour, TourBlock, TourStop};
use super::hash;
use super::record::{Lines, Ordered, StepId, StopId, TourId, WhereAt};
use super::store::{self, Part, StoreError};
use super::time;
use crate::index::{Index, Kind, Node};
use crate::project::Project;
use crate::source;

// ------------------------------------------------------------------------------------------------------------------- compare

/// What a block is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    /// its lines are where they were, counted from the node's first line
    Fresh,
    /// the same lines, somewhere else in its parent's lines: fresh, at these file lines
    Moved(Lines),
    /// not found: shown at its old offset, cut to the node's end
    Changed,
}

/// What a stop is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopState {
    Fresh,
    Changed,
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCheck {
    pub id: StepId,
    pub state: BlockState,
    /// the block's lines now: where it was found, or where it was (counted from the node's first line, cut to the node's end) when it was not
    pub now: (u32, u32),
}

/// One stop against its node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopCheck {
    /// `Changed` when the intro or any block is; `Fresh` otherwise (a moved block is fresh). `Gone` is [`StopCheck::gone`].
    pub state: StopState,
    /// the stop's own hash no longer matches (never for a file or a folder: they have none)
    pub intro_changed: bool,
    /// every block, in reading order
    pub blocks: Vec<BlockCheck>,
}

impl StopCheck {
    pub fn gone() -> StopCheck {
        StopCheck {
            state: StopState::Gone,
            intro_changed: false,
            blocks: Vec::new(),
        }
    }
}

/// A file's lines, split as `source.rs` splits them.
fn file_lines(text: &str) -> Vec<&str> {
    source::split_lines(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// A node's lines now, 1-based: a def's from its range, a file's `1` to its last line.
fn node_range(node: &Node, total: u32) -> (u32, u32) {
    match node.range {
        Some(range) => (range.start.line + 1, range.end.line + 1),
        None => (1, total.max(1)),
    }
}

struct Lens<'a> {
    all: Vec<&'a str>,
    markers: &'a [&'a str],
}

impl Lens<'_> {
    fn is_marker(&self, line: &str) -> bool {
        self.markers
            .iter()
            .any(|id| !id.is_empty() && line.contains(id))
    }

    /// The hash of lines `first..=last` (1-based), markers left out; the lines that are not there are not counted.
    fn hash(&self, first: u32, last: u32) -> hash::Hash {
        let from = (first.max(1) as usize - 1).min(self.all.len());
        let to = (last as usize).min(self.all.len()).max(from);
        hash::of(&self.all[from..to], self.markers)
    }

    /// Every run of `count` lines that are not markers (a run starts and ends on one; marker lines between are inside it and count for nothing)
    /// inside lines `from..=to`, with the block's hash: their file lines.
    fn runs(&self, from: u32, to: u32, count: usize, want: &[hash::Hash]) -> BTreeSet<(u32, u32)> {
        let kept: Vec<u32> = (from.max(1)..=to.min(self.all.len() as u32))
            .filter(|n| !self.is_marker(self.all[*n as usize - 1]))
            .collect();
        let mut found = BTreeSet::new();
        if count == 0 || kept.len() < count {
            return found;
        }
        for i in 0..=kept.len() - count {
            let (a, b) = (kept[i], kept[i + count - 1]);
            if want.contains(&self.hash(a, b)) {
                found.insert((a, b));
            }
        }
        found
    }
}

/// Compares one stop with the node it was found at: `node`'s lines now, `file_text` the file as it is on disk, `marker_ids` the ID texts a line may be
/// a marker by (`add::marker_ids`). 03 §2, steps 1 to 4.
///
/// A block is fresh when its lines, counted from the node's first line as they were counted from the stop's, have its hash. Else it is moved when
/// there is **exactly one** run of its length inside its parent block's lines now (the node's for a top block) with its hash: two equal runs are
/// not guessed between. Else it is changed. A detail is judged on its own hash: a fresh detail can sit in a changed parent.
pub fn compare(stop: &TourStop, node: &Node, file_text: &str, marker_ids: &[&str]) -> StopCheck {
    let lens = Lens {
        all: file_lines(file_text),
        markers: marker_ids,
    };
    let total = lens.all.len() as u32;
    let (first, last) = node_range(node, total);
    let intro_changed = match (&stop.hash, node.kind) {
        (Some(want), Kind::Function | Kind::Class) => {
            let now = lens.hash(first, last);
            now != *want && Some(now) != stop.earlier_hash
        }
        _ => false,
    };
    let mut blocks = Vec::new();
    // how many marker lines a block's lines may have held when it was written: each ID has one line
    let marker_slack = marker_ids.iter().filter(|id| !id.is_empty()).count();
    if let Some(written) = stop.lines {
        walk(
            &stop.blocks,
            (first, last),
            &lens,
            (written.first, first, last),
            marker_slack,
            &mut blocks,
        );
    } else {
        // a written stop with no lines (a hand edit): its blocks cannot be placed
        flatten(&stop.blocks, &mut |b| {
            blocks.push(BlockCheck {
                id: b.id,
                state: BlockState::Changed,
                now: (b.lines.first, b.lines.last),
            });
        });
    }
    let changed = intro_changed || blocks.iter().any(|b| b.state == BlockState::Changed);
    StopCheck {
        state: if changed {
            StopState::Changed
        } else {
            StopState::Fresh
        },
        intro_changed,
        blocks,
    }
}

/// A block's lines hash to its hash, or to the one its last `rebase` replaced (the code before V-NOC's own edit: not saved yet, or undone).
fn accepts(block: &TourBlock, now: hash::Hash) -> bool {
    now == block.hash || Some(now) == block.earlier_hash
}

fn flatten<'a>(blocks: &'a [TourBlock], visit: &mut dyn FnMut(&'a TourBlock)) {
    for block in blocks {
        visit(block);
        flatten(&block.details, visit);
    }
}

/// `region` is the lines blocks at this level must be inside now; `at` is (the stop's first line as written, the node's first line now, its last).
fn walk(
    blocks: &[TourBlock],
    region: (u32, u32),
    lens: &Lens,
    at: (u32, u32, u32),
    slack: usize,
    out: &mut Vec<BlockCheck>,
) {
    let (written_first, node_first, node_last) = at;
    for block in blocks {
        let length = block.lines.last - block.lines.first + 1;
        let offset = block.lines.first.saturating_sub(written_first);
        let base = node_first + offset;
        let end = base + length - 1;
        let in_place = block.lines.first >= written_first
            && end <= node_last
            && end <= lens.all.len() as u32
            && accepts(block, lens.hash(base, end));
        let (state, now) = if in_place {
            (BlockState::Fresh, (base, end))
        } else {
            let count = length as usize;
            let want: Vec<hash::Hash> = std::iter::once(block.hash)
                .chain(block.earlier_hash)
                .collect();
            let mut found = BTreeSet::new();
            for k in (count.saturating_sub(slack).max(1)..=count).rev() {
                found.extend(lens.runs(region.0, region.1, k, &want));
            }
            match (found.len(), found.first()) {
                (1, Some(&(a, b))) => (
                    BlockState::Moved(Lines::new(a, b).expect("a run is a range of lines")),
                    (a, b),
                ),
                // shown at its old offset, cut to the node's end
                _ => (BlockState::Changed, (base, end.min(node_last))),
            }
        };
        out.push(BlockCheck {
            id: block.id,
            state,
            now,
        });
        walk(&block.details, now, lens, at, slack, out);
    }
}

// ------------------------------------------------------------------------------------------------------------------ the record

/// `01-on-disk.md` §9's `check` part: what the last check found, and what it was true of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recorded {
    /// the tour file's length when it was checked
    pub tour_len: u64,
    pub at: String,
    /// every file it looked at, by repo path: `[size, changed-time in ns]`, or `null` when it was not there
    pub files: BTreeMap<String, Option<[u64; 2]>>,
    /// only what was not fresh: a stop not named is fresh
    pub stops: BTreeMap<StopId, StopState>,
    pub blocks: BTreeMap<StepId, StopState>,
    /// blocks found at other lines
    pub moved: BTreeMap<StepId, [u32; 2]>,
    /// where a stop that is *changed* is now, by what the check found: its name and path as they are, which may not be what it was written with
    /// (found by its ID after a rename or a move), and its lines. An addition to 01 §9's record, read as empty when it is not there.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub now: BTreeMap<StopId, Now>,
}

/// A changed stop's node as it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Now {
    /// `""` for a file
    pub qualname: String,
    pub path: String,
    /// a def's lines; none for a file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<[u32; 2]>,
}

impl Recorded {
    /// The answer for a tour with no written stop: nothing to look at, fresh, true of a tour file of this length.
    pub fn empty(tour_len: u64) -> Recorded {
        Recorded {
            tour_len,
            at: time::now(),
            files: BTreeMap::new(),
            stops: BTreeMap::new(),
            blocks: BTreeMap::new(),
            moved: BTreeMap::new(),
            now: BTreeMap::new(),
        }
    }

    /// The tour's word: fresh when no stop is changed or gone.
    pub fn fresh(&self) -> bool {
        self.stops.is_empty()
    }

    pub fn stop(&self, id: StopId) -> StopState {
        self.stops.get(&id).copied().unwrap_or(StopState::Fresh)
    }

    pub fn block(&self, id: StepId) -> BlockState {
        if self.blocks.contains_key(&id) {
            BlockState::Changed
        } else if let Some([a, b]) = self.moved.get(&id).copied() {
            Lines::new(a, b).map_or(BlockState::Changed, BlockState::Moved)
        } else {
            BlockState::Fresh
        }
    }

    /// How many stops are changed, and how many are gone.
    pub fn counts(&self) -> (usize, usize) {
        let gone = self
            .stops
            .values()
            .filter(|s| **s == StopState::Gone)
            .count();
        (self.stops.len() - gone, gone)
    }
}

/// A file's size and changed-time in ns: `None` when it is not there (or cannot be read: it is then not the file that was recorded).
fn stat(path: &Path) -> Option<[u64; 2]> {
    let meta = fs::metadata(path).ok()?;
    let nanos = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
    Some([meta.len(), nanos])
}

/// Whether the record still holds (03 §3, rung 2): the tour's file has the recorded length, and every recorded file has the recorded size and
/// changed-time (one recorded as not there is still not there). One `stat` each; nothing is read.
pub fn valid(recorded: &Recorded, tour_path: &Path, project_root: &Path) -> bool {
    let len = fs::metadata(tour_path).map(|m| m.len()).ok();
    if len != Some(recorded.tour_len) {
        return false;
    }
    recorded
        .files
        .iter()
        .all(|(path, was)| stat(&project_root.join(path)) == *was)
}

/// The recorded check of a tour, when there is one and it is valid: no index, no driver, no source read. `project_root` is the git root.
pub fn recall(
    vnoc_root: &Path,
    project_root: &Path,
    id: &TourId,
    tour_path: &Path,
) -> Option<Recorded> {
    let recorded: Recorded =
        serde_json::from_value(store::read_state(vnoc_root, id).check?).ok()?;
    valid(&recorded, tour_path, project_root).then_some(recorded)
}

// -------------------------------------------------------------------------------------------------------------------- the check

/// The code as it is now: what finding needs.
pub struct Code<'a> {
    pub index: &'a Index,
    pub project: &'a Project,
    /// `index.tree(..)`
    pub tree: &'a Node,
}

/// Reads one file of the project (an absolute path). The tests count its calls.
pub type Reader<'a> = &'a mut dyn FnMut(&Path) -> io::Result<String>;

/// A file as the check has it: what `stat` said just before it was read, and its text (none for a folder, or a file that was not there).
struct Seen {
    stat: Option<[u64; 2]>,
    text: Option<String>,
}

struct Files<'a> {
    root: PathBuf,
    read: Reader<'a>,
    seen: HashMap<RepoPath, Seen>,
}

impl Files<'_> {
    /// Looks at a file once: stat first, then read (a file that changes in between makes the record invalid at the next look, not wrong).
    fn look(&mut self, path: &RepoPath, read_it: bool) -> &Seen {
        let absolute = self.root.join(path.as_str());
        let entry = self.seen.entry(path.clone()).or_insert_with(|| Seen {
            stat: stat(&absolute),
            text: None,
        });
        if read_it && entry.text.is_none() && entry.stat.is_some() && !absolute.is_dir() {
            entry.text = (self.read)(&absolute).ok();
        }
        entry
    }
}

/// A real check of a tour: finds every written stop, reads each file once, compares, and says what it found and what it was true of. Nothing is
/// written (see [`fresh`], which records). `tour_len` is the tour file's length from **before** it was read.
pub fn check(code: &Code, read: Reader, tour: &Tour, tour_len: u64) -> Recorded {
    check_placed(code, read, tour, tour_len).0
}

/// Where the tour's stops and blocks are **now**, by the check that found them: what a pin records as `where` (01 §8) and what its files are
/// (the stops' own, by the path they are found at, which a stop found by its ID may not have been written with). Meaningful for the stops and blocks
/// the check found fresh: a changed one is where the check could put it, a gone one is not here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placed {
    /// every written stop that was found and is not a folder: its path and the lines it has now, in reading order of the route
    pub stops: Vec<(StopId, WhereAt)>,
    /// every block of the stops found, in reading order
    pub blocks: Vec<(StepId, Lines, StopId)>,
}

impl Placed {
    /// The files of the stops: each path once, in the order the route meets them.
    pub fn files(&self) -> Vec<RepoPath> {
        let mut seen = BTreeSet::new();
        self.stops
            .iter()
            .filter(|(_, at)| seen.insert(at.path.clone()))
            .map(|(_, at)| at.path.clone())
            .collect()
    }

    /// The pin's `where`: every stop (by its own id, its intro) and every block, with the path of its stop and its lines now.
    pub fn places(&self) -> Ordered<StepId, WhereAt> {
        let mut out = Vec::new();
        for (stop, at) in &self.stops {
            out.push((StepId::intro(*stop), at.clone()));
            for (id, lines, owner) in &self.blocks {
                if owner == stop {
                    out.push((
                        *id,
                        WhereAt {
                            path: at.path.clone(),
                            lines: *lines,
                        },
                    ));
                }
            }
        }
        Ordered(out)
    }
}

/// [`check`], and where the stops and blocks are now.
pub fn check_placed(code: &Code, read: Reader, tour: &Tour, tour_len: u64) -> (Recorded, Placed) {
    let mut placed = Placed::default();
    let mut files = Files {
        root: code.project.git_root().to_path_buf(),
        read,
        seen: HashMap::new(),
    };
    let mut recorded = Recorded::empty(tour_len);
    for stop in tour.stops.iter().filter(|s| s.written) {
        // a stop names its file even when it is gone: the file coming back must make the record invalid
        files.look(&stop.path, false);
        let result = match find(code.index, code.project, code.tree, stop) {
            None => StopCheck::gone(),
            Some(node) => match place_of(node) {
                Some((path, _, _)) if node.kind != Kind::Folder => {
                    let text = files.look(&path, true).text.clone();
                    match text {
                        Some(text) => {
                            let ids = marker_ids(code.index, node);
                            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
                            let total = file_lines(&text).len() as u32;
                            let (first, last) = node_range(node, total);
                            if let Some(lines) = Lines::new(first, last) {
                                placed.stops.push((
                                    stop.id,
                                    WhereAt {
                                        path: path.clone(),
                                        lines,
                                    },
                                ));
                            }
                            compare(stop, node, &text, &ids)
                        }
                        None => StopCheck::gone(),
                    }
                }
                Some((path, _, _)) => {
                    files.look(&path, false);
                    StopCheck {
                        state: StopState::Fresh,
                        intro_changed: false,
                        blocks: Vec::new(),
                    }
                }
                None => StopCheck::gone(),
            },
        };
        if result.state != StopState::Fresh {
            recorded.stops.insert(stop.id, result.state);
        }
        if result.state == StopState::Changed
            && let Some(node) = find(code.index, code.project, code.tree, stop)
            && let Some((path, qualname, _)) = place_of(node)
        {
            recorded.now.insert(
                stop.id,
                Now {
                    qualname,
                    path: path.to_string(),
                    lines: node.range.map(|r| [r.start.line + 1, r.end.line + 1]),
                },
            );
        }
        for block in result.blocks {
            if let Some(lines) = Lines::new(block.now.0, block.now.1) {
                placed.blocks.push((block.id, lines, stop.id));
            }
            match block.state {
                BlockState::Fresh => {}
                BlockState::Moved(lines) => {
                    recorded.moved.insert(block.id, [lines.first, lines.last]);
                }
                BlockState::Changed => {
                    recorded.blocks.insert(block.id, StopState::Changed);
                }
            }
        }
    }
    recorded.files = files
        .seen
        .into_iter()
        .map(|(path, seen)| (path.to_string(), seen.stat))
        .collect();
    (recorded, placed)
}

/// Where an answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// the recorded check, still valid: `stat`s only
    Record,
    /// a check made now, and recorded
    Check,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mode {
    /// check anew whatever is recorded
    pub again: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub from: Source,
    pub recorded: Recorded,
}

/// The tour's freshness for a caller that has the code: the record when it is valid and `again` is not asked, else a check, recorded
/// (`write_state_part`, which keeps the place). A caller that has not built the code yet asks [`recall`] first.
pub fn fresh(
    code: &Code,
    read: Reader,
    vnoc_root: &Path,
    tour: &Tour,
    tour_path: &Path,
    tour_len: u64,
    mode: Mode,
) -> Result<Answer, StoreError> {
    if !mode.again
        && let Some(recorded) = recall(vnoc_root, code.project.git_root(), &tour.id, tour_path)
    {
        return Ok(Answer {
            from: Source::Record,
            recorded,
        });
    }
    let recorded = check(code, read, tour, tour_len);
    record(vnoc_root, &tour.id, &recorded)?;
    Ok(Answer {
        from: Source::Check,
        recorded,
    })
}

/// Writes the answer as the `check` part of the tour's state file.
pub fn record(vnoc_root: &Path, id: &TourId, recorded: &Recorded) -> Result<(), StoreError> {
    let value = serde_json::to_value(recorded).expect("a record is JSON");
    store::write_state_part(vnoc_root, id, Part::Check, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;
    use crate::tours::record::print_line;
    use std::cell::Cell;
    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    const HEAD: &str =
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;
    const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    /// `f` at lines 3-11, as it was written about.
    const ORIGINAL: &str = "# top\n\ndef f(x):\n    a = 1\n    b = 2\n    c = 3\n    d = 4\n    e = 5\n    g = 6\n    h = 7\n    return a\n";

    fn text_hash(text: &str, first: u32, last: u32) -> String {
        hash::of(&hash::lines_of(text, first, last), &[]).to_string()
    }

    /// The tour of one stop on `f` (3-11 of `ORIGINAL`) with blocks `b1` 4-6, its detail `b2` 5-5, and `b3` 8-11, hashed from `ORIGINAL`.
    fn tour_of_f() -> Tour {
        let h = |a, b| text_hash(ORIGINAL, a, b);
        let lines = [
            format!(
                r#"{{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","lines":[3,11],"hash":"{}","text":"t"}}"#,
                h(3, 11)
            ),
            format!(
                r#"{{"type":"step","id":"s1.b1","stop":"s1","lines":[4,6],"hash":"{}","title":"One","text":"t"}}"#,
                h(4, 6)
            ),
            format!(
                r#"{{"type":"step","id":"s1.b2","stop":"s1","parent":"s1.b1","lines":[5,5],"hash":"{}","title":"Two","text":"t"}}"#,
                h(5, 5)
            ),
            format!(
                r#"{{"type":"step","id":"s1.b3","stop":"s1","lines":[8,11],"hash":"{}","title":"Three","text":"t"}}"#,
                h(8, 11)
            ),
        ];
        fold(&format!("{HEAD}\n{}\n", lines.join("\n"))).unwrap()
    }

    fn node(first: u32, last: u32, kind: Kind) -> Node {
        let at = |line| Position { line, character: 0 };
        Node {
            id: "a.py::f".to_string(),
            kind,
            name: "f".to_string(),
            path: "a.py".to_string(),
            qualname: (kind != Kind::File).then(|| "f".to_string()),
            range: (kind != Kind::File).then_some(Range {
                start: at(first - 1),
                end: at(last - 1),
            }),
            syntax_error: false,
            node_id: None,
            docs: 0,
            link: None,
            children: vec![],
        }
    }

    fn states(c: &StopCheck) -> Vec<(String, BlockState)> {
        c.blocks
            .iter()
            .map(|b| (b.id.to_string(), b.state))
            .collect()
    }

    fn moved(a: u32, b: u32) -> BlockState {
        BlockState::Moved(Lines::new(a, b).unwrap())
    }

    /// `f` with these lines changed: 1-based line number → its new text; and lines inserted after a line number.
    fn edited(replace: &[(usize, &str)], insert_after: &[(usize, &str)], above: &str) -> String {
        let mut out = String::from(above);
        for (i, line) in ORIGINAL.lines().enumerate() {
            let n = i + 1;
            match replace.iter().find(|(r, _)| *r == n) {
                Some((_, new)) => out.push_str(&format!("{new}\n")),
                None => out.push_str(&format!("{line}\n")),
            }
            for (_, extra) in insert_after.iter().filter(|(a, _)| *a == n) {
                out.push_str(&format!("{extra}\n"));
            }
        }
        out
    }

    fn compare_f(text: &str, first: u32, last: u32, markers: &[&str]) -> StopCheck {
        let tour = tour_of_f();
        compare(
            &tour.stops[0],
            &node(first, last, Kind::Function),
            text,
            markers,
        )
    }

    #[test]
    fn code_that_did_not_change_is_fresh_everywhere() {
        let c = compare_f(ORIGINAL, 3, 11, &[]);
        assert_eq!(c.state, StopState::Fresh);
        assert!(!c.intro_changed);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), BlockState::Fresh),
                ("s1.b2".to_string(), BlockState::Fresh),
                ("s1.b3".to_string(), BlockState::Fresh)
            ]
        );
    }

    #[test]
    fn a_line_changed_inside_a_detail_changes_the_detail_its_block_and_the_stop_and_not_the_other_block()
     {
        let text = edited(&[(5, "    b = 22")], &[], "");
        let c = compare_f(&text, 3, 11, &[]);
        assert_eq!(c.state, StopState::Changed);
        assert!(c.intro_changed);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), BlockState::Changed),
                ("s1.b2".to_string(), BlockState::Changed),
                ("s1.b3".to_string(), BlockState::Fresh)
            ]
        );
    }

    #[test]
    fn a_detail_can_be_fresh_inside_a_changed_block() {
        // line 4 is in b1 (4-6) and not in b2 (5-5)
        let text = edited(&[(4, "    a = 11")], &[], "");
        let c = compare_f(&text, 3, 11, &[]);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), BlockState::Changed),
                ("s1.b2".to_string(), BlockState::Fresh),
                ("s1.b3".to_string(), BlockState::Fresh)
            ]
        );
        assert_eq!(c.state, StopState::Changed);
    }

    #[test]
    fn a_line_added_above_the_blocks_moves_them_and_the_intro_is_changed_because_the_function_is() {
        let text = edited(&[], &[(3, "    note = 0")], "");
        let c = compare_f(&text, 3, 12, &[]);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), moved(5, 7)),
                ("s1.b2".to_string(), moved(6, 6)),
                ("s1.b3".to_string(), moved(9, 12))
            ]
        );
        assert!(
            c.intro_changed,
            "the function is not the lines the intro was written about"
        );
        assert_eq!(c.state, StopState::Changed);
    }

    #[test]
    fn the_function_moved_down_the_file_is_fresh_in_place_and_not_moved() {
        let text = edited(&[], &[], "import os\nimport sys\n\n\n");
        let c = compare_f(&text, 7, 15, &[]);
        assert_eq!(c.state, StopState::Fresh);
        assert!(c.blocks.iter().all(|b| b.state == BlockState::Fresh));
    }

    #[test]
    fn an_id_line_added_inside_changes_nothing_but_where_the_blocks_are() {
        let text = edited(&[], &[(3, &format!("    \"\"\"ID: {ID}\"\"\""))], "");
        let c = compare_f(&text, 3, 12, &[ID]);
        assert_eq!(
            c.state,
            StopState::Fresh,
            "the marker line is not in the hash"
        );
        assert!(!c.intro_changed);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), moved(5, 7)),
                ("s1.b2".to_string(), moved(6, 6)),
                ("s1.b3".to_string(), moved(9, 12))
            ]
        );
        // without the rule it is changed: the id is not told to the check
        assert_eq!(compare_f(&text, 3, 12, &[]).state, StopState::Changed);
    }

    #[test]
    fn a_block_that_the_marker_line_lands_in_is_found_without_the_marker() {
        // a block of the whole header, 3-6, written before the ID line was added at 4
        let h = text_hash(ORIGINAL, 3, 6);
        let stop = format!(
            r#"{{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","lines":[3,11],"hash":"{}","text":"t"}}
{{"type":"step","id":"s1.b1","stop":"s1","lines":[3,6],"hash":"{h}","title":"One","text":"t"}}"#,
            text_hash(ORIGINAL, 3, 11)
        );
        let tour = fold(&format!("{HEAD}\n{stop}\n")).unwrap();
        let text = edited(&[], &[(3, &format!("    \"\"\"ID: {ID}\"\"\""))], "");
        let c = compare(&tour.stops[0], &node(3, 12, Kind::Function), &text, &[ID]);
        assert_eq!(c.state, StopState::Fresh);
        assert_eq!(states(&c), [("s1.b1".to_string(), moved(3, 7))]);
    }

    #[test]
    fn two_equal_runs_are_not_guessed_between() {
        let original =
            "def g():\n    x = 1\n    x = 1\n    y = 2\n    x = 1\n    x = 1\n    return y\n";
        let h = |a, b| text_hash(original, a, b);
        let lines = format!(
            r#"{{"type":"stop","id":"s1","path":"a.py","qualname":"g","kind":"function","lines":[1,7],"hash":"{}","text":"t"}}
{{"type":"step","id":"s1.b1","stop":"s1","lines":[2,3],"hash":"{}","title":"One","text":"t"}}
{{"type":"step","id":"s1.b2","stop":"s1","lines":[4,4],"hash":"{}","title":"Two","text":"t"}}"#,
            h(1, 7),
            h(2, 3),
            h(4, 4)
        );
        let tour = fold(&format!("{HEAD}\n{lines}\n")).unwrap();
        // untouched: in place
        let same = compare(&tour.stops[0], &node(1, 7, Kind::Function), original, &[]);
        assert_eq!(same.state, StopState::Fresh);
        // a line added at the top: the `x = 1` pair is at 3-4 and 6-7 now, and `y = 2` has one place
        let text = original.replacen("def g():\n", "def g():\n    z = 0\n", 1);
        let c = compare(&tour.stops[0], &node(1, 8, Kind::Function), &text, &[]);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), BlockState::Changed),
                ("s1.b2".to_string(), moved(5, 5))
            ],
            "a block that cannot be placed is changed, not guessed"
        );
    }

    #[test]
    fn a_changed_block_is_cut_to_the_node_s_end_and_so_are_its_details_looked_for_there() {
        // the function shrank: b3 (8-11) is past its end now
        let text = "# top\n\ndef f(x):\n    a = 1\n    b = 2\n    c = 3\n    return a\n";
        let c = compare_f(text, 3, 7, &[]);
        assert_eq!(
            states(&c),
            [
                ("s1.b1".to_string(), BlockState::Fresh),
                ("s1.b2".to_string(), BlockState::Fresh),
                ("s1.b3".to_string(), BlockState::Changed)
            ]
        );
        assert_eq!(c.state, StopState::Changed);
    }

    #[test]
    fn a_file_and_a_folder_intro_is_never_changed_and_a_file_s_blocks_are_hashed() {
        let h = text_hash(ORIGINAL, 4, 6);
        let lines = format!(
            r#"{{"type":"stop","id":"s1","path":"a.py","qualname":"","kind":"file","lines":[1,11],"text":"t"}}
{{"type":"step","id":"s1.b1","stop":"s1","lines":[4,6],"hash":"{h}","title":"One","text":"t"}}
{{"type":"stop","id":"s2","path":"pkg","qualname":"","kind":"folder","text":"t"}}"#
        );
        let tour = fold(&format!("{HEAD}\n{lines}\n")).unwrap();
        let file = |text: &str, total: u32| {
            let mut n = node(1, total, Kind::File);
            n.id = "a.py".to_string();
            compare(&tour.stops[0], &n, text, &[])
        };
        let same = file(ORIGINAL, 11);
        assert_eq!((same.state, same.intro_changed), (StopState::Fresh, false));
        // a byte changed somewhere else in the file: still fresh. Inside the block: changed
        let elsewhere = ORIGINAL.replace("return a", "return b");
        assert_eq!(file(&elsewhere, 11).state, StopState::Fresh);
        let inside = ORIGINAL.replace("b = 2", "b = 3");
        let c = file(&inside, 11);
        assert_eq!((c.state, c.intro_changed), (StopState::Changed, false));
        let folder = compare(&tour.stops[1], &node(1, 1, Kind::Folder), "", &[]);
        assert_eq!(folder.state, StopState::Fresh);
    }

    // ------------------------------------------------------------------------------------------------ the record and the check

    fn def(qualname: &str, from: u32, to: u32) -> Def {
        let at = |line| Position { line, character: 0 };
        Def {
            qualname: qualname.to_string(),
            name: qualname.to_string(),
            kind: DefKind::Function,
            parent: String::new(),
            parent_kind: None,
            range: Range {
                start: at(from),
                end: at(to),
            },
            params: vec![],
            id: None,
            syntax_error: false,
        }
    }

    /// A project folder: `a.py` (`ORIGINAL`), `b.py`, and the index of what is on disk now.
    struct Fixture {
        dir: PathBuf,
        project: Project,
        vnoc_root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Fixture {
            let dir =
                std::env::temp_dir().join(format!("vnoc-check-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join(".vnoc/local/tours")).unwrap();
            fs::write(dir.join("a.py"), ORIGINAL).unwrap();
            fs::write(dir.join("b.py"), "def other():\n    return 1\n").unwrap();
            let dir = dunce::canonicalize(dir).unwrap();
            let project = Project::open(&dir).unwrap();
            let vnoc_root = dir.join(".vnoc");
            Fixture {
                dir,
                project,
                vnoc_root,
            }
        }

        /// The index of the files that are there, outlining `f` of `a.py` by its lines.
        fn index(&self) -> Index {
            let mut files = Vec::new();
            if let Ok(text) = fs::read_to_string(self.dir.join("a.py")) {
                let lines: Vec<&str> = text.lines().collect();
                let at = lines
                    .iter()
                    .position(|l| l.starts_with("def f("))
                    .map(|i| i as u32);
                files.push((
                    RepoPath::new("a.py").unwrap(),
                    at.map(|i| vec![def("f", i, lines.len() as u32 - 1)])
                        .unwrap_or_default(),
                ));
            }
            if self.dir.join("b.py").exists() {
                files.push((RepoPath::new("b.py").unwrap(), vec![def("other", 0, 1)]));
            }
            Index::for_tests(files, vec![], vec![])
        }

        /// Writes the tour file (the first line and `lines`) and returns its folded tour and path.
        fn tour(&self, lines: &[String]) -> (Tour, PathBuf) {
            let path = self.vnoc_root.join("local/tours/t-0000.jsonl");
            let mut text = format!("{HEAD}\n");
            for line in lines {
                text.push_str(line);
                text.push('\n');
            }
            fs::write(&path, &text).unwrap();
            (fold(&text).unwrap(), path)
        }

        fn tour_f(&self) -> (Tour, PathBuf) {
            let mut text = String::new();
            let tour = tour_of_f();
            for stop in &tour.stops {
                text.push_str(&format!(
                    r#"{{"type":"stop","id":"s1","path":"a.py","qualname":"f","kind":"function","lines":[3,11],"hash":"{}","text":"t"}}"#,
                    stop.hash.unwrap()
                ));
                text.push('\n');
                for b in &stop.blocks {
                    text.push_str(&step_line(b, None));
                    text.push('\n');
                    for d in &b.details {
                        text.push_str(&step_line(d, Some(b.id)));
                        text.push('\n');
                    }
                }
            }
            let lines: Vec<String> = text.lines().map(String::from).collect();
            self.tour(&lines)
        }

        /// A check, counting the files it read.
        fn fresh(&self, tour: &Tour, path: &Path, mode: Mode) -> (Answer, usize) {
            let index = self.index();
            let tree = index.tree(&self.project.name(), self.project.prefix());
            let code = Code {
                index: &index,
                project: &self.project,
                tree: &tree,
            };
            let reads = Cell::new(0usize);
            let mut read = |p: &Path| {
                reads.set(reads.get() + 1);
                source::read_text(p)
            };
            let len = fs::metadata(path).unwrap().len();
            let answer = fresh(&code, &mut read, &self.vnoc_root, tour, path, len, mode).unwrap();
            (answer, reads.get())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn step_line(block: &TourBlock, parent: Option<StepId>) -> String {
        print_line(&crate::tours::record::Record::Step(
            crate::tours::record::StepRecord {
                id: block.id,
                stop: block.id.stop(),
                parent,
                lines: block.lines,
                hash: block.hash,
                title: block.title.clone(),
                text: block.text.clone(),
            },
        ))
        .trim_end()
        .to_string()
    }

    const NOW: Mode = Mode { again: false };
    const AGAIN: Mode = Mode { again: true };

    /// Makes a file's size differ, so its recorded size is not its size.
    fn touch(path: &Path) {
        let mut text = fs::read_to_string(path).unwrap();
        text.push_str("# touched\n");
        fs::write(path, text).unwrap();
    }

    #[test]
    fn a_check_reads_each_file_once_and_the_second_ask_reads_none() {
        let f = Fixture::new("twice");
        let (tour, path) = f.tour_f();
        let (first, reads) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            (first.from, reads),
            (Source::Check, 1),
            "one stop, one file, read once"
        );
        assert!(first.recorded.fresh());
        assert_eq!(
            first
                .recorded
                .files
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a.py"]
        );
        let (second, reads) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            (second.from, reads),
            (Source::Record, 0),
            "from the record: stats only"
        );
        assert_eq!(second.recorded, first.recorded);
        // `recall` alone gives it too, with no index and no reader
        assert_eq!(
            recall(&f.vnoc_root, &f.dir, &tour.id, &path),
            Some(first.recorded)
        );
    }

    #[test]
    fn touching_a_file_of_the_tour_reads_again_and_touching_another_file_does_not() {
        let f = Fixture::new("touch");
        let (tour, path) = f.tour_f();
        assert_eq!(f.fresh(&tour, &path, NOW).1, 1);
        touch(&f.dir.join("b.py"));
        let (answer, reads) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            (answer.from, reads),
            (Source::Record, 0),
            "b.py is not in the tour"
        );
        touch(&f.dir.join("a.py"));
        let (answer, reads) = f.fresh(&tour, &path, NOW);
        assert_eq!((answer.from, reads), (Source::Check, 1));
        // the new answer is recorded: the next ask is from the record again
        assert_eq!(f.fresh(&tour, &path, NOW).0.from, Source::Record);
    }

    #[test]
    fn again_reads_whatever_is_recorded_and_a_new_stop_makes_the_record_invalid() {
        let f = Fixture::new("again");
        let (tour, path) = f.tour_f();
        f.fresh(&tour, &path, NOW);
        let (answer, reads) = f.fresh(&tour, &path, AGAIN);
        assert_eq!((answer.from, reads), (Source::Check, 1));
        // a stop is added: the tour file is longer
        let more = r#"{"type":"stop","id":"s2","path":"b.py","qualname":"other","kind":"function","lines":[1,2],"hash":"0000000000000001","text":"t"}"#;
        let mut lines: Vec<String> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .skip(1)
            .map(String::from)
            .collect();
        lines.push(more.to_string());
        let (longer, path) = f.tour(&lines);
        let (answer, reads) = f.fresh(&longer, &path, NOW);
        assert_eq!(
            (answer.from, reads),
            (Source::Check, 2),
            "both files are read now"
        );
        assert_eq!(
            answer.recorded.stop(StopId::new(2).unwrap()),
            StopState::Changed,
            "its hash is made up"
        );
    }

    #[test]
    fn a_deleted_file_is_gone_and_a_file_that_comes_back_makes_the_tour_fresh_again() {
        let f = Fixture::new("gone");
        let (tour, path) = f.tour_f();
        assert!(f.fresh(&tour, &path, NOW).0.recorded.fresh());
        fs::remove_file(f.dir.join("a.py")).unwrap();
        let (answer, _) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            answer.from,
            Source::Check,
            "the file's stat is not the recorded one"
        );
        assert_eq!(
            answer.recorded.stop(StopId::new(1).unwrap()),
            StopState::Gone
        );
        assert_eq!(answer.recorded.files["a.py"], None, "recorded as not there");
        // asked again with the file still gone: the record holds
        assert_eq!(f.fresh(&tour, &path, NOW).0.from, Source::Record);
        // put back: the record is not valid any more, and the check says fresh
        fs::write(f.dir.join("a.py"), ORIGINAL).unwrap();
        let (answer, _) = f.fresh(&tour, &path, NOW);
        assert_eq!(answer.from, Source::Check);
        assert!(answer.recorded.fresh(), "{:?}", answer.recorded);
    }

    #[test]
    fn a_function_renamed_with_no_id_is_gone_and_an_edit_inside_is_changed_with_its_block() {
        let f = Fixture::new("renamed");
        let (tour, path) = f.tour_f();
        fs::write(
            f.dir.join("a.py"),
            ORIGINAL.replace("def f(x)", "def renamed(x)"),
        )
        .unwrap();
        // the index here finds `f` by its `def f(` line, which is gone: the stop is gone
        let (answer, _) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            answer.recorded.stop(StopId::new(1).unwrap()),
            StopState::Gone
        );
        assert_eq!(answer.recorded.counts(), (0, 1));
        fs::write(f.dir.join("a.py"), ORIGINAL.replace("b = 2", "b = 3")).unwrap();
        let (answer, _) = f.fresh(&tour, &path, NOW);
        assert_eq!(
            answer.recorded.stop(StopId::new(1).unwrap()),
            StopState::Changed
        );
        assert_eq!(answer.recorded.counts(), (1, 0));
        let b = |n| StepId::block(StopId::new(1).unwrap(), n).unwrap();
        assert_eq!(answer.recorded.block(b(1)), BlockState::Changed);
        assert_eq!(answer.recorded.block(b(2)), BlockState::Changed);
        assert_eq!(answer.recorded.block(b(3)), BlockState::Fresh);
    }

    #[test]
    fn a_moved_block_is_recorded_with_its_new_lines_and_the_record_round_trips() {
        let f = Fixture::new("moved");
        let (tour, path) = f.tour_f();
        fs::write(f.dir.join("a.py"), edited(&[], &[(3, "    note = 0")], "")).unwrap();
        let (answer, _) = f.fresh(&tour, &path, NOW);
        let b1 = StepId::block(StopId::new(1).unwrap(), 1).unwrap();
        assert_eq!(answer.recorded.block(b1), moved(5, 7));
        let value = serde_json::to_value(&answer.recorded).unwrap();
        assert_eq!(value["moved"]["s1.b1"], serde_json::json!([5, 7]));
        assert_eq!(value["stops"]["s1"], "changed");
        let back: Recorded = serde_json::from_value(value).unwrap();
        assert_eq!(back, answer.recorded);
    }

    #[test]
    fn writing_the_check_keeps_the_place_and_a_record_that_does_not_parse_is_no_record() {
        let f = Fixture::new("state");
        let (tour, path) = f.tour_f();
        store::write_state_part(
            &f.vnoc_root,
            &tour.id,
            Part::Place,
            serde_json::json!({"at": "cover"}),
        )
        .unwrap();
        f.fresh(&tour, &path, NOW);
        let state = store::read_state(&f.vnoc_root, &tour.id);
        assert_eq!(state.place, Some(serde_json::json!({"at": "cover"})));
        assert!(state.check.is_some());
        store::write_state_part(
            &f.vnoc_root,
            &tour.id,
            Part::Check,
            serde_json::json!({"nonsense": true}),
        )
        .unwrap();
        assert_eq!(recall(&f.vnoc_root, &f.dir, &tour.id, &path), None);
        assert_eq!(f.fresh(&tour, &path, NOW).0.from, Source::Check);
    }
}
