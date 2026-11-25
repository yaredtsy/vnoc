//! What `tour list` and `tour show` print, from the replayed tour alone (`docs/26-tours/02-commands.md` §6, `03-core.md` §7).
//!
//! Nothing here looks at the code: no check, no find. A stop whose node is gone prints as it was written, and says nothing yet (that is the next
//! step). The values are structs that serialize (`--json`), and the text is made from the same values, so the two cannot differ.

use serde::{Deserialize, Serialize};
use vnoc_domain::{LocationKey, NodeKind};

use super::check::{Answer, BlockState, Recorded, Source, StopState};
use super::fold::{State, Tour, TourBlock, TourStop, Unreadable, steps_in_order};
use super::on::{self, How};
use super::place::{self, At, Place};
use super::record::{Commit, StepId, TourId};
use super::store::{Row, Where};
use std::collections::BTreeMap;

/// Text under a row is wrapped at this many columns.
const WRAP: usize = 100;

/// `private` or `project`.
pub fn place_word(place: Where) -> &'static str {
    match place {
        Where::Private => "private",
        Where::Project => "project",
    }
}

/// `writing`, `stopping`, `stopped`, `finished`.
pub fn state_word(state: State) -> &'static str {
    match state {
        State::Writing => "writing",
        State::Stopping => "stopping",
        State::Stopped => "stopped",
        State::Finished => "finished",
    }
}

/// `1 stop`, `9 stops`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The icon of a kind in the outline: `ƒ` a function, `◆` a class, `▪` a file, `□` a folder (the design's tour list); `◌` for a planned stop (no kind yet), `?` for a kind
/// this version does not know.
fn icon(kind: Option<&NodeKind>) -> &'static str {
    match kind {
        None => "◌",
        Some(NodeKind::Function) => "ƒ",
        Some(NodeKind::Class) => "◆",
        Some(NodeKind::File) => "▪",
        Some(NodeKind::Folder) => "□",
        _ => "?",
    }
}

/// `ƒ checkout_order   app/checkout.py:11-30`: what a row says of a stop, from the stop's own record.
pub fn stop_headline(stop: &TourStop) -> String {
    let (label, place) = (stop_label(stop), stop_place(stop));
    if label == place {
        // a folder at the top says its name once
        format!("{} {label}", icon(stop.kind.as_ref()))
    } else {
        format!("{} {label}   {place}", icon(stop.kind.as_ref()))
    }
}

/// `7` of a commit, as git's short form.
fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}

/// What the tour says of being pinned: `pinned at 8a1c2e4`, and `(not in this repository)` after it when git does not have the commit (a shallow
/// clone, a teammate who has not fetched it: 01 §8); a tour with no `pin` record that still holds is `not pinned`.
fn pin_words(tour: &Tour, in_repository: bool) -> String {
    match (&tour.pin, tour.pinned()) {
        (Some(pin), true) if !in_repository => format!(
            "pinned at {} (not in this repository)",
            short(pin.commit.as_str())
        ),
        (Some(pin), true) => format!("pinned at {}", short(pin.commit.as_str())),
        _ if matches!(
            tour.state,
            State::Writing | State::Stopping | State::Stopped
        ) =>
        {
            "not pinned (being written)".to_string()
        }
        _ => "not pinned".to_string(),
    }
}

/// How a stop is named in a row: a def's qualname, a file's name, a folder's name with a `/`.
fn stop_label(stop: &TourStop) -> String {
    if !stop.qualname.is_empty() {
        return stop.qualname.clone();
    }
    let path = stop.path.as_str();
    let last = path.rsplit('/').next().unwrap_or(path);
    if stop.kind == Some(NodeKind::Folder) {
        format!("{last}/")
    } else {
        last.to_string()
    }
}

/// Where a stop's lines are: `app/checkout.py:11-30`, the path alone for a folder. The stop's own record, as it was written.
fn stop_place(stop: &TourStop) -> String {
    match (stop.kind.as_ref(), stop.lines) {
        (Some(NodeKind::Folder), _) => format!("{}/", stop.path.as_str().trim_end_matches('/')),
        (_, Some(lines)) if lines.first == lines.last => {
            format!("{}:{}", stop.path, lines.first)
        }
        (_, Some(lines)) => format!("{}:{}-{}", stop.path, lines.first, lines.last),
        (_, None) => stop.path.to_string(),
    }
}

// ----------------------------------------------------------------------------------------------------------------------- show

/// One block of a stop, in reading order.
#[derive(Debug, Clone, Serialize)]
pub struct BlockView {
    pub id: String,
    /// the block this is a detail of
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// 0 at the top of the stop, 1 for a detail
    pub depth: usize,
    pub lines: [u32; 2],
    pub title: String,
    pub text: String,
    /// with a check: `changed` or `moved`; a fresh block says nothing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<&'static str>,
    /// a moved block's lines now
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<[u32; 2]>,
}

/// One step of a stop in reading order, for the stage (03 §8): the intro first, then every block and detail. A page draws these and computes none of it.
#[derive(Debug, Clone, Serialize)]
pub struct StepView {
    pub id: String,
    /// what the stage draws (design 03 §3, 05 §2): `intro`, `block`, `detail`, `short` (a stop under 8 lines: its only step), `contents` (a folder, or a file with
    /// no blocks: no code to light), `same_as` (explained before), `planned` (not written yet). The cover and the end are places, not steps
    pub kind: &'static str,
    /// 0 the intro, 1 a block, 2 a detail
    pub depth: usize,
    /// the block a detail is in
    pub parent: Option<String>,
    pub title: Option<String>,
    /// the intro's text, or the block's (empty for a stop that is planned and not written)
    pub text: String,
    /// the lines now, for a block (moved blocks already moved); none for an intro
    pub lines: Option<[u32; 2]>,
    /// with a check: `fresh` or `changed` (a stop that is gone has every step `changed`); without one, not said
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fresh: Option<&'static str>,
}

/// One stop of the route.
#[derive(Debug, Clone, Serialize)]
pub struct StopView {
    /// the number on screen: the place on the route, from 1
    pub number: usize,
    pub id: String,
    pub depth: usize,
    /// `planned`: on the route and not written yet
    pub planned: bool,
    pub kind: Option<String>,
    pub name: String,
    pub path: String,
    pub qualname: String,
    /// the node's key in the index's tree, `Node::id` as `vnoc/structure` gives it (a path, or `path::qualname#n`), from what the stop wrote, as `on::Map`
    /// finds a node (no code is read): a page can set the selection with it and read the node's code. `null` for a stop a check found gone
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<[u32; 2]>,
    /// the node's ID, when it had one when the stop was written
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// the intro and the blocks, steps in all (a planned stop has the intro only)
    pub steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// for a stop reached by a call (`from` and `line`): the step of the `from` stop whose lines hold that line (the deepest one), else that stop's intro. "Called on
    /// line N" jumps there
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_as: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// with a check: `changed`, `gone`, or `moved` (fresh, with a block at other lines); a fresh stop says nothing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<&'static str>,
    /// with a check, for a stop that is changed: where its node is now (its name and path may not be what it was written with)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now: Option<super::check::Now>,
    /// with `--as-written`, for a stop of a pinned tour that is changed or gone: the code of the pinned commit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_written: Option<AsWritten>,
    pub blocks: Vec<BlockView>,
    /// the stop's steps in reading order, the intro first: what the stage plays
    pub reading: Vec<StepView>,
}

/// A stop's code as the pinned commit has it (01 §8): the stop's lines there, and the lines its blocks light there (the whole of it for a stop with no
/// blocks).
#[derive(Debug, Clone, Serialize)]
pub struct AsWritten {
    /// the first 7 characters of the commit
    pub commit: String,
    pub path: String,
    /// where the stop was at that commit
    pub lines: [u32; 2],
    pub code: Vec<String>,
    /// the lit lines, as ranges of file lines
    pub lit: Vec<[u32; 2]>,
}

impl AsWritten {
    /// `as written at 8a1c2e4: app/inventory.py:12-15`, then every line with its number, the lit ones starting with `>`.
    fn text(&self) -> String {
        let [first, last] = self.lines;
        let place = if first == last {
            format!("{}:{first}", self.path)
        } else {
            format!("{}:{first}-{last}", self.path)
        };
        let mut text = format!("      as written at {}: {place}\n", self.commit);
        let width = last.to_string().len();
        for (n, line) in self.code.iter().enumerate() {
            let number = first + n as u32;
            let lit = self.lit.iter().any(|[a, b]| (*a..=*b).contains(&number));
            let row = format!(
                "      {} {number:>width$}  {line}",
                if lit { '>' } else { ' ' }
            );
            text.push_str(row.trim_end());
            text.push('\n');
        }
        text
    }
}

/// What a check said of the whole tour, and where the answer came from.
#[derive(Debug, Clone, Serialize)]
pub struct CheckInfo {
    pub fresh: bool,
    /// `record` or `check`
    pub from: Source,
    /// when the check was made
    pub at: String,
    pub changed: usize,
    pub gone: usize,
}

/// A tour as `show` has it.
#[derive(Debug, Clone, Serialize)]
pub struct TourView {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub detail: String,
    pub state: &'static str,
    /// `private` or `project`
    pub folder: &'static str,
    pub written: usize,
    pub planned: usize,
    /// the steps of the written stops
    pub steps: usize,
    /// `steps` × 20 seconds, up to a whole minute
    pub minutes: usize,
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// for a pinned tour, once git was asked: whether the pinned commit is in this repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_in_repository: Option<bool>,
    /// with `--check`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckInfo>,
    pub stops: Vec<StopView>,
    /// the planned stop the agent is on: the first stop of the route that is planned and not written, while the tour is being written; else none
    pub writing: Option<String>,
    /// where the reader is, as the tour is now (`place::settle`); the cover for a tour nobody has opened
    pub place: PlaceView,
    /// what the cover of a finished tour may offer, by the settings; `null` when there is nothing to offer, and for a tour that is not finished
    pub offers: Option<OffersView>,
    /// the lines of the file that could not be read, `line 7: why`
    pub problems: Vec<String>,
}

