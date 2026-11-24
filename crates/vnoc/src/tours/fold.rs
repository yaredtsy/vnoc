//! Replaying a tour: lines in, a [`Tour`] out (`docs/26-tours/03-core.md` §1, `01-on-disk.md` §3, §4 and §10). Pure: no disk, no index.
//!
//! The first line must be a `tour` record of schema 1. Every later line is one record, applied in order: a line that does not parse is one
//! [`Problem`] and is skipped, a `type` this version does not know is skipped with none, a last line with no `\n` is not read (it is being written
//! this moment), and `\r\n` ends a line like `\n`.
//!
//! Where a rule needs "what is there now" (a stop's `from`, a block's `parent`, the stop a block belongs to), the replay looks at the tour **as it
//! is when that line is read**, the way a log is read: a name that points at nothing yet, or at something dropped, puts the stop or block at the top.

use std::collections::BTreeMap;
use std::fmt;

use vnoc_domain::{NodeId, NodeKind, RepoPath};

use super::record::{
    Detail, Lines, PinRecord, PlanStop, Record, StepId, StepRecord, StopId, StopRecord, TourId,
    TourRecord, parse_line,
};

/// One line of the file that was skipped, and why. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub why: String,
}

/// The tour cannot be read at all (§10): nothing of it is shown but this.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{why}")]
pub struct Unreadable {
    pub why: String,
}

/// Who is writing, from `halt`, `halted`, `resume` and `end` (01 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// no `end`, and no `halt` since the last `resume`
    Writing,
    /// a `halt` with no `halted` after it
    Stopping,
    /// a `halted` with no `resume` after it
    Stopped,
    /// an `end`. Whatever comes after it (a `stop`, a `step`, a `halt`) is an edit of a finished tour, which stays finished
    Finished,
}

/// One block of a stop, with the blocks that are its details, in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TourBlock {
    pub id: StepId,
    pub lines: Lines,
    pub hash: String,
    pub title: String,
    pub text: String,
    pub details: Vec<TourBlock>,
}

/// A stop on the route: written, or only planned (`written` is false, and what a written stop has is `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TourStop {
    pub id: StopId,
    pub written: bool,
    pub path: RepoPath,
    pub qualname: String,
    pub occurrence: u32,
    pub kind: Option<NodeKind>,
    pub node: Option<NodeId>,
    pub lines: Option<Lines>,
    pub hash: Option<String>,
    /// The stop this one is nested in: its `from` when that stop is on the route, else `None` (the stop is at the top).
    pub from: Option<StopId>,
    pub line: Option<u32>,
    pub same_as: Option<StopId>,
    /// the intro
    pub text: Option<String>,
    /// 0 at the top, else the `from` stop's depth + 1
    pub depth: usize,
    /// the top blocks, in the order they were added; each holds its details
    pub blocks: Vec<TourBlock>,
}

/// How big a tour is (03 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub written: usize,
    pub planned: usize,
    /// a planned stop is counted as two
    pub steps: usize,
    /// `steps` × 20 seconds, up to a whole minute
    pub minutes: usize,
}

/// A tour as its log says it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tour {
    pub id: TourId,
    pub title: String,
    pub question: Option<String>,
    pub author: Option<String>,
    pub at: String,
    pub detail: Detail,
    /// the `at` of the last `end`
    pub ended: Option<String>,
    pub state: State,
    /// the route, in reading order
    pub stops: Vec<TourStop>,
    /// the last `pin` record, whether or not it still holds: [`Tour::pinned`] says
    pub pin: Option<PinRecord>,
    pub problems: Vec<Problem>,
    /// the lines of the last `pin`, and of the last `stop`, `step` or `drop` (0: none)
    pin_line: usize,
    edit_line: usize,
    /// The highest stop number, and for each stop the highest block number, **ever used** in the file: in a `plan`, `stop`, `step` or `drop` line,
    /// and in the first line's `next`, also for what was dropped or replaced since. New ids count from these (01 §5: an id is never reused).
    highest_stop: u32,
    highest_blocks: BTreeMap<StopId, u32>,
}

impl Tour {
    /// The highest stop number ever used in this tour's file (0: none).
    pub fn highest_stop(&self) -> u32 {
        self.highest_stop
    }

    /// The highest block number ever used under this stop, at any depth (0: none).
    pub fn highest_block(&self, stop: StopId) -> u32 {
        self.highest_blocks.get(&stop).copied().unwrap_or(0)
    }

    /// The last `pin` comes after every `stop`, `step` and `drop`.
    pub fn pinned(&self) -> bool {
        self.pin.is_some() && self.pin_line > self.edit_line
    }

    pub fn stop(&self, id: StopId) -> Option<&TourStop> {
        self.stops.iter().find(|s| s.id == id)
    }

    /// The number on screen: the stop's place on the route, from 1.
    pub fn number(&self, id: StopId) -> Option<usize> {
        self.stops.iter().position(|s| s.id == id).map(|i| i + 1)
    }

    pub fn size(&self) -> Size {
        let written = self.stops.iter().filter(|s| s.written).count();
        let planned = self.stops.len() - written;
        let steps = self
            .stops
            .iter()
            .map(|s| {
                if s.written {
                    steps_in_order(s).len()
                } else {
                    2
                }
            })
            .sum();
        Size {
            written,
            planned,
            steps,
            minutes: (steps * 20).div_ceil(60),
        }
    }
}

/// The steps of a stop in reading order: the intro (the stop's own id), then each top block followed by its details, depth first. What `Next`
/// walks. A planned stop has its intro only.
pub fn steps_in_order(stop: &TourStop) -> Vec<StepId> {
    fn walk(blocks: &[TourBlock], out: &mut Vec<StepId>) {
        for block in blocks {
            out.push(block.id);
            walk(&block.details, out);
        }
    }
    let mut steps = vec![StepId::intro(stop.id)];
    if stop.written {
        walk(&stop.blocks, &mut steps);
    }
    steps
}

/// The complete lines of `text`, numbered from 1, without their `\n` or `\r\n`. A last piece with no `\n` is not a line yet.
fn complete_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n')
        .filter(|piece| piece.ends_with('\n'))
        .enumerate()
        .map(|(i, piece)| {
            let line = &piece[..piece.len() - 1];
            (i + 1, line.strip_suffix('\r').unwrap_or(line))
        })
}

/// The first line: a `tour` record of schema 1.
fn head(first: &str) -> Result<TourRecord, Unreadable> {
    let unreadable = |why: String| Unreadable { why };
    let value: serde_json::Value = serde_json::from_str(first)
        .map_err(|e| unreadable(format!("the first line is not JSON: {e}")))?;
    if value.get("type").and_then(|t| t.as_str()) != Some("tour") {
        return Err(unreadable(
            "the first line is not a `tour` record".to_string(),
        ));
    }
    match value.get("schema").and_then(|s| s.as_u64()) {
        Some(1) => {}
        Some(n) => {
            return Err(unreadable(format!(
                "this tour is schema {n}; this version of V-NOC reads schema 1"
            )));
        }
        None => {
            return Err(unreadable(
                "the first line has no `schema` number".to_string(),
            ));
        }
    }
    match parse_line(first) {
        Ok(Record::Tour(tour)) => Ok(tour),
        Ok(_) => Err(unreadable(
            "the first line is not a `tour` record".to_string(),
        )),
        Err(e) => Err(unreadable(format!("the first line is not readable: {e}"))),
    }
}

