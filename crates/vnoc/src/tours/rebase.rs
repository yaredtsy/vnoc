//! V-NOC's own edit of a file must not make a tour stale (`docs/26-tours/01-on-disk.md` §7).
//!
//! Every writer of an ID marker goes through `identity::attach::apply_edits`, which knows the file before the edit and after it. It calls [`rebase`]:
//! each tour with a stop in that file gets one `rebase` record holding, for every stop and block that was **fresh against `before`**, the lines it
//! has in `after` and what they hash to. A stop that was changed before stays as it is, so a rebase never hides an edit someone else made.
//!
//! Where the defs are comes from the driver's outline of `before` and of `after`, not from the index: a process that writes two markers into one
//! file (`vnoc tour ids`, `vnoc repair`) has an index from before the first, and the ranges of the second would be a line off. Where a **block** is
//! in `after` comes from the edits themselves ([`Shift`]): a block has no other anchor, and the one-line docstring that becomes three has no line
//! of the block's old text left to slide to. The def's own range is checked against the driver's: a mapping that disagrees leaves the stop alone.
//!
//! Lock order: the caller holds the project's write lock; this takes the tours lock after it, never the other way round.

use std::collections::BTreeMap;
use std::path::Path;

use vnoc_domain::RepoPath;
use vnoc_driver_protocol::{Def, DefKind, OutlineResult, Range, TextEdit};

use super::check::{self, BlockState, StopState};
use super::find::{find, place_of};
use super::fold::{Tour, TourBlock, TourStop};
use super::hash;
use super::record::{Lines, Ordered, RebaseRecord, Rebased, Record, StepId, StopId, TourId};
use super::store::{self, Located, StoreError};
use crate::driver_host::{DriverHost, HostError};
use crate::index::{Index, Kind, Node};
use crate::project::Project;
use crate::source;
use crate::userdata::ToursLock;

#[derive(Debug, thiserror::Error)]
pub enum RebaseError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the driver could not read the file: {0}")]
    Driver(#[from] HostError),
}

// ---------------------------------------------------------------------------------------------------------------------- shifting

/// Where a line of the text before an edit is in the text after it. The edits are the driver's plan: all against the text before, none overlapping.
/// A line is `None` when an edit replaces text that spans it, which a marker never does: the caller leaves what it cannot place.
pub struct Shift<'a> {
    edits: &'a [TextEdit],
    /// the text before, split as `source.rs` splits it
    lines: Vec<&'a str>,
}

impl<'a> Shift<'a> {
    pub fn new(before: &'a str, edits: &'a [TextEdit]) -> Shift<'a> {
        Shift {
            edits,
            lines: source::split_lines(before.strip_prefix('\u{feff}').unwrap_or(before)),
        }
    }

    fn newlines(edit: &TextEdit) -> i64 {
        edit.new_text.matches('\n').count() as i64
    }

    /// How many lines the edit adds (a replacement across lines takes its own away).
    fn delta(edit: &TextEdit) -> i64 {
        Self::newlines(edit) - i64::from(edit.range.end.line - edit.range.start.line)
    }

    /// Whether `character` is at or past the end of a line of the text before (UTF-16 units, as the driver counts).
    fn at_end_of(&self, line: u32, character: u32) -> bool {
        let length: usize = self
            .lines
            .get(line as usize)
            .map_or(0, |text| text.encode_utf16().count());
        character as usize >= length
    }

    /// The line (0-based) where a block that begins with line `line` begins after the edits: what was inserted at the very start of the line is
    /// before the block, and what was inserted further along the line is inside it.
    pub fn first(&self, line: u32) -> Option<u32> {
        let mut at = i64::from(line);
        for edit in self.edits {
            let (from, to) = (edit.range.start, edit.range.end);
            if to.line < line {
                at += Self::delta(edit);
            } else if from.line > line {
                // below the block's first line: not its business
            } else if from.line == line && from.character > 0 {
                // the line's own start is before the edit
            } else if from == to && from.line == line {
                at += Self::newlines(edit); // inserted at the start of the line: the line moves down
            } else {
                return None;
            }
        }
        u32::try_from(at).ok()
    }

    /// The line (0-based) where a block that ends with line `line` ends after the edits: what was inserted at the end of that line is after the
    /// block, and what was inserted before the end of it moves the end down.
    pub fn last(&self, line: u32) -> Option<u32> {
        let mut at = i64::from(line);
        for edit in self.edits {
            let (from, to) = (edit.range.start, edit.range.end);
            if to.line < line {
                at += Self::delta(edit);
            } else if from.line > line {
                // after the block
            } else if from == to && from.line == line {
                if !self.at_end_of(line, from.character) {
                    at += Self::newlines(edit);
                }
            } else if to.line == line {
                at += Self::delta(edit); // a replacement that ends on this line: the rest of the line follows it
            } else {
                return None;
            }
        }
        u32::try_from(at).ok()
    }

    /// Lines `first..=last` (1-based) of the text before, as lines of the text after.
    pub fn lines(&self, first: u32, last: u32) -> Option<(u32, u32)> {
        let a = self.first(first.checked_sub(1)?)? + 1;
        let b = self.last(last.checked_sub(1)?)? + 1;
        (a <= b).then_some((a, b))
    }
}