fn blocks_of(blocks: &[TourBlock], parent: Option<StepId>, depth: usize, out: &mut Vec<BlockView>) {
    for block in blocks {
        out.push(BlockView {
            id: block.id.to_string(),
            parent: parent.map(|p| p.to_string()),
            depth,
            lines: [block.lines.first, block.lines.last],
            title: block.title.clone(),
            text: block.text.clone(),
            check: None,
            moved_to: None,
        });
        blocks_of(&block.details, Some(block.id), depth + 1, out);
    }
}

/// A stop's key in the index's tree: its path for a file or a folder, its location key for a def. A planned stop has one too: it is where the plan said it will
/// be, and the stage shows its code while the text is not written (the tree may not have the node: then the key finds nothing).
fn tree_key(stop: &TourStop) -> Option<String> {
    key_of(stop.path.as_str(), &stop.qualname, stop.occurrence)
}

fn key_of(path: &str, qualname: &str, occurrence: u32) -> Option<String> {
    if qualname.is_empty() {
        return Some(path.to_string());
    }
    let path = vnoc_domain::RepoPath::new(path).ok()?;
    LocationKey::new(path, qualname, occurrence)
        .ok()
        .map(|key| key.to_string())
}

/// Where a check found a stop's node: its key follows it. The occurrence is the stop's own when the name did not change, else 1 (a check does not say).
fn key_now(stop: &StopView, now: &super::check::Now) -> Option<String> {
    if now.path == stop.path && now.qualname == stop.qualname {
        return stop.key.clone();
    }
    key_of(&now.path, &now.qualname, 1)
}

/// The step of the `from` stop that holds the line a stop was reached from: the deepest block whose lines (as written, as the line was) contain it, else the intro.
/// `None` for a stop that was not reached by a call, or whose `from` the tour does not have.
fn line_step(tour: &Tour, stop: &TourStop) -> Option<String> {
    let (from, line) = (stop.from?, stop.line?);
    let outer = tour.stop(from)?;
    let mut blocks = Vec::new();
    if outer.written {
        blocks_of(&outer.blocks, None, 0, &mut blocks);
    }
    let deepest = blocks
        .iter()
        .filter(|b| b.lines[0] <= line && line <= b.lines[1])
        .max_by_key(|b| b.depth);
    Some(deepest.map_or_else(|| StepId::intro(from).to_string(), |b| b.id.clone()))
}

/// What a stop's first step is (design 03 §3, as 05 §2 corrects it): not written, explained before, contents (a folder; a file with no blocks), short (a
/// node under [`rules::SHORT`] lines, which is one step), else the intro of a stop that has blocks. A file with blocks is code, like a class.
fn intro_kind(stop: &TourStop) -> &'static str {
    let lines = stop.lines.map(|l| l.last.saturating_sub(l.first) + 1);
    match stop.kind.as_ref() {
        _ if !stop.written => "planned",
        _ if stop.same_as.is_some() => "same_as",
        Some(NodeKind::Folder) => "contents",
        Some(NodeKind::File) if stop.blocks.is_empty() => "contents",
        _ if stop.blocks.is_empty() && lines.is_some_and(|n| n < super::rules::SHORT) => "short",
        _ => "intro",
    }
}

fn stop_view(number: usize, stop: &TourStop) -> StopView {
    let mut blocks = Vec::new();
    if stop.written {
        blocks_of(&stop.blocks, None, 0, &mut blocks);
    }
    let mut reading = vec![StepView {
        id: StepId::intro(stop.id).to_string(),
        kind: intro_kind(stop),
        depth: 0,
        parent: None,
        title: None,
        text: stop.text.clone().unwrap_or_default(),
        lines: None,
        fresh: None,
    }];
    reading.extend(blocks.iter().map(|b| StepView {
        id: b.id.clone(),
        kind: if b.depth == 0 { "block" } else { "detail" },
        depth: b.depth + 1,
        parent: b.parent.clone(),
        title: Some(b.title.clone()),
        text: b.text.clone(),
        lines: Some(b.lines),
        fresh: None,
    }));
    StopView {
        number,
        id: stop.id.to_string(),
        depth: stop.depth,
        planned: !stop.written,
        kind: stop.kind.as_ref().map(|k| k.as_str().to_string()),
        name: stop_label(stop),
        path: stop.path.to_string(),
        qualname: stop.qualname.clone(),
        key: tree_key(stop),
        lines: stop.lines.map(|l| [l.first, l.last]),
        node: stop.node.map(|n| n.to_string()),
        steps: steps_in_order(stop).len(),
        from: stop.from.map(|f| f.to_string()),
        line: stop.line,
        line_step: None,
        same_as: stop.same_as.map(|s| s.to_string()),
        text: stop.text.clone(),
        check: None,
        now: None,
        as_written: None,
        blocks,
        reading,
    }
}

/// The "stop 3 of 10 · 2 of 5" numbers of a place on a step (`place::position`, the code `where` uses): the stop's place on the route and how many stops there
/// are, the step's place in its stop and how many steps it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionView {
    pub stop: usize,
    pub of: usize,
    pub step: usize,
    pub steps: usize,
}

/// A place as the pages get it: the place itself (what the state file keeps), the current stop and the numbers. The state file does not have the last two: they
/// are made from the tour as it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    #[serde(flatten)]
    pub place: Place,
    /// the current stop's id; `null` at the cover and at the end
    pub stop: Option<String>,
    /// `null` at the cover and at the end
    pub position: Option<PositionView>,
    /// how many written stops the reader has not read to their last step (not in `seen`): the end card says "all seen" or "N not seen"
    pub unseen: usize,
    /// the first written stop on the route that is not in `seen`; `null` when every written stop is: the end card's "Go to the first one I skipped"
    pub first_unseen: Option<String>,
}

/// The place for the pages: settled against the tour (a step the tour no longer has moves to the nearest one), with its stop and position.
pub fn place_view(tour: &Tour, place: Place) -> PlaceView {
    let place = place::settle(tour, &place);
    let stop = match place.at {
        At::Step(step) => Some(step.stop().to_string()),
        At::Cover | At::End => None,
    };
    let position = place::position(tour, place.at).map(|p| PositionView {
        stop: p.stop,
        of: p.stops,
        step: p.step,
        steps: p.steps,
    });
    let unseen = tour
        .stops
        .iter()
        .filter(|s| s.written && !place.seen.contains(&s.id))
        .count();
    let first_unseen = tour
        .stops
        .iter()
        .find(|s| s.written && !place.seen.contains(&s.id))
        .map(|s| s.id.to_string());
    PlaceView {
        place,
        stop,
        position,
        unseen,
        first_unseen,
    }
}

/// The tour as `show` prints it, from its replay.
pub fn tour_view(tour: &Tour, place: Where) -> TourView {
    let stops: Vec<StopView> = tour
        .stops
        .iter()
        .enumerate()
        .map(|(i, stop)| {
            let mut view = stop_view(i + 1, stop);
            view.line_step = line_step(tour, stop);
            view
        })
        .collect();
    let size = tour.size();
    let steps = stops.iter().filter(|s| !s.planned).map(|s| s.steps).sum();
    TourView {
        id: tour.id.to_string(),
        title: tour.title.clone(),
        question: tour.question.clone(),
        author: tour.author.clone(),
        detail: serde_json::to_value(tour.detail)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        state: state_word(tour.state),
        folder: place_word(place),
        written: size.written,
        planned: size.planned,
        steps,
        minutes: size.minutes,
        pinned: tour.pinned(),
        pin: tour
            .pin
            .as_ref()
            .filter(|_| tour.pinned())
            .map(|p| p.commit.to_string()),
        pin_in_repository: None,
        check: None,
        writing: (tour.state == State::Writing)
            .then(|| stops.iter().find(|s| s.planned).map(|s| s.id.clone()))
            .flatten(),
        place: place_view(tour, Place::cover()),
        offers: None,
        stops,
        problems: tour
            .problems
            .iter()
            .map(|p| format!("line {}: {}", p.line, p.why))
            .collect(),
    }
}

/// Puts a check's answer on the view: the mark of every stop and block that is not fresh, and what the whole said.
pub fn mark(view: &mut TourView, answer: &Answer) {
    let recorded = &answer.recorded;
    let (changed, gone) = recorded.counts();
    view.check = Some(CheckInfo {
        fresh: recorded.fresh(),
        from: answer.from,
        at: recorded.at.clone(),
        changed,
        gone,
    });
    for stop in &mut view.stops {
        let Ok(id) = stop.id.parse() else { continue };
        let mut any_moved = false;
        for block in &mut stop.blocks {
            let Ok(step) = block.id.parse::<StepId>() else {
                continue;
            };
            match recorded.block(step) {
                BlockState::Fresh => {}
                BlockState::Changed => block.check = Some("changed"),
                BlockState::Moved(lines) => {
                    any_moved = true;
                    block.check = Some("moved");
                    block.moved_to = Some([lines.first, lines.last]);
                }
            }
        }
        stop.now = recorded.now.get(&id).cloned();
        // the key follows the check: a stop found at another name or path (changed, or only moved) is keyed by where it is now
        if let Some(found) = recorded.now.get(&id).or(recorded.displaced.get(&id))
            && stop.key.is_some()
        {
            stop.key = key_now(stop, found);
        }
        if !stop.planned {
            mark_steps(stop, id, recorded);
        }
        if recorded.stop(id) == StopState::Gone {
            stop.key = None;
        }
        stop.check = match recorded.stop(id) {
            StopState::Gone => Some("gone"),
            StopState::Changed => Some("changed"),
            StopState::Fresh if any_moved => Some("moved"),
            StopState::Fresh => None,
        };
    }
}

/// Where a step's lines are now (03 §8): `stop_written` is the stop's own lines as it was written, `found` where a check put the node (a changed stop's `now`,
/// a fresh one's `displaced`), `block` a block's written lines and what the check says of it. A block that is not `moved` is where it was **counted from the
/// node's first line**, cut to the node's end (check.rs `walk`); a moved one has its new lines; none for a stop that is gone; for a stop's intro (`block` none) the
/// node's lines, as found or as written.
pub fn step_lines(
    stop_written: Option<[u32; 2]>,
    found: Option<&super::check::Now>,
    stop_state: StopState,
    block: Option<([u32; 2], BlockState)>,
) -> Option<[u32; 2]> {
    let (delta, node_last) = match (found.and_then(|n| n.lines), stop_written) {
        (Some([first, last]), Some(written)) => (i64::from(first) - i64::from(written[0]), last),
        (Some([_, last]), None) => (0, last),
        (None, _) => (0, u32::MAX),
    };
    let shifted = |line: u32| u32::try_from((i64::from(line) + delta).max(1)).unwrap_or(line);
    match (block, stop_state) {
        (_, StopState::Gone) => None,
        (Some((lines, state)), _) => Some(match state {
            BlockState::Moved(l) => [l.first, l.last],
            BlockState::Fresh => [shifted(lines[0]), shifted(lines[1])],
            BlockState::Changed => [shifted(lines[0]), shifted(lines[1]).min(node_last)],
        }),
        (None, _) => found.and_then(|n| n.lines).or(stop_written),
    }
}

