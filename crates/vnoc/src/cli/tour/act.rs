use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::args::{
    TourDeleteArgs, TourDropArgs, TourEditArgs, TourExportArgs, TourIdsArgs, TourImportArgs,
    TourPinArgs, TourPlanArgs, TourResumeArgs, TourSettingsArgs, TourShareArgs,
};
use crate::cli::nodes::{Lookup, Target, doc_path};
use crate::cli::out::{self, Render};
use crate::cli::project::{Opened, open_from, open_without_driver};
use crate::cli::write::take_lock as take_write_lock;
use crate::cli::{Failure, Global};
use crate::tours::act::{self, ActError, Ask, IdsWritten, PinOut, SettingShown, SettingsShown};
use crate::tours::add::{MAX_STOPS, block_of, did_you_mean_stops, nested_in, plan_route};
use crate::tours::check::{self, Recorded};
use crate::tours::doc;
use crate::tours::document::{self, ImportError};
use crate::tours::finish::{self as finishing, Step};
use crate::tours::pin::{self, Pinnable};
use crate::tours::record::{Bare, DropRecord, Record, StopId};
use crate::tours::serve::short_commit;
use crate::tours::settings::{self, Choice, Key, Origin};
use crate::tours::store::{self, Located, Row, StoreError, Where};
use crate::tours::{State, Tour, TourId, steps_in_order, view};
use crate::userdata::ToursLock;

use super::{
    IdsAfterEnd, IdsStep, Lazy, act_failure, chosen, detail_of, find_all, no_such, plan_rows,
    refused_with_hint, render, resolve, settings_failure, steps_words, stopped, stops_words,
    store_failure, take_lock,
};

/// The tour as it is now, under the lock the caller holds. A tour that cannot be read cannot be written to.
pub(super) fn read_tour(vnoc_root: &Path, id: &TourId) -> Result<crate::tours::Tour, Failure> {
    act::read_tour(vnoc_root, id).map_err(|e| match e {
        ActError::Store(e) => store_failure(e),
        ActError::Environment(said) => Failure::Environment(said),
        other => Failure::Usage(other.to_string()),
    })
}

/// What `plan` wrote.
#[derive(Serialize)]
struct Planned {
    schema: u32,
    command: &'static str,
    tour: String,
    /// the ids of the stops planned, in the order given
    planned: Vec<String>,
    /// stops written, and stops on the route in all
    written: usize,
    stops: usize,
    #[serde(skip)]
    rows: String,
}

impl Render for Planned {
    fn text(&self, _quiet: bool) -> String {
        format!(
            "planned {}   · {} of {} stops written\n{}",
            stops_words(self.planned.len()),
            self.written,
            self.stops,
            self.rows
        )
    }
}

pub(super) async fn plan(global: &Global, args: &TourPlanArgs) -> Result<(), Failure> {
    if args.line.is_some() && (args.from.is_none() || args.nodes.len() != 1) {
        return Err(Failure::Usage(
            "--line goes with --from and one node".to_string(),
        ));
    }
    if args.nodes.len() > MAX_STOPS {
        return Err(Failure::Usage(format!(
            "{} stops planned, the most one tour holds is {MAX_STOPS}. Plan fewer, and a second tour for the rest",
            args.nodes.len()
        )));
    }
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let tour_id = resolve(global, "tour plan", &vnoc_root, &args.tour)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &tour_id) {
        return Err(store_failure(StoreError::InBoth(tour_id)));
    }
    // what needs no driver: the tour is not stopped, and not finished
    {
        let lock = take_lock(vnoc_root.clone()).await?;
        let tour = read_tour(&vnoc_root, &tour_id)?;
        stopped(global, "tour plan", &vnoc_root, &lock, &tour)?;
        finished(&tour)?;
    }
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, args.nodes.len());
    let nodes = find_all(global, "tour plan", &lookup, &args.nodes)?;

    let lock = take_lock(vnoc_root.clone()).await?;
    let tour = read_tour(&vnoc_root, &tour_id)?;
    stopped(global, "tour plan", &vnoc_root, &lock, &tour)?;
    finished(&tour)?;
    let planned = {
        let index = opened.index.read().expect("the index lock is not poisoned");
        plan_route(
            &tour,
            &index,
            &opened.project,
            &lookup.tree,
            &nodes,
            args.from,
            args.line,
        )
    };
    let records = match planned {
        Ok(records) => records,
        Err(refusals) => {
            return refused_with_hint(
                global,
                "tour plan",
                Some(&tour_id),
                &refusals,
                "fix them, then send the plan again",
            );
        }
    };
    store::append_locked(&lock, &vnoc_root, &tour_id, &records).map_err(store_failure)?;
    drop(lock);
    let ids: Vec<StopId> = match records.first() {
        Some(Record::Plan(plan)) => plan.stops.iter().map(|s| s.id).collect(),
        _ => Vec::new(),
    };
    let after = read_tour(&vnoc_root, &tour_id)?;
    let size = after.size();
    render(
        &Planned {
            schema: 1,
            command: "tour plan",
            tour: tour_id.to_string(),
            planned: ids.iter().map(ToString::to_string).collect(),
            written: size.written,
            stops: after.stops.len(),
            rows: plan_rows(&after, &ids),
        },
        global,
    )
}