// ---------------------------------------------------------------------------------------------------------------------- the plan

/// A stop's node, by name, in the file: `qualname` is `""` for the file itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub qualname: String,
    pub occurrence: u32,
}

/// One edit of one file, as the writer knows it.
pub struct Edited<'a> {
    /// the file's repo path
    pub path: &'a RepoPath,
    /// the text the plan was made from, and that text with the planned edits applied
    pub before: &'a str,
    pub after: &'a str,
    pub edits: &'a [TextEdit],
}

/// The n-th def (1-based) of this name, in source order.
fn def_named<'a>(outline: &'a OutlineResult, place: &Place) -> Option<&'a Def> {
    outline
        .defs
        .iter()
        .filter(|def| def.qualname == place.qualname)
        .nth(place.occurrence.checked_sub(1)? as usize)
}

fn within(inner: &Range, outer: &Range) -> bool {
    let at = |p: vnoc_driver_protocol::Position| (p.line, p.character);
    at(outer.start) <= at(inner.start) && at(inner.end) <= at(outer.end)
}

/// The ID texts a line of this node's code may be a marker by: its own and those of the defs inside it, and for a file the file's.
fn marker_ids(outline: &OutlineResult, def: Option<&Def>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    match def {
        None => {
            ids.extend(outline.file_id.clone());
            ids.extend(outline.defs.iter().filter_map(|d| d.id.clone()));
        }
        Some(def) => ids.extend(
            outline
                .defs
                .iter()
                .filter(|d| within(&d.range, &def.range))
                .filter_map(|d| d.id.clone()),
        ),
    }
    ids
}

/// The index's kind of a def, for `compare`.
fn kind_of(def: Option<&Def>) -> Kind {
    match def.map(|d| d.kind) {
        None => Kind::File,
        Some(DefKind::Class) => Kind::Class,
        Some(DefKind::Function) => Kind::Function,
    }
}

/// A node with only what `compare` reads: its kind and its range.
fn shell(def: Option<&Def>) -> Node {
    Node {
        id: String::new(),
        kind: kind_of(def),
        name: String::new(),
        path: String::new(),
        qualname: def.map(|d| d.qualname.clone()),
        range: def.map(|d| d.range),
        syntax_error: false,
        node_id: None,
        docs: 0,
        link: None,
        children: Vec::new(),
    }
}

fn blocks_of<'a>(blocks: &'a [TourBlock], out: &mut BTreeMap<StepId, &'a TourBlock>) {
    for block in blocks {
        out.insert(block.id, block);
        blocks_of(&block.details, out);
    }
}