/// A stop's steps with what the check says of each: its lines now, and `fresh` or `changed`.
fn mark_steps(stop: &mut StopView, id: super::record::StopId, recorded: &Recorded) {
    let stop_state = recorded.stop(id);
    let found = recorded.now.get(&id).or(recorded.displaced.get(&id));
    let written = stop.lines;
    let word = |changed: bool| if changed { "changed" } else { "fresh" };
    for step in &mut stop.reading {
        let Ok(at) = step.id.parse::<StepId>() else {
            continue;
        };
        // the step's own word: the intro says what its stop says; a block says what its block says, and all of a stop that is gone are changed
        let (fresh, block) = if at.is_intro() {
            (stop_state == StopState::Fresh, None)
        } else {
            let (Some(lines), state) = (step.lines, recorded.block(at)) else {
                continue;
            };
            (
                stop_state != StopState::Gone && !matches!(state, BlockState::Changed),
                Some((lines, state)),
            )
        };
        step.fresh = Some(word(!fresh));
        if !at.is_intro() {
            step.lines = step_lines(written, found, stop_state, block);
        }
    }
}

/// `quick`, `normal`, `detailed`.
pub fn detail_word(detail: super::record::Detail) -> &'static str {
    match detail {
        super::record::Detail::Quick => "quick",
        super::record::Detail::Normal => "normal",
        super::record::Detail::Detailed => "detailed",
    }
}

/// What an agent is told when its write is refused because the reader pressed Stop (02 §5), word for word.
pub fn stopped_text(id: &TourId, written: usize) -> String {
    let stops = count(written, "stop", "stops");
    let said = if written == 0 {
        "Tour stopped.".to_string()
    } else {
        format!("Tour stopped at stop {written}.")
    };
    format!(
        "stopped: the reader stopped this tour ({stops} written). Add nothing more.\nSay one line: \"{said}\"\nTo go on later, only if the user asks: vnoc tour resume {id}\n"
    )
}

/// `fresh`, `1 stop changed`, `2 stops gone`, `1 changed, 1 gone`: the tour's word.
pub fn check_word(changed: usize, gone: usize) -> String {
    let stops = |n: usize| count(n, "stop", "stops");
    match (changed, gone) {
        (0, 0) => "fresh".to_string(),
        (c, 0) => format!("{} changed", stops(c)),
        (0, g) => format!("{} gone", stops(g)),
        (c, g) => format!("{c} changed, {g} gone"),
    }
}

/// `(checked now)`; for a record that holds, `(nothing changed since 08:20 UTC)` when it is from today and `(nothing changed since 2026-11-23)` when it is
/// older, or `(as checked at 08:20 UTC)` when it says some stop is not fresh.
pub fn when_words(from: Source, fresh: bool, at: &str, today: &str) -> String {
    match from {
        Source::Check => "(checked now)".to_string(),
        Source::Record | Source::Memory if !fresh => {
            let (day, time) = at.split_once('T').unwrap_or((at, ""));
            let when = if day == today.split('T').next().unwrap_or("") && time.len() >= 5 {
                format!("{} UTC", &time[..5])
            } else {
                day.to_string()
            };
            format!("(as checked at {when})")
        }
        Source::Record | Source::Memory => {
            let (day, time) = at.split_once('T').unwrap_or((at, ""));
            let since = if day == today.split('T').next().unwrap_or("") && time.len() >= 5 {
                format!("{} UTC", &time[..5])
            } else {
                day.to_string()
            };
            format!("(nothing changed since {since})")
        }
    }
}

fn lines_words(first: u32, last: u32) -> String {
    if first == last {
        format!("line {first} is")
    } else {
        format!("lines {first}-{last} are")
    }
}

/// One line for each stop that is not fresh, as `check` prints them (02 §6):
/// `s1 changed: retry_charge billing/charge.py:11-22 · s1.b2 line 15 is no longer there`, `s3 gone: ChargeError billing/charge.py is not in the code`.
/// The place is the stop's own record, as it was written; a block is named by the lines it was written about.
pub fn stale_lines(tour: &Tour, recorded: &Recorded) -> Vec<String> {
    let mut out = Vec::new();
    for stop in tour.stops.iter().filter(|s| s.written) {
        match recorded.stop(stop.id) {
            StopState::Fresh => {}
            StopState::Gone => {
                let mut line = format!("{} gone: {}", stop.id, stop_label(stop));
                let place = stop.path.to_string();
                if stop_label(stop) != place {
                    line.push_str(&format!(" {place}"));
                }
                line.push_str(" is not in the code");
                out.push(line);
            }
            StopState::Changed => {
                // where the stop is now, when the check said: a stop found by its ID may have another name or path than it was written with
                let (name, place, was) = match recorded.now.get(&stop.id) {
                    Some(now) => {
                        let name = if now.qualname.is_empty() {
                            now.path.rsplit('/').next().unwrap_or(&now.path).to_string()
                        } else {
                            now.qualname.clone()
                        };
                        let place = match now.lines {
                            Some([a, b]) if a == b => format!("{}:{a}", now.path),
                            Some([a, b]) => format!("{}:{a}-{b}", now.path),
                            None => now.path.clone(),
                        };
                        (name, place, Some(now))
                    }
                    None => (stop_label(stop), stop_place(stop), None),
                };
                let was_name =
                    (name != stop_label(stop)).then(|| format!(" (was {})", stop_label(stop)));
                let was_path = was
                    .filter(|now| now.path != stop.path.as_str())
                    .map(|_| format!(" (was {})", stop.path));
                let mut line = format!(
                    "{} changed: {name}{} {place}{}",
                    stop.id,
                    was_name.unwrap_or_default(),
                    was_path.unwrap_or_default()
                );
                let mut stack: Vec<&TourBlock> = stop.blocks.iter().rev().collect();
                while let Some(block) = stack.pop() {
                    if recorded.block(block.id) == BlockState::Changed {
                        line.push_str(&format!(
                            " · {} {} no longer there",
                            block.id,
                            lines_words(block.lines.first, block.lines.last)
                        ));
                    }
                    stack.extend(block.details.iter().rev());
                }
                out.push(line);
            }
        }
    }
    out
}

/// `6 of 10 stops` while some are planned, `9 stops` after.
pub fn stops_words(written: usize, planned: usize) -> String {
    if planned == 0 {
        count(written, "stop", "stops")
    } else {
        format!("{written} of {} stops", written + planned)
    }
}

/// `private · writing · 6 of 10 stops · 15 steps · by claude-code · not pinned (being written)`.
fn facts(tour: &Tour, view: &TourView) -> String {
    let mut facts = vec![
        view.folder.to_string(),
        view.state.to_string(),
        stops_words(view.written, view.planned),
        count(view.steps, "step", "steps"),
    ];
    if let Some(author) = &view.author {
        facts.push(format!("by {author}"));
    }
    facts.push(pin_words(tour, view.pin_in_repository != Some(false)));
    facts.join(" · ")
}

/// `title   "question"`.
fn head(title: &str, question: Option<&str>) -> String {
    match question {
        Some(q) => format!("{title}   \"{q}\""),
        None => title.to_string(),
    }
}

/// `← s1 line 13`, `← s1`, and `= s2`, in one string; empty for a stop at the top that repeats nothing.
fn tail(stop: &StopView) -> String {
    let mut tail = String::new();
    if let Some(from) = &stop.from {
        tail.push_str(&format!("← {from}"));
        if let Some(line) = stop.line {
            tail.push_str(&format!(" line {line}"));
        }
    }
    if let Some(same) = &stop.same_as {
        if !tail.is_empty() {
            tail.push_str("   ");
        }
        tail.push_str(&format!("= {same}"));
    }
    if let Some(mark) = stop.check {
        if !tail.is_empty() {
            tail.push_str("   ");
        }
        tail.push_str(mark);
    }
    tail
}

/// A text wrapped at [`WRAP`] columns after `indent`, every line indented by it. Words are not cut: a word longer than the line has a line of its own.
fn wrapped(text: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let room = WRAP.saturating_sub(indent).max(20);
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > room {
            out.push_str(&format!("{pad}{line}\n"));
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(&format!("{pad}{line}\n"));
    }
    out
}