/// A finished tour is not planned on: the request's mistake (exit `2`).
fn finished(tour: &Tour) -> Result<(), Failure> {
    if tour.state == crate::tours::State::Finished {
        return Err(Failure::Usage(
            "this tour is finished: nothing more can be planned on it".to_string(),
        ));
    }
    Ok(())
}

/// Step 1 of 02 §5, for `finish` and `import`: by `[tours] ids` and the two flags (`--ids`, `--no-ids`), the IDs are written, or the line for the agent to ask the
/// user is said, or nothing. `run` is false for a tour that was finished already (the setting does not offer again). `opened` is the index when the caller has
/// one; else it is built, and only for a tour with a stop that had no ID when it was written. No driver is a note, not a failure.
pub(super) async fn ids_after_end(
    global: &Global,
    project: &crate::project::Project,
    vnoc_root: &Path,
    tour: &Tour,
    flags: (bool, bool),
    run: bool,
    opened: Option<&Opened>,
) -> Result<IdsAfterEnd, Failure> {
    let read = settings::read(vnoc_root).map_err(settings_failure)?;
    let choice = finishing::effective(read.ids.value, flags.0, flags.1);
    let mut done = IdsAfterEnd {
        ids: None,
        text: String::new(),
        notes: read.problems,
    };
    let writes_ids = project_writes_ids(project)?;
    if run && let Some(sentence) = settings::does_nothing_here(choice, writes_ids) {
        // the setting asks for what the project forbids: said instead of written
        done.notes.push(sentence);
    }
    // a stop whose node had an ID when it was written has nothing to give: no index is built for a tour with no such stop
    let could = tour.stops.iter().any(|s| s.written && s.node.is_none());
    if !(run && choice != Choice::Never && could && writes_ids) {
        return Ok(done);
    }
    let built;
    let opened = match opened {
        Some(opened) => Some(opened),
        None => match open_from(global).await {
            Ok(o) => {
                built = o;
                Some(&built)
            }
            Err(Failure::Environment(why)) => {
                done.notes.push(format!(
                    "could not tell which stops have no ID, so none were written or offered: {why}"
                ));
                None
            }
            Err(e) => return Err(e),
        },
    };
    let missing = opened.map_or(0, |o| finishing::missing(&picks_of(o, tour, &[])));
    match finishing::step(choice, true, missing) {
        Step::Nothing => {}
        Step::Ask { missing } => {
            done.text = format!("{}\n", finishing::ask_line(&tour.id, missing));
            done.ids = Some(IdsStep::Ask {
                missing,
                command: format!("vnoc tour ids {}", tour.id),
            });
        }
        Step::Write => {
            let opened = opened.expect("a stop is missing an ID only when the index was built");
            let written = write_ids(global, opened, vnoc_root, &tour.id, &[]).await?;
            done.text = written.text(false);
            done.ids = Some(IdsStep::Written {
                stops: written.stops,
                changed: written.changed,
                recorded: written.recorded,
            });
        }
    }
    Ok(done)
}

// ------------------------------------------------------------------------------------------------------------------------------- the pin

/// The project writes no ID lines: `[ids] default = "none"` in `.vnoc/vnoc.toml`. `Err` for a file that cannot be read.
pub(super) fn project_writes_ids(project: &crate::project::Project) -> Result<bool, Failure> {
    act::project_writes_ids(project).map_err(|e| act_failure(e, project))
}

/// A real check of the tour against the code as it is now, and where its stops and blocks are (what a pin records).
fn placed_check(opened: &Opened, tour: &Tour, len: u64) -> (Recorded, check::Placed) {
    act::placed_check(&opened.project, &opened.index, tour, len)
}

/// The stops of the tour with what there is to do for each, from the index as it is now.
pub(super) fn picks_of(
    opened: &Opened,
    tour: &Tour,
    named: &[crate::tours::StopId],
) -> Vec<(crate::tours::StopId, crate::tours::ids::Pick)> {
    act::picks_of(&opened.project, &opened.index, tour, named)
}