/// `lines` of `text` without a marker line at either end: a marker put at the first line of a block (`"""ID: …"""` goes in before the statement the block
/// begins with) is not the block's. What the hash leaves out, a block does not start or end on, as `check` slides it (`Lens::runs`).
fn without_marker_ends(text: &str, lines: (u32, u32), ids: &[&str]) -> (u32, u32) {
    let all = source::split_lines(text.strip_prefix('\u{feff}').unwrap_or(text));
    let is_marker = |n: u32| {
        all.get(n as usize - 1)
            .is_some_and(|line| ids.iter().any(|id| !id.is_empty() && line.contains(id)))
    };
    let (mut first, mut last) = lines;
    while first < last && is_marker(first) {
        first += 1;
    }
    while last > first && is_marker(last) {
        last -= 1;
    }
    (first, last)
}

fn hash_of(text: &str, lines: (u32, u32), ids: &[&str]) -> hash::Hash {
    hash::of(&hash::lines_of(text, lines.0, lines.1), ids)
}

/// What one stop gets, if anything: `None` when it is not fresh against `before`, or cannot be placed in `after`.
fn plan_stop(
    stop: &TourStop,
    place: &Place,
    edited: &Edited,
    outlines: (&OutlineResult, &OutlineResult),
    shift: &Shift,
) -> Option<Vec<(StepId, Rebased)>> {
    let (before_outline, after_outline) = outlines;
    let is_file = place.qualname.is_empty();
    let before_def = (!is_file)
        .then(|| def_named(before_outline, place))
        .flatten();
    let after_def = (!is_file)
        .then(|| def_named(after_outline, place))
        .flatten();
    if !is_file && (before_def.is_none() || after_def.is_none()) {
        return None;
    }
    let ids_before = marker_ids(before_outline, before_def);
    let ids_before: Vec<&str> = ids_before.iter().map(String::as_str).collect();
    let ids_after = marker_ids(after_outline, after_def);
    let ids_after: Vec<&str> = ids_after.iter().map(String::as_str).collect();

    // against `before`: only what is fresh moves on
    let result = check::compare(stop, &shell(before_def), edited.before, &ids_before);
    if result.state != StopState::Fresh {
        return None;
    }

    let mut entries: Vec<(StepId, Rebased)> = Vec::new();
    // the stop itself: a file has no hash and keeps its lines (`1` to the last, as it was written)
    if let (Some(before_def), Some(after_def)) = (before_def, after_def) {
        let was = (
            before_def.range.start.line + 1,
            before_def.range.end.line + 1,
        );
        let now = (after_def.range.start.line + 1, after_def.range.end.line + 1);
        if shift.lines(was.0, was.1) != Some(now) {
            tracing::warn!(
                "tour rebase: the edit moves {} to a place the driver does not give; its stops are left as they are",
                place.qualname
            );
            return None;
        }
        let lines = Lines::new(now.0, now.1)?;
        let hash = hash_of(edited.after, now, &ids_after);
        if stop.lines != Some(lines) || stop.hash != Some(hash) {
            entries.push((
                StepId::intro(stop.id),
                Rebased {
                    lines,
                    hash: Some(hash),
                },
            ));
        }
    }

    let mut written = BTreeMap::new();
    blocks_of(&stop.blocks, &mut written);
    for block in result.blocks {
        let Some(old) = written.get(&block.id) else {
            continue;
        };
        if matches!(block.state, BlockState::Changed) {
            continue;
        }
        // where it was in `before` (found there: in place, or slid), then where the edits put that
        let Some(now) = shift
            .lines(block.now.0, block.now.1)
            .map(|lines| without_marker_ends(edited.after, lines, &ids_after))
        else {
            continue;
        };
        let Some(lines) = Lines::new(now.0, now.1) else {
            continue;
        };
        let hash = hash_of(edited.after, now, &ids_after);
        if old.lines != lines || old.hash != hash {
            entries.push((
                block.id,
                Rebased {
                    lines,
                    hash: Some(hash),
                },
            ));
        }
    }
    Some(entries)
}

