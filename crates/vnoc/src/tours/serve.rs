//! What the commands and the core answer with, from one place: the three rungs of a tour's freshness (03 §3), and the views built from a read tour, a
//! check and a place. `vnoc tour show`, `list`, `where` and `go` print them; `vnoc/tour`, `vnoc/tours` and `vnoc/tourGo` (`server.rs`) answer with them. Nothing
//! here prints, and nothing here starts a driver: the caller that needs the index gives it ([`check_needed`]).

use std::collections::BTreeMap;
use std::path::Path;

use super::add::block_of;
use super::check::{self, Answer, Code, Mode, Recorded, Source, StopState};
use super::place::{self, At, Place};
use super::record::{Commit, StepId, StopId};
use super::store::{Row, StoreError};
use super::{Tour, TourId, pin, time, view};
use crate::project::Project;
use crate::source;

/// A tour to find the freshness of: what the check is made true of is the file as it was **before** it was read.
pub struct Subject<'a> {
    pub id: &'a TourId,
    pub path: &'a Path,
    pub len: u64,
    pub tour: &'a Tour,
}

pub fn subject(row: &Row) -> Option<Subject<'_>> {
    row.tour.as_ref().ok().map(|tour| Subject {
        id: &row.id,
        path: &row.path,
        len: row.len,
        tour,
    })
}

/// The first two rungs for each tour, by id (03 §3): the recorded check when it still holds (`stat`s, no index), else a tour with no written stop (nothing to look
/// at: recorded now), else it is one that needs a check ([`Resolved::need`]).
pub struct Resolved<'s, 't> {
    pub done: BTreeMap<TourId, Answer>,
    /// the subjects the third rung has to check: give them to [`check_needed`] with the code
    pub need: Vec<&'s Subject<'t>>,
}

pub fn resolve<'s, 't>(
    vnoc_root: &Path,
    project_root: &Path,
    subjects: &'s [Subject<'t>],
    mode: Mode,
) -> Result<Resolved<'s, 't>, StoreError> {
    let mut done = BTreeMap::new();
    let mut need = Vec::new();
    for s in subjects {
        if done.contains_key(s.id) {
            continue;
        }
        if !mode.again
            && let Some(recorded) = check::recall(vnoc_root, project_root, s.id, s.path)
        {
            done.insert(
                s.id.clone(),
                Answer {
                    from: Source::Record,
                    recorded,
                },
            );
        } else if s.tour.stops.iter().any(|stop| stop.written) {
            need.push(s);
        } else {
            let recorded = Recorded::empty(s.len);
            check::record(vnoc_root, s.id, &recorded)?;
            done.insert(
                s.id.clone(),
                Answer {
                    from: Source::Check,
                    recorded,
                },
            );
        }
    }
    Ok(Resolved { done, need })
}