/// The pin of one tour (01 §8, 02 §5 and §8): `act::pin_begin` asks what needs no code, the index is opened for the rest, `act::pin_finish` does it.
pub(super) async fn pin_tour(
    global: &Global,
    lazy: &mut Lazy,
    project: &crate::project::Project,
    vnoc_root: &Path,
    id: &TourId,
    ask: &Ask<'_>,
) -> Result<act::Pinned, Failure> {
    let (measured, head) =
        match act::pin_begin(project, vnoc_root, id, ask).map_err(|e| act_failure(e, project))? {
            act::Begun::Done(pinned) => return Ok(pinned),
            act::Begun::Check { measured, head } => (measured, head),
        };
    let opened = match lazy.get(global).await {
        Ok(opened) => opened,
        Err(Failure::Environment(why)) => return Ok(act::not_checked(ask, id, why)),
        Err(e) => return Err(e),
    };
    act::pin_finish(project, &opened.index, vnoc_root, id, ask, &measured, head)
        .map_err(|e| act_failure(e, project))
}

/// Gives the stops' nodes an ID: the project's write lock first, then (inside each marker's edit, and at the end) the tours lock: never the other way round.
pub(super) async fn write_ids(
    global: &Global,
    opened: &Opened,
    vnoc_root: &Path,
    id: &TourId,
    named: &[crate::tours::StopId],
) -> Result<IdsWritten, Failure> {
    let project = &opened.project;
    let _one_writer = take_write_lock(project).await?;
    let mut env = crate::identity::Env {
        project,
        index: &opened.index,
        host: &opened.host,
        edits: crate::identity::EditSink::Disk,
    };
    match act::write_ids(&mut env, vnoc_root, id, named).await {
        Err(ActError::NoStop { said, near }) => {
            no_such(global, "tour ids", "no_stop", said, near)?;
            unreachable!("a refusal is a failure")
        }
        done => done.map_err(|e| act_failure(e, project)),
    }
}

/// `can be pinned` for a finished, fresh tour that is not pinned and whose files are all committed; `can be pinned again` for one that is pinned
/// at a commit HEAD does not have behind it (01 §8). The code is read only when git says the rest holds.
pub(super) async fn can_pin_words(
    global: &Global,
    lazy: &mut Lazy,
    project: &crate::project::Project,
    row: &Row,
    tour: &Tour,
) -> Option<&'static str> {
    let holds = tour.pin.as_ref().filter(|_| tour.pinned());
    if holds.is_some_and(|pin| pin::reachable(project, &pin.commit)) {
        return None;
    }
    let head = pin::head(project, tour).ok()?;
    let opened = lazy.get(global).await.ok()?;
    let (recorded, placed) = placed_check(opened, tour, row.len);
    match pin::can_pin(project, tour, head, &recorded, &placed) {
        Pinnable::Yes { .. } if holds.is_some() => Some("can be pinned again"),
        Pinnable::Yes { .. } => Some("can be pinned"),
        Pinnable::No(_) => None,
    }
}

/// One tour of `vnoc tour pin`.
#[derive(Serialize)]
struct PinnedTour {
    tour: String,
    /// it was pinned, and its commit is reachable from HEAD: nothing was done
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    already: bool,
    #[serde(flatten)]
    out: PinOut,
    #[serde(skip)]
    lines: Vec<String>,
}

#[derive(Serialize)]
struct PinAnswer {
    schema: u32,
    command: &'static str,
    tours: Vec<PinnedTour>,
    /// how many tours were pinned by this call
    pinned: usize,
}

impl Render for PinAnswer {
    fn text(&self, _quiet: bool) -> String {
        if self.tours.is_empty() {
            return "nothing to pin: every finished tour is pinned\n".to_string();
        }
        let headed = self.tours.len() > 1;
        let mut text = String::new();
        for tour in &self.tours {
            for (i, line) in tour.lines.iter().enumerate() {
                // with several tours each block says whose it is, on its first line
                if headed
                    && i == 0
                    && !line.starts_with("pinned ")
                    && !line.starts_with("committed ")
                {
                    text.push_str(&format!("{}  ", tour.tour));
                }
                text.push_str(line);
                text.push('\n');
            }
        }
        text
    }
}