/// The `rebase` record of one tour for this edit, or `None` when nothing changed place or hash. `places` are the tour's stops that are in the file.
pub fn plan(
    tour: &Tour,
    places: &BTreeMap<StopId, Place>,
    edited: &Edited,
    outlines: (&OutlineResult, &OutlineResult),
) -> Option<RebaseRecord> {
    let shift = Shift::new(edited.before, edited.edits);
    let mut steps: Vec<(StepId, Rebased)> = Vec::new();
    for stop in tour.stops.iter().filter(|s| s.written) {
        if let Some(place) = places.get(&stop.id)
            && let Some(entries) = plan_stop(stop, place, edited, outlines, &shift)
        {
            steps.extend(entries);
        }
    }
    (!steps.is_empty()).then_some(RebaseRecord {
        steps: Ordered(steps),
    })
}

// ---------------------------------------------------------------------------------------------------------------------- writing

/// The stops of each readable tour that are in `path` now: found as `tours::find` finds them (by ID, else by name), whatever path they were written
/// with. A tour in both folders is not touched (01 §1: nothing is merged).
fn candidates(
    vnoc_root: &Path,
    project: &Project,
    index: &Index,
    path: &RepoPath,
) -> Vec<(TourId, BTreeMap<StopId, Place>)> {
    let listed = store::list(vnoc_root);
    let mut tree: Option<Node> = None;
    let mut found = Vec::new();
    for row in &listed.tours {
        let Ok(tour) = &row.tour else { continue };
        if !matches!(
            store::locate(vnoc_root, &row.id),
            Located::Private(_) | Located::Project(_)
        ) {
            continue;
        }
        // a stop with no ID is found by its name, in its own file: no tree is built for a tour that cannot be in this one
        let could = |s: &&TourStop| s.written && (s.node.is_some() || s.path == *path);
        if !tour.stops.iter().any(|s| could(&s)) {
            continue;
        }
        let tree = tree.get_or_insert_with(|| index.tree(&project.name(), project.prefix()));
        let mut places = BTreeMap::new();
        for stop in tour.stops.iter().filter(could) {
            if let Some(node) = find(index, project, tree, stop)
                && node.kind != Kind::Folder
                && let Some((at, qualname, occurrence)) = place_of(node)
                && at == *path
            {
                places.insert(
                    stop.id,
                    Place {
                        qualname,
                        occurrence,
                    },
                );
            }
        }
        if !places.is_empty() {
            found.push((row.id.clone(), places));
        }
    }
    found
}