/// Replays a tour's file. `Err` when it cannot be read at all (an empty file, a first line that is not a `tour` record of schema 1).
pub fn fold(text: &str) -> Result<Tour, Unreadable> {
    let mut lines = complete_lines(text);
    let Some((_, first)) = lines.next() else {
        return Err(Unreadable {
            why: "the file has no complete first line".to_string(),
        });
    };
    let first = head(first)?;
    let (highest_stop, highest_blocks) = first.next.as_ref().map_or((0, BTreeMap::new()), |next| {
        (next.stop, next.blocks.clone())
    });
    let mut replay = Replay {
        tour: Tour {
            id: first.id,
            title: first.title,
            question: first.question,
            author: first.author,
            at: first.at,
            detail: first.detail,
            ended: None,
            state: State::Writing,
            stops: Vec::new(),
            pin: None,
            problems: Vec::new(),
            pin_line: 0,
            edit_line: 0,
            highest_stop,
            highest_blocks,
        },
        flow: State::Writing,
        finished: false,
    };
    for (n, line) in lines {
        // a blank line (a hand edit, a merge) is nothing
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(record) => replay.apply(n, record),
            Err(e) => replay.problem(n, e.to_string()),
        }
    }
    replay.finish();
    Ok(replay.tour)
}

struct Replay {
    tour: Tour,
    /// where the last of `halt`, `halted` and `resume` left things
    flow: State,
    finished: bool,
}

impl Replay {
    fn problem(&mut self, line: usize, why: String) {
        self.tour.problems.push(Problem { line, why });
    }

    /// Remembers the numbers a line uses, whether or not the line then changes anything (a block of a stop that is not there is a problem, and its
    /// number is still used).
    fn note_used(&mut self, record: &Record) {
        let tour = &mut self.tour;
        let mut stop = |id: StopId| tour.highest_stop = tour.highest_stop.max(id.number());
        match record {
            Record::Plan(plan) => plan.stops.iter().for_each(|s| stop(s.id)),
            Record::Stop(record) => stop(record.id),
            Record::Step(record) => stop(record.id.stop()),
            Record::Drop(record) => stop(record.id.stop()),
            _ => {}
        }
        if let Record::Step(StepRecord { id, .. }) | Record::Drop(super::record::DropRecord { id }) =
            record
            && let Some(block) = id.block_number()
        {
            let highest = tour.highest_blocks.entry(id.stop()).or_insert(0);
            *highest = (*highest).max(block);
        }
    }

    fn apply(&mut self, n: usize, record: Record) {
        self.note_used(&record);
        match record {
            Record::Tour(_) => self.problem(n, "a `tour` record after the first line".to_string()),
            Record::Meta(meta) => {
                let tour = &mut self.tour;
                if let Some(title) = meta.title {
                    tour.title = title;
                }
                if meta.question.is_some() {
                    tour.question = meta.question;
                }
                if let Some(detail) = meta.detail {
                    tour.detail = detail;
                }
            }
            Record::Plan(plan) => {
                for planned in plan.stops {
                    self.plan_stop(planned);
                }
            }
            Record::Stop(stop) => {
                self.tour.edit_line = n;
                self.write_stop(stop);
            }
            Record::Step(step) => {
                self.tour.edit_line = n;
                self.write_step(n, step);
            }
            Record::Drop(drop) => {
                self.tour.edit_line = n;
                if drop.id.is_intro() {
                    self.drop_stop(drop.id.stop());
                } else if let Some(stop) = self.stop_mut(drop.id.stop()) {
                    remove_block(&mut stop.blocks, drop.id);
                }
            }
            Record::Ids(ids) => {
                for (stop, node) in ids.nodes.0 {
                    if let Some(stop) = self.stop_mut(stop) {
                        stop.node = Some(node);
                    }
                }
            }
            Record::Pin(pin) => {
                self.tour.pin = Some(pin);
                self.tour.pin_line = n;
            }
            Record::Halt(_) => self.flow = State::Stopping,
            Record::Halted(_) => self.flow = State::Stopped,
            Record::Resume(_) => self.flow = State::Writing,
            Record::End(end) => {
                self.finished = true;
                self.tour.ended = Some(end.at);
                self.leave_the_route();
            }
            Record::Other => {}
        }
    }

    fn stop_mut(&mut self, id: StopId) -> Option<&mut TourStop> {
        self.tour.stops.iter_mut().find(|s| s.id == id)
    }

    fn has_stop(&self, id: StopId) -> bool {
        self.tour.stops.iter().any(|s| s.id == id)
    }

    /// The stop with this id and every stop nested in it, at any depth.
    fn subtree(&self, id: StopId) -> Vec<StopId> {
        let mut found = vec![id];
        loop {
            let before = found.len();
            for stop in &self.tour.stops {
                if stop.from.is_some_and(|f| found.contains(&f)) && !found.contains(&stop.id) {
                    found.push(stop.id);
                }
            }
            if found.len() == before {
                return found;
            }
        }
    }

    /// A `from` that is on the route now, and is not the stop itself or a stop nested in it (that would be a loop); else `None`.
    fn parent_for(&self, id: StopId, from: Option<StopId>) -> Option<StopId> {
        let from = from?;
        (self.has_stop(from) && !self.subtree(id).contains(&from)).then_some(from)
    }

    /// Where a new stop goes: after the last stop nested in its parent (or the parent itself), else at the end.
    fn place(&mut self, stop: TourStop) {
        let at = stop.from.and_then(|parent| {
            let family = self.subtree(parent);
            self.tour
                .stops
                .iter()
                .rposition(|s| family.contains(&s.id))
                .map(|last| last + 1)
        });
        match at {
            Some(i) => self.tour.stops.insert(i, stop),
            None => self.tour.stops.push(stop),
        }
    }

    fn plan_stop(&mut self, planned: PlanStop) {
        if self.has_stop(planned.id) {
            return; // a planned id that is on the route already is not planned again
        }
        let from = self.parent_for(planned.id, planned.from);
        self.place(TourStop {
            id: planned.id,
            written: false,
            path: planned.path,
            qualname: planned.qualname,
            occurrence: planned.occurrence.get(),
            kind: None,
            node: None,
            lines: None,
            hash: None,
            from,
            line: planned.line,
            same_as: planned.same_as,
            text: None,
            depth: 0,
            blocks: Vec::new(),
        });
    }

    /// A written stop: a new one goes to its place; one whose id is on the route already (planned, or written before) replaces it where it stands
    /// and clears its blocks.
    fn write_stop(&mut self, record: StopRecord) {
        let from = self.parent_for(record.id, record.from);
        let stop = TourStop {
            id: record.id,
            written: true,
            path: record.path,
            qualname: record.qualname,
            occurrence: record.occurrence.get(),
            kind: Some(record.kind),
            node: record.node,
            lines: record.lines,
            hash: record.hash,
            from,
            line: record.line,
            same_as: record.same_as,
            text: Some(record.text),
            depth: 0,
            blocks: Vec::new(),
        };
        match self.tour.stops.iter().position(|s| s.id == record.id) {
            Some(i) => self.tour.stops[i] = stop,
            None => self.place(stop),
        }
    }