/// What `show` prints: the title, the facts, one row per stop (details of a block with `with_text`), then the lines of the file that were skipped.
///
/// ```text
/// checkout_order   "what happens when the payment times out"
/// private · writing · 3 of 4 stops · 8 steps · by claude-code · not pinned (being written)
///  1  s1  ƒ checkout_order    app/checkout.py:11-30   5 steps
///  2  s2    ƒ check_stock     app/inventory.py:12-15  1 step    ← s1 line 13
/// ```
pub fn outline_text(tour: &Tour, view: &TourView, with_text: bool) -> String {
    let mut text = format!(
        "{}\n{}\n",
        head(&view.title, view.question.as_deref()),
        facts(tour, view)
    );
    if let Some(check) = &view.check {
        text.push_str(&format!(
            "{} {}\n",
            check_word(check.changed, check.gone),
            when_words(check.from, check.fresh, &check.at, &super::time::now())
        ));
    }
    let id_width = view.stops.iter().map(|s| s.id.len()).max().unwrap_or(2);
    let rows: Vec<(String, String, String)> = view
        .stops
        .iter()
        .map(|s| {
            let kind = tour.stops[s.number - 1].kind.as_ref();
            let label = stop_label(&tour.stops[s.number - 1]);
            // a changed stop that a check found under another name: the name it has now, and the one it was written with
            let shown_name = match &s.now {
                Some(now) if !now.qualname.is_empty() && now.qualname != label => {
                    format!("{} (was {label})", now.qualname)
                }
                _ => s.name.clone(),
            };
            let name = format!("{}{} {shown_name}", "  ".repeat(s.depth), icon(kind));
            let place = if s.planned && s.lines.is_none() {
                // not written: the name it will be found by
                if s.qualname.is_empty() {
                    s.path.clone()
                } else {
                    format!("{}::{}", s.path, s.qualname)
                }
            } else if let Some(now) = &s.now {
                match now.lines {
                    Some([a, b]) if a == b => format!("{}:{a}", now.path),
                    Some([a, b]) => format!("{}:{a}-{b}", now.path),
                    None => now.path.clone(),
                }
            } else {
                stop_place(&tour.stops[s.number - 1])
            };
            // a folder at the top says its name once
            let place = if place == label { String::new() } else { place };
            let steps = if s.planned {
                "planned".to_string()
            } else {
                count(s.steps, "step", "steps")
            };
            (name, place, steps)
        })
        .collect();
    let name_width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
    let place_width = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    let steps_width = rows.iter().map(|r| r.2.chars().count()).max().unwrap_or(0);
    let number_width = view.stops.len().to_string().len().max(2);
    for (stop, (name, place, steps)) in view.stops.iter().zip(&rows) {
        let tail = tail(stop);
        // a column nothing fills (only folders at the top) is not there
        let place = if place_width == 0 {
            String::new()
        } else {
            format!("{place:<place_width$}  ")
        };
        let mut row = format!(
            "{number:>number_width$}  {id:<id_width$}  {name:<name_width$}  {place}{steps:<steps_width$}  {tail}",
            number = stop.number,
            id = stop.id,
        );
        row.truncate(row.trim_end().len());
        text.push_str(&row);
        text.push('\n');
        if let Some(written) = &stop.as_written {
            text.push_str(&written.text());
        }
        if with_text {
            text.push_str(&stop_text(stop));
        }
    }
    if !view.problems.is_empty() {
        text.push_str(&format!(
            "{} in the file were skipped:\n",
            count(view.problems.len(), "line", "lines")
        ));
        for problem in &view.problems {
            text.push_str(&format!("  {problem}\n"));
        }
    }
    text
}

/// The intro of a stop and each of its blocks, under its row: the id and the lines of a block, then its title, then its text.
fn stop_text(stop: &StopView) -> String {
    let mut text = String::new();
    if let Some(intro) = &stop.text {
        text.push_str(&wrapped(intro, 6));
    }
    for block in &stop.blocks {
        let indent = 6 + 2 * block.depth;
        let lines = if block.lines[0] == block.lines[1] {
            block.lines[0].to_string()
        } else {
            format!("{}-{}", block.lines[0], block.lines[1])
        };
        let mark = match (block.check, block.moved_to) {
            (Some("moved"), Some([a, b])) if a == b => format!("  · moved to {a}"),
            (Some("moved"), Some([a, b])) => format!("  · moved to {a}-{b}"),
            (Some(word), _) => format!("  · {word}"),
            (None, _) => String::new(),
        };
        text.push_str(&format!(
            "{}{}  {lines}  {}{mark}\n",
            " ".repeat(indent - 2),
            block.id,
            block.title
        ));
        text.push_str(&wrapped(&block.text, indent));
    }
    text
}

// ----------------------------------------------------------------------------------------------------------------------- list

/// One tour of `list`.
#[derive(Debug, Clone, Serialize)]
pub struct ListRowView {
    pub id: String,
    /// `private` or `project`
    pub folder: &'static str,
    /// the same id is in the project's folder too: this is the private copy
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub private_copy: bool,
    pub readable: bool,
    /// why a tour cannot be read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// for a pinned tour: whether git has the pinned commit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_in_repository: Option<bool>,
    /// the lines of the file that were skipped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<usize>,
    /// the recorded (or just made) check: `fresh` or `changed`; absent when there is none that holds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<&'static str>,
    /// with `--on`: how the tour is on the node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<OnView>,
    /// `steps` × 20 seconds, up to a whole minute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<usize>,
    /// being written (`writing` or `stopping`) and nothing added for [`QUIET_SECS`]: "no news"
    pub quiet: bool,
    /// the first stop: the row's icon
    pub root: Option<RootView>,
    /// where the reader is; none for a tour nobody has opened here ("new")
    pub place: Option<PlaceSummary>,
    /// what the cover of a finished tour may offer, by the settings; `null` when there is nothing to offer, and for a tour that is not finished
    pub offers: Option<OffersView>,
    /// the row's second line (design 07 §3): `shared ·` for a project tour, the one state word that applies, the size. See [`row_words`]
    pub words: String,
    #[serde(skip)]
    pins: String,
}

/// What the cover of a finished tour offers (05 §3), by `[tours] ids` and `[tours] commit`: only for `"ask"`. `"always"` was done at `finish` and `"never"` is
/// not offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OffersView {
    /// how many written stops have no node ID: `Write IDs`
    pub ids: usize,
    /// the files of the tour that keep it from being pinned because they are not committed: `Commit and Pin…`
    pub commit: Vec<String>,
}

/// A tour's first stop, for the row's icon.
#[derive(Debug, Clone, Serialize)]
pub struct RootView {
    pub kind: String,
    pub name: String,
}

/// Where the reader is, in a row: the number of the stop on the route (none on the cover and the end), `cover`, `step` or `end`, and how many stops
/// have been read to their last step.
#[derive(Debug, Clone, Serialize)]
pub struct PlaceSummary {
    pub stop: Option<usize>,
    pub at: &'static str,
    pub seen: usize,
}

/// A tour being written whose file has not grown for this many seconds is quiet.
pub const QUIET_SECS: u64 = 5 * 60;

/// The row's `place`: `place` is the reader's place as it is written (`None`: nobody has opened this tour here), `tour` the tour it is a place in.
pub fn place_summary(tour: &Tour, place: Option<&Place>) -> Option<PlaceSummary> {
    let settled = place::settle(tour, place?);
    let (stop, at) = match settled.at {
        At::Cover => (None, "cover"),
        At::End => (None, "end"),
        At::Step(_) => (place::position(tour, settled.at).map(|p| p.stop), "step"),
    };
    Some(PlaceSummary {
        stop,
        at,
        seen: settled.seen.len(),
    })
}

/// The row's `root`: the first stop's kind and name; none for a tour with no stop, or whose first stop is planned (it has no kind yet).
pub fn root_of(tour: &Tour) -> Option<RootView> {
    let stop = tour.stops.first()?;
    Some(RootView {
        kind: stop.kind.as_ref()?.as_str().to_string(),
        name: stop_label(stop),
    })
}

/// Whether a tour is quiet: being written, and its file last changed `age` seconds ago or more.
pub fn is_quiet(state: State, age_secs: Option<u64>) -> bool {
    matches!(state, State::Writing | State::Stopping) && age_secs.is_some_and(|a| a >= QUIET_SECS)
}

/// How a tour is on a node (`list --on`).
#[derive(Debug, Clone, Serialize)]
pub struct OnView {
    /// `starts`, `stops` or `inside`
    pub how: &'static str,
    /// the numbers on screen of the stops that are the node
    pub stops: Vec<usize>,
    /// stops inside the node
    pub inside: usize,
    /// where a click opens
    pub first: String,
    /// the stops of the route
    pub of: usize,
    /// line 2 of the row
    pub words: String,
}

impl OnView {
    pub fn of(on: &on::On) -> OnView {
        let (how, stops, inside) = match &on.how {
            How::Starts => ("starts", vec![1], on.inside),
            How::Stops(numbers) => ("stops", numbers.clone(), on.inside),
            How::Inside(n) => ("inside", Vec::new(), *n),
        };
        OnView {
            how,
            stops,
            inside,
            first: on.first.to_string(),
            of: on.of,
            words: on::words(on),
        }
    }
}

/// The rows of `list`, in the order the store gives them (newest first, what cannot be read last). The same id in both folders: the project's row,
/// then the private one marked `(private copy)`.
/// `checks` are the answers known (recorded and valid, or just made); a fresh one is said in the row only with `say_fresh` (`list --check`).
/// `in_repository` says whether git has a pinned commit (`pin::has_commit`), asked only for a tour whose pin still holds.
pub fn list_rows<R: std::borrow::Borrow<Row>>(
    rows: &[R],
    checks: &BTreeMap<TourId, Recorded>,
    say_fresh: bool,
    in_repository: &dyn Fn(&Commit) -> bool,
) -> Vec<ListRowView> {
    let rows: Vec<&Row> = rows.iter().map(std::borrow::Borrow::borrow).collect();
    rows.iter()
        .map(|&row| {
            let copy = row.place == Where::Private
                && rows
                    .iter()
                    .any(|other| other.id == row.id && other.place == Where::Project);
            match &row.tour {
                Ok(tour) => {
                    let view = tour_view(tour, row.place);
                    let recorded = checks.get(&row.id);
                    let held = match &tour.pin {
                        Some(pin) if tour.pinned() => in_repository(&pin.commit),
                        _ => true,
                    };
                    let mut pins = pin_words(tour, held);
                    match recorded {
                        Some(r) if !r.fresh() => pins.push_str(" · changed"),
                        Some(_) if say_fresh => pins.push_str(" · fresh"),
                        _ => {}
                    }
                    ListRowView {
                        id: view.id.clone(),
                        folder: view.folder,
                        private_copy: copy,
                        readable: true,
                        problem: None,
                        title: Some(view.title.clone()),
                        question: view.question.clone(),
                        author: view.author.clone(),
                        state: Some(view.state),
                        written: Some(view.written),
                        planned: Some(view.planned),
                        steps: Some(view.steps),
                        pinned: Some(view.pinned),
                        pin: view.pin.clone(),
                        pin_in_repository: view.pin.as_ref().map(|_| held),
                        skipped: (!view.problems.is_empty()).then_some(view.problems.len()),
                        check: recorded.map(|r| if r.fresh() { "fresh" } else { "changed" }),
                        on: None,
                        minutes: Some(tour.size().minutes),
                        quiet: false,
                        root: root_of(tour),
                        place: None,
                        offers: None,
                        words: String::new(),
                        pins,
                    }
                }
                Err(Unreadable { why, .. }) => ListRowView {
                    id: row.id.to_string(),
                    folder: place_word(row.place),
                    private_copy: copy,
                    readable: false,
                    problem: Some(why.clone()),
                    title: None,
                    question: None,
                    author: None,
                    state: None,
                    written: None,
                    planned: None,
                    steps: None,
                    pinned: None,
                    pin: None,
                    pin_in_repository: None,
                    skipped: None,
                    check: None,
                    on: None,
                    minutes: None,
                    quiet: false,
                    root: None,
                    place: None,
                    offers: None,
                    words: "cannot be read".to_string(),
                    pins: String::new(),
                },
            }
        })
        .collect()
}