/// After V-NOC edited `edited.path`: writes one `rebase` record to each tour that has a stop in it and something to move. Returns how many tours it
/// wrote to. The caller holds the project's write lock. A tour with nothing in the file costs no driver call.
pub async fn rebase(
    vnoc_root: &Path,
    project: &Project,
    index: &std::sync::RwLock<Index>,
    host: &DriverHost,
    edited: &Edited<'_>,
) -> Result<usize, RebaseError> {
    let candidates = {
        let index = index.read().expect("the index lock is not poisoned");
        candidates(vnoc_root, project, &index, edited.path)
    };
    if candidates.is_empty() {
        return Ok(0);
    }
    let file = edited.path.as_str();
    let before_outline = host.outline_text(file, edited.before).await?;
    let after_outline = host.outline_text(file, edited.after).await?;

    let lock = ToursLock::take(vnoc_root).map_err(StoreError::from)?;
    let mut wrote = 0;
    let mut first_error = None;
    for (id, places) in candidates {
        // read again under the lock: another writer may have added a stop since
        let tour = match store::locate(vnoc_root, &id) {
            Located::Private(_) | Located::Project(_) => match store::read(vnoc_root, &id) {
                Ok((_, Ok(tour))) => tour,
                _ => continue,
            },
            _ => continue,
        };
        let Some(record) = plan(&tour, &places, edited, (&before_outline, &after_outline)) else {
            continue;
        };
        match store::append_locked(&lock, vnoc_root, &id, &[Record::Rebase(record)]) {
            Ok(()) => wrote += 1,
            Err(e) => first_error = first_error.or(Some(e)),
        }
    }
    match first_error {
        Some(e) => Err(e.into()),
        None => Ok(wrote),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::Position;

    fn insert(line: u32, character: u32, text: &str) -> TextEdit {
        let at = Position { line, character };
        TextEdit {
            range: Range { start: at, end: at },
            new_text: text.into(),
        }
    }

    fn replace(from: (u32, u32), to: (u32, u32), text: &str) -> TextEdit {
        let at = |(line, character)| Position { line, character };
        TextEdit {
            range: Range {
                start: at(from),
                end: at(to),
            },
            new_text: text.into(),
        }
    }

    /// `(first, last)` 1-based, before → after, for each case.
    #[test]
    fn lines_move_with_the_edits_that_add_lines() {
        // 1 `def f():`  2 `    """Charge once."""`  3 `    a = 1`  4 `    return a`  5 `# tail`
        let before = "def f():\n    \"\"\"Charge once.\"\"\"\n    a = 1\n    return a\n# tail\n";
        // a one-line docstring becomes three lines: the text goes in before the closing quotes
        let docstring = [insert(1, 19, "\n\n    ID: X\n    ")];
        // no docstring: a line goes in after the def line
        let line_in = [insert(1, 0, "    \"\"\"ID: X\"\"\"\n")];
        // a comment at the very top of the file (`# FileID:`)
        let top = [insert(0, 0, "# FileID: X\n")];
        // appended after the last character of a line
        let at_end = [insert(3, 12, "\n    # ID: X")];
        // (what, edits, [(before first, last, after first, last)])
        type Moves = Vec<((u32, u32), (u32, u32))>;
        let cases: Vec<(&str, &[TextEdit], Moves)> = vec![
            (
                "docstring rewritten into more lines",
                &docstring,
                vec![
                    ((1, 4), (1, 7)), // the whole function: its end moves 3
                    ((2, 2), (2, 5)), // the docstring line: it ends 3 lower, begins where it did
                    ((3, 4), (6, 7)), // the body, below the edit: both move 3
                    ((1, 2), (1, 5)), // the def line and the docstring
                    ((5, 5), (8, 8)), // below
                ],
            ),
            (
                "a line inserted after the def line",
                &line_in,
                vec![
                    ((1, 4), (1, 5)),
                    ((2, 3), (3, 4)),
                    ((1, 1), (1, 1)), // the def line is above the edit
                    ((2, 2), (3, 3)),
                    ((5, 5), (6, 6)),
                ],
            ),
            (
                "a comment at the top of the file",
                &top,
                vec![((1, 4), (2, 5)), ((1, 1), (2, 2)), ((5, 5), (6, 6))],
            ),
            (
                "appended after the end of a line",
                &at_end,
                vec![
                    ((3, 4), (3, 4)), // the block ends at the line the marker follows: the marker is after it
                    ((1, 4), (1, 4)),
                    ((1, 5), (1, 6)), // a block that holds the line after it holds the marker
                    ((5, 5), (6, 6)),
                ],
            ),
        ];
        for (what, edits, table) in cases {
            let shift = Shift::new(before, edits);
            for ((a, b), want) in table {
                assert_eq!(shift.lines(a, b), Some(want), "{what}: {a}-{b}");
            }
        }
        // the text the edits make is what the numbers say
        let after = crate::identity::edits::apply_text_edits(before, &docstring).unwrap();
        assert_eq!(after.lines().nth(5), Some("    a = 1"));
    }

    #[test]
    fn a_replacement_across_a_block_is_not_placed() {
        let before = "a\nb\nc\nd\n";
        let edits = [replace((1, 0), (2, 1), "x")];
        let shift = Shift::new(before, &edits);
        assert_eq!(shift.lines(1, 1), Some((1, 1)));
        assert_eq!(shift.lines(2, 2), None, "inside the replaced text");
        assert_eq!(shift.lines(4, 4), Some((3, 3)), "below it, one line less");
        assert_eq!(shift.lines(1, 3), Some((1, 2)));
    }
}