    /// A block goes under its `parent` when that is a block of the same stop that is there now; else at the top of its stop. A block whose id the
    /// stop has already is replaced **in place**: it keeps its position among its siblings and its details, and takes the new lines, hash, title
    /// and text (its `parent` is not looked at again: the block stays where it is).
    fn write_step(&mut self, n: usize, record: StepRecord) {
        let Some(stop) = self.tour.stops.iter_mut().find(|s| s.id == record.stop) else {
            return self.problem(
                n,
                format!("the block {} is of a stop that is not there", record.id),
            );
        };
        if !stop.written {
            return self.problem(
                n,
                format!(
                    "the block {} is of {}, which is only planned",
                    record.id, record.stop
                ),
            );
        }
        if let Some(old) = find_block(&mut stop.blocks, record.id) {
            old.lines = record.lines;
            old.hash = record.hash;
            old.title = record.title;
            old.text = record.text;
            return;
        }
        let block = TourBlock {
            id: record.id,
            lines: record.lines,
            hash: record.hash,
            title: record.title,
            text: record.text,
            details: Vec::new(),
        };
        let under = record
            .parent
            .filter(|p| !p.is_intro() && p.stop() == record.stop)
            .and_then(|p| find_block(&mut stop.blocks, p));
        match under {
            Some(parent) => parent.details.push(block),
            None => stop.blocks.push(block),
        }
    }

    /// Takes a stop off the route, and every stop nested in it.
    fn drop_stop(&mut self, id: StopId) {
        if !self.has_stop(id) {
            return;
        }
        let gone = self.subtree(id);
        self.tour.stops.retain(|s| !gone.contains(&s.id));
    }

    /// At an `end`, the planned stops that were never written leave the route. A written stop that was nested in one moves up to the nearest stop
    /// above it that stays (the planned stop's own `from`, and if that leaves too, its `from`), or to the top when there is none; and its `line`,
    /// which was a line of the stop that left, is dropped.
    fn leave_the_route(&mut self) {
        let planned_from: Vec<(StopId, Option<StopId>)> = self
            .tour
            .stops
            .iter()
            .filter(|s| !s.written)
            .map(|s| (s.id, s.from))
            .collect();
        for stop in self.tour.stops.iter_mut().filter(|s| s.written) {
            let mut moved = false;
            while let Some(parent) = stop.from {
                let Some((_, above)) = planned_from.iter().find(|(id, _)| *id == parent) else {
                    break;
                };
                stop.from = *above;
                moved = true;
            }
            if moved {
                stop.line = None;
            }
        }
        self.tour.stops.retain(|s| s.written);
    }

    fn finish(&mut self) {
        // the depths, from the `from` chains (a chain is never a loop: `parent_for` refuses one)
        for i in 0..self.tour.stops.len() {
            let mut depth = 0;
            let mut at = self.tour.stops[i].from;
            while let Some(parent) = at {
                depth += 1;
                at = self
                    .tour
                    .stops
                    .iter()
                    .find(|s| s.id == parent)
                    .and_then(|s| s.from);
                if depth > self.tour.stops.len() {
                    break;
                }
            }
            self.tour.stops[i].depth = depth;
        }
        self.tour.state = if self.finished {
            State::Finished
        } else {
            self.flow
        };
    }
}

fn find_block(blocks: &mut [TourBlock], id: StepId) -> Option<&mut TourBlock> {
    for block in blocks {
        if block.id == id {
            return Some(block);
        }
        if let Some(found) = find_block(&mut block.details, id) {
            return Some(found);
        }
    }
    None
}

