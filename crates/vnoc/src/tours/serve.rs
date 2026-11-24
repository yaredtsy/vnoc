//! What the commands and the core answer with, from one place: the three rungs of a tour's freshness (03 §3), and the views built from a read tour, a
//! check and a place. `vnoc tour show`, `list`, `where` and `go` print them; `vnoc/tour`, `vnoc/tours` and `vnoc/tourGo` (`server.rs`) answer with them. Nothing
//! here prints, and nothing here starts a driver: the caller that needs the index gives it ([`check_needed`]).

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::SystemTime;

use super::add::{block_of, did_you_mean_stops};
use super::check::{self, Answer, Code, Mode, Recorded, Source, StopState};
use super::on::{self, On, Query};
use super::place::{self, At, By, Move, Place};
use super::record::{Commit, StepId, StopId};
use super::settings;
use super::store::{self, Halt, Row, StoreError, Where};
use super::{Tour, TourId, pin, steps_in_order, time, view};
use crate::index::Index;
use crate::project::Project;
use crate::source;
use crate::userdata::{ToursLock, layout};

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

/// One stop's code as the pinned commit has it (03 §5): the stop's lines as the pin's `where` has them, and the lines its blocks light there, exactly
/// (a stop with no blocks lights all of it). `None` when the pin has no place for the stop (a folder has none), or the commit cannot give the file: it is not
/// in this repository, or has no such file.
pub fn as_written_of(
    project: &Project,
    pin: &crate::tours::record::PinRecord,
    stop: StopId,
    blocks: &[StepId],
) -> Option<view::AsWritten> {
    let at = |step: StepId| {
        pin.places
            .0
            .iter()
            .find(|(id, _)| *id == step)
            .map(|(_, w)| w)
    };
    let place = at(StepId::intro(stop))?;
    let lit: Vec<[u32; 2]> = if blocks.is_empty() {
        vec![[place.lines.first, place.lines.last]]
    } else {
        blocks
            .iter()
            .filter_map(|b| at(*b))
            .map(|w| [w.lines.first, w.lines.last])
            .collect()
    };
    let code = pin::as_written(
        project,
        pin.commit.as_str(),
        place.path.as_str(),
        place.lines.first,
        place.lines.last,
    )?;
    Some(view::AsWritten {
        commit: short_commit(&pin.commit),
        path: place.path.to_string(),
        lines: [place.lines.first, place.lines.last],
        code,
        lit,
    })
}

fn block_ids(stop: &view::StopView) -> Vec<StepId> {
    stop.blocks
        .iter()
        .filter_map(|b| b.id.parse::<StepId>().ok())
        .collect()
}

/// Puts the code of the pinned commit on each stop that a check found changed or gone (`show --as-written`, 03 §5): [`as_written_of`] for each. A stop the commit
/// cannot give is shown as one that is not pinned is.
pub fn written_code(
    project: &Project,
    pin: &crate::tours::record::PinRecord,
    view: &mut view::TourView,
) {
    for stop in &mut view.stops {
        if !matches!(stop.check, Some("changed" | "gone")) {
            continue;
        }
        let Ok(id) = stop.id.parse::<StopId>() else {
            continue;
        };
        stop.as_written = as_written_of(project, pin, id, &block_ids(stop));
    }
}

/// `vnoc/tourCode`: one stop's code as written, for any stop of a pinned tour, changed or not. `None` when the tour is not pinned, the commit is not in this
/// repository, or the stop is planned (it has no code yet). A stop the tour does not have is [`GoError::NoSuch`], in the words `go` uses.
pub fn stop_code(
    project: &Project,
    tour: &Tour,
    stop: StopId,
) -> Result<Option<view::AsWritten>, GoError> {
    let Some(found) = tour.stop(stop) else {
        return Err(no_such_stop(tour, stop));
    };
    let Some(pin) = tour.pin.as_ref().filter(|_| tour.pinned()) else {
        return Ok(None);
    };
    if !found.written {
        return Ok(None);
    }
    let shown = view::tour_view(tour, Where::Private);
    let blocks = shown
        .stops
        .iter()
        .find(|s| s.id == stop.to_string())
        .map(block_ids)
        .unwrap_or_default();
    Ok(as_written_of(project, pin, stop, &blocks))
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
        let lines = view::step_lines(
            stop.lines.map(|l| [l.first, l.last]),
            found,
            stop_state,
            block.map(|b| ([b.lines.first, b.lines.last], recorded.block(b.id))),
        );
        shown.lines = lines;
        view.check = Some(state);
        view.code = where_code(project, tour, step, state, &path, lines);
    }
    view.step = Some(shown);
    view
}

// ----------------------------------------------------------------------------------------------------------------- the views of a command and a request