/// The third rung: a check of each tour that needs one (each file read once), recorded, put into `done`.
pub fn check_needed(
    code: &Code,
    vnoc_root: &Path,
    need: &[&Subject<'_>],
    done: &mut BTreeMap<TourId, Answer>,
) -> Result<(), StoreError> {
    for s in need {
        let mut read = |path: &Path| source::read_text(path);
        let recorded = check::check(code, &mut read, s.tour, s.len);
        check::record(vnoc_root, s.id, &recorded)?;
        done.insert(
            s.id.clone(),
            Answer {
                from: Source::Check,
                recorded,
            },
        );
    }
    Ok(())
}

pub fn short_commit(commit: &Commit) -> String {
    commit.as_str()[..7].to_string()
}

/// Puts the code of the pinned commit on each stop that a check found changed or gone (`show --as-written`, 03 §5): the stop's lines as the pin's
/// `where` has them, and the lines its blocks light there, exactly (a stop with no blocks lights all of it). A stop the commit cannot give (it is not
/// in this repository, or has no such file) is shown as one that is not pinned is.
pub fn written_code(
    project: &Project,
    pin: &crate::tours::record::PinRecord,
    view: &mut view::TourView,
) {
    let at = |step: StepId| {
        pin.places
            .0
            .iter()
            .find(|(id, _)| *id == step)
            .map(|(_, w)| w)
    };
    for stop in &mut view.stops {
        if !matches!(stop.check, Some("changed" | "gone")) {
            continue;
        }
        let Ok(id) = stop.id.parse::<StopId>() else {
            continue;
        };
        let Some(place) = at(StepId::intro(id)) else {
            continue;
        };
        let lit: Vec<[u32; 2]> = if stop.blocks.is_empty() {
            vec![[place.lines.first, place.lines.last]]
        } else {
            stop.blocks
                .iter()
                .filter_map(|b| b.id.parse::<StepId>().ok().and_then(at))
                .map(|w| [w.lines.first, w.lines.last])
                .collect()
        };
        if let Some(code) = pin::as_written(
            project,
            pin.commit.as_str(),
            place.path.as_str(),
            place.lines.first,
            place.lines.last,
        ) {
            stop.as_written = Some(view::AsWritten {
                commit: short_commit(&pin.commit),
                path: place.path.to_string(),
                lines: [place.lines.first, place.lines.last],
                code,
                lit,
            });
        }
    }
}

/// The lines `first..=last` (1-based) of a file of the project as it is now, split as everything here splits source. `None` when it cannot be read
/// or has no such lines.
pub fn lines_now(project: &Project, path: &str, first: u32, last: u32) -> Option<Vec<String>> {
    let text = source::read_text(&project.git_root().join(path)).ok()?;
    let all = source::split_lines(&text);
    let from = (first.max(1) - 1) as usize;
    if from >= all.len() || last < first {
        return None;
    }
    let to = (last as usize).min(all.len());
    Some(all[from..to].iter().map(|l| (*l).to_string()).collect())
}

/// The code under a step, as `where` shows it (02 §7): fresh or moved, the lines now; changed or gone in a pinned tour whose commit gives the lines,
/// those, with the line that says so; otherwise the lines now, if there are any, and `the lines may not match`. At most [`view::WHERE_CODE_LINES`].
pub fn where_code(
    project: &Project,
    tour: &Tour,
    step: StepId,
    state: &str,
    path: &str,
    lines: Option<[u32; 2]>,
) -> Option<view::WhereCode> {
    let code = |source, commit, path: &str, first: u32, mut text: Vec<String>, note| {
        let more = text.len().saturating_sub(view::WHERE_CODE_LINES);
        text.truncate(view::WHERE_CODE_LINES);
        view::WhereCode {
            source,
            commit,
            path: path.to_string(),
            first,
            lines: text,
            more,
            note,
        }
    };
    let pinned = tour.pin.as_ref().filter(|_| tour.pinned());
    if matches!(state, "changed" | "gone")
        && let Some(pin) = pinned
        && let Some((_, written)) = pin.places.0.iter().find(|(id, _)| *id == step)
        && let Some(text) = pin::as_written(
            project,
            pin.commit.as_str(),
            written.path.as_str(),
            written.lines.first,
            written.lines.last,
        )
    {
        let short = short_commit(&pin.commit);
        return Some(code(
            "as_written",
            Some(short.clone()),
            written.path.as_str(),
            written.lines.first,
            text,
            Some(format!("as written at {short}: the code changed since")),
        ));
    }
    let [first, last] = lines?;
    let text = lines_now(project, path, first, last)?;
    let note = matches!(state, "changed" | "gone").then(|| "the lines may not match".to_string());
    Some(code("now", None, path, first, text, note))
}

/// The view of the tour's place: `place` is one the tour has ([`place::settle`]); `recorded` is a check of the tour that holds.
pub fn where_view(
    project: &Project,
    tour: &Tour,
    place: &Place,
    recorded: &Recorded,
) -> view::WhereView {
    let (before, after) = place::around(tour, place.at);
    let neighbour = |at: At| -> view::Neighbour {
        let title = match at {
            At::Step(step) if !step.is_intro() => tour
                .stop(step.stop())
                .and_then(|s| block_of(s, step))
                .map(|(b, _)| b.title.clone()),
            _ => None,
        };
        view::Neighbour {
            at: at.to_string(),
            title,
        }
    };
    let moved = (!place.moved.is_empty()).then(|| place.moved.clone());
    let moved_ago = moved
        .as_deref()
        .and_then(time::parse)
        .map(|then| time::parse(&time::now()).unwrap_or(then) - then);
    let mut view = view::WhereView {
        tour: tour.id.to_string(),
        title: tour.title.clone(),
        question: tour.question.clone(),
        state: view::state_word(tour.state),
        open: place.open,
        by: moved.as_ref().map(|_| place.by),
        moved,
        moved_ago,
        at: place.at.to_string(),
        here: match place.at {
            At::Cover => "cover",
            At::End => "end",
            At::Step(step) if step.is_intro() => "intro",
            At::Step(_) => "block",
        },
        stops: tour.stops.len(),
        steps: place::steps(tour).len(),
        stop: None,
        step: None,
        check: None,
        code: None,
        // the cover has nothing before it and the end nothing after: `prev` and `next` stay
        before: (place.at != At::Cover).then(|| neighbour(before)),
        after: (place.at != At::End).then(|| neighbour(after)),
        seen: place.seen.iter().map(ToString::to_string).collect(),
        back: place.back.iter().map(ToString::to_string).collect(),
    };
    let At::Step(step) = place.at else {
        return view;
    };
    let (Some(stop), Some(position)) = (tour.stop(step.stop()), place::position(tour, place.at))
    else {
        return view;
    };
    view.stop = Some(view::WhereStop {
        id: stop.id.to_string(),
        number: position.stop,
        of: position.stops,
        name: if stop.qualname.is_empty() {
            stop.path.to_string()
        } else {
            stop.qualname.clone()
        },
        kind: stop.kind.as_ref().map(|k| format!("{k:?}").to_lowercase()),
        path: stop.path.to_string(),
        planned: !stop.written,
    });
    let block = (!step.is_intro())
        .then(|| block_of(stop, step).map(|(b, _)| b))
        .flatten();
    let mut shown = view::WhereStep {
        id: step.to_string(),
        kind: if step.is_intro() { "intro" } else { "block" },
        number: position.step,
        of: position.steps,
        title: block.map(|b| b.title.clone()),
        text: match block {
            Some(b) => Some(b.text.clone()),
            None => stop.text.clone(),
        },
        lines: None,
    };
    if stop.written {
        // what a check says of this step, and where its lines are now: a stop that changed has `now`, one that only moved has `displaced`, a block
        // that moved has its new lines; else the lines it was written with
        let stop_state = recorded.stop(stop.id);
        let state = match (stop_state, block) {
            (StopState::Gone, _) => "gone",
            (StopState::Changed, None) => "changed",
            (_, None) => "fresh",
            (_, Some(b)) => match recorded.block(b.id) {
                check::BlockState::Fresh => "fresh",
                check::BlockState::Moved(_) => "moved",
                check::BlockState::Changed => "changed",
            },
        };
        let found = recorded
            .now
            .get(&stop.id)
            .or(recorded.displaced.get(&stop.id));
        let path = found.map_or_else(|| stop.path.to_string(), |n| n.path.clone());
        // how far the node is from where it was written: a block that is not `moved` is where it was **counted from the node's first line**
        // (check.rs `walk`), cut to the node's end
        let (delta, node_last) = match (found.and_then(|n| n.lines), stop.lines) {
            (Some([first, last]), Some(written)) => {
                (i64::from(first) - i64::from(written.first), last)
            }
            (Some([_, last]), None) => (0, last),
            (None, _) => (0, u32::MAX),
        };
        let shifted = |line: u32| u32::try_from((i64::from(line) + delta).max(1)).unwrap_or(line);
        let lines = match (block, stop_state) {
            (_, StopState::Gone) => None,
            (Some(b), _) => Some(match recorded.block(b.id) {
                check::BlockState::Moved(l) => [l.first, l.last],
                check::BlockState::Fresh => [shifted(b.lines.first), shifted(b.lines.last)],
                check::BlockState::Changed => {
                    [shifted(b.lines.first), shifted(b.lines.last).min(node_last)]
                }
            }),
            (None, _) => found
                .and_then(|n| n.lines)
                .or_else(|| stop.lines.map(|l| [l.first, l.last])),
        };
        shown.lines = lines;
        view.check = Some(state);
        view.code = where_code(project, tour, step, state, &path, lines);
    }
    view.step = Some(shown);
    view
}