/// The state word a row says (design 07 §3: one word, the first that applies, in this order): `writing`, `stopping` or `stopped` while the tour is not
/// finished, `no news` for one that is being written and quiet, `changed` when a check says so, `stop 4` when the reader is on a stop (where you are),
/// `finished` when every stop was read to its last step, `new` when nobody opened it here; else how many stops were read: `seen 2 of 9`.
fn row_state_word(row: &ListRowView) -> String {
    let written = row.written.unwrap_or(0);
    match (row.state, &row.place) {
        _ if row.quiet => "no news".to_string(),
        (Some("stopping"), _) => "stopping".to_string(),
        (Some("stopped"), _) => "stopped".to_string(),
        (Some("writing"), _) => "writing".to_string(),
        _ if row.check == Some("changed") => "changed".to_string(),
        (_, Some(PlaceSummary { stop: Some(n), .. })) => format!("stop {n}"),
        (_, Some(place)) if written > 0 && place.seen >= written => "finished".to_string(),
        (_, None) => "new".to_string(),
        (_, Some(place)) => format!("seen {} of {written}", place.seen),
    }
}

/// The second line of a row, built from the pieces `list` prints: `shared · ` for a project tour (05 §3), the state word, the size (`stops_words`).
/// `writing · 6 of 10 stops`, `stop 4 · 9 stops`, `finished · 9 stops`, `new · 1 stop`, `shared · changed · 3 stops`. A tour that cannot be read says so.
/// It is made once the row has its `place` and `quiet` (`serve::list_rows`).
pub fn row_words(row: &ListRowView, when: &str) -> String {
    if !row.readable {
        return "cannot be read".to_string();
    }
    let mut parts = Vec::new();
    if row.folder == "project" {
        parts.push("shared".to_string());
    }
    parts.push(row_state_word(row));
    parts.push(stops_words(
        row.written.unwrap_or(0),
        row.planned.unwrap_or(0),
    ));
    if !when.is_empty() {
        parts.push(when.to_string());
    }
    parts.join(" · ")
}

/// When a tour was started (`at`), in words, as of `now` (both `2026-11-23T18:40:02Z`): `just now` under a minute, `5 min ago` under an hour, `3 h ago` under a
/// day; then by the UTC calendar day: `yesterday`, else the date, `12 Nov`, with the year when it is not this year. Nothing for a time that cannot be read. The
/// relative words do not depend on a time zone (the file keeps UTC and the core has no zone); only `yesterday` and the date do.
pub fn when_word(at: &str, now: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (Some(then), Some(now)) = (super::time::parse(at), super::time::parse(now)) else {
        return String::new();
    };
    let age = now - then;
    match age {
        i64::MIN..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", age / 60),
        3600..=86_399 => format!("{} h ago", age / 3600),
        _ => {
            let (day, today) = (then.div_euclid(86_400), now.div_euclid(86_400));
            if today - day == 1 {
                return "yesterday".to_string();
            }
            let (year, month, date) = super::time::civil_from_days(day);
            let (this_year, ..) = super::time::civil_from_days(today);
            let name = MONTHS[(month as usize).saturating_sub(1).min(11)];
            if year == this_year {
                format!("{date} {name}")
            } else {
                format!("{date} {name} {year}")
            }
        }
    }
}

/// What `list` prints: a row a tour, in columns.
///
/// ```text
/// checkout-order-7k2q  writing   6 of 10 stops  private  not pinned (being written)  checkout_order  "what happens …"
/// ```
pub fn list_text(rows: &[ListRowView]) -> String {
    let cells: Vec<[String; 6]> = rows
        .iter()
        .map(|r| {
            if !r.readable {
                return [
                    r.id.clone(),
                    "cannot be read".to_string(),
                    String::new(),
                    folder_cell(r),
                    String::new(),
                    r.problem.clone().unwrap_or_default(),
                ];
            }
            let title = r.title.clone().unwrap_or_default();
            let said = match &r.question {
                Some(q) => format!("{title}   \"{q}\""),
                None => title,
            };
            [
                r.id.clone(),
                r.state.unwrap_or_default().to_string(),
                stops_words(r.written.unwrap_or(0), r.planned.unwrap_or(0)),
                folder_cell(r),
                r.pins.clone(),
                said,
            ]
        })
        .collect();
    let width = |i: usize| cells_width(&cells, i);
    let widths: Vec<usize> = (0..5).map(width).collect();
    let mut text = String::new();
    for (at, cell) in cells.iter().enumerate() {
        let mut row = String::new();
        for (i, w) in widths.iter().enumerate() {
            row.push_str(&format!("{:<w$}  ", cell[i], w = *w));
        }
        row.push_str(&cell[5]);
        text.push_str(row.trim_end());
        text.push('\n');
        if let Some(on) = &rows[at].on {
            text.push_str(&format!("  {}\n", on.words));
        }
    }
    text
}

fn folder_cell(r: &ListRowView) -> String {
    if r.private_copy {
        format!("{} (private copy)", r.folder)
    } else {
        r.folder.to_string()
    }
}

fn cells_width(cells: &[[String; 6]], i: usize) -> usize {
    cells
        .iter()
        .map(|c| c[i].chars().count())
        .max()
        .unwrap_or(0)
}

// ----------------------------------------------------------------------------------------------------------------------------------- where

/// The most lines of code `where` prints under a step: more are counted, not printed.
pub const WHERE_CODE_LINES: usize = 40;

/// A step next to the place: where `prev` and `next` would go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Neighbour {
    /// `s3`, `s3.b2`, `cover` or `end`
    pub at: String,
    /// a block's title; none for an intro, the cover and the end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Neighbour {
    /// `s3 (intro)`, `s3.b2 "Take the units"`, `cover`.
    fn words(&self) -> String {
        match &self.title {
            Some(title) => format!("{} \"{title}\"", self.at),
            None if self.at.starts_with('s') && !self.at.contains('.') => {
                format!("{} (intro)", self.at)
            }
            None => self.at.clone(),
        }
    }
}

/// The stop the place is in.
#[derive(Debug, Clone, Serialize)]
pub struct WhereStop {
    pub id: String,
    /// its place on the route, from 1
    pub number: usize,
    /// how many stops the route has
    pub of: usize,
    pub name: String,
    pub kind: Option<String>,
    pub path: String,
    /// not written yet: its step is its intro with no text
    pub planned: bool,
}

/// The step the place is on.
#[derive(Debug, Clone, Serialize)]
pub struct WhereStep {
    pub id: String,
    /// `intro` or `block`
    pub kind: &'static str,
    /// the step's place in its stop, from 1
    pub number: usize,
    /// how many steps the stop has
    pub of: usize,
    /// a block's title; an intro has none
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// the intro's text or the block's; none for a planned stop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// the lines the step lights, now (or as written, when `code` says so)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<[u32; 2]>,
}