/// The rows of `list` and of `vnoc/tours`: [`view::list_rows`], and for each readable tour the three things a row says that the replay alone does not: where the
/// reader is (`place_of`: the place as it is written, `None` for a tour nobody has opened), and whether the tour is quiet (its file's time).
pub fn list_rows<R: Borrow<Row>>(
    project: &Project,
    rows: &[R],
    known: &BTreeMap<TourId, Recorded>,
    say_fresh: bool,
    place_of: &dyn Fn(&TourId) -> Option<Place>,
) -> Vec<view::ListRowView> {
    let mut out = view::list_rows(rows, known, say_fresh, &|commit| {
        pin::has_commit(project, commit)
    });
    let offering = Offering::of(project);
    let readable: Vec<&Tour> = rows
        .iter()
        .filter_map(|row| row.borrow().tour.as_ref().ok())
        .collect();
    let mut offers = offering.offers_of(project, &readable).into_iter();
    for (shown, row) in out.iter_mut().zip(rows) {
        let row: &Row = row.borrow();
        let Ok(tour) = &row.tour else { continue };
        shown.offers = offers.next().flatten();
        let age = std::fs::metadata(&row.path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .map(|d| d.as_secs());
        shown.quiet = view::is_quiet(tour.state, age);
        shown.place = view::place_summary(tour, place_of(&row.id).as_ref());
    }
    out
}

/// The view of a tour as `show` and `vnoc/tour` have it, before a check is put on it: the pin (whether git has the commit), and the place as the tour is now.
pub fn show_view(
    project: &Project,
    tour: &Tour,
    folder: Where,
    place: Option<&Place>,
) -> view::TourView {
    let mut shown = view::tour_view(tour, folder);
    if let Some(pin) = tour.pin.as_ref().filter(|_| tour.pinned()) {
        shown.pin_in_repository = Some(pin::has_commit(project, &pin.commit));
    }
    shown.place = place::settle(tour, &place.cloned().unwrap_or_default());
    shown.offers = Offering::of(project).offers(project, tour);
    shown
}

/// What the cover of a finished tour may offer (05 §3), by the two settings of `[tours]`. Read once for a list of rows.
pub struct Offering {
    ids: settings::Choice,
    commit: settings::Choice,
    /// the project writes ID lines at all (`[ids] default` is not `none`)
    writes_ids: bool,
}

impl Offering {
    /// The settings as `vnoc.toml` has them. A file that cannot be read offers nothing: the settings command says why.
    pub fn of(project: &Project) -> Offering {
        let never = Offering {
            ids: settings::Choice::Never,
            commit: settings::Choice::Never,
            writes_ids: false,
        };
        let Ok(read) = settings::read(&store::vnoc_root(project.git_root())) else {
            return never;
        };
        Offering {
            ids: read.ids.value,
            commit: read.commit.value,
            writes_ids: super::act::project_writes_ids(project).unwrap_or(false),
        }
    }

    /// `ids`: the written stops that have no node ID, when `[tours] ids` is `"ask"` and the project writes ID lines. `commit`: the files that keep the tour
    /// from being pinned because they are not committed, when `[tours] commit` is `"ask"` and the tour is not pinned at a commit HEAD has behind it
    /// ([`pin::uncommitted_files`]: by git alone). `None` when neither has anything, and for a tour that is not finished.
    pub fn offers(&self, project: &Project, tour: &Tour) -> Option<view::OffersView> {
        self.offers_of(project, &[tour]).pop().flatten()
    }

    /// [`Offering::offers`] for the rows of a list: **one** `git status` for all the rows (the union of their paths), and one question "does HEAD have this
    /// commit behind it" for each distinct pinned commit. One answer per tour, in the order given; the same answers as asking one tour at a time.
    pub fn offers_of(&self, project: &Project, tours: &[&Tour]) -> Vec<Option<view::OffersView>> {
        let asks = self.commit == settings::Choice::Ask;
        let mut reachable: BTreeMap<String, bool> = BTreeMap::new();
        // the tours whose commit files are looked at: finished, and not pinned at a commit HEAD has behind it
        let looked_at: Vec<bool> = tours
            .iter()
            .map(|tour| {
                if !asks || tour.state != super::State::Finished {
                    return false;
                }
                let held = tour
                    .pin
                    .as_ref()
                    .filter(|_| tour.pinned())
                    .is_some_and(|pin| {
                        *reachable
                            .entry(pin.commit.as_str().to_string())
                            .or_insert_with(|| pin::reachable(project, &pin.commit))
                    });
                !held
            })
            .collect();
        let candidates: Vec<&Tour> = tours
            .iter()
            .zip(&looked_at)
            .filter(|(_, looked)| **looked)
            .map(|(tour, _)| *tour)
            .collect();
        let mut files = pin::uncommitted_files_of(project, &candidates).into_iter();
        tours
            .iter()
            .zip(&looked_at)
            .map(|(tour, looked)| {
                if tour.state != super::State::Finished {
                    return None;
                }
                let ids = if self.ids == settings::Choice::Ask && self.writes_ids {
                    tour.stops
                        .iter()
                        .filter(|s| s.written && s.node.is_none())
                        .count()
                } else {
                    0
                };
                let commit: Vec<String> = if *looked {
                    files
                        .next()
                        .unwrap_or_default()
                        .iter()
                        .map(|f| f.path.to_string())
                        .collect()
                } else {
                    Vec::new()
                };
                (ids > 0 || !commit.is_empty()).then_some(view::OffersView { ids, commit })
            })
            .collect()
    }
}

/// The rows of `list --on` and of `vnoc/tours { on }`: only the tours that pass through the node, in the order of `rows`, each with how (`on`). `tours` are the
/// rows' own tours, one for one, and `found` is what the map said.
pub fn rows_on<R: Borrow<Row>>(
    rows: Vec<view::ListRowView>,
    tours: &[R],
    found: &[On],
) -> Vec<view::ListRowView> {
    rows.into_iter()
        .zip(tours)
        .filter_map(|(mut shown, row)| {
            let row: &Row = row.borrow();
            let on = found
                .iter()
                .find(|on| on.tour == row.id && on.place == row.place)?;
            shown.on = Some(view::OnView::of(on));
            Some(shown)
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------------------------------------ a move

/// Why a move was not made.
#[derive(Debug)]
pub enum GoError {
    Store(StoreError),
    /// a jump to a stop or a block the tour does not have: `code` is `no_stop` or `no_block`, `near` what could have been meant
    NoSuch {
        code: &'static str,
        said: String,
        near: Vec<String>,
    },
}

impl From<StoreError> for GoError {
    fn from(e: StoreError) -> GoError {
        GoError::Store(e)
    }
}

impl std::fmt::Display for GoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoError::Store(e) => e.fmt(f),
            GoError::NoSuch { said, .. } => f.write_str(said),
        }
    }
}

/// What a move did.
pub struct Gone {
    pub tour: Tour,
    /// the place now: written, unless `nothing_to_go_back_to`
    pub place: Place,
    /// `back` with nothing to pop: the place stays and nothing is written
    pub nothing_to_go_back_to: bool,
}

/// A stop the tour does not have: what `go` says, and the stops near the one asked for.
pub fn no_such_stop(tour: &Tour, stop: StopId) -> GoError {
    GoError::NoSuch {
        code: "no_stop",
        said: format!("no stop {stop} in this tour"),
        near: did_you_mean_stops(tour, stop)
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}

/// The tour as it is, or why it cannot be moved in.
fn readable_tour(measured: &store::Measured, id: &TourId) -> Result<Tour, StoreError> {
    match &measured.tour {
        Ok(tour) => Ok(tour.clone()),
        Err(e) => Err(StoreError::Unreadable {
            id: id.clone(),
            why: e.why.clone(),
            too_new: e.too_new,
        }),
    }
}

/// `vnoc tour go`, and `vnoc/tourGo`: the tours lock holds from reading the tour and the place to writing the place, so two moves made together each apply to
/// the place as it is then. Blocks (the lock); the core calls it from `spawn_blocking`.
pub fn go(vnoc_root: &Path, id: &TourId, wanted: Move, by: By) -> Result<Gone, GoError> {
    let lock = ToursLock::take(vnoc_root).map_err(StoreError::from)?;
    let measured = store::read_measured(vnoc_root, id)?;
    let tour = readable_tour(&measured, id)?;
    if let Move::Jump(At::Step(step)) = wanted
        && !place::has_step(&tour, step)
    {
        return Err(match tour.stop(step.stop()) {
            None => no_such_stop(&tour, step.stop()),
            Some(stop) => GoError::NoSuch {
                code: "no_block",
                said: format!("no block {step} in this tour"),
                near: steps_in_order(stop)
                    .iter()
                    .skip(1)
                    .map(ToString::to_string)
                    .collect(),
            },
        });
    }
    let was = store::read_place(vnoc_root, id).unwrap_or_default();
    let settled = place::settle(&tour, &was);
    let nothing_to_go_back_to = wanted == Move::Back && settled.back.is_empty();
    let now = if nothing_to_go_back_to {
        settled
    } else {
        let mut moved = place::go(&tour, &settled, wanted);
        moved.by = by;
        store::write_place_locked(&lock, vnoc_root, id, &moved)?;
        moved
    };
    Ok(Gone {
        tour,
        place: now,
        nothing_to_go_back_to,
    })
}

/// `vnoc/tourOpen`: a window shows this tour (`open`), or does not any more. The place is kept (`moved` and `by` too: opening is not a move); written only when
/// `open` changes. The place a tour nobody has opened gets is the cover.
pub fn set_open(vnoc_root: &Path, id: &TourId, open: bool) -> Result<(Place, bool), StoreError> {
    let lock = ToursLock::take(vnoc_root)?;
    let measured = store::read_measured(vnoc_root, id)?;
    let tour = readable_tour(&measured, id)?;
    let was = store::read_place(vnoc_root, id);
    let mut place = place::settle(&tour, &was.clone().unwrap_or_default());
    if was.is_some() && place.open == open {
        return Ok((place, false));
    }
    place.open = open;
    store::write_place_locked(&lock, vnoc_root, id, &place)?;
    Ok((place, true))
}

// -------------------------------------------------------------------------------------------------------------------- what a file change is to a tour

/// What a path of `vnoc/filesChanged` is to the tours (03 §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// under `.vnoc/tours/` or `.vnoc/local/tours/`: a tour's file (`Some`), or something in those folders that is not named like one (a folder, a temporary
    /// file: the folders are looked at again)
    Tour(Option<TourId>),
    /// a tour's state file, `.vnoc/local/tour-state/<id>.json`
    State(TourId),
    /// the tours lock, and what else is in `local/tour-state/`: a tour does not care
    Ignored,
    /// any other path: the index's, and the answers of the tours that read it
    Other,
}

pub fn classify(vnoc_root: &Path, path: &Path) -> Touch {
    let named = |dir: PathBuf, suffix: &str| -> Option<TourId> {
        let name = path.strip_prefix(&dir).ok()?.to_str()?;
        TourId::parse(name.strip_suffix(suffix)?).ok()
    };
    let (project_dir, private_dir) = (
        layout::tours_dir(vnoc_root),
        layout::local_tours_dir(vnoc_root),
    );
    if path.starts_with(&project_dir) || path.starts_with(&private_dir) {
        let id = named(project_dir, ".jsonl").or_else(|| named(private_dir, ".jsonl"));
        return Touch::Tour(id);
    }
    let state_dir = layout::tour_state_dir(vnoc_root);
    if path.starts_with(&state_dir) {
        return named(state_dir, ".json").map_or(Touch::Ignored, Touch::State);
    }
    if path == layout::tours_lock_file(vnoc_root) {
        return Touch::Ignored;
    }
    Touch::Other
}

// --------------------------------------------------------------------------------------------------------------------- what the core holds

/// The file a tour was replayed from, as it was: its length and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

struct Kept {
    row: Arc<Row>,
    stamp: Option<Stamp>,
}

/// An answer the core gave (rung 1), with the files it read: the answer holds until one of them is named by `vnoc/filesChanged`, or the tour's own file changes.
struct Remembered {
    recorded: Recorded,
    files: BTreeSet<String>,
}

#[derive(Default)]
struct Held {
    kept: BTreeMap<(TourId, Where), Kept>,
    answers: BTreeMap<TourId, Remembered>,
    /// the place of each tour as this core last wrote it or told the editor of it: a state file is a move only when its place is not this
    placed: BTreeMap<TourId, Place>,
    /// the checks (rung 3) made, for the tests
    checks: usize,
    /// which tours pass through which node (03 §6): from the tours replayed, and put in again for one tour when that tour is replayed
    on: on::Map,
}

impl Held {
    /// A tour replayed: kept, and put in the map of nodes in place of what it was.
    fn keep(&mut self, key: (TourId, Where), row: Arc<Row>, stamp: Option<Stamp>) {
        self.on.set(key.clone(), row.tour.as_ref().ok());
        self.kept.insert(key, Kept { row, stamp });
    }

    /// A tour that is not there any more.
    fn forget(&mut self, key: &(TourId, Where)) {
        self.kept.remove(key);
        self.on.set(key.clone(), None);
    }
}

/// What a question about freshness is asked of: the project, and the index when there is one (it is built after the editor says it is ready).
pub struct Ask<'a> {
    pub vnoc_root: &'a Path,
    pub project: &'a Project,
    pub index: Option<&'a RwLock<Index>>,
}