pub(super) async fn pin_command(global: &Global, args: &TourPinArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut wanted = Vec::new();
    for text in &args.tours {
        let id = resolve(global, "tour pin", &vnoc_root, text)?;
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let named = !wanted.is_empty();
    let rows = chosen(store::list(&vnoc_root).tours, &wanted);
    let mut lazy = Lazy::default();
    let mut tours = Vec::new();
    for row in &rows {
        let tour = match &row.tour {
            Ok(tour) => tour,
            Err(e) if named => {
                return Err(Failure::Usage(format!(
                    "the tour {} cannot be read: {e}",
                    row.id
                )));
            }
            Err(_) => continue,
        };
        if let Located::Both { .. } = store::locate(&vnoc_root, &row.id) {
            if named {
                return Err(store_failure(StoreError::InBoth(row.id.clone())));
            }
            continue;
        }
        // pinned, and the commit is behind HEAD: nothing to do
        if let Some(commit) = act::pinned_already(&project, tour) {
            if named {
                tours.push(PinnedTour {
                    tour: row.id.to_string(),
                    already: true,
                    lines: vec![format!(
                        "{} is pinned at {} already",
                        row.id,
                        short_commit(&commit)
                    )],
                    out: PinOut::pinned(&commit, None),
                });
            }
            continue;
        }
        // with no tour named: the finished ones that are not pinned, or whose pin cannot be reached
        if !named && tour.state != State::Finished {
            continue;
        }
        let ask = Ask::Pin {
            commit: args.commit,
            message: args.message.as_deref(),
        };
        let done = pin_tour(global, &mut lazy, &project, &vnoc_root, &row.id, &ask).await?;
        tours.push(PinnedTour {
            tour: row.id.to_string(),
            already: false,
            lines: done.lines,
            out: done.out,
        });
    }
    let refused = tours.iter().any(|t| t.out.refused);
    let pinned = tours
        .iter()
        .filter(|t| t.out.state == "pinned" && !t.already)
        .count();
    render(
        &PinAnswer {
            schema: 1,
            command: "tour pin",
            tours,
            pinned,
        },
        global,
    )?;
    if refused {
        Err(Failure::Reported)
    } else {
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------- drop, resume, edit, halt

/// A tour is named, in one folder only, and the lock is taken: what every write after this does.
struct Writing {
    vnoc_root: PathBuf,
    id: TourId,
    lock: ToursLock,
    tour: Tour,
}

async fn writing(
    global: &Global,
    command: &'static str,
    text: &str,
) -> Result<(crate::project::Project, Writing), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, command, &vnoc_root, text)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &id) {
        return Err(store_failure(StoreError::InBoth(id)));
    }
    let lock = take_lock(vnoc_root.clone()).await?;
    let tour = read_tour(&vnoc_root, &id)?;
    Ok((
        project,
        Writing {
            vnoc_root,
            id,
            lock,
            tour,
        },
    ))
}

#[derive(Serialize)]
struct Dropped {
    schema: u32,
    command: &'static str,
    tour: String,
    dropped: String,
    /// the blocks that went with it, and the stops nested in it
    blocks: usize,
    stops: usize,
    /// the stop was planned and never written
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    was_planned: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    unpinned: bool,
}

impl Render for Dropped {
    fn text(&self, _quiet: bool) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut went = Vec::new();
        if self.blocks > 0 {
            went.push(plural(self.blocks, "block", "blocks"));
        }
        if self.stops > 0 {
            went.push(format!(
                "{} nested in it",
                plural(self.stops, "stop", "stops")
            ));
        }
        let mut text = format!("dropped {}", self.dropped);
        if self.was_planned {
            text.push_str(" (it was planned, not written)");
        }
        if !went.is_empty() {
            text.push_str(&format!("   ({} went with it)", went.join(" and ")));
        }
        text.push('\n');
        if self.unpinned {
            text.push_str("not pinned any more: the tour changed after it was pinned\n");
        }
        text
    }
}

pub(super) async fn drop_id(global: &Global, args: &TourDropArgs) -> Result<(), Failure> {
    let (_project, w) = writing(global, "tour drop", &args.tour).await?;
    stopped(global, "tour drop", &w.vnoc_root, &w.lock, &w.tour)?;
    let tour = &w.tour;
    let id = args.id;
    let Some(stop) = tour.stop(id.stop()) else {
        let near = did_you_mean_stops(tour, id.stop());
        return no_such(
            global,
            "tour drop",
            "no_stop",
            format!("no stop {} in this tour", id.stop()),
            near.iter().map(ToString::to_string).collect(),
        );
    };
    // what goes with it
    let (blocks, stops) = if id.is_intro() {
        let nested = tour
            .stops
            .iter()
            .filter(|s| nested_in(tour, s.id, stop.id))
            .collect::<Vec<_>>();
        let blocks = steps_in_order(stop).len() - 1
            + nested
                .iter()
                .map(|s| steps_in_order(s).len() - 1)
                .sum::<usize>();
        (blocks, nested.len())
    } else {
        match block_of(stop, id) {
            Some((block, _)) => {
                let mut details = 0;
                let mut stack: Vec<&crate::tours::TourBlock> = block.details.iter().collect();
                while let Some(b) = stack.pop() {
                    details += 1;
                    stack.extend(b.details.iter());
                }
                (details, 0)
            }
            None => {
                let have: Vec<String> = steps_in_order(stop)
                    .iter()
                    .skip(1)
                    .map(ToString::to_string)
                    .collect();
                let said = format!("no block {id} in this tour");
                return no_such(global, "tour drop", "no_block", said, have);
            }
        }
    };
    let was_pinned = tour.pinned();
    store::append_locked(
        &w.lock,
        &w.vnoc_root,
        &w.id,
        &[Record::Drop(DropRecord { id })],
    )
    .map_err(store_failure)?;
    render(
        &Dropped {
            schema: 1,
            command: "tour drop",
            tour: w.id.to_string(),
            dropped: id.to_string(),
            blocks,
            stops,
            was_planned: id.is_intro() && !stop.written,
            unpinned: was_pinned,
        },
        global,
    )
}

