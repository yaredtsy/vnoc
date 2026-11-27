use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::cli::args::{
    TourCheckArgs, TourGoArgs, TourListArgs, TourRouteArgs, TourShowArgs, TourWhereArgs,
};
use crate::cli::nodes::{Lookup, NodeError, Target};
use crate::cli::out::{self, Render};
use crate::cli::project::{open_from, open_without_driver};
use crate::cli::style;
use crate::cli::{Failure, Global};
use crate::tours::check::{self, Answer, Code, Mode, Recorded, Source};
use crate::tours::on::{self, Map, Query};
use crate::tours::place::{self, At, By, Move, Place};
use crate::tours::route;
use crate::tours::serve::{self, Subject, subject, where_view, written_code};
use crate::tours::store::{self, Where};
use crate::tours::{State, Tour, TourId, time, view};

use super::act::can_pin_words;
use super::look;
use super::{
    Lazy, build_route, chosen, missing_error, no_such, refused_with, render, render_as, resolve,
    store_failure,
};

/// What `route` prints: the route as data, and its text.
#[derive(Serialize)]
struct Routed {
    schema: u32,
    command: &'static str,
    node: String,
    depth: u32,
    sizes: Vec<route::Size>,
    stops: Vec<route::Entry>,
    cut: bool,
    /// the driver has no call trees for this node: the route is the node alone
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    no_calls: bool,
    #[serde(skip)]
    text: String,
}

impl Render for Routed {
    fn text(&self, _quiet: bool) -> String {
        self.text.clone()
    }
}

pub(super) async fn route(global: &Global, args: &TourRouteArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let built = build_route(
        global,
        "tour route",
        &opened,
        &lookup,
        &args.node,
        args.depth,
    )
    .await?;
    let mut text = built.route.text();
    if built.no_calls {
        text.push_str("the driver gives no calls for this node: the route is the node alone\n");
    }
    let route = built.route;
    render_as(
        &Routed {
            schema: 1,
            command: "tour route",
            node: route.node,
            depth: route.depth,
            sizes: route.sizes,
            stops: route.stops,
            cut: route.cut,
            no_calls: built.no_calls,
            text,
        },
        global,
        look::Kind::Route,
    )
}