/// Takes a block and its details out of a stop's blocks. Whether it was there.
fn remove_block(blocks: &mut Vec<TourBlock>, id: StepId) -> bool {
    if let Some(i) = blocks.iter().position(|b| b.id == id) {
        blocks.remove(i);
        return true;
    }
    blocks.iter_mut().any(|b| remove_block(&mut b.details, id))
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            State::Writing => "writing",
            State::Stopping => "stopping",
            State::Stopped => "stopped",
            State::Finished => "finished",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str =
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;
    const NODE: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    /// A written stop `s<n>` (with its `from`, when given).
    fn stop(n: u32, from: Option<u32>) -> String {
        let from = from.map_or(String::new(), |f| format!(r#","from":"s{f}","line":3"#));
        format!(
            r#"{{"type":"stop","id":"s{n}","path":"a.py","qualname":"f{n}","kind":"function","lines":[1,20],"hash":"h{n}"{from},"text":"intro {n}"}}"#
        )
    }

    /// A block `s<n>.b<m>`, a detail of `parent` when given (an id as text).
    fn step(n: u32, m: u32, parent: Option<&str>) -> String {
        let parent = parent.map_or(String::new(), |p| format!(r#","parent":"{p}""#));
        format!(
            r#"{{"type":"step","id":"s{n}.b{m}","stop":"s{n}"{parent},"lines":[2,5],"hash":"k{m}","title":"b{m}","text":"block {m}"}}"#
        )
    }

    fn plan(stops: &[(u32, Option<u32>)]) -> String {
        let list: Vec<String> = stops
            .iter()
            .map(|(n, from)| {
                let from = from.map_or(String::new(), |f| format!(r#","from":"s{f}","line":3"#));
                format!(r#"{{"id":"s{n}","path":"a.py","qualname":"f{n}"{from}}}"#)
            })
            .collect();
        format!(r#"{{"type":"plan","stops":[{}]}}"#, list.join(","))
    }

    /// The file: the first line, then these, each ended by `\n`.
    fn log(lines: &[String]) -> String {
        let mut text = format!("{HEAD}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    fn tour(lines: &[String]) -> Tour {
        fold(&log(lines)).unwrap()
    }

    /// The route as `s1:0`, `s2:1`: id and depth.
    fn route(tour: &Tour) -> Vec<String> {
        tour.stops
            .iter()
            .map(|s| format!("{}:{}", s.id, s.depth))
            .collect()
    }

    fn steps(tour: &Tour, stop: u32) -> Vec<String> {
        let stop = tour.stop(StopId::new(stop).unwrap()).unwrap();
        steps_in_order(stop).iter().map(|s| s.to_string()).collect()
    }

    fn problems(tour: &Tour) -> Vec<usize> {
        tour.problems.iter().map(|p| p.line).collect()
    }

    // ---- the example of 01-on-disk.md §4, as it is printed there

    const MINICART: &str = r#"{"type":"tour","schema":1,"id":"checkout-order-7k2q","title":"checkout_order","question":"what happens when the payment times out","author":"claude-code","at":"2026-11-23T18:40:02Z","detail":"normal"}
{"type":"plan","stops":[{"id":"s1","path":"app/checkout.py","qualname":"checkout_order"},{"id":"s2","path":"app/inventory.py","qualname":"check_stock","from":"s1","line":13},{"id":"s3","path":"app/inventory.py","qualname":"reserve_stock","from":"s1","line":16}]}
{"type":"stop","id":"s1","path":"app/checkout.py","qualname":"checkout_order","kind":"function","node":"0Q8V5W4T2F7KX9M3B6D1RZ4H7K","lines":[11,30],"hash":"9f2c61a07b3e4d15","text":"checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records the sale, in that order."}
{"type":"step","id":"s1.b1","stop":"s1","lines":[11,24],"hash":"41c07d92aa10b6e3","title":"Hold the stock, then charge","text":"Nothing is charged until the items are set aside, and nothing stays set aside if the charge fails."}
{"type":"step","id":"s1.b2","stop":"s1","parent":"s1.b1","lines":[11,16],"hash":"7be1a0c4d95f2233","title":"Check the stock, then hold it","text":"check_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them while this payment runs."}
{"type":"stop","id":"s2","path":"app/inventory.py","qualname":"check_stock","kind":"function","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s1","line":13,"text":"check_stock answers one question: are there at least `quantity` units of this SKU? An unknown SKU counts as zero. It reads and never changes the stock."}
{"type":"halt"}
{"type":"halted"}
{"type":"resume"}
{"type":"stop","id":"s4","path":"app/inventory.py","qualname":"check_stock","kind":"function","lines":[12,15],"hash":"0b7e55c1d2f4a960","from":"s3","line":21,"same_as":"s2","text":"Called again here: reserve_stock does not trust its caller to have checked."}
{"type":"end","at":"2026-11-23T18:47:31Z"}
{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"2026-11-23T18:47:31Z","where":{"s1":["app/checkout.py",11,30],"s1.b1":["app/checkout.py",11,24],"s1.b2":["app/checkout.py",11,16],"s2":["app/inventory.py",12,15],"s4":["app/inventory.py",12,15]}}
{"type":"ids","nodes":{"s2":"3M1X9C7B5N2V8K4J6H0G1F3D5S"}}
"#;

    #[test]
    fn the_minicart_example_as_printed() {
        let t = fold(MINICART).unwrap();
        assert_eq!(t.id.as_str(), "checkout-order-7k2q");
        assert_eq!(t.title, "checkout_order");
        assert_eq!(
            t.question.as_deref(),
            Some("what happens when the payment times out")
        );
        assert_eq!(t.author.as_deref(), Some("claude-code"));
        assert_eq!(t.detail, Detail::Normal);
        assert!(t.problems.is_empty());
        assert_eq!(t.state, State::Finished);
        assert_eq!(t.ended.as_deref(), Some("2026-11-23T18:47:31Z"));
        assert!(
            t.pinned(),
            "the pin is after every stop, step and drop; the `ids` after it is not an edit"
        );
        // s3 was planned and never written: the `end` takes it off the route, and s4, which was reached from it, moves up to s3's own `from`, s1
        assert_eq!(route(&t), ["s1:0", "s2:1", "s4:1"]);
        assert_eq!(t.number(StopId::new(4).unwrap()), Some(3));
        let s1 = t.stop(StopId::new(1).unwrap()).unwrap();
        assert_eq!(s1.node.map(|n| n.to_string()).as_deref(), Some(NODE));
        assert_eq!(s1.lines, Lines::new(11, 30));
        assert_eq!(steps(&t, 1), ["s1", "s1.b1", "s1.b2"]);
        assert_eq!(s1.blocks.len(), 1, "s1.b2 is a detail of s1.b1");
        assert_eq!(s1.blocks[0].details[0].id.to_string(), "s1.b2");
        let s2 = t.stop(StopId::new(2).unwrap()).unwrap();
        assert_eq!(
            s2.node.map(|n| n.to_string()).as_deref(),
            Some("3M1X9C7B5N2V8K4J6H0G1F3D5S")
        );
        assert_eq!((s2.from, s2.line), (StopId::new(1), Some(13)));
        let s4 = t.stop(StopId::new(4).unwrap()).unwrap();
        assert_eq!(
            (s4.same_as, s4.from, s4.line),
            (StopId::new(2), StopId::new(1), None),
            "its line was a line of s3, which left"
        );
        assert_eq!(steps(&t, 4), ["s4"]);
        assert_eq!(
            t.size(),
            Size {
                written: 3,
                planned: 0,
                steps: 5,
                minutes: 2
            }
        );
    }

    #[test]
    fn the_minicart_example_with_the_stop_it_leaves_out_written() {
        // 01 §4 says the example is shortened: s3's own line is not in it. With it, s4 is nested in s3 and nothing leaves the route.
        let s3 = r#"{"type":"stop","id":"s3","path":"app/inventory.py","qualname":"reserve_stock","kind":"function","lines":[18,30],"hash":"aa","from":"s1","line":16,"text":"reserve"}"#;
        let marker = r#"{"type":"halt"}"#;
        let text = MINICART.replace(marker, &format!("{s3}\n{marker}"));
        let t = fold(&text).unwrap();
        assert_eq!(route(&t), ["s1:0", "s2:1", "s3:1", "s4:2"]);
        assert_eq!(
            t.stop(StopId::new(4).unwrap()).unwrap().from,
            StopId::new(3)
        );
        assert!(t.problems.is_empty());
        assert_eq!(t.state, State::Finished);
        assert!(t.pinned(), "s3's line is before the pin, as the others are");
    }

    // ---- the first line

    #[test]
    fn a_file_whose_first_line_is_not_a_tour_of_schema_1_is_unreadable() {
        let tour_line = |extra: &str| {
            format!(
                r#"{{"type":"tour",{extra}"id":"t-0000","title":"T","at":"x","detail":"normal"}}"#
            )
        };
        let cases: Vec<(&str, String)> = vec![
            ("an empty file", String::new()),
            ("only a newline", "\n".to_string()),
            ("a first line with no line end", HEAD.to_string()),
            ("a first line that is cut off", format!("{}\n", &HEAD[..30])),
            ("a first line that is not JSON", "# a tour\n".to_string()),
            ("a first line that is a list", "[]\n".to_string()),
            ("another type first", format!("{}\n{HEAD}\n", stop(1, None))),
            ("a halt first", "{\"type\":\"halt\"}\n".to_string()),
            ("no type", "{\"schema\":1}\n".to_string()),
            ("schema 2", format!("{}\n", tour_line(r#""schema":2,"#))),
            ("schema 0", format!("{}\n", tour_line(r#""schema":0,"#))),
            ("schema 2 and nothing else a schema 1 has", "{\"type\":\"tour\",\"schema\":2,\"new\":true}\n".to_string()),
            ("no schema", format!("{}\n", tour_line(""))),
            ("a schema that is text", format!("{}\n", tour_line(r#""schema":"1","#))),
            ("schema 1 and no title", "{\"type\":\"tour\",\"schema\":1,\"id\":\"t-0000\",\"at\":\"x\",\"detail\":\"normal\"}\n".to_string()),
            ("a tour id that is not one", "{\"type\":\"tour\",\"schema\":1,\"id\":\"T-0000\",\"title\":\"T\",\"at\":\"x\",\"detail\":\"normal\"}\n".to_string()),
        ];
        for (what, text) in cases {
            let err = fold(&text).expect_err(what);
            assert!(!err.why.is_empty(), "{what}");
        }
        assert!(fold(&format!("{}\n", tour_line(r#""schema":1,"#))).is_ok());
        let two = fold(&format!("{}\n", tour_line(r#""schema":2,"#))).unwrap_err();
        assert!(two.why.contains("schema 2"), "{}", two.why);
    }

    // ---- the lines after it

    #[test]
    fn a_broken_middle_line_is_one_problem_and_the_rest_loads() {
        let t = tour(&[
            stop(1, None),
            "this is not a record".to_string(),
            stop(2, None),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert_eq!(
            problems(&t),
            [3],
            "the file's third line: the tour line, then s1, then it"
        );
        assert!(!t.problems[0].why.is_empty());

        let t = tour(&[
            stop(1, None),
            r#"{"type":"step","id":"s1.b1"}"#.to_string(),
            "this is not a record".to_string(),
            r#"{"type":"drop"}"#.to_string(),
            stop(2, None),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert_eq!(problems(&t), [3, 4, 5]);
    }

    #[test]
    fn a_blank_line_is_skipped_with_no_problem() {
        let t = tour(&[
            stop(1, None),
            "".to_string(),
            "   ".to_string(),
            "\t".to_string(),
            stop(2, None),
            "broken".to_string(),
            "".to_string(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert_eq!(
            problems(&t),
            [7],
            "only the broken line, by its number in the file"
        );
        // with `\r\n` ends, and a blank line right after the first
        let text = format!("{HEAD}\r\n\r\n{}\r\n \r\n", stop(1, None));
        let t = fold(&text).unwrap();
        assert_eq!(route(&t), ["s1:0"]);
        assert!(t.problems.is_empty());
        // the first line is not skipped: a file that starts with a blank line is not a tour
        assert!(fold(&format!("\n{HEAD}\n")).is_err());
    }

    #[test]
    fn a_stop_whose_path_goes_up_is_one_problem_and_the_rest_loads() {
        let bad = r#"{"type":"stop","id":"s2","path":"../secret.py","qualname":"f","kind":"function","text":"t"}"#;
        let t = tour(&[stop(1, None), bad.to_string(), stop(3, None)]);
        assert_eq!(route(&t), ["s1:0", "s3:0"]);
        assert_eq!(problems(&t), [3]);
        assert!(t.problems[0].why.contains(".."), "{}", t.problems[0].why);
    }

    #[test]
    fn a_type_this_version_does_not_know_is_skipped_with_no_problem() {
        let t = tour(&[
            stop(1, None),
            r#"{"type":"question","at":"s1","text":"why?"}"#.to_string(),
            r#"{"type":"answer","text":"because"}"#.to_string(),
            stop(2, None),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert!(t.problems.is_empty());
    }

    #[test]
    fn a_key_this_version_does_not_know_in_a_known_record_is_read() {
        let first = format!(
            "{}\n",
            HEAD.replace(
                "\"detail\":\"normal\"",
                "\"detail\":\"normal\",\"colour\":\"red\""
            )
        );
        let text = format!(
            "{first}{}\n",
            stop(1, None).replace("\"text\"", "\"future\":[1,2],\"text\"")
        );
        let t = fold(&text).unwrap();
        assert_eq!(route(&t), ["s1:0"]);
        assert!(t.problems.is_empty());
    }

    #[test]
    fn a_last_line_with_no_line_end_is_not_read_and_is_no_problem() {
        let whole = log(&[stop(1, None)]);
        // a record that is complete, but whose `\n` is not written yet
        let t = fold(&format!("{whole}{}", stop(2, None))).unwrap();
        assert_eq!(route(&t), ["s1:0"]);
        assert!(t.problems.is_empty());
        // a record cut in the middle
        let t = fold(&format!("{whole}{}", &stop(2, None)[..40])).unwrap();
        assert_eq!(route(&t), ["s1:0"]);
        assert!(t.problems.is_empty());
        // the next append starts with `\n`: the torn piece is then one broken line, and what follows is read
        let t = fold(&format!(
            "{whole}{}\n{}\n",
            &stop(2, None)[..40],
            stop(3, None)
        ))
        .unwrap();
        assert_eq!(route(&t), ["s1:0", "s3:0"]);
        assert_eq!(problems(&t), [3]);
        // a `halt` with no line end does not make the tour stopping
        let t = fold(&format!("{whole}{{\"type\":\"halt\"}}")).unwrap();
        assert_eq!(t.state, State::Writing);
    }

    #[test]
    fn crlf_ends_a_line_like_lf() {
        let text = log(&[
            stop(1, None),
            step(1, 1, None),
            r#"{"type":"end","at":"x"}"#.to_string(),
        ])
        .replace('\n', "\r\n");
        let t = fold(&text).unwrap();
        assert_eq!(route(&t), ["s1:0"]);
        assert_eq!(steps(&t, 1), ["s1", "s1.b1"]);
        assert_eq!(t.state, State::Finished);
        assert!(t.problems.is_empty());
        // a mixture, and a torn last line after a `\r\n`
        let text = format!(
            "{HEAD}\r\n{}\n{}\r\n{{\"type\":\"ha",
            stop(1, None),
            stop(2, None)
        );
        let t = fold(&text).unwrap();
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert!(t.problems.is_empty());
    }

    #[test]
    fn a_second_tour_record_is_one_problem() {
        let t = tour(&[stop(1, None), HEAD.to_string(), stop(2, None)]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert_eq!(problems(&t), [3]);
        assert_eq!(t.title, "T");
    }

    // ---- the route

    #[test]
    fn a_plan_then_the_stops_that_fill_it() {
        let t = tour(&[plan(&[(1, None), (2, Some(1)), (3, None)])]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s3:0"]);
        assert!(
            t.stops
                .iter()
                .all(|s| !s.written && s.text.is_none() && s.kind.is_none())
        );
        assert_eq!(t.stops[1].line, Some(3));
        assert_eq!(t.stops[1].from, StopId::new(1));
        assert_eq!(t.state, State::Writing);

        // filled out of order: each replaces where it stands
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, None)]),
            stop(3, None),
            stop(1, None),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s3:0"]);
        let written: Vec<bool> = t.stops.iter().map(|s| s.written).collect();
        assert_eq!(written, [true, false, true]);
        assert_eq!(t.stops[0].text.as_deref(), Some("intro 1"));
        assert_eq!(t.stops[1].text, None);
        assert_eq!(
            t.size(),
            Size {
                written: 2,
                planned: 1,
                steps: 4,
                minutes: 2
            }
        );

        // a plan after a plan adds the stops that are new; one that is on the route already is not planned again
        let t = tour(&[
            plan(&[(1, None)]),
            stop(1, None),
            plan(&[(1, None), (2, None)]),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert!(t.stops[0].written && !t.stops[1].written);
    }

    #[test]
    fn a_stop_with_no_plan_joins_the_end_of_the_route() {
        let t = tour(&[stop(1, None), stop(2, None), stop(5, None)]);
        assert_eq!(route(&t), ["s1:0", "s2:0", "s5:0"]);
        assert!(t.stops.iter().all(|s| s.written));
        assert_eq!(t.number(StopId::new(5).unwrap()), Some(3));
        assert_eq!(t.number(StopId::new(3).unwrap()), None);
    }

    #[test]
    fn a_stop_that_is_written_again_keeps_its_place_and_loses_its_blocks() {
        let again = stop(2, None).replace("intro 2", "intro two");
        let t = tour(&[
            stop(1, None),
            stop(2, None),
            step(2, 1, None),
            step(2, 2, Some("s2.b1")),
            stop(3, None),
            again,
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0", "s3:0"]);
        assert_eq!(t.stops[1].text.as_deref(), Some("intro two"));
        assert_eq!(steps(&t, 2), ["s2"], "its blocks are gone");
        // a nested stop keeps what is nested in it
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            stop(3, Some(2)),
            stop(1, None),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s3:2"]);
        // the same id with a new `from` moves its depth, not its place in the list
        let t = tour(&[
            stop(1, None),
            stop(2, None),
            stop(3, None),
            stop(3, Some(1)),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:0", "s3:1"]);
        // a stop cannot be nested in itself or in what is nested in it
        let t = tour(&[stop(1, None), stop(2, Some(1)), stop(1, Some(2))]);
        assert_eq!(route(&t), ["s1:0", "s2:1"]);
        let t = tour(&[stop(1, None), stop(1, Some(1))]);
        assert_eq!(route(&t), ["s1:0"]);
    }

    #[test]
    fn nesting_is_by_from_and_a_stop_nested_late_sits_after_its_siblings() {
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            stop(3, None),
            stop(4, Some(1)),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s4:1", "s3:0"]);
        assert_eq!(t.number(StopId::new(4).unwrap()), Some(3));
        // after the whole subtree of its parent, whatever the depth
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            stop(3, Some(2)),
            stop(4, None),
            stop(5, Some(1)),
            stop(6, Some(2)),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s3:2", "s6:2", "s5:1", "s4:0"]);
        // nested in a stop that has none yet: right after it
        let t = tour(&[stop(1, None), stop(2, None), stop(3, Some(1))]);
        assert_eq!(route(&t), ["s1:0", "s3:1", "s2:0"]);
        // planned stops nest as written ones do, and a written stop fills a planned one in place
        let t = tour(&[plan(&[(1, None), (2, None)]), stop(3, Some(1))]);
        assert_eq!(route(&t), ["s1:0", "s3:1", "s2:0"]);
        // a `from` that names nothing: at the top, at the end
        let t = tour(&[stop(1, None), stop(2, Some(9))]);
        assert_eq!(route(&t), ["s1:0", "s2:0"]);
        assert_eq!(t.stops[1].from, None);
        // a `from` that is dropped: at the top
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            r#"{"type":"drop","id":"s1"}"#.to_string(),
            stop(3, Some(1)),
            stop(4, Some(3)),
        ]);
        assert_eq!(route(&t), ["s3:0", "s4:1"]);
        assert_eq!(t.stops[0].from, None);
    }

    #[test]
    fn dropping_a_stop_takes_the_stops_nested_in_it_and_dropping_a_block_takes_its_details() {
        let drop = |id: &str| format!(r#"{{"type":"drop","id":"{id}"}}"#);
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            stop(3, Some(2)),
            stop(4, Some(1)),
            stop(5, None),
            drop("s2"),
        ]);
        assert_eq!(route(&t), ["s1:0", "s4:1", "s5:0"], "s3 was nested in s2");
        let t = tour(&[
            stop(1, None),
            stop(2, Some(1)),
            stop(3, Some(2)),
            stop(4, None),
            drop("s1"),
        ]);
        assert_eq!(route(&t), ["s4:0"]);
        // blocks
        let blocks = [
            stop(1, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b1")),
            step(1, 3, Some("s1.b2")),
            step(1, 4, Some("s1.b1")),
            step(1, 5, None),
        ];
        let with = |extra: &[String]| {
            let mut all = blocks.to_vec();
            all.extend_from_slice(extra);
            tour(&all)
        };
        assert_eq!(
            steps(&with(&[]), 1),
            ["s1", "s1.b1", "s1.b2", "s1.b3", "s1.b4", "s1.b5"]
        );
        assert_eq!(
            steps(&with(&[drop("s1.b2")]), 1),
            ["s1", "s1.b1", "s1.b4", "s1.b5"]
        );
        assert_eq!(steps(&with(&[drop("s1.b1")]), 1), ["s1", "s1.b5"]);
        assert_eq!(
            steps(&with(&[drop("s1.b3")]), 1),
            ["s1", "s1.b1", "s1.b2", "s1.b4", "s1.b5"]
        );
        assert_eq!(
            steps(&with(&[drop("s1.b5")]), 1),
            ["s1", "s1.b1", "s1.b2", "s1.b3", "s1.b4"]
        );
        // a drop of what is not there is nothing
        let t = with(&[drop("s1.b9"), drop("s7"), drop("s7.b1")]);
        assert_eq!(steps(&t, 1).len(), 6);
        assert!(t.problems.is_empty());
        // the intro's id is the stop's: dropping it drops the stop
        assert!(with(&[drop("s1")]).stops.is_empty());
        // an id that is dropped can be written again, and is then a new block at the end
        let t = with(&[drop("s1.b4"), step(1, 4, None)]);
        assert_eq!(
            steps(&t, 1),
            ["s1", "s1.b1", "s1.b2", "s1.b3", "s1.b5", "s1.b4"]
        );
        // a block named as the parent of a block that came after the drop is not there: the new block is at the top
        let t = with(&[drop("s1.b1"), step(1, 6, Some("s1.b1"))]);
        assert_eq!(steps(&t, 1), ["s1", "s1.b5", "s1.b6"]);
    }

    #[test]
    fn a_block_whose_parent_is_missing_or_elsewhere_is_at_the_top() {
        let t = tour(&[
            stop(1, None),
            stop(2, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b7")), // no such block
            step(1, 3, Some("s2.b1")), // a block of another stop (and not there either)
            step(2, 1, None),
            step(1, 4, Some("s2.b1")), // a block that is there, in another stop
            step(1, 5, Some("s1")),    // the intro is not a parent
            step(1, 6, Some("s1.b6")), // itself: not there yet
            step(1, 7, Some("s1.b8")), // a block that comes later
            step(1, 8, None),
        ]);
        assert_eq!(
            steps(&t, 1),
            [
                "s1", "s1.b1", "s1.b2", "s1.b3", "s1.b4", "s1.b5", "s1.b6", "s1.b7", "s1.b8"
            ]
        );
        let top: Vec<String> = t.stops[0].blocks.iter().map(|b| b.id.to_string()).collect();
        assert_eq!(top.len(), 8, "every block but none nested: {top:?}");
        assert!(t.stops[0].blocks.iter().all(|b| b.details.is_empty()));
        assert!(t.problems.is_empty());
    }

    #[test]
    fn a_block_of_a_stop_that_is_not_written_is_one_problem() {
        let t = tour(&[
            step(3, 1, None), // no such stop
            plan(&[(1, None)]),
            step(1, 1, None), // only planned
            stop(1, None),
            step(1, 2, None),
            r#"{"type":"drop","id":"s1"}"#.to_string(),
            step(1, 3, None), // dropped
        ]);
        assert_eq!(problems(&t), [2, 4, 8]);
        assert!(t.stops.is_empty());
        let t = tour(&[plan(&[(1, None)]), step(1, 1, None), stop(1, None)]);
        assert_eq!(
            steps(&t, 1),
            ["s1"],
            "the block that came before its stop is not kept"
        );
    }

    #[test]
    fn a_block_written_again_is_replaced_in_place_and_keeps_its_details() {
        let again = step(1, 1, None)
            .replace("block 1", "new text")
            .replace("\"k1\"", "\"new\"")
            .replace("[2,5]", "[7,9]");
        let t = tour(&[
            stop(1, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b1")),
            step(1, 3, None),
            again,
        ]);
        // the same place among its siblings (first), and its detail still under it
        assert_eq!(steps(&t, 1), ["s1", "s1.b1", "s1.b2", "s1.b3"]);
        let b1 = &t.stops[0].blocks[0];
        assert_eq!(b1.id.to_string(), "s1.b1");
        assert_eq!(
            (b1.text.as_str(), b1.hash.as_str(), b1.lines),
            ("new text", "new", Lines::new(7, 9).unwrap())
        );
        assert_eq!(b1.details.len(), 1);
        assert_eq!(b1.details[0].text, "block 2");
        // a detail replaced in place stays under its parent, in its place; a new `parent` in the record does not move it
        let detail_again = step(1, 3, Some("s1.b1")).replace("block 3", "again");
        let t = tour(&[
            stop(1, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b1")),
            step(1, 3, Some("s1.b2")),
            step(1, 4, Some("s1.b1")),
            detail_again,
        ]);
        assert_eq!(steps(&t, 1), ["s1", "s1.b1", "s1.b2", "s1.b3", "s1.b4"]);
        assert_eq!(t.stops[0].blocks[0].details[0].details[0].text, "again");
        assert!(t.problems.is_empty());
        // a block that was dropped is not replaced: a new block with its id is added at the end
        let t = tour(&[
            stop(1, None),
            step(1, 1, None),
            step(1, 2, None),
            r#"{"type":"drop","id":"s1.b1"}"#.to_string(),
            step(1, 1, None),
        ]);
        assert_eq!(steps(&t, 1), ["s1", "s1.b2", "s1.b1"]);
    }

    #[test]
    fn the_reading_order_of_a_stop_is_its_intro_then_each_block_with_its_details_after_it() {
        // two top blocks, the first with two details, one of which has a detail; added in an order that is not the reading order
        let t = tour(&[
            stop(1, None),
            step(1, 1, None),
            step(1, 4, None),
            step(1, 2, Some("s1.b1")),
            step(1, 5, Some("s1.b2")),
            step(1, 3, Some("s1.b1")),
        ]);
        assert_eq!(
            steps(&t, 1),
            ["s1", "s1.b1", "s1.b2", "s1.b5", "s1.b3", "s1.b4"]
        );
        // the plain case of the design: two blocks, the first with two details
        let t = tour(&[
            stop(1, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b1")),
            step(1, 3, Some("s1.b1")),
            step(1, 4, None),
        ]);
        assert_eq!(steps(&t, 1), ["s1", "s1.b1", "s1.b2", "s1.b3", "s1.b4"]);
        let b1 = &t.stops[0].blocks[0];
        assert_eq!(
            b1.details
                .iter()
                .map(|b| b.id.to_string())
                .collect::<Vec<_>>(),
            ["s1.b2", "s1.b3"]
        );
        assert_eq!(
            (b1.title.as_str(), b1.text.as_str(), b1.hash.as_str()),
            ("b1", "block 1", "k1")
        );
        assert_eq!(b1.lines, Lines::new(2, 5).unwrap());
        // a stop with no blocks, and a planned stop, have their intro only
        let t = tour(&[plan(&[(1, None), (2, None)]), stop(2, None)]);
        assert_eq!(steps(&t, 1), ["s1"]);
        assert_eq!(steps(&t, 2), ["s2"]);
    }

    // ---- the first line's values, ids, pin

    #[test]
    fn meta_replaces_what_it_names() {
        let t = tour(&[
            stop(1, None),
            r#"{"type":"meta","title":"Second title"}"#.to_string(),
            r#"{"type":"meta","question":"why?","detail":"detailed"}"#.to_string(),
        ]);
        assert_eq!(t.title, "Second title");
        assert_eq!(t.question.as_deref(), Some("why?"));
        assert_eq!(t.detail, Detail::Detailed);
        assert_eq!(
            t.id.as_str(),
            "t-0000",
            "the id does not change with the title"
        );
        assert_eq!(t.author, None);
        // what it does not name stays
        let t = tour(&[
            r#"{"type":"meta"}"#.to_string(),
            r#"{"type":"meta","title":"A"}"#.to_string(),
            r#"{"type":"meta","title":"B"}"#.to_string(),
        ]);
        assert_eq!((t.title.as_str(), t.detail), ("B", Detail::Normal));
    }

    #[test]
    fn ids_sets_the_node_of_a_stop() {
        let ids = |pairs: &[(u32, &str)]| {
            let list: Vec<String> = pairs
                .iter()
                .map(|(n, id)| format!(r#""s{n}":"{id}""#))
                .collect();
            format!(r#"{{"type":"ids","nodes":{{{}}}}}"#, list.join(","))
        };
        let t = tour(&[stop(1, None), stop(2, None), ids(&[(2, NODE), (9, NODE)])]);
        assert_eq!(t.stops[0].node, None);
        assert_eq!(
            t.stops[1].node.map(|n| n.to_string()).as_deref(),
            Some(NODE)
        );
        assert!(
            t.problems.is_empty(),
            "an ids for a stop that is not there is nothing"
        );
        // it replaces a node, and a stop written again has what its own record says
        let other = "3M1X9C7B5N2V8K4J6H0G1F3D5S";
        let t = tour(&[stop(1, None), ids(&[(1, NODE)]), ids(&[(1, other)])]);
        assert_eq!(
            t.stops[0].node.map(|n| n.to_string()).as_deref(),
            Some(other)
        );
        let t = tour(&[stop(1, None), ids(&[(1, NODE)]), stop(1, None)]);
        assert_eq!(t.stops[0].node, None);
        // a stop that has a node in its own record
        let with_node = stop(1, None).replace(r#""lines""#, &format!(r#""node":"{NODE}","lines""#));
        let t = tour(&[with_node]);
        assert_eq!(
            t.stops[0].node.map(|n| n.to_string()).as_deref(),
            Some(NODE)
        );
    }

    fn pin() -> String {
        r#"{"type":"pin","commit":"f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a","at":"x","where":{"s1":["a.py",1,20]}}"#.to_string()
    }

    #[test]
    fn a_pin_holds_until_a_stop_a_step_or_a_drop_comes_after_it() {
        let end = r#"{"type":"end","at":"x"}"#.to_string();
        let drop = r#"{"type":"drop","id":"s9"}"#.to_string();
        let ids = format!(r#"{{"type":"ids","nodes":{{"s1":"{NODE}"}}}}"#);
        let cases: Vec<(&str, Vec<String>, bool)> = vec![
            ("no pin", vec![stop(1, None)], false),
            (
                "a pin after the only stop",
                vec![stop(1, None), pin()],
                true,
            ),
            ("a pin with no stops at all", vec![pin()], true),
            (
                "a stop after the pin",
                vec![stop(1, None), pin(), stop(2, None)],
                false,
            ),
            (
                "a step after the pin",
                vec![stop(1, None), pin(), step(1, 1, None)],
                false,
            ),
            (
                "a drop after the pin (of anything)",
                vec![stop(1, None), pin(), drop.clone()],
                false,
            ),
            (
                "a second pin after that",
                vec![stop(1, None), pin(), step(1, 1, None), pin()],
                true,
            ),
            (
                "an end, an ids, a halt after the pin",
                vec![
                    stop(1, None),
                    pin(),
                    end.clone(),
                    ids,
                    r#"{"type":"halt"}"#.to_string(),
                ],
                true,
            ),
            (
                "a plan after the pin",
                vec![stop(1, None), pin(), plan(&[(2, None)])],
                true,
            ),
            (
                "a stop before the pin, a broken line after it",
                vec![stop(1, None), pin(), "broken".to_string()],
                true,
            ),
            (
                "a stop that is broken after the pin",
                vec![
                    stop(1, None),
                    pin(),
                    r#"{"type":"stop","id":"s2"}"#.to_string(),
                ],
                true,
            ),
            (
                "the pin first, then the stop, then the end",
                vec![pin(), stop(1, None), end],
                false,
            ),
        ];
        for (what, lines, pinned) in cases {
            let t = tour(&lines);
            assert_eq!(t.pinned(), pinned, "{what}");
            assert_eq!(
                t.pin.is_some(),
                lines.iter().any(|l| l.contains("\"pin\"")),
                "{what}"
            );
        }
        let t = tour(&[stop(1, None), pin()]);
        let pin = t.pin.unwrap();
        assert_eq!(
            pin.commit.as_str(),
            "f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a"
        );
        assert_eq!(pin.places.0.len(), 1);
        assert_eq!(pin.places.0[0].0.to_string(), "s1");
        assert_eq!(pin.places.0[0].1.path.as_str(), "a.py");
    }

    #[test]
    fn a_pin_a_step_after_it_and_a_second_pin() {
        let a = tour(&[stop(1, None), pin()]);
        assert!(a.pinned());
        let b = tour(&[stop(1, None), pin(), step(1, 1, None)]);
        assert!(!b.pinned(), "the pin is still there, and no longer holds");
        assert!(b.pin.is_some());
        let c = tour(&[stop(1, None), pin(), step(1, 1, None), pin()]);
        assert!(c.pinned());
    }

    // ---- the state

    #[test]
    fn the_state_from_halt_halted_resume_and_end() {
        use State::*;
        let table: Vec<(Vec<&str>, State)> = vec![
            (vec![], Writing),
            (vec!["halt"], Stopping),
            (vec!["halt", "halted"], Stopped),
            (vec!["halt", "halted", "resume"], Writing),
            (vec!["halt", "resume"], Writing),
            (vec!["halt", "halted", "resume", "halt"], Stopping),
            (vec!["halt", "halted", "resume", "halt", "halted"], Stopped),
            (vec!["halt", "halted", "halt"], Stopping),
            (vec!["halted"], Stopped),
            (vec!["resume"], Writing),
            (vec!["end"], Finished),
            (vec!["halt", "end"], Finished),
            (vec!["halt", "halted", "end"], Finished),
            (vec!["halt", "halted", "resume", "end"], Finished),
            (vec!["end", "halt"], Finished),
            (vec!["end", "halt", "halted"], Finished),
            (vec!["end", "halt", "halted", "resume"], Finished),
            (vec!["end", "end"], Finished),
        ];
        let line = |kind: &str| match kind {
            "end" => r#"{"type":"end","at":"x"}"#.to_string(),
            other => format!(r#"{{"type":"{other}"}}"#),
        };
        for (sequence, want) in &table {
            let mut all = vec![stop(1, None)];
            all.extend(sequence.iter().map(|k| line(k)));
            let t = tour(&all);
            assert_eq!(t.state, *want, "{sequence:?}");
            assert_eq!(t.ended.is_some(), sequence.contains(&"end"), "{sequence:?}");
        }
        // every order of up to four of the four kinds, against the table of 01 §4 read as a rule:
        // an `end` anywhere is finished; else the last of halt, halted and resume says; else writing
        let kinds = ["halt", "halted", "resume", "end"];
        let mut sequences: Vec<Vec<&str>> = vec![vec![]];
        for _ in 0..4 {
            let longer: Vec<Vec<&str>> = sequences
                .iter()
                .flat_map(|s| kinds.iter().map(move |k| [s.as_slice(), &[*k]].concat()))
                .collect();
            for s in &longer {
                let want = if s.contains(&"end") {
                    Finished
                } else {
                    match s.last() {
                        Some(&"halt") => Stopping,
                        Some(&"halted") => Stopped,
                        _ => Writing,
                    }
                };
                let all: Vec<String> = s.iter().map(|k| line(k)).collect();
                assert_eq!(tour(&all).state, want, "{s:?}");
            }
            sequences = longer;
        }
        assert_eq!(sequences.len(), 256);
        // a stop, a step and a drop after an end are edits of a finished tour: it stays finished
        let t = tour(&[
            stop(1, None),
            line("end"),
            stop(2, None),
            step(2, 1, None),
            r#"{"type":"drop","id":"s1"}"#.to_string(),
        ]);
        assert_eq!(t.state, State::Finished);
        assert_eq!(route(&t), ["s2:0"]);
        assert_eq!(State::Stopping.to_string(), "stopping");
    }

    #[test]
    fn an_end_takes_the_planned_stops_off_the_route_and_a_stop_nested_in_one_moves_up() {
        let end = || r#"{"type":"end","at":"x"}"#.to_string();
        // the planned stop was at the top: its nested stop goes to the top, and its line goes
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, None), (4, Some(3))]),
            stop(1, None),
            stop(4, Some(3)),
            end(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s4:0"]);
        assert_eq!((t.stops[1].from, t.stops[1].line), (None, None));
        // the planned stop was nested: its nested stop moves up to the planned stop's own `from`, with no line
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, Some(2))]),
            stop(1, None),
            stop(3, Some(2)),
            end(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s3:1"]);
        assert_eq!((t.stops[1].from, t.stops[1].line), (StopId::new(1), None));
        // several levels of planned stops leave at once: up to the nearest one that stays
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3))]),
            stop(1, None),
            stop(4, Some(3)),
            end(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s4:1"]);
        assert_eq!((t.stops[1].from, t.stops[1].line), (StopId::new(1), None));
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3))]),
            stop(1, None),
            stop(2, Some(1)),
            stop(4, Some(3)),
            end(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1", "s4:2"], "s3 left, s2 stays");
        assert_eq!(t.stops[2].from, StopId::new(2));
        // a stop whose parent stays keeps its `from` and its line
        let t = tour(&[
            plan(&[(1, None), (2, Some(1)), (3, Some(1))]),
            stop(1, None),
            stop(2, Some(1)),
            end(),
        ]);
        assert_eq!(route(&t), ["s1:0", "s2:1"]);
        assert_eq!(
            (t.stops[1].from, t.stops[1].line),
            (StopId::new(1), Some(3))
        );
        // before the end, they are there
        let t = tour(&[plan(&[(1, None), (2, Some(1))]), stop(1, None)]);
        assert_eq!(route(&t), ["s1:0", "s2:1"]);
        assert_eq!(t.size().planned, 1);
    }

    // ---- sizes

    #[test]
    fn the_sizes_and_the_minutes() {
        let size = |lines: &[String]| tour(lines).size();
        // (steps, minutes): 20 seconds a step, up to a whole minute
        assert_eq!(
            size(&[]),
            Size {
                written: 0,
                planned: 0,
                steps: 0,
                minutes: 0
            }
        );
        assert_eq!(
            size(&[stop(1, None)]),
            Size {
                written: 1,
                planned: 0,
                steps: 1,
                minutes: 1
            }
        );
        assert_eq!(
            size(&[stop(1, None), step(1, 1, None), step(1, 2, None)]).minutes,
            1,
            "3 steps: exactly 60 s"
        );
        assert_eq!(
            size(&[
                stop(1, None),
                step(1, 1, None),
                step(1, 2, None),
                step(1, 3, None)
            ]),
            Size {
                written: 1,
                planned: 0,
                steps: 4,
                minutes: 2
            }
        );
        // blocks of every depth count; a planned stop counts two
        let t = size(&[
            plan(&[(1, None), (2, None), (3, None)]),
            stop(1, None),
            step(1, 1, None),
            step(1, 2, Some("s1.b1")),
            step(1, 3, Some("s1.b2")),
            stop(2, None),
        ]);
        assert_eq!(
            t,
            Size {
                written: 2,
                planned: 1,
                steps: 4 + 1 + 2,
                minutes: 3
            }
        );
        // 9 steps is exactly 3 minutes; 10 is 4
        let nine: Vec<String> = std::iter::once(stop(1, None))
            .chain((1..=8).map(|m| step(1, m, None)))
            .collect();
        assert_eq!(
            size(&nine),
            Size {
                written: 1,
                planned: 0,
                steps: 9,
                minutes: 3
            }
        );
        let ten: Vec<String> = nine.iter().cloned().chain([step(1, 9, None)]).collect();
        assert_eq!(size(&ten).minutes, 4);
        // a dropped stop, and a dropped block, are not counted
        let t = size(&[
            stop(1, None),
            step(1, 1, None),
            stop(2, None),
            r#"{"type":"drop","id":"s2"}"#.to_string(),
            r#"{"type":"drop","id":"s1.b1"}"#.to_string(),
        ]);
        assert_eq!(
            t,
            Size {
                written: 1,
                planned: 0,
                steps: 1,
                minutes: 1
            }
        );
    }
}