#[derive(Serialize)]
struct Resumed {
    schema: u32,
    command: &'static str,
    tour: String,
    /// `resume` was written; false when the tour was not stopped
    resumed: bool,
}

impl Render for Resumed {
    fn text(&self, _quiet: bool) -> String {
        if self.resumed {
            format!("resumed {}: add is accepted again\n", self.tour)
        } else {
            "not stopped: nothing to resume\n".to_string()
        }
    }
}

pub(super) async fn resume(global: &Global, args: &TourResumeArgs) -> Result<(), Failure> {
    let (_project, w) = writing(global, "tour resume", &args.tour).await?;
    let stopped = matches!(w.tour.state, State::Stopping | State::Stopped);
    if stopped {
        store::append_locked(&w.lock, &w.vnoc_root, &w.id, &[Record::Resume(Bare {})])
            .map_err(store_failure)?;
    }
    render(
        &Resumed {
            schema: 1,
            command: "tour resume",
            tour: w.id.to_string(),
            resumed: stopped,
        },
        global,
    )
}

#[derive(Serialize)]
struct Halted {
    schema: u32,
    command: &'static str,
    tour: String,
    /// `written`, `already` or `finished`
    halt: &'static str,
}

impl Render for Halted {
    fn text(&self, _quiet: bool) -> String {
        match self.halt {
            "written" => format!("stopping {}: the next add is refused\n", self.tour),
            "already" => format!("{} is stopping or stopped already\n", self.tour),
            _ => format!("{} is finished: nothing to stop\n", self.tour),
        }
    }
}

pub(super) async fn halt(global: &Global, args: &TourResumeArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour halt", &vnoc_root, &args.tour)?;
    let root = vnoc_root.clone();
    let did = {
        let id = id.clone();
        tokio::task::spawn_blocking(move || store::halt(&root, &id))
            .await
            .map_err(|e| Failure::Environment(e.to_string()))?
            .map_err(store_failure)?
    };
    render(
        &Halted {
            schema: 1,
            command: "tour halt",
            tour: id.to_string(),
            halt: match did {
                store::Halt::Written => "written",
                store::Halt::Already => "already",
                store::Halt::Finished => "finished",
            },
        },
        global,
    )
}