/// The answer for each tour, by id, in three rungs (03 §3): the recorded check when it still holds (`stat`s, no index), else a check. The index
/// is built (once, by `lazy`) only if some tour needs the check, and a tour with no written stop needs no code at all.
async fn answers(
    global: &Global,
    lazy: &mut Lazy,
    project: &crate::project::Project,
    vnoc_root: &Path,
    subjects: &[Subject<'_>],
    mode: Mode,
) -> Result<BTreeMap<TourId, Answer>, Failure> {
    let mut resolved =
        serve::resolve(vnoc_root, project.git_root(), subjects, mode).map_err(store_failure)?;
    if resolved.need.is_empty() {
        return Ok(resolved.done);
    }
    let opened = lazy.get(global).await?;
    let index = opened.index.read().expect("the index lock is not poisoned");
    let tree = index.tree(&opened.project.name(), opened.project.prefix());
    let code = Code {
        index: &index,
        project: &opened.project,
        tree: &tree,
    };
    serve::check_needed(&code, vnoc_root, &resolved.need, &mut resolved.done)
        .map_err(store_failure)?;
    Ok(resolved.done)
}

/// One block of a stop that a check did not find as it was written.
#[derive(Serialize)]
struct CheckedBlock {
    id: String,
    /// `changed` or `moved`
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    moved_to: Option<[u32; 2]>,
}

/// One stop that is changed or gone, or fresh with a block at other lines.
#[derive(Serialize)]
struct CheckedStop {
    id: String,
    /// `changed`, `gone`, or `fresh` for a stop that only has a moved block
    state: &'static str,
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[u32; 2]>,
    blocks: Vec<CheckedBlock>,
}

#[derive(Serialize)]
struct CheckedTour {
    id: String,
    readable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    problem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    /// `record` or `check`
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<Source>,
    /// when the answer was made
    #[serde(skip_serializing_if = "Option::is_none")]
    at: Option<String>,
    changed: usize,
    gone: usize,
    stops: Vec<CheckedStop>,
    /// `can be pinned` or `can be pinned again`: a finished, fresh tour that a pin can be made of now (nothing is written)
    #[serde(skip_serializing_if = "Option::is_none")]
    can_pin: Option<&'static str>,
    #[serde(skip)]
    lines: Vec<String>,
    #[serde(skip)]
    word: String,
    #[serde(skip)]
    title: String,
}

fn checked_stops(tour: &Tour, recorded: &Recorded) -> Vec<CheckedStop> {
    let mut out = Vec::new();
    for stop in tour.stops.iter().filter(|s| s.written) {
        let mut blocks = Vec::new();
        let mut stack: Vec<&crate::tours::TourBlock> = stop.blocks.iter().rev().collect();
        while let Some(block) = stack.pop() {
            match recorded.block(block.id) {
                check::BlockState::Fresh => {}
                check::BlockState::Changed => blocks.push(CheckedBlock {
                    id: block.id.to_string(),
                    state: "changed",
                    moved_to: None,
                }),
                check::BlockState::Moved(lines) => blocks.push(CheckedBlock {
                    id: block.id.to_string(),
                    state: "moved",
                    moved_to: Some([lines.first, lines.last]),
                }),
            }
            stack.extend(block.details.iter().rev());
        }
        let state = recorded.stop(stop.id);
        if state == check::StopState::Fresh && blocks.is_empty() {
            continue;
        }
        let view = view::tour_view(tour, Where::Private);
        let shown = &view.stops[tour.number(stop.id).unwrap_or(1) - 1];
        out.push(CheckedStop {
            id: stop.id.to_string(),
            state: match state {
                check::StopState::Fresh => "fresh",
                check::StopState::Changed => "changed",
                check::StopState::Gone => "gone",
            },
            // where the stop is now, when the check said (a stop found by its ID may have another name or path than it was written with)
            name: recorded
                .now
                .get(&stop.id)
                .map_or_else(|| shown.name.clone(), |n| n.qualname.clone())
                .clone(),
            path: recorded
                .now
                .get(&stop.id)
                .map_or_else(|| shown.path.clone(), |n| n.path.clone()),
            lines: recorded
                .now
                .get(&stop.id)
                .map_or(shown.lines, |n| n.lines.or(shown.lines)),
            blocks,
        });
    }
    out
}

#[derive(Serialize)]
struct Checked {
    schema: u32,
    command: &'static str,
    tours: Vec<CheckedTour>,
    /// tours with a stop that is changed or gone
    stale: usize,
    /// tours that cannot be read
    unreadable: usize,
}

impl Render for Checked {
    fn text(&self, quiet: bool) -> String {
        if self.tours.is_empty() {
            return "no tours\n".to_string();
        }
        let width = style::terminal_width();
        let ids: Vec<String> = self.tours.iter().map(|t| t.id.clone()).collect();
        let mut text = String::new();
        if !quiet {
            let n = self.tours.len();
            let mut said = format!("{n} {}", if n == 1 { "tour" } else { "tours" });
            said.push_str(&match self.stale {
                0 => " · all fresh".to_string(),
                s => format!(" · {s} with stale stops"),
            });
            if self.unreadable > 0 {
                said.push_str(&format!(" · {} cannot be read", self.unreadable));
            }
            let mark = if self.stale + self.unreadable > 0 {
                look::Mark::Needs
            } else {
                look::Mark::Done
            };
            text.push_str(&look::check_header(&said, mark));
        }
        for tour in &self.tours {
            let (state, details) = if !tour.readable {
                (look::Checked::Unreadable, vec![tour.word.clone()])
            } else if tour.fresh == Some(false) {
                // the tour's word and when it was checked, then a line for each stop that is not as it was written
                let mut details = vec![format!("{}  {}", tour.word, tour.lines[0])];
                details.extend(tour.lines.iter().skip(1).cloned());
                (look::Checked::Changed, details)
            } else {
                // up to date: a line, and what can be done with it
                let more = tour.can_pin.map(|says| vec![says.to_string()]);
                (look::Checked::UpToDate, more.unwrap_or_default())
            };
            let title = if tour.readable {
                tour.title.as_str()
            } else {
                ""
            };
            text.push_str(&look::check_block(
                state, title, &tour.id, &details, &ids, width,
            ));
        }
        text
    }
}

pub(super) async fn check(global: &Global, args: &TourCheckArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut wanted = Vec::new();
    for text in &args.tours {
        let id = resolve(global, "tour check", &vnoc_root, text)?;
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let rows = chosen(store::list(&vnoc_root).tours, &wanted);
    let subjects: Vec<Subject> = rows.iter().filter_map(subject).collect();
    let mut lazy = Lazy::default();
    let mode = Mode { again: args.again };
    let done = answers(global, &mut lazy, &project, &vnoc_root, &subjects, mode).await?;
    let today = time::now();
    let mut tours = Vec::new();
    for row in &rows {
        let id = row.id.to_string();
        let (Ok(tour), Some(answer)) = (&row.tour, done.get(&row.id)) else {
            let problem = row.tour.as_ref().err().map(|e| e.why.clone());
            tours.push(CheckedTour {
                id,
                readable: false,
                word: format!("cannot be read: {}", problem.as_deref().unwrap_or("")),
                title: String::new(),
                lines: vec![],
                fresh: None,
                from: None,
                at: None,
                changed: 0,
                gone: 0,
                stops: vec![],
                can_pin: None,
                problem,
            });
            continue;
        };
        let (changed, gone) = answer.recorded.counts();
        let mut lines = vec![view::when_words(
            answer.from,
            answer.recorded.fresh(),
            &answer.recorded.at,
            &today,
        )];
        lines.extend(view::stale_lines(tour, &answer.recorded));
        let can_pin = if answer.recorded.fresh() && tour.state == State::Finished {
            can_pin_words(global, &mut lazy, &project, row, tour).await
        } else {
            None
        };
        let mut word = view::check_word(changed, gone);
        if let Some(says) = can_pin {
            word.push_str(&format!(" · {says}"));
        }
        tours.push(CheckedTour {
            id,
            readable: true,
            problem: None,
            fresh: Some(answer.recorded.fresh()),
            from: Some(answer.from),
            at: Some(answer.recorded.at.clone()),
            changed,
            gone,
            stops: checked_stops(tour, &answer.recorded),
            can_pin,
            lines,
            word,
            title: tour.title.clone(),
        });
    }
    let stale = tours.iter().filter(|t| t.fresh == Some(false)).count();
    let unreadable = tours.iter().filter(|t| !t.readable).count();
    out::render(
        &Checked {
            schema: 1,
            command: "tour check",
            tours,
            stale,
            unreadable,
        },
        global,
    )
}

// --------------------------------------------------------------------------------------------------------------------- list

#[derive(Serialize)]
struct Listed {
    schema: u32,
    command: &'static str,
    /// `list --on`: the node the tours are on, as the index names it
    #[serde(skip_serializing_if = "Option::is_none")]
    on: Option<String>,
    tours: Vec<view::ListRowView>,
    /// what was wrong with the folders: a file that is not a tour's
    problems: Vec<String>,
    /// the tour whose place moved last, the one `vnoc tour where` answers for: the list says `last read` of it
    #[serde(skip)]
    last_read: Option<String>,
}

impl Render for Listed {
    fn text(&self, quiet: bool) -> String {
        if self.tours.is_empty() {
            if let Some(on) = &self.on {
                return format!("no tours on {on}\n");
            }
            let mut text = "no tours\n".to_string();
            if !quiet {
                text.push_str("start one with: vnoc tour start --title \"…\"\n");
            }
            return text;
        }
        look::list(
            &self.tours,
            self.last_read.as_deref(),
            &time::now(),
            style::terminal_width(),
        )
    }
}

pub(super) async fn list(global: &Global, args: &TourListArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let listed = store::list(&vnoc_root);
    if !global.json {
        for problem in &listed.problems {
            out::note(problem);
        }
    }
    let mut lazy = Lazy::default();

    // the node, if one was asked for: the one thing here that needs the index
    let mut on_node: Option<(String, Query)> = None;
    if let Some(text) = &args.on {
        let opened = lazy.get(global).await?;
        let lookup = Lookup::new(opened, 1);
        let node = match lookup.resolve(text) {
            Ok(Target::Node(node)) => node,
            Ok(Target::Missing(missing)) => {
                return refused_with(
                    global,
                    "tour list",
                    missing_error(&missing.was, &missing.id),
                );
            }
            Err(error) => return refused_with(global, "tour list", error),
        };
        let Some(query) = Query::of(node) else {
            let error = NodeError::new("root", on::ROOT_IS_EVERY_TOURS);
            return refused_with(global, "tour list", error);
        };
        on_node = Some((node.location(), query));
    }

    // what is known of each tour: a check made now (`--check`), else the record where it still holds, by `stat`s alone
    let subjects: Vec<Subject> = listed.tours.iter().filter_map(subject).collect();
    let known: BTreeMap<TourId, Recorded> = if args.check {
        answers(
            global,
            &mut lazy,
            &project,
            &vnoc_root,
            &subjects,
            Mode::default(),
        )
        .await?
        .into_iter()
        .map(|(id, answer)| (id, answer.recorded))
        .collect()
    } else {
        subjects
            .iter()
            .filter_map(|s| {
                check::recall(&vnoc_root, project.git_root(), s.id, s.path)
                    .map(|recorded| (s.id.clone(), recorded))
            })
            .collect()
    };
    let mut rows = serve::list_rows(&project, &listed.tours, &known, args.check, &|id| {
        store::read_place(&vnoc_root, id)
    });

    if let Some((_, query)) = &on_node {
        // by what the tours wrote: no check, no code read
        let map = Map::of(
            listed
                .tours
                .iter()
                .filter_map(|r| Some(((r.id.clone(), r.place), r.tour.as_ref().ok()?))),
        );
        rows = serve::rows_on(rows, &listed.tours, &map.on(query));
    }
    out::render(
        &Listed {
            schema: 1,
            command: "tour list",
            on: on_node.map(|(location, _)| location),
            tours: rows,
            problems: listed.problems,
            last_read: last_moved(&vnoc_root).map(|id| id.to_string()),
        },
        global,
    )
}

// --------------------------------------------------------------------------------------------------------------------- show

#[derive(Serialize)]
struct Shown {
    schema: u32,
    command: &'static str,
    readable: bool,
    #[serde(flatten)]
    tour: view::TourView,
    #[serde(skip)]
    text: String,
}

impl Render for Shown {
    fn text(&self, _quiet: bool) -> String {
        self.text.clone()
    }
}

/// A tour that cannot be read: the answer says so, and it is not a failure of the command (`show` told you what there is).
#[derive(Serialize)]
struct ShownUnreadable {
    schema: u32,
    command: &'static str,
    tour: String,
    readable: bool,
    /// `private` or `project`
    folder: &'static str,
    problem: String,
}

impl Render for ShownUnreadable {
    fn text(&self, _quiet: bool) -> String {
        format!(
            "{}   {}\ncannot be read: {}\n",
            self.tour, self.folder, self.problem
        )
    }
}

pub(super) async fn show(global: &Global, args: &TourShowArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let tour_id = resolve(global, "tour show", &vnoc_root, &args.tour)?;
    let read = store::read_measured(&vnoc_root, &tour_id).map_err(store_failure)?;
    match &read.tour {
        Ok(tour) => {
            let mut tour_view = serve::show_view(
                &project,
                tour,
                read.place,
                &read.path,
                store::read_place(&vnoc_root, &tour_id).as_ref(),
            );
            let holds = tour.pin.as_ref().filter(|_| tour.pinned());
            if args.as_written && holds.is_none() {
                return Err(Failure::Usage(format!(
                    "this tour is not pinned: there is no commit to read it as written from (vnoc tour pin {tour_id})"
                )));
            }
            if args.check || args.as_written {
                let subjects = [Subject {
                    id: &tour_id,
                    path: &read.path,
                    len: read.len,
                    tour,
                }];
                let mut lazy = Lazy::default();
                let done = answers(
                    global,
                    &mut lazy,
                    &project,
                    &vnoc_root,
                    &subjects,
                    Mode::default(),
                )
                .await?;
                if let Some(answer) = done.get(&tour_id) {
                    view::mark(&mut tour_view, answer);
                }
            }
            if let Some(pin) = holds.filter(|_| args.as_written) {
                written_code(&project, pin, &mut tour_view);
            }
            let text = view::outline_text(tour, &tour_view, args.text);
            render_as(
                &Shown {
                    schema: 1,
                    command: "tour show",
                    readable: true,
                    tour: tour_view,
                    text,
                },
                global,
                look::Kind::Outline,
            )
        }
        Err(unreadable) => render(
            &ShownUnreadable {
                schema: 1,
                command: "tour show",
                tour: tour_id.to_string(),
                readable: false,
                folder: view::place_word(read.place),
                problem: unreadable.why.clone(),
            },
            global,
        ),
    }
}

// --------------------------------------------------------------------------------------------------------------- where and go

/// What `where` and `go` print: the view, and the text made from it.
#[derive(Serialize)]
struct WhereOut {
    schema: u32,
    command: &'static str,
    /// `go`: the move as it was typed
    #[serde(rename = "move", skip_serializing_if = "Option::is_none")]
    moved_by: Option<String>,
    /// `go back` with nothing to go back to: the place stays
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    nothing_to_go_back_to: bool,
    #[serde(flatten)]
    view: view::WhereView,
    #[serde(skip)]
    text: String,
}

impl Render for WhereOut {
    fn text(&self, _quiet: bool) -> String {
        if self.nothing_to_go_back_to {
            format!("nothing to go back to\n{}", self.text)
        } else {
            self.text.clone()
        }
    }
}

/// Nothing to say a place of: no tour was named and none has been opened or moved.
#[derive(Serialize)]
struct NoPlaceYet {
    schema: u32,
    command: &'static str,
    tour: Option<String>,
    message: &'static str,
}

impl Render for NoPlaceYet {
    fn text(&self, _quiet: bool) -> String {
        format!("{}\nname one: vnoc tour where <tour>\n", self.message)
    }
}

/// The tour whose place moved last: the greatest `moved` of the places that are written, the id breaking a tie. `None` when no tour has a place.
fn last_moved(vnoc_root: &Path) -> Option<TourId> {
    let mut best: Option<(String, TourId)> = None;
    for row in store::list(vnoc_root).tours {
        let Some(found) = store::read_place(vnoc_root, &row.id) else {
            continue;
        };
        let key = (found.moved, row.id);
        if best.as_ref().is_none_or(|b| key > *b) {
            best = Some(key);
        }
    }
    best.map(|(_, id)| id)
}

/// A tour as `where` and `go` need it: readable (a tour that is not is the request's mistake, exit 2; one of a newer schema the environment's).
fn readable(measured: &store::Measured, id: &TourId) -> Result<Tour, Failure> {
    match &measured.tour {
        Ok(tour) => Ok(tour.clone()),
        Err(e) => {
            let said = format!("the tour {id} cannot be read: {e}");
            Err(if e.too_new {
                Failure::Environment(said)
            } else {
                Failure::Usage(said)
            })
        }
    }
}

/// The view of a tour's place: its recorded check when it holds, else a check (recorded), then [`where_view`] and its text.
async fn place_answer(
    global: &Global,
    project: &crate::project::Project,
    vnoc_root: &Path,
    id: &TourId,
    place: &Place,
) -> Result<(view::WhereView, String), Failure> {
    let measured = store::read_measured(vnoc_root, id).map_err(store_failure)?;
    let tour = readable(&measured, id)?;
    // the place as the tour is now: a step that is gone is read as the nearest that is there, and this is not written back
    let settled = place::settle(&tour, place);
    let subjects = [Subject {
        id,
        path: &measured.path,
        len: measured.len,
        tour: &tour,
    }];
    let mut lazy = Lazy::default();
    let done = answers(
        global,
        &mut lazy,
        project,
        vnoc_root,
        &subjects,
        Mode::default(),
    )
    .await?;
    let recorded = done
        .get(id)
        .map_or_else(|| Recorded::empty(measured.len), |a| a.recorded.clone());
    let view = where_view(project, &tour, &settled, &recorded);
    let stop = match settled.at {
        At::Step(step) => tour.stop(step.stop()),
        _ => None,
    };
    let text = view::where_text(&view, stop);
    Ok((view, text))
}

pub(super) async fn where_command(global: &Global, args: &TourWhereArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = match &args.tour {
        Some(text) => resolve(global, "tour where", &vnoc_root, text)?,
        None => match last_moved(&vnoc_root) {
            Some(id) => id,
            None => {
                return render(
                    &NoPlaceYet {
                        schema: 1,
                        command: "tour where",
                        tour: None,
                        message: "no tour has been opened or moved yet",
                    },
                    global,
                );
            }
        },
    };
    // a tour nobody has moved is on its cover
    let place = store::read_place(&vnoc_root, &id).unwrap_or_default();
    let (view, text) = place_answer(global, &project, &vnoc_root, &id, &place).await?;
    render_as(
        &WhereOut {
            schema: 1,
            command: "tour where",
            moved_by: None,
            nothing_to_go_back_to: false,
            view,
            text,
        },
        global,
        look::Kind::Where,
    )
}

pub(super) async fn go_command(global: &Global, args: &TourGoArgs) -> Result<(), Failure> {
    let wanted = Move::parse(&args.r#move).map_err(Failure::Usage)?;
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour go", &vnoc_root, &args.tour)?;
    // the tours lock holds from reading the tour and the place to writing the place: two moves made together each apply to the place as it is then
    let gone = {
        let (vnoc_root, id) = (vnoc_root.clone(), id.clone());
        tokio::task::spawn_blocking(move || serve::go(&vnoc_root, &id, wanted, By::Command))
            .await
            .map_err(|e| Failure::Environment(e.to_string()))?
    };
    let gone = match gone {
        Ok(gone) => gone,
        Err(serve::GoError::Store(e)) => return Err(store_failure(e)),
        Err(serve::GoError::NoSuch { code, said, near }) => {
            return no_such(global, "tour go", code, said, near);
        }
    };
    let (view, text) = place_answer(global, &project, &vnoc_root, &id, &gone.place).await?;
    render_as(
        &WhereOut {
            schema: 1,
            command: "tour go",
            moved_by: Some(args.r#move.clone()),
            nothing_to_go_back_to: gone.nothing_to_go_back_to,
            view,
            text,
        },
        global,
        look::Kind::Where,
    )
}