/// The code under the step, and which code it is.
#[derive(Debug, Clone, Serialize)]
pub struct WhereCode {
    /// `now`: the file as it is; `as_written`: the pinned commit's
    pub source: &'static str,
    /// the first 7 characters of the commit, for `as_written`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub path: String,
    /// the file line of the first line printed
    pub first: u32,
    /// the lines, at most [`WHERE_CODE_LINES`]; every one is lit
    pub lines: Vec<String>,
    /// how many lines more there were
    #[serde(skip_serializing_if = "is_zero")]
    pub more: usize,
    /// `as written at 8a1c2e4: the code changed since`, `the lines may not match`: said when the code is not simply what the step is about
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// What `vnoc tour where` says: the place and the step as one object (02 §12). `at` is `cover`, `end` or a step; `stop` and `step` are there on a step.
#[derive(Debug, Clone, Serialize)]
pub struct WhereView {
    pub tour: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// `writing`, `stopping`, `stopped` or `finished`
    pub state: &'static str,
    /// a window is showing this tour
    pub open: bool,
    /// when the place last moved; none for a tour nobody has moved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved: Option<String>,
    /// how long ago, in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_ago: Option<i64>,
    /// `reader` or `command`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<super::place::By>,
    pub at: String,
    /// `cover`, `intro`, `block` or `end`
    pub here: &'static str,
    /// how many stops and steps the tour has now
    pub stops: usize,
    pub steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<WhereStop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<WhereStep>,
    /// what a check says of this step: `fresh`, `moved`, `changed` or `gone`; none for the cover, the end and a planned stop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<WhereCode>,
    /// where `prev` goes; none on the cover, which has nothing before it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Neighbour>,
    /// where `next` goes; none on the end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Neighbour>,
    /// the stops read to their last step
    pub seen: Vec<String>,
    /// where each jump came from, the latest last
    pub back: Vec<String>,
}

/// `ƒ reserve_stock   app/inventory.py:21-22`: the stop's icon and name, then where its lit lines are (`lines`, as found now or as written), or only its path
/// for a stop with none.
pub fn step_headline(stop: &TourStop, path: &str, lines: Option<[u32; 2]>) -> String {
    let (icon, label) = (icon(stop.kind.as_ref()), stop_label(stop));
    let place = match lines {
        Some([a, b]) if a == b => format!("{path}:{a}"),
        Some([a, b]) => format!("{path}:{a}-{b}"),
        None => path.to_string(),
    };
    if label == place {
        format!("{icon} {label}")
    } else {
        format!("{icon} {label}   {place}")
    }
}

/// The text of `where`, from the same values as its `--json`:
///
/// ```text
/// checkout-order-7k2q   "checkout_order"   open on the canvas · moved 12 s ago
/// stop 3 of 10 · step 2 of 3     s3.b1
/// ƒ reserve_stock   app/inventory.py:21-22
///   Refuse what is not there
///   It checks the stock again by itself. …
///   > 21  if not check_stock(sku, quantity):
///   > 22      raise ValueError(f"Not enough stock for {sku}")
/// before: s3 (intro) · after: s3.b2 "Take the units, warn when few are left"
/// ```
pub fn where_text(view: &WhereView, stop: Option<&TourStop>) -> String {
    let moved = match (&view.moved, view.moved_ago) {
        (Some(_), Some(ago)) => format!("moved {}", super::time::ago(ago)),
        _ => "never moved".to_string(),
    };
    let open = if view.open {
        "open on the canvas"
    } else {
        "not open"
    };
    let mut text = format!("{}   \"{}\"   {open} · {moved}\n", view.tour, view.title);
    match (&view.stop, &view.step) {
        (Some(s), Some(step)) => {
            let planned = if s.planned { " · planned" } else { "" };
            text.push_str(&format!(
                "stop {} of {} · step {} of {}{planned}     {}\n",
                s.number, s.of, step.number, step.of, step.id
            ));
            let headline = match stop {
                Some(stop) => {
                    let path = view.code.as_ref().map_or(s.path.as_str(), |c| &c.path);
                    let lines = step.lines;
                    step_headline(stop, path, lines)
                }
                None => format!("{}   {}", s.name, s.path),
            };
            let mark = match view.check {
                Some("fresh") | None => String::new(),
                Some(word) => format!("   {word}"),
            };
            text.push_str(&format!("{headline}{mark}\n"));
            if let Some(title) = &step.title {
                text.push_str(&format!("  {title}\n"));
            }
            if let Some(body) = &step.text {
                text.push_str(&wrapped(body, 2));
            }
            if s.planned {
                text.push_str("  planned: not written yet\n");
            }
            if let Some(code) = &view.code {
                if let Some(note) = &code.note {
                    text.push_str(&format!("  {note}\n"));
                }
                let last = code.first + code.lines.len().saturating_sub(1) as u32;
                let width = last.to_string().len();
                for (n, line) in code.lines.iter().enumerate() {
                    let row = format!("  > {:>width$}  {line}", code.first + n as u32);
                    text.push_str(row.trim_end());
                    text.push('\n');
                }
                if code.more > 0 {
                    text.push_str(&format!("  … {} more\n", count(code.more, "line", "lines")));
                }
            }
        }
        _ => {
            let counts = format!(
                "{} · {}",
                count(view.stops, "stop", "stops"),
                count(view.steps, "step", "steps")
            );
            text.push_str(&format!("{} · {counts}\n", view.at));
            if let Some(question) = &view.question {
                text.push_str(&format!("  \"{question}\"\n"));
            }
        }
    }
    let mut around = Vec::new();
    if let Some(before) = &view.before {
        around.push(format!("before: {}", before.words()));
    }
    if let Some(after) = &view.after {
        around.push(format!("after: {}", after.words()));
    }
    if !around.is_empty() {
        text.push_str(&around.join(" · "));
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;

    const HEAD: &str = r#"{"type":"tour","schema":1,"id":"checkout-order-7k2q","title":"checkout_order","question":"what happens when the payment times out","author":"claude-code","at":"2026-11-23T18:40:02Z","detail":"normal"}"#;
    const H: &str = "0000000000000001";

    fn stop(id: u32, path: &str, qualname: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"stop","id":"s{id}","path":"{path}","qualname":"{qualname}","kind":"function","lines":[11,30],"hash":"{H}"{extra},"text":"intro of s{id}"}}"#
        )
    }

    fn tour(lines: &[String]) -> Tour {
        let mut text = format!("{HEAD}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        fold(&text).unwrap()
    }

    #[test]
    fn the_outline_has_a_row_a_stop_with_where_it_came_from_and_what_it_repeats() {
        let t = tour(&[
            stop(1, "app/checkout.py", "checkout_order", ""),
            r#"{"type":"step","id":"s1.b1","stop":"s1","lines":[11,24],"hash":"0000000000000001","title":"T","text":"x"}"#.to_string(),
            stop(2, "app/inventory.py", "check_stock", r#","from":"s1","line":13"#),
            stop(3, "app/inventory.py", "check_stock", r#","from":"s2","line":3,"same_as":"s2""#),
            r#"{"type":"plan","stops":[{"id":"s4","path":"app/checkout.py","qualname":"cancel_checkout","from":"s1","line":21}]}"#.to_string(),
        ]);
        let shown = tour_view(&t, Where::Private);
        assert_eq!(
            outline_text(&t, &shown, false),
            "checkout_order   \"what happens when the payment times out\"\n\
             private · writing · 3 of 4 stops · 4 steps · by claude-code · not pinned (being written)\n \
             1  s1  ƒ checkout_order     app/checkout.py:11-30             2 steps\n \
             2  s2    ƒ check_stock      app/inventory.py:11-30            1 step   ← s1 line 13\n \
             3  s3      ƒ check_stock    app/inventory.py:11-30            1 step   ← s2 line 3   = s2\n \
             4  s4    ◌ cancel_checkout  app/checkout.py::cancel_checkout  planned  ← s1 line 21\n"
        );
    }

    #[test]
    fn with_text_the_intro_and_every_block_are_under_their_row_wrapped_and_indented_by_depth() {
        let long = "word ".repeat(40);
        let t = tour(&[
            stop(1, "a.py", "f", "").replace("intro of s1", long.trim()),
            r#"{"type":"step","id":"s1.b1","stop":"s1","lines":[11,24],"hash":"0000000000000001","title":"First","text":"One."}"#.to_string(),
            r#"{"type":"step","id":"s1.b2","stop":"s1","parent":"s1.b1","lines":[12,12],"hash":"0000000000000001","title":"Detail","text":"Two."}"#.to_string(),
        ]);
        let shown = tour_view(&t, Where::Private);
        let text = outline_text(&t, &shown, true);
        let rows: Vec<&str> = text.lines().skip(2).collect();
        assert_eq!(rows[0], " 1  s1  ƒ f  a.py:11-30  3 steps");
        // 40 words of 5 characters wrapped under 6 spaces: no line past 100 columns, every one indented
        for line in &rows[1..4] {
            assert!(line.starts_with("      word"), "{line}");
            assert!(line.chars().count() <= 100, "{line}");
        }
        assert_eq!(
            &rows[4..],
            [
                "    s1.b1  11-24  First",
                "      One.",
                "      s1.b2  12  Detail",
                "        Two.",
            ]
        );
    }

    #[test]
    fn a_folder_at_the_top_says_its_name_once_and_a_planned_stop_says_planned() {
        let folder =
            r#"{"type":"stop","id":"s1","path":"app","qualname":"","kind":"folder","text":"t"}"#;
        let t = tour(&[folder.to_string()]);
        let text = outline_text(&t, &tour_view(&t, Where::Project), false);
        assert_eq!(text.lines().nth(2).unwrap(), " 1  s1  □ app/  1 step");
        assert_eq!(
            text.lines().nth(1).unwrap(),
            "project · writing · 1 stop · 1 step · by claude-code · not pinned (being written)"
        );
    }

    #[test]
    fn a_finished_tour_says_not_pinned_without_the_being_written_and_the_skipped_lines_are_listed()
    {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            "this is not a record".to_string(),
            r#"{"type":"end","at":"2026-11-23T18:47:31Z"}"#.to_string(),
        ]);
        let text = outline_text(&t, &tour_view(&t, Where::Private), false);
        assert!(
            text.contains(
                " · finished · 1 stop · 1 step · by claude-code · not pinned
"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "1 line in the file were skipped:
  line 3: "
            ),
            "{text}"
        );
    }

    fn row(id: &str, place: Where, tour: Folded) -> Row {
        Row {
            id: TourId::parse(id).unwrap(),
            place,
            path: std::path::PathBuf::new(),
            tour,
            len: 0,
        }
    }
    use crate::tours::record::TourId;
    use crate::tours::store::Folded;

    #[test]
    fn the_list_has_a_row_a_tour_and_marks_a_private_copy_and_a_tour_that_cannot_be_read() {
        let a = tour(&[stop(1, "a.py", "f", "")]);
        let rows = [
            row("checkout-order-7k2q", Where::Project, Ok(a.clone())),
            row("checkout-order-7k2q", Where::Private, Ok(a)),
            row(
                "broken-0000",
                Where::Private,
                Err(Unreadable {
                    why: "the first line is not JSON".to_string(),
                    too_new: false,
                }),
            ),
        ];
        let views = list_rows(&rows, &BTreeMap::new(), false, &|_| true);
        assert_eq!(
            list_text(&views),
            "checkout-order-7k2q  writing         1 stop  project                 not pinned (being written)  checkout_order   \"what happens when the payment times out\"\n\
             checkout-order-7k2q  writing         1 stop  private (private copy)  not pinned (being written)  checkout_order   \"what happens when the payment times out\"\n\
             broken-0000          cannot be read          private                                             the first line is not JSON\n"
        );
        assert!(views[1].private_copy);
        assert!(!views[0].private_copy);
        assert!(!views[2].readable);
    }

    #[test]
    fn the_stopped_text_is_the_one_of_the_plan_word_for_word() {
        let id = TourId::parse("checkout-order-7k2q").unwrap();
        assert_eq!(
            stopped_text(&id, 6),
            "stopped: the reader stopped this tour (6 stops written). Add nothing more.\nSay one line: \"Tour stopped at stop 6.\"\nTo go on later, only if the user asks: vnoc tour resume checkout-order-7k2q\n"
        );
        assert!(stopped_text(&id, 1).contains("(1 stop written)"));
        assert!(
            stopped_text(&id, 0).contains("(0 stops written)")
                && stopped_text(&id, 0).contains("\"Tour stopped.\"")
        );
    }

    fn step(id: &str, stop: u32, parent: Option<&str>, lines: [u32; 2]) -> String {
        let parent = parent.map_or(String::new(), |p| format!(r#","parent":"{p}""#));
        format!(
            r#"{{"type":"step","id":"{id}","stop":"s{stop}"{parent},"lines":[{},{}],"hash":"{H}","title":"T {id}","text":"text {id}"}}"#,
            lines[0], lines[1]
        )
    }

    /// A step as the tests compare it: id, depth, lines, word.
    type Read = (String, usize, Option<[u32; 2]>, Option<&'static str>);

    fn reading(shown: &TourView, stop: usize) -> Vec<Read> {
        shown.stops[stop]
            .reading
            .iter()
            .map(|s| (s.id.clone(), s.depth, s.lines, s.fresh))
            .collect()
    }

    #[test]
    fn a_stops_steps_are_in_reading_order_with_their_depth_and_parent_and_no_word_without_a_check()
    {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            step("s1.b2", 1, Some("s1.b1"), [14, 15]),
            step("s1.b3", 1, None, [22, 28]),
            r#"{"type":"plan","stops":[{"id":"s2","path":"a.py","qualname":"g"}]}"#.to_string(),
        ]);
        let shown = tour_view(&t, Where::Private);
        assert_eq!(
            reading(&shown, 0),
            [
                ("s1".to_string(), 0, None, None),
                ("s1.b1".to_string(), 1, Some([12, 20]), None),
                ("s1.b2".to_string(), 2, Some([14, 15]), None),
                ("s1.b3".to_string(), 1, Some([22, 28]), None),
            ]
        );
        assert_eq!(shown.stops[0].reading[2].parent.as_deref(), Some("s1.b1"));
        assert_eq!(shown.stops[0].reading[0].text, "intro of s1");
        // a stop that is planned has its intro and no text yet
        assert_eq!(reading(&shown, 1), [("s2".to_string(), 0, None, None)]);
        assert_eq!(shown.stops[1].reading[0].text, "");
        // the agent is on the planned stop while the tour is being written
        assert_eq!(shown.writing.as_deref(), Some("s2"));
        assert_eq!(shown.minutes, 2, "four steps, 20 seconds each");
    }

    #[test]
    fn a_check_gives_each_step_its_lines_now_and_its_own_word() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            step("s1.b2", 1, Some("s1.b1"), [14, 15]),
            step("s1.b3", 1, None, [22, 28]),
            stop(2, "b.py", "g", ""),
            step("s2.b1", 2, None, [12, 20]),
        ]);
        let mut shown = tour_view(&t, Where::Private);
        let s = |n: &str| n.parse::<StepId>().unwrap();
        let mut recorded = Recorded::empty(0);
        // s1 is fresh and two lines down; its block b2 is changed, b3 moved to the top
        recorded.displaced.insert(
            "s1".parse().unwrap(),
            super::super::check::Now {
                qualname: "f".into(),
                path: "a.py".into(),
                lines: Some([13, 32]),
            },
        );
        recorded.blocks.insert(s("s1.b2"), StopState::Changed);
        recorded.moved.insert(s("s1.b3"), [14, 15]);
        // s2 is gone
        recorded
            .stops
            .insert("s2".parse().unwrap(), StopState::Gone);
        mark(
            &mut shown,
            &Answer {
                from: Source::Check,
                recorded,
            },
        );
        assert_eq!(
            reading(&shown, 0),
            [
                ("s1".to_string(), 0, None, Some("fresh")),
                ("s1.b1".to_string(), 1, Some([14, 22]), Some("fresh")),
                ("s1.b2".to_string(), 2, Some([16, 17]), Some("changed")),
                ("s1.b3".to_string(), 1, Some([14, 15]), Some("fresh")),
            ],
            "the lines are the written ones two down; the moved block is where it moved to"
        );
        assert_eq!(
            reading(&shown, 1),
            [
                ("s2".to_string(), 0, None, Some("changed")),
                ("s2.b1".to_string(), 1, None, Some("changed")),
            ],
            "a stop that is gone has no lines, and every step of it says so"
        );
    }

    #[test]
    fn a_row_says_where_the_reader_is_what_the_tour_starts_with_and_whether_it_is_quiet() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            stop(2, "b.py", "g", ""),
        ]);
        let at = |text: &str| Place {
            at: text.parse().unwrap(),
            seen: vec!["s1".parse().unwrap()],
            ..Place::cover()
        };
        assert!(place_summary(&t, None).is_none());
        let on = place_summary(&t, Some(&at("s2"))).unwrap();
        assert_eq!((on.stop, on.at, on.seen), (Some(2), "step", 1));
        let on = place_summary(&t, Some(&at("cover"))).unwrap();
        assert_eq!((on.stop, on.at), (None, "cover"));
        let on = place_summary(&t, Some(&at("end"))).unwrap();
        assert_eq!((on.stop, on.at), (None, "end"));
        // a place whose step the tour no longer has is read as the nearest that is there
        let on = place_summary(&t, Some(&at("s9.b1")));
        assert_eq!(on.unwrap().at, "cover");
        let root = root_of(&t).unwrap();
        assert_eq!((root.kind.as_str(), root.name.as_str()), ("function", "f"));
        assert!(root_of(&tour(&[])).is_none());
        assert!(!is_quiet(State::Writing, Some(QUIET_SECS - 1)));
        assert!(is_quiet(State::Writing, Some(QUIET_SECS)));
        assert!(is_quiet(State::Stopping, Some(QUIET_SECS + 1)));
        assert!(!is_quiet(State::Stopped, Some(QUIET_SECS * 10)));
        assert!(!is_quiet(State::Finished, Some(QUIET_SECS * 10)));
        assert!(!is_quiet(State::Writing, None));
    }

    fn stop_of_kind(
        id: u32,
        path: &str,
        qualname: &str,
        kind: &str,
        lines: &str,
        extra: &str,
    ) -> String {
        format!(
            r#"{{"type":"stop","id":"s{id}","path":"{path}","qualname":"{qualname}","kind":"{kind}"{lines}{extra},"text":"intro of s{id}"}}"#
        )
    }

    #[test]
    fn every_step_says_its_kind_as_the_stage_draws_it() {
        let t = tour(&[
            // s1: a function with a block and a detail of it
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            step("s1.b2", 1, Some("s1.b1"), [13, 14]),
            // s2: a node of 4 lines is one step; s3 is a file with no blocks, s4 a folder, s5 explained before
            stop_of_kind(2, "a.py", "g", "function", r#","lines":[40,43]"#, ""),
            stop_of_kind(3, "b.py", "", "file", r#","lines":[1,60]"#, ""),
            stop_of_kind(4, "app", "", "folder", "", ""),
            stop(5, "a.py", "g", r#","same_as":"s2""#),
            // s6: a file with blocks is code, like a class; a stop of 8 lines is not short
            stop_of_kind(6, "c.py", "", "file", r#","lines":[1,60]"#, ""),
            step("s6.b1", 6, None, [2, 9]),
            stop_of_kind(7, "a.py", "h", "function", r#","lines":[50,57]"#, ""),
            r#"{"type":"plan","stops":[{"id":"s8","path":"a.py","qualname":"later"}]}"#.to_string(),
        ]);
        let shown = tour_view(&t, Where::Private);
        let kinds: Vec<(String, Vec<&str>)> = shown
            .stops
            .iter()
            .map(|s| (s.id.clone(), s.reading.iter().map(|r| r.kind).collect()))
            .collect();
        let kinds: Vec<(&str, &[&str])> = kinds
            .iter()
            .map(|(i, k)| (i.as_str(), k.as_slice()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("s1", &["intro", "block", "detail"][..]),
                ("s2", &["short"][..]),
                ("s3", &["contents"][..]),
                ("s4", &["contents"][..]),
                ("s5", &["same_as"][..]),
                ("s6", &["intro", "block"][..]),
                ("s7", &["intro"][..]),
                ("s8", &["planned"][..]),
            ]
        );
    }

    fn row_of(tour: Tour, place: Where) -> ListRowView {
        list_rows(
            &[row("checkout-order-7k2q", place, Ok(tour))],
            &BTreeMap::new(),
            false,
            &|_| true,
        )
        .remove(0)
    }

    fn at(stop: Option<usize>, seen: usize) -> Option<PlaceSummary> {
        Some(PlaceSummary {
            stop,
            at: if stop.is_some() { "step" } else { "cover" },
            seen,
        })
    }

    #[test]
    fn a_rows_words_are_shared_then_the_one_state_word_that_applies_then_the_size() {
        let writing = tour(&[
            stop(1, "a.py", "f", ""),
            r#"{"type":"plan","stops":[{"id":"s2","path":"a.py","qualname":"g"},{"id":"s3","path":"a.py","qualname":"h"}]}"#.to_string(),
        ]);
        let finished = tour(&[
            stop(1, "a.py", "f", ""),
            stop(2, "a.py", "g", ""),
            r#"{"type":"end","at":"2026-11-23T18:47:31Z"}"#.to_string(),
        ]);
        let stopped = tour(&[
            stop(1, "a.py", "f", ""),
            r#"{"type":"plan","stops":[{"id":"s2","path":"a.py","qualname":"g"}]}"#.to_string(),
            r#"{"type":"halt"}"#.to_string(),
            r#"{"type":"halted"}"#.to_string(),
        ]);
        let stopping = tour(&[stop(1, "a.py", "f", ""), r#"{"type":"halt"}"#.to_string()]);

        let words = |mut r: ListRowView, f: &dyn Fn(&mut ListRowView)| {
            f(&mut r);
            row_words(&r, "")
        };
        let nothing = |_: &mut ListRowView| {};
        // in the order the design gives them: writing, stopped, no news, changed, stop N, finished, new
        assert_eq!(
            words(row_of(writing.clone(), Where::Private), &nothing),
            "writing · 1 of 3 stops"
        );
        assert_eq!(
            words(row_of(stopping, Where::Private), &nothing),
            "stopping · 1 stop"
        );
        assert_eq!(
            words(row_of(stopped, Where::Private), &nothing),
            "stopped · 1 of 2 stops"
        );
        assert_eq!(
            words(row_of(writing.clone(), Where::Private), &|r| r.quiet = true),
            "no news · 1 of 3 stops"
        );
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &|r| r.check =
                Some("changed")),
            "changed · 2 stops"
        );
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &|r| r.place =
                at(Some(2), 1)),
            "stop 2 · 2 stops"
        );
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &|r| r.place =
                at(None, 2)),
            "finished · 2 stops"
        );
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &nothing),
            "new · 2 stops"
        );
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &|r| r.place =
                at(None, 1)),
            "seen 1 of 2 · 2 stops"
        );
        // the first that applies wins: changed over where you are, no news over writing, stopped over where you are
        assert_eq!(
            words(row_of(finished.clone(), Where::Private), &|r| {
                r.check = Some("changed");
                r.place = at(Some(1), 0)
            }),
            "changed · 2 stops"
        );
        assert_eq!(
            words(row_of(writing.clone(), Where::Private), &|r| {
                r.quiet = true;
                r.place = at(Some(1), 0)
            }),
            "no news · 1 of 3 stops"
        );
        // a project tour is shared; one stop is "1 stop"
        assert_eq!(
            words(row_of(finished, Where::Project), &nothing),
            "shared · new · 2 stops"
        );
        let one = tour(&[
            stop(1, "a.py", "f", ""),
            r#"{"type":"end","at":"2026-11-23T18:47:31Z"}"#.to_string(),
        ]);
        assert_eq!(words(row_of(one, Where::Private), &nothing), "new · 1 stop");
        // a tour that cannot be read
        let broken = list_rows(
            &[row(
                "broken-0000",
                Where::Private,
                Err(Unreadable {
                    why: "x".into(),
                    too_new: false,
                }),
            )],
            &BTreeMap::new(),
            false,
            &|_| true,
        );
        assert_eq!(broken[0].words, "cannot be read");
    }

    #[test]
    fn a_place_says_its_stop_and_its_position_the_numbers_of_stop_3_of_10_2_of_5() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            step("s1.b2", 1, Some("s1.b1"), [13, 14]),
            stop(2, "a.py", "g", ""),
        ]);
        let place = |at: &str| {
            let mut p = Place::cover();
            p.at = At::parse(at).unwrap();
            place_view(&t, p)
        };
        let on = place("s1.b2");
        assert_eq!(on.stop.as_deref(), Some("s1"));
        assert_eq!(
            on.position,
            Some(PositionView {
                stop: 1,
                of: 2,
                step: 3,
                steps: 3
            })
        );
        let second = place("s2");
        assert_eq!(
            (second.stop.as_deref(), second.position),
            (
                Some("s2"),
                Some(PositionView {
                    stop: 2,
                    of: 2,
                    step: 1,
                    steps: 1
                })
            )
        );
        for at in ["cover", "end"] {
            let p = place(at);
            assert_eq!((p.stop, p.position), (None, None), "{at}");
        }
        // a step the tour has not got is moved to the nearest, and its stop is that one's
        assert_eq!(place("s1.b9").stop.as_deref(), Some("s1"));
        // on the wire it is the state file's place with two keys more
        let json = serde_json::to_value(on).unwrap();
        assert_eq!(json["at"], "s1.b2");
        assert_eq!(json["stop"], "s1");
        assert_eq!(
            json["position"],
            serde_json::json!({"stop": 1, "of": 2, "step": 3, "steps": 3})
        );
        assert!(json["open"].is_boolean() && json["seen"].is_array());
    }

    #[test]
    fn a_stops_key_follows_where_a_check_found_it() {
        use crate::tours::check::{Now, Recorded};
        let t = tour(&[stop(1, "a.py", "f", ""), stop(2, "a.py", "g", "")]);
        let mut shown = tour_view(&t, Where::Private);
        assert_eq!(shown.stops[0].key.as_deref(), Some("a.py::f"));
        let mut recorded = Recorded::empty(0);
        recorded.now.insert(
            t.stops[0].id,
            Now {
                qualname: "f2".into(),
                path: "b.py".into(),
                lines: Some([3, 9]),
            },
        );
        // a stop found at the same place keeps its key
        recorded.now.insert(
            t.stops[1].id,
            Now {
                qualname: "g".into(),
                path: "a.py".into(),
                lines: Some([3, 9]),
            },
        );
        mark(
            &mut shown,
            &Answer {
                from: Source::Check,
                recorded,
            },
        );
        assert_eq!(shown.stops[0].key.as_deref(), Some("b.py::f2"));
        assert_eq!(shown.stops[1].key.as_deref(), Some("a.py::g"));
    }

    #[test]
    fn a_stop_reached_by_a_call_says_the_step_of_its_caller_that_holds_the_line() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            step("s1.b1", 1, None, [12, 20]),
            step("s1.b2", 1, Some("s1.b1"), [14, 15]),
            // inside a detail, inside a block (not in the detail), outside every block, on the block's first and last lines
            stop(2, "a.py", "g", r#","from":"s1","line":14"#),
            stop(3, "a.py", "h", r#","from":"s1","line":17"#),
            stop(4, "a.py", "i", r#","from":"s1","line":25"#),
            stop(5, "a.py", "j", r#","from":"s1","line":12"#),
            stop(6, "a.py", "k", r#","from":"s1","line":20"#),
            // reached by containment: no line, no step; a planned caller has only its intro
            stop(7, "a.py", "l", r#","from":"s1""#),
            r#"{"type":"plan","stops":[{"id":"s8","path":"a.py","qualname":"m"},{"id":"s9","path":"a.py","qualname":"n","from":"s8","line":3}]}"#.to_string(),
        ]);
        let shown = tour_view(&t, Where::Private);
        let steps: Vec<(&str, Option<&str>)> = shown
            .stops
            .iter()
            .map(|s| (s.id.as_str(), s.line_step.as_deref()))
            .collect();
        assert_eq!(
            steps,
            [
                ("s1", None),
                ("s2", Some("s1.b2")),
                ("s3", Some("s1.b1")),
                ("s4", Some("s1")),
                ("s5", Some("s1.b1")),
                ("s6", Some("s1.b1")),
                ("s7", None),
                ("s8", None),
                ("s9", Some("s8")),
            ]
        );
    }

    #[test]
    fn a_place_says_how_many_written_stops_are_not_seen_yet() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            stop(2, "a.py", "g", ""),
            r#"{"type":"plan","stops":[{"id":"s3","path":"a.py","qualname":"h"}]}"#.to_string(),
        ]);
        let with = |seen: &[u32]| {
            let mut p = Place::cover();
            p.seen = seen
                .iter()
                .map(|n| crate::tours::record::StopId::new(*n).unwrap())
                .collect();
            place_view(&t, p).unseen
        };
        assert_eq!(
            with(&[]),
            2,
            "at the start, every written stop; the planned one is not counted"
        );
        assert_eq!(with(&[1]), 1);
        assert_eq!(with(&[1, 2]), 0, "at the end of a tour read in full");
        assert_eq!(with(&[2]), 1, "visiting out of order");
        assert_eq!(
            with(&[1, 2, 9]),
            0,
            "a stop the tour has not got is dropped by settling"
        );
        let json = serde_json::to_value(place_view(&t, Place::cover())).unwrap();
        assert_eq!(json["unseen"], 2);
    }

    #[test]
    fn a_tour_says_when_it_was_started_by_the_boundaries_of_the_words() {
        let now = "2026-11-25T12:00:00Z";
        let at = |when: &str| when_word(when, now);
        assert_eq!(at("2026-11-25T12:00:00Z"), "just now");
        assert_eq!(at("2026-11-25T11:59:01Z"), "just now", "59 seconds");
        assert_eq!(at("2026-11-25T11:59:00Z"), "1 min ago", "a minute");
        assert_eq!(at("2026-11-25T11:01:00Z"), "59 min ago");
        assert_eq!(at("2026-11-25T11:00:00Z"), "1 h ago", "an hour");
        assert_eq!(
            at("2026-11-24T12:00:01Z"),
            "23 h ago",
            "23 h 59 min 59 s is still hours"
        );
        assert_eq!(
            at("2026-11-25T13:00:00Z"),
            "just now",
            "a clock that moved is not in the future"
        );
        // a day or more: by the calendar day (UTC): yesterday, else the date; the year when it is not this year
        assert_eq!(
            at("2026-11-24T12:00:00Z"),
            "yesterday",
            "exactly a day, on the day before"
        );
        assert_eq!(at("2026-11-24T00:00:00Z"), "yesterday");
        assert_eq!(at("2026-11-23T23:59:59Z"), "23 Nov", "two days of calendar");
        assert_eq!(at("2026-11-01T08:00:00Z"), "1 Nov");
        assert_eq!(at("2026-01-02T08:00:00Z"), "2 Jan");
        assert_eq!(at("2025-12-31T23:00:00Z"), "31 Dec 2025");
        // after midnight the same age reads as a day; yesterday across a month end
        assert_eq!(
            when_word("2026-10-31T09:00:00Z", "2026-11-01T10:00:00Z"),
            "yesterday"
        );
        assert_eq!(
            when_word("2026-11-25T09:00:00Z", "2026-11-26T08:00:00Z"),
            "23 h ago",
            "under a day is hours, even across midnight"
        );
        // a time that cannot be read says nothing
        assert_eq!(at("yesterday"), "");
        assert_eq!(when_word(now, "now"), "");
    }

    #[test]
    fn a_rows_words_end_with_when_and_say_nothing_more_when_there_is_no_time() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            r#"{"type":"end","at":"2026-11-23T18:47:31Z"}"#.to_string(),
        ]);
        let r = row_of(t, Where::Project);
        assert_eq!(row_words(&r, "2 h ago"), "shared · new · 1 stop · 2 h ago");
        assert_eq!(row_words(&r, ""), "shared · new · 1 stop");
    }

    #[test]
    fn a_place_names_the_first_written_stop_not_seen_in_route_order() {
        let t = tour(&[
            stop(1, "a.py", "f", ""),
            stop(2, "a.py", "g", r#","from":"s1","line":14"#),
            stop(3, "a.py", "h", ""),
            r#"{"type":"plan","stops":[{"id":"s4","path":"a.py","qualname":"i"}]}"#.to_string(),
        ]);
        let first = |seen: &[u32]| {
            let mut p = Place::cover();
            p.seen = seen
                .iter()
                .map(|n| crate::tours::record::StopId::new(*n).unwrap())
                .collect();
            place_view(&t, p).first_unseen
        };
        // the route order is s1, s2 (under s1), s3: not the order they were written in
        assert_eq!(first(&[]).as_deref(), Some("s1"), "none seen");
        assert_eq!(first(&[1]).as_deref(), Some("s2"));
        assert_eq!(
            first(&[1, 3]).as_deref(),
            Some("s2"),
            "skipped one in the middle"
        );
        assert_eq!(first(&[2, 3]).as_deref(), Some("s1"));
        assert_eq!(
            first(&[1, 2, 3]),
            None,
            "all seen: the planned stop is not written, so it is not skipped"
        );
        let json = serde_json::to_value(place_view(&t, Place::cover())).unwrap();
        assert_eq!(json["first_unseen"], "s1");
    }
}