#[derive(Serialize)]
struct Edited {
    schema: u32,
    command: &'static str,
    tour: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl Render for Edited {
    fn text(&self, _quiet: bool) -> String {
        let mut text = format!("edited {}\n", self.tour);
        for (what, value) in [
            ("title", &self.title),
            ("question", &self.question),
            ("detail", &self.detail),
        ] {
            if let Some(value) = value {
                text.push_str(&format!("  {what}: {value}\n"));
            }
        }
        text
    }
}

pub(super) async fn edit(global: &Global, args: &TourEditArgs) -> Result<(), Failure> {
    let title = args.title.as_deref().map(str::trim);
    let question = args.question.as_deref().map(str::trim);
    let project = open_without_driver(global)?;
    let root = project.name();
    let named = store::Named {
        dir: Some(project.open_dir()),
        root: &root,
    };
    let problems = store::check_edit(title, question, &named);
    if !problems.is_empty() {
        return Err(Failure::Usage(problems.join("\n  ")));
    }
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour edit", &vnoc_root, &args.tour)?;
    act::edit(
        &vnoc_root,
        &id,
        title,
        question,
        args.detail.map(detail_of),
        &named,
    )
    .map_err(|e| act_failure(e, &project))?;
    render(
        &Edited {
            schema: 1,
            command: "tour edit",
            tour: id.to_string(),
            title: title.map(String::from),
            question: question.map(String::from),
            detail: args
                .detail
                .map(|d| view::detail_word(detail_of(d)).to_string()),
        },
        global,
    )
}

// ------------------------------------------------------------------------------------------------------- share, delete

#[derive(Serialize)]
struct Shared {
    schema: u32,
    command: &'static str,
    tour: String,
    /// `project` or `private`: where the file is now
    folder: &'static str,
    path: String,
}

impl Render for Shared {
    fn text(&self, quiet: bool) -> String {
        if self.folder == "project" {
            let mut text = format!("shared {}   project   {}\n", self.tour, self.path);
            if !quiet {
                text.push_str("commit it to share it with the project\n");
            }
            text
        } else {
            format!("made {} private   {}\n", self.tour, self.path)
        }
    }
}

pub(super) async fn share(global: &Global, args: &TourShareArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour share", &vnoc_root, &args.tour)?;
    let to = if args.private {
        Where::Private
    } else {
        Where::Project
    };
    let root = vnoc_root.clone();
    let moved = {
        let id = id.clone();
        tokio::task::spawn_blocking(move || store::share(&root, &id, to))
            .await
            .map_err(|e| Failure::Environment(e.to_string()))?
            .map_err(store_failure)?
    };
    render(
        &Shared {
            schema: 1,
            command: "tour share",
            tour: id.to_string(),
            folder: view::place_word(to),
            path: doc_path(&project, &moved),
        },
        global,
    )
}

// ----------------------------------------------------------------------------------------------------------------------- ids

impl Render for IdsWritten {
    fn text(&self, _quiet: bool) -> String {
        let written = self.stops.iter().filter(|s| s.state == "written").count();
        let mut text = if self.stops.is_empty() {
            format!("{}: no written stop to give an ID\n", self.tour)
        } else if written == 0 {
            format!(
                "{}: nothing to write; every stop's node has an ID\n",
                self.tour
            )
        } else {
            let had = self.stops.iter().filter(|s| s.state == "had").count();
            format!(
                "{}: wrote {} for {} of {} stops{}\n",
                self.tour,
                if written == 1 { "an ID" } else { "IDs" },
                written,
                self.stops.len(),
                match had {
                    0 => String::new(),
                    n => format!("; {n} already had one"),
                }
            )
        };
        for (stop, word) in self.stops.iter().zip(&self.words) {
            if !word.is_empty() {
                text.push_str(&format!("  {:<4} {word}\n", stop.stop));
            }
        }
        if !self.changed.is_empty() {
            text.push_str("changed\n");
            for file in &self.changed {
                text.push_str(&format!("  {file}\n"));
            }
            text.push_str("read these files again before you edit them\n");
        }
        text
    }
}

pub(super) async fn ids(global: &Global, args: &TourIdsArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let vnoc_root = store::vnoc_root(opened.project.git_root());
    let id = resolve(global, "tour ids", &vnoc_root, &args.tour)?;
    let written = write_ids(global, &opened, &vnoc_root, &id, &args.stops).await?;
    render(&written, global)
}

// ------------------------------------------------------------------------------------------------------------------ settings

impl Render for SettingsShown {
    fn text(&self, _quiet: bool) -> String {
        let row = |key: &str, s: &SettingShown| {
            let from = match s.from {
                Origin::File => "from .vnoc/vnoc.toml",
                Origin::Default => "the default",
            };
            format!("{key:<7}{:<8}{from}\n", s.value.as_str())
        };
        let mut text = String::new();
        for key in &self.changed {
            let now = if *key == "ids" {
                &self.ids
            } else {
                &self.commit
            };
            text.push_str(&format!("set {key} = {}\n", now.value));
        }
        text.push_str(&row("ids", &self.ids));
        text.push_str(&row("commit", &self.commit));
        if let Some(sentence) = &self.does_nothing {
            text.push_str(&format!("{sentence}\n"));
        }
        for problem in &self.problems {
            text.push_str(&format!("note: {problem}\n"));
        }
        text
    }
}

pub(super) async fn settings(global: &Global, args: &TourSettingsArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let choice = |arg: crate::cli::args::ChoiceArg| match arg {
        crate::cli::args::ChoiceArg::Always => Choice::Always,
        crate::cli::args::ChoiceArg::Ask => Choice::Ask,
        crate::cli::args::ChoiceArg::Never => Choice::Never,
    };
    let set: Vec<(Key, Choice)> = [(Key::Ids, args.ids), (Key::Commit, args.commit)]
        .into_iter()
        .filter_map(|(key, arg)| Some((key, choice(arg?))))
        .collect();
    // the project's file: under the project's write lock, as everything else that writes `.vnoc/`
    let _one_writer = if set.is_empty() {
        None
    } else {
        Some(take_write_lock(&project).await?)
    };
    let shown = act::settings(&project, &set).map_err(|e| act_failure(e, &project))?;
    render(&shown, global)
}

// -------------------------------------------------------------------------------------------------------------- export, import

pub(super) async fn export(global: &Global, args: &TourExportArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour export", &vnoc_root, &args.tour)?;
    // the name each stop has now where it is found; where it is not, or there is no driver, the name it was written with
    let text = match open_from(global).await {
        Ok(opened) => {
            let index = opened.index.read().expect("the index lock is not poisoned");
            document::export(&project, Some(&index), &vnoc_root, &id, args.flat)
        }
        Err(Failure::Environment(_)) => {
            document::export(&project, None, &vnoc_root, &id, args.flat)
        }
        Err(e) => return Err(e),
    }
    .map_err(|e| act_failure(e, &project))?;
    out::raw(&text)
}

/// The document's text from a file or standard input: at most one megabyte and a byte are read, and over a megabyte it is refused by `doc::parse`.
fn read_document(source: &str) -> Result<String, Failure> {
    let limit = doc::MAX_BYTES as u64 + 1;
    let mut bytes = Vec::new();
    let read = if source == "-" {
        std::io::stdin().lock().take(limit).read_to_end(&mut bytes)
    } else {
        std::fs::File::open(source).and_then(|file| file.take(limit).read_to_end(&mut bytes))
    };
    read.map_err(|e| Failure::Usage(format!("could not read the document from {source}: {e}")))?;
    if bytes.len() > doc::MAX_BYTES {
        // too big to be a document: `parse` says so, from the length, without reading it
        return Ok("x".repeat(bytes.len()));
    }
    String::from_utf8(bytes)
        .map_err(|_| Failure::Usage(format!("the document in {source} is not UTF-8")))
}

#[derive(Serialize)]
struct Imported {
    schema: u32,
    command: &'static str,
    tour: String,
    replaced: bool,
    stops: usize,
    steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    kept: Option<usize>,
    #[serde(rename = "new", skip_serializing_if = "Option::is_none")]
    added: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gone: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ids: Option<IdsStep>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin: Option<PinOut>,
    #[serde(skip)]
    ids_text: String,
    #[serde(skip)]
    pin_text: String,
    #[serde(skip)]
    pinned_at: Option<String>,
}

impl Render for Imported {
    fn text(&self, _quiet: bool) -> String {
        let stops = format!(
            "{} {}",
            self.stops,
            if self.stops == 1 { "stop" } else { "stops" }
        );
        let mut text = if self.replaced {
            format!(
                "replaced {}  {stops} ({} kept, {} new, {} gone)\n",
                self.tour,
                self.kept.unwrap_or(0),
                self.added.unwrap_or(0),
                self.gone.unwrap_or(0)
            )
        } else {
            format!(
                "imported {}  {stops}, {}\n",
                self.tour,
                steps_words(self.steps)
            )
        };
        if let Some(at) = &self.pinned_at {
            text.pop();
            text.push_str(&format!(" · pinned at {at}\n"));
        }
        text.push_str(&self.ids_text);
        text.push_str(&self.pin_text);
        for note in &self.notes {
            text.push_str(&format!("note: {note}\n"));
        }
        text
    }
}

pub(super) async fn import(global: &Global, args: &TourImportArgs) -> Result<(), Failure> {
    let hint = "fix them, then import the whole document again";
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    // what needs no index: the document, its shape, and the tour it replaces
    let text = read_document(&args.file)?;
    let flat = match doc::parse(&text).and_then(|d| doc::flatten(&d)) {
        Ok(flat) => flat,
        Err(refusals) => return refused_with_hint(global, "tour import", None, &refusals, hint),
    };
    let replace_id = match &args.replace {
        Some(text) => {
            let id = resolve(global, "tour import", &vnoc_root, text)?;
            if let Located::Both { .. } = store::locate(&vnoc_root, &id) {
                return Err(store_failure(StoreError::InBoth(id)));
            }
            Some(id)
        }
        None => None,
    };

    // the code as it is now: every `node` of the document read as a command reads it, before the index is held
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, flat.stops.len());
    let mut nodes: BTreeMap<&str, Result<&crate::index::Node, String>> = BTreeMap::new();
    for stop in &flat.stops {
        nodes
            .entry(stop.node.as_str())
            .or_insert_with(|| match lookup.resolve(&stop.node) {
                Ok(Target::Node(node)) => Ok(node),
                Ok(Target::Missing(missing)) => Err(format!(
                    "{} is Missing: its code is gone. A stop is about code that is there",
                    missing.was
                )),
                Err(error) => Err(error.message),
            });
    }
    let resolve_node = |text: &str| {
        nodes
            .get(text)
            .cloned()
            .unwrap_or_else(|| Err(format!("{text} was not looked up")))
    };