/// Why freshness could not be answered.
#[derive(Debug)]
pub enum Cannot {
    Store(StoreError),
    /// the answer needs a check, and a check needs the index, which is not given: ask again with it
    NeedsIndex,
}

impl From<StoreError> for Cannot {
    fn from(e: StoreError) -> Cannot {
        Cannot::Store(e)
    }
}

/// The tours the core has replayed, the answers it gave about their freshness, and where it last put each place. Owned by the server, not by the index: a tour
/// never moves the index's revision (D12), and nothing here touches the index but to read it for a check.
#[derive(Default)]
pub struct Tours {
    held: Mutex<Held>,
}

impl Tours {
    fn held(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The tours that pass through the node (03 §6), from what they wrote: no check, no code. In the order of the tours' keys; [`rows_on`] puts them in the
    /// order of a list.
    pub fn on(&self, query: &Query) -> Vec<On> {
        self.held().on.on(query)
    }

    /// How many tours pass through each node of the tree that any does (`vnoc/tourCounts`): [`on::Map::counts`].
    pub fn counts(&self, tree: &crate::index::Node) -> BTreeMap<String, usize> {
        self.held().on.counts(tree)
    }

    /// How many times a tour was put in the map of nodes: what the tests count.
    pub fn on_builds(&self) -> usize {
        self.held().on.built()
    }

    /// How many checks (rung 3) were made: what the tests count.
    pub fn checks_made(&self) -> usize {
        self.held().checks
    }

    /// Every tour of both folders, in the order of `list`, brought up to date by `stat`: a file whose length or time is not the one replayed is replayed again, a
    /// new one is read, one that went is forgotten. The ids that changed in any of those ways (their remembered answers are dropped).
    pub fn sync(&self, vnoc_root: &Path) -> (Vec<Arc<Row>>, Vec<TourId>) {
        let scanned = store::scan(vnoc_root);
        let mut held = self.held();
        let mut changed = BTreeSet::new();
        let present: BTreeSet<(TourId, Where)> = scanned
            .files
            .iter()
            .map(|(id, place, _)| (id.clone(), *place))
            .collect();
        let gone: Vec<(TourId, Where)> = held
            .kept
            .keys()
            .filter(|key| !present.contains(*key))
            .cloned()
            .collect();
        for key in gone {
            changed.insert(key.0.clone());
            held.forget(&key);
        }
        for (id, place, path) in scanned.files {
            let stamp = stamp_of(&path);
            let current = held
                .kept
                .get(&(id.clone(), place))
                .is_some_and(|k| k.stamp == stamp && k.row.path == path);
            if !current {
                changed.insert(id.clone());
                let row = Arc::new(store::row_at(id.clone(), place, path));
                held.keep((id, place), row, stamp);
            }
        }
        for id in &changed {
            held.answers.remove(id);
        }
        let mut rows: Vec<Arc<Row>> = held.kept.values().map(|k| k.row.clone()).collect();
        store::sort_rows(&mut rows);
        (rows, changed.into_iter().collect())
    }

    /// A tour's file changed (`vnoc/filesChanged` named it): replay it whatever its length and time say, and forget what was answered about it.
    pub fn replay(&self, vnoc_root: &Path, id: &TourId) {
        let mut found = Vec::new();
        match store::locate(vnoc_root, id) {
            store::Located::None => {}
            store::Located::Private(path) => found.push((Where::Private, path)),
            store::Located::Project(path) => found.push((Where::Project, path)),
            store::Located::Both { private, project } => {
                found.push((Where::Project, project));
                found.push((Where::Private, private));
            }
        }
        let mut held = self.held();
        for place in [Where::Private, Where::Project] {
            held.forget(&(id.clone(), place));
        }
        held.answers.remove(id);
        for (place, path) in found {
            let stamp = stamp_of(&path);
            let row = Arc::new(store::row_at(id.clone(), place, path));
            held.keep((id.clone(), place), row, stamp);
        }
    }

    /// The tour with this id, as `show` reads it (the project's copy when the id is in both), replayed only when its file is not the one replayed.
    pub fn row(&self, vnoc_root: &Path, id: &TourId) -> Option<Arc<Row>> {
        let (place, path) = match store::locate(vnoc_root, id) {
            store::Located::None => {
                let mut held = self.held();
                for place in [Where::Private, Where::Project] {
                    held.forget(&(id.clone(), place));
                }
                held.answers.remove(id);
                return None;
            }
            store::Located::Private(path) => (Where::Private, path),
            store::Located::Project(path) | store::Located::Both { project: path, .. } => {
                (Where::Project, path)
            }
        };
        let stamp = stamp_of(&path);
        let mut held = self.held();
        if let Some(k) = held.kept.get(&(id.clone(), place))
            && k.stamp == stamp
            && k.row.path == path
        {
            return Some(k.row.clone());
        }
        let row = Arc::new(store::row_at(id.clone(), place, path));
        held.answers.remove(id);
        held.keep((id.clone(), place), row.clone(), stamp);
        Some(row)
    }

    /// The answers the core remembers (rung 1), for the tours given: what a row of the list says for free.
    pub fn remembered(&self, ids: &[TourId]) -> BTreeMap<TourId, Recorded> {
        let held = self.held();
        ids.iter()
            .filter_map(|id| Some((id.clone(), held.answers.get(id)?.recorded.clone())))
            .collect()
    }

    /// The answer for each tour, by id, in the three rungs of 03 §3 with the core's memory in front of them: what the core said last time and has not been told
    /// is wrong; the recorded check; a check (which needs the index: [`Cannot::NeedsIndex`] when `ask` has none). What is answered is remembered, with the files it read.
    pub fn answers(
        &self,
        ask: &Ask,
        rows: &[Arc<Row>],
        mode: Mode,
    ) -> Result<BTreeMap<TourId, Answer>, Cannot> {
        let mut done = BTreeMap::new();
        {
            let mut held = self.held();
            for row in rows {
                if mode.again {
                    held.answers.remove(&row.id);
                } else if row.tour.is_ok()
                    && !done.contains_key(&row.id)
                    && let Some(r) = held.answers.get(&row.id)
                {
                    done.insert(
                        row.id.clone(),
                        Answer {
                            from: Source::Memory,
                            recorded: r.recorded.clone(),
                        },
                    );
                }
            }
        }
        let asked: Vec<&Arc<Row>> = rows.iter().filter(|r| !done.contains_key(&r.id)).collect();
        let subjects: Vec<Subject> = asked.iter().filter_map(|r| subject(r)).collect();
        let mut resolved = resolve(ask.vnoc_root, ask.project.git_root(), &subjects, mode)?;
        let mut checked = 0;
        if !resolved.need.is_empty() {
            let index = ask.index.ok_or(Cannot::NeedsIndex)?;
            let index = index.read().unwrap_or_else(|e| e.into_inner());
            let tree = index.tree(&ask.project.name(), ask.project.prefix());
            let code = Code {
                index: &index,
                project: ask.project,
                tree: &tree,
            };
            checked = resolved.need.len();
            check_needed(&code, ask.vnoc_root, &resolved.need, &mut resolved.done)?;
        }
        let mut held = self.held();
        held.checks += checked;
        for row in asked {
            let Some(answer) = resolved.done.get(&row.id) else {
                continue;
            };
            // a check records itself in the state file, which the watcher will report: a place that was there before is not a move
            if answer.from == Source::Check
                && !held.placed.contains_key(&row.id)
                && let Some(place) = store::read_place(ask.vnoc_root, &row.id)
            {
                held.placed.insert(row.id.clone(), place);
            }
            // a tour replayed while it was being answered is not the one this is true of
            let current = held
                .kept
                .get(&(row.id.clone(), row.place))
                .is_some_and(|k| Arc::ptr_eq(&k.row, row));
            if current {
                held.answers.insert(
                    row.id.clone(),
                    Remembered {
                        recorded: answer.recorded.clone(),
                        files: answer.recorded.files.keys().cloned().collect(),
                    },
                );
            }
        }
        done.extend(resolved.done);
        Ok(done)
    }

    /// `vnoc/filesChanged` named these files of the project (repo paths, `/`): the answers of the tours that read any of them are dropped. The ids.
    pub fn drop_answers_reading(&self, paths: &[String]) -> Vec<TourId> {
        let mut held = self.held();
        let mut dropped = Vec::new();
        held.answers.retain(|id, r| {
            let reads = paths.iter().any(|p| r.files.contains(p));
            if reads {
                dropped.push(id.clone());
            }
            !reads
        });
        dropped
    }

    /// This core wrote (or announced) the place of a tour: a state file with this place is not a move.
    pub fn note_place(&self, id: &TourId, place: &Place) {
        self.held().placed.insert(id.clone(), place.clone());
    }

    /// A state file changed: the place in it, as the tour is now, when it is not the one this core last wrote or announced (and then it is noted, so the next rewrite of
    /// the file for its recorded check, which does not touch the place, says nothing).
    pub fn place_moved(&self, vnoc_root: &Path, id: &TourId) -> Option<Place> {
        let written = store::read_place(vnoc_root, id)?;
        {
            let mut held = self.held();
            if held.placed.get(id) == Some(&written) {
                return None;
            }
            held.placed.insert(id.clone(), written.clone());
        }
        Some(match store::read(vnoc_root, id) {
            Ok((_, Ok(tour))) => place::settle(&tour, &written),
            _ => written,
        })
    }

    /// `vnoc/tourGo`: [`go`], and the place it wrote is noted so the watcher's word of the write is not an echo.
    pub fn go(&self, vnoc_root: &Path, id: &TourId, wanted: Move) -> Result<Gone, GoError> {
        let gone = go(vnoc_root, id, wanted, By::Reader)?;
        if !gone.nothing_to_go_back_to {
            self.note_place(id, &gone.place);
        }
        Ok(gone)
    }

    /// `vnoc/tourOpen`: [`set_open`], noted the same way.
    pub fn open(&self, vnoc_root: &Path, id: &TourId, open: bool) -> Result<Place, StoreError> {
        let (place, wrote) = set_open(vnoc_root, id, open)?;
        if wrote {
            self.note_place(id, &place);
        }
        Ok(place)
    }

    /// `vnoc/tourHalt`: `store::halt`, the function of `vnoc tour halt`, and the tour as it is after.
    pub fn halt(&self, vnoc_root: &Path, id: &TourId) -> Result<(Halt, Tour), StoreError> {
        let did = store::halt(vnoc_root, id)?;
        self.replay(vnoc_root, id);
        let tour = match self.row(vnoc_root, id).map(|r| r.tour.clone()) {
            Some(Ok(tour)) => tour,
            _ => return Err(StoreError::NotFound(id.clone())),
        };
        Ok((did, tour))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::record::{Bare, Record};
    use vnoc_domain::{NodeKind, RepoPath};

    fn id(text: &str) -> TourId {
        TourId::parse(text).unwrap()
    }

    /// A project folder with `.vnoc/`, fresh for this test; the tours store's root.
    fn vnoc_root(label: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-serve-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        crate::testdir::remove_at_end(&d);
        std::fs::create_dir_all(d.join(".vnoc")).unwrap();
        d.join(".vnoc")
    }

    fn started(root: &Path) -> TourId {
        let project = root.parent().unwrap();
        store::start(
            project,
            store::StartParams {
                title: "T".into(),
                question: None,
                author: None,
                detail: crate::tours::record::Detail::Normal,
            },
        )
        .unwrap()
        .id
    }

    // ---- where a path goes (03 §8)

    #[test]
    fn a_path_goes_to_the_tours_the_state_the_bin_or_the_index() {
        let root = Path::new("/p/.vnoc");
        let class = |p: &str| classify(root, Path::new(p));
        let tour = id("orders-7k2q");
        // a tour's file, in either folder
        assert_eq!(
            class("/p/.vnoc/tours/orders-7k2q.jsonl"),
            Touch::Tour(Some(tour.clone()))
        );
        assert_eq!(
            class("/p/.vnoc/local/tours/orders-7k2q.jsonl"),
            Touch::Tour(Some(tour.clone()))
        );
        // something in those folders that is not named like one: the folders are looked at again
        assert_eq!(class("/p/.vnoc/tours"), Touch::Tour(None));
        assert_eq!(class("/p/.vnoc/local/tours"), Touch::Tour(None));
        assert_eq!(class("/p/.vnoc/tours/notes.txt"), Touch::Tour(None));
        assert_eq!(
            class("/p/.vnoc/local/tours/.orders-7k2q.tmp"),
            Touch::Tour(None)
        );
        // a state file; the folder and a temporary file of an atomic write are nobody's
        assert_eq!(
            class("/p/.vnoc/local/tour-state/orders-7k2q.json"),
            Touch::State(tour)
        );
        assert_eq!(class("/p/.vnoc/local/tour-state"), Touch::Ignored);
        assert_eq!(
            class("/p/.vnoc/local/tour-state/.orders-7k2q.json.tmp1"),
            Touch::Ignored
        );
        // the lock
        assert_eq!(class("/p/.vnoc/local/tours.lock"), Touch::Ignored);
        // everything else is the index's, `.vnoc/` or not
        for other in [
            "/p/billing/charge.py",
            "/p/.vnoc/nodes/AB/ABCD/node.toml",
            "/p/.vnoc/vnoc.toml",
            "/p/.vnoc/local/other.json",
            "/p/.vnoc/tours-extra/x.jsonl",
            "/elsewhere/.vnoc/tours/orders-7k2q.jsonl",
        ] {
            assert_eq!(class(other), Touch::Other, "{other}");
        }
    }

    // ---- the place

    #[test]
    fn a_state_file_is_a_move_only_when_its_place_is_not_the_one_the_core_last_wrote() {
        let root = vnoc_root("place");
        let tour = started(&root);
        let tours = Tours::default();
        let at = |text: &str| Place {
            at: text.parse().unwrap(),
            ..Place::cover()
        };
        // no state file yet: nothing moved
        assert_eq!(tours.place_moved(&root, &tour), None);
        // another process writes a place: a move, once
        store::write_place(&root, &tour, &at("end")).unwrap();
        assert_eq!(tours.place_moved(&root, &tour).unwrap().at, At::End);
        assert_eq!(tours.place_moved(&root, &tour), None, "told already");
        // the file rewritten for its check keeps the place: not a move
        store::write_check(&root, &tour, serde_json::json!({ "x": 1 })).unwrap();
        assert_eq!(tours.place_moved(&root, &tour), None);
        // the core's own write is noted: not an echo
        let mine = at("cover");
        store::write_place(&root, &tour, &mine).unwrap();
        tours.note_place(&tour, &mine);
        assert_eq!(tours.place_moved(&root, &tour), None);
        // the place is read as the tour is now: a step that is not there is the nearest one
        store::write_place(&root, &tour, &at("s4.b2")).unwrap();
        assert_eq!(tours.place_moved(&root, &tour).unwrap().at, At::Cover);
    }

    // ---- what is held

    #[test]
    fn the_tours_are_replayed_when_their_file_changes_and_not_before() {
        let root = vnoc_root("held");
        let tour = started(&root);
        let tours = Tours::default();
        let first = tours.row(&root, &tour).unwrap();
        assert!(
            Arc::ptr_eq(&first, &tours.row(&root, &tour).unwrap()),
            "not read again"
        );
        let (rows, changed) = tours.sync(&root);
        assert_eq!(rows.len(), 1);
        assert!(
            changed.is_empty(),
            "the file is the one replayed: {changed:?}"
        );
        assert!(Arc::ptr_eq(&first, &rows[0]));
        // a line is appended: the length is not the one replayed
        store::append(&root, &tour, &[Record::Halt(Bare {})]).unwrap();
        let (rows, changed) = tours.sync(&root);
        assert_eq!(changed, std::slice::from_ref(&tour));
        assert!(!Arc::ptr_eq(&first, &rows[0]));
        // told by the watcher, it is read again whatever the file says
        let held = tours.row(&root, &tour).unwrap();
        tours.replay(&root, &tour);
        assert!(!Arc::ptr_eq(&held, &tours.row(&root, &tour).unwrap()));
        // a tour that went is forgotten
        std::fs::remove_file(&held.path).unwrap();
        assert_eq!(tours.sync(&root).1, std::slice::from_ref(&tour));
        assert!(tours.row(&root, &tour).is_none());
    }

    #[test]
    fn what_an_answer_read_drops_it_and_a_tour_with_nothing_written_needs_no_index() {
        let root = vnoc_root("answers");
        let tour = started(&root);
        let project = Project::open(root.parent().unwrap()).unwrap();
        let tours = Tours::default();
        let rows = vec![tours.row(&root, &tour).unwrap()];
        let ask = Ask {
            vnoc_root: &root,
            project: &project,
            index: None,
        };
        // nothing is written: fresh with no code to look at, and no index is asked for
        let first = tours.answers(&ask, &rows, Mode::default()).unwrap();
        assert_eq!(first[&tour].from, Source::Check);
        assert!(first[&tour].recorded.fresh());
        let second = tours.answers(&ask, &rows, Mode::default()).unwrap();
        assert_eq!(second[&tour].from, Source::Memory);
        // `again` asks anew
        let again = tours.answers(&ask, &rows, Mode { again: true }).unwrap();
        assert_eq!(again[&tour].from, Source::Check);
        // no file was read by it (none is written about), so none drops it; the tour's own file does
        assert!(tours.drop_answers_reading(&["a.py".to_string()]).is_empty());
        assert_eq!(tours.remembered(std::slice::from_ref(&tour)).len(), 1);
        tours.replay(&root, &tour);
        assert!(tours.remembered(&[tour]).is_empty());
    }

    // ---- the map of nodes (03 §6)

    fn tour_text(id: &str, qualnames: &[&str]) -> String {
        let mut text = format!(
            r#"{{"type":"tour","schema":1,"id":"{id}","title":"T","at":"x","detail":"normal"}}"#
        );
        for (n, q) in qualnames.iter().enumerate() {
            text.push_str(&format!(
                r#"
{{"type":"stop","id":"s{}","path":"x.py","qualname":"{q}","kind":"function","text":"t"}}"#,
                n + 1
            ));
        }
        text.push('\n');
        text
    }

    fn query(qualname: &str) -> Query {
        Query {
            id: None,
            path: RepoPath::new("x.py").unwrap(),
            qualname: qualname.to_string(),
            occurrence: 1,
            kind: NodeKind::Function,
        }
    }

    #[test]
    fn the_map_of_nodes_is_built_for_a_tour_when_it_is_replayed_and_for_no_request_that_changed_nothing()
     {
        let dir = vnoc_root("on");
        let tours = layout::local_tours_dir(&dir);
        std::fs::create_dir_all(&tours).unwrap();
        let file = |id: &str| tours.join(format!("{id}.jsonl"));
        std::fs::write(file("a-0000"), tour_text("a-0000", &["run"])).unwrap();
        std::fs::write(file("b-0000"), tour_text("b-0000", &["run", "help"])).unwrap();

        let held = Tours::default();
        let on = |q: &str| -> Vec<String> {
            held.on(&query(q))
                .iter()
                .map(|o| o.tour.to_string())
                .collect()
        };
        let (rows, changed) = held.sync(&dir);
        assert_eq!((rows.len(), changed.len()), (2, 2));
        assert_eq!(held.on_builds(), 2);
        assert_eq!(on("run"), ["a-0000", "b-0000"]);

        // requests that change nothing build nothing
        for _ in 0..3 {
            let (_, changed) = held.sync(&dir);
            assert!(changed.is_empty());
            held.row(&dir, &TourId::parse("a-0000").unwrap()).unwrap();
            assert_eq!(on("help"), ["b-0000"]);
        }
        assert_eq!(held.on_builds(), 2);

        // one tour's file changed and was replayed: that tour is put in again, the other is not
        std::fs::write(file("a-0000"), tour_text("a-0000", &["run", "help"])).unwrap();
        held.replay(&dir, &TourId::parse("a-0000").unwrap());
        assert_eq!(held.on_builds(), 3);
        assert_eq!(on("help"), ["a-0000", "b-0000"]);
        let (_, changed) = held.sync(&dir);
        assert!(changed.is_empty(), "replay kept the file's stamp");
        assert_eq!(held.on_builds(), 3);

        // a tour that went is in none of the maps
        std::fs::remove_file(file("b-0000")).unwrap();
        let (_, changed) = held.sync(&dir);
        assert_eq!(changed.len(), 1);
        assert_eq!(on("run"), ["a-0000"]);
        assert_eq!(held.on_builds(), 3);
    }
}