    let written = {
        // with `--replace`, the lock for the rest: the ids come from the file read under it, and the new file is written under the same lock (01 §5)
        let lock = match &replace_id {
            Some(_) => Some(take_lock(vnoc_root.clone()).await?),
            None => None,
        };
        let index = opened.index.read().expect("the index lock is not poisoned");
        document::write_import(
            &opened.project,
            &index,
            &vnoc_root,
            &resolve_node,
            &flat,
            replace_id.as_ref().zip(lock.as_ref()),
        )
    };
    let written = match written {
        Ok(written) => written,
        Err(ImportError::Refused(refusals)) => {
            return refused_with_hint(global, "tour import", replace_id.as_ref(), &refusals, hint);
        }
        Err(ImportError::Act(e)) => return Err(act_failure(e, &project)),
    };
    let (tour_id, replaced) = (written.id.clone(), written.replaced);
    let kept = written.kept.zip(written.gone);

    // what was written, as the replay has it, and then what `finish` does after the end (02 §5)
    let tour = read_tour(&vnoc_root, &tour_id)?;
    let size = view::tour_view(&tour, Where::Private);
    let step = ids_after_end(
        global,
        &project,
        &vnoc_root,
        &tour,
        (false, false),
        true,
        Some(&opened),
    )
    .await?;
    // step 2, as `finish` does it: the code is read again when the IDs step wrote into it
    let wrote = matches!(&step.ids, Some(IdsStep::Written { changed, .. }) if !changed.is_empty());
    let mut lazy = if wrote {
        drop(opened);
        Lazy::default()
    } else {
        Lazy(Some(opened))
    };
    let read = settings::read(&vnoc_root).map_err(settings_failure)?;
    let ask = Ask::Finish {
        commit: read.commit.value,
        message: None,
    };
    let pinned = pin_tour(global, &mut lazy, &project, &vnoc_root, &tour_id, &ask).await?;
    let mut pin_text = pinned.lines.join("\n");
    if !pin_text.is_empty() {
        pin_text.push('\n');
    }
    let stops = size.written;
    render(
        &Imported {
            schema: 1,
            command: "tour import",
            tour: tour_id.to_string(),
            replaced,
            stops,
            steps: size.steps,
            kept: kept.map(|(k, _)| k),
            added: kept.map(|(k, _)| stops.saturating_sub(k)),
            gone: kept.map(|(_, g)| g),
            ids: step.ids,
            notes: step.notes,
            pin: Some(pinned.out),
            ids_text: step.text,
            pin_text,
            pinned_at: pinned.at,
        },
        global,
    )
}

#[derive(Serialize)]
struct DeletedTour {
    tour: String,
    /// the folder it was in
    was: &'static str,
    /// the trash entry: what a restore takes
    entry: String,
    /// where its file is now
    path: String,
    /// its state file went with it
    state: bool,
}

#[derive(Serialize)]
struct Deleted {
    schema: u32,
    command: &'static str,
    tours: Vec<DeletedTour>,
}

impl Render for Deleted {
    fn text(&self, _quiet: bool) -> String {
        self.tours
            .iter()
            .map(|t| format!("deleted {}   {}   → {}\n", t.tour, t.was, t.path))
            .collect()
    }
}

pub(super) async fn delete(global: &Global, args: &TourDeleteArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut ids: Vec<TourId> = Vec::new();
    for text in &args.tours {
        let id = resolve(global, "tour delete", &vnoc_root, text)?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    // all or none: one that cannot be deleted stops the command before the first moves
    let mut problems = Vec::new();
    for id in &ids {
        match store::locate(&vnoc_root, id) {
            Located::Both { .. } => problems.push(StoreError::InBoth(id.clone()).to_string()),
            _ => {
                if let Ok((_, Ok(tour))) = store::read(&vnoc_root, id)
                    && matches!(tour.state, State::Writing | State::Stopping)
                {
                    problems.push(format!(
                        "the tour {id} is being written ({}): stop it first",
                        tour.state
                    ));
                }
            }
        }
    }
    if !problems.is_empty() {
        return Err(Failure::Usage(problems.join("\n  ")));
    }
    let mut tours = Vec::new();
    for id in ids {
        let root = vnoc_root.clone();
        let gone = tokio::task::spawn_blocking(move || store::delete(&root, &id))
            .await
            .map_err(|e| Failure::Environment(e.to_string()))?
            .map_err(store_failure)?;
        let file = store::trash_path(&vnoc_root, &gone.entry);
        tours.push(DeletedTour {
            tour: gone.id.to_string(),
            was: view::place_word(gone.was),
            entry: gone.entry,
            path: doc_path(&project, &file),
            state: gone.with_state,
        });
    }
    render(
        &Deleted {
            schema: 1,
            command: "tour delete",
            tours,
        },
        global,
    )
}
