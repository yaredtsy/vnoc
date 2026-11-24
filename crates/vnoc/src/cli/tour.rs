//! `vnoc tour start | add | finish | check | list | show` (`docs/26-tours/02-commands.md` §2 – §6, §12): parse, call `tours/`, print.
//!
//! Every command here is a thin handler: the files, the rules and the text of a tour are `crates/vnoc/src/tours/`. Only `add` always reads the code; `check`, `list --on` and
//! `show --check` start a driver only when a tour's recorded check no longer holds, or a node is to be found. Exit codes as `02` §12: a refusal and a tour, stop or node that is not there are `2`, the lock and the
//! environment `3`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::args::{
    DetailArg, TourAddArgs, TourCheckArgs, TourCommand, TourDeleteArgs, TourDropArgs, TourEditArgs,
    TourExportArgs, TourFinishArgs, TourGoArgs, TourIdsArgs, TourImportArgs, TourListArgs,
    TourPinArgs, TourPlanArgs, TourResumeArgs, TourRouteArgs, TourSettingsArgs, TourShareArgs,
    TourShowArgs, TourStartArgs, TourWhereArgs,
};
use super::nodes::{Hints, Lookup, NodeError, Target, doc_path};
use super::out::{self, Render};
use super::project::{Opened, open_from, open_without_driver};
use super::write::{attach_failure, take_lock as take_write_lock};
use super::{Failure, Global};
use crate::calls::{self, CallsError};
use crate::source;
use crate::tours::act::{
    self, ActError, Ask, IdStop, IdsWritten, PinOut, SettingShown, SettingsShown,
};
use crate::tours::add::{
    AddOptions, MAX_STOPS, block_of, did_you_mean_stops, nested_in, plan_more, plan_route,
    plan_stop, planned_stops,
};
use crate::tours::check::{self, Answer, Code, Mode, Recorded, Source};
use crate::tours::doc;
use crate::tours::find::find;
use crate::tours::finish::{self as finishing, Step};
use crate::tours::on::{self, Map, Query};
use crate::tours::pin::{self, Pinnable};
use crate::tours::place::{self, At, By, Move, Place};
use crate::tours::record::{Bare, Detail, DropRecord, EndRecord, Record, StopId};
use crate::tours::route;
use crate::tours::rules::Refusal;
use crate::tours::serve::{self, Subject, short_commit, subject, where_view, written_code};
use crate::tours::settings::{self, Choice, Key, Origin};
use crate::tours::store::{self, Located, Pick, Row, StartParams, StoreError, Where};
use crate::tours::{State, Tour, TourId, hash, steps_in_order, time, view};
use crate::userdata::ToursLock;

/// The most of a stop's text that is read before it is called too big: `body::read` says the size, and refuses anything over 64 KB.
const BODY_READ_CAP: u64 = 16 * 1024 * 1024;

pub async fn run(global: &Global, command: &TourCommand) -> Result<(), Failure> {
    match command {
        TourCommand::Start(args) => start(global, args).await,
        TourCommand::Add(args) => add(global, args).await,
        TourCommand::Plan(args) => plan(global, args).await,
        TourCommand::Route(args) => route(global, args).await,
        TourCommand::Finish(args) => finish(global, args).await,
        TourCommand::Pin(args) => pin_command(global, args).await,
        TourCommand::List(args) => list(global, args).await,
        TourCommand::Show(args) => show(global, args).await,
        TourCommand::Where(args) => where_command(global, args).await,
        TourCommand::Go(args) => go_command(global, args).await,
        TourCommand::Check(args) => check(global, args).await,
        TourCommand::Drop(args) => drop_id(global, args).await,
        TourCommand::Resume(args) => resume(global, args).await,
        TourCommand::Edit(args) => edit(global, args).await,
        TourCommand::Ids(args) => ids(global, args).await,
        TourCommand::Export(args) => export(global, args).await,
        TourCommand::Import(args) => import(global, args).await,
        TourCommand::Settings(args) => settings(global, args).await,
        TourCommand::Share(args) => share(global, args).await,
        TourCommand::Delete(args) => delete(global, args).await,
        TourCommand::Halt(args) => halt(global, args).await,
    }
}

// --------------------------------------------------------------------------------------------------------------------- shared

/// What a failure of the store is to a command: a tour that is not there, in two places, or too big is the request's (`2`); the lock, the disk and
/// the project's settings are the environment's (`3`).
fn store_failure(e: StoreError) -> Failure {
    match e {
        StoreError::NotFound(_)
        | StoreError::InBoth(_)
        | StoreError::TooBig(_)
        | StoreError::Refused(_) => Failure::Usage(e.to_string()),
        // a tour of a newer schema is for a newer V-NOC (the environment); a broken one is the request's
        StoreError::Unreadable { too_new: true, .. } => Failure::Environment(e.to_string()),
        StoreError::Unreadable { .. } => Failure::Usage(e.to_string()),
        StoreError::Lock(_)
        | StoreError::Io { .. }
        | StoreError::Init(_)
        | StoreError::NoFreeId => Failure::Environment(e.to_string()),
    }
}

/// A sentence (and a list under it) for a request that cannot be met: with `--json` one value on standard output, else standard error. Exit `2`.
fn refused_with(global: &Global, command: &'static str, error: NodeError) -> Result<(), Failure> {
    #[derive(Serialize)]
    struct Said {
        schema: u32,
        command: &'static str,
        error: serde_json::Value,
    }
    if global.json {
        out::json(&Said {
            schema: 1,
            command,
            error: error.json(),
        })?;
    } else {
        error.print();
    }
    Err(Failure::Reported)
}

/// The tour the command line names, or why it names none: the start of an id that starts one tour is enough, an id that starts several lists them,
/// one that starts none says what is nearest ("did you mean").
fn resolve(
    global: &Global,
    command: &'static str,
    vnoc_root: &Path,
    text: &str,
) -> Result<TourId, Failure> {
    let (error, items) = match store::pick(vnoc_root, text) {
        Pick::One(id) => return Ok(id),
        Pick::Several(ids) => (
            NodeError::new(
                "ambiguous",
                format!("\"{text}\" starts {} tours", ids.len()),
            ),
            ("name one of", ids),
        ),
        Pick::Nothing { near } => (
            NodeError::new("no_tour", format!("no tour {text}")),
            ("did you mean", near),
        ),
    };
    let mut error = error;
    if !items.1.is_empty() {
        let items_text: Vec<String> = items.1.iter().map(ToString::to_string).collect();
        error.message = format!(
            "{} · {}: {}",
            error.message,
            items.0,
            items_text.join(" · ")
        );
        error.hints = Some(Hints {
            lead: items.0.to_string(),
            items: items_text,
            more: 0,
        });
    }
    refused_with(global, command, error)?;
    unreachable!("a refusal is a failure")
}

/// The tours lock, waited for off the runtime's thread (it sleeps between tries).
async fn take_lock(vnoc_root: PathBuf) -> Result<ToursLock, Failure> {
    tokio::task::spawn_blocking(move || ToursLock::take(&vnoc_root))
        .await
        .map_err(|e| Failure::Environment(e.to_string()))?
        .map_err(|e| Failure::Environment(e.to_string()))
}

/// What an action's failure is to a command: the request's mistakes are `2`, the environment's `3`.
fn act_failure(e: ActError, project: &crate::project::Project) -> Failure {
    match e {
        ActError::Store(e) => store_failure(e),
        ActError::Usage(said) | ActError::NoStop { said, .. } => Failure::Usage(said),
        ActError::Attach(e) => attach_failure(e, project),
        ActError::Environment(said) => Failure::Environment(said),
    }
}

fn settings_failure(e: settings::SettingsError) -> Failure {
    match e {
        settings::SettingsError::NoFile => Failure::Usage(e.to_string()),
        _ => Failure::Environment(e.to_string()),
    }
}

/// The tour as it is now, under the lock the caller holds. A tour that cannot be read cannot be written to.
fn read_tour(vnoc_root: &Path, id: &TourId) -> Result<crate::tours::Tour, Failure> {
    act::read_tour(vnoc_root, id).map_err(|e| match e {
        ActError::Store(e) => store_failure(e),
        ActError::Environment(said) => Failure::Environment(said),
        other => Failure::Usage(other.to_string()),
    })
}

/// `1 step`, `5 steps`.
fn steps_words(n: usize) -> String {
    format!("{n} {}", if n == 1 { "step" } else { "steps" })
}

// --------------------------------------------------------------------------------------------------------------------- start

#[derive(Serialize)]
struct Started {
    schema: u32,
    command: &'static str,
    tour: String,
    /// `private`: every tour starts in `.vnoc/local/tours/`
    folder: &'static str,
    path: String,
    /// `.vnoc/` was made by this command
    created_vnoc: bool,
    /// the files that made: `vnoc.toml` and `.gitignore`, when `.vnoc/` was made
    created: Vec<String>,
    /// `--plan`: the ids of the stops planned
    #[serde(skip_serializing_if = "Vec::is_empty")]
    planned: Vec<String>,
    /// `--plan`: a row for each planned stop
    #[serde(skip)]
    rows: String,
}

impl Render for Started {
    fn text(&self, quiet: bool) -> String {
        let mut text = format!("started {}   {}   {}\n", self.tour, self.folder, self.path);
        if self.created_vnoc {
            text.push_str("created .vnoc/ (vnoc.toml, .gitignore)\n");
        }
        if !self.planned.is_empty() {
            text.push_str(&format!(
                "planned {}\n{}",
                stops_words(self.planned.len()),
                self.rows
            ));
        }
        if !quiet {
            text.push_str(&format!(
                "next: vnoc tour add {} <node> --body-file -   (vnoc guide tour)\n",
                self.tour
            ));
        }
        text
    }
}

fn detail_of(arg: DetailArg) -> Detail {
    match arg {
        DetailArg::Quick => Detail::Quick,
        DetailArg::Normal => Detail::Normal,
        DetailArg::Detailed => Detail::Detailed,
    }
}

/// Finds every node of a call before anything is written: the nodes, or **every** unknown one with its "did you mean" (`start --plan`, `plan`). Exit `2`.
fn find_all<'l>(
    global: &Global,
    command: &'static str,
    lookup: &'l Lookup<'_>,
    texts: &[String],
) -> Result<Vec<&'l crate::index::Node>, Failure> {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    for text in texts {
        match lookup.resolve(text) {
            Ok(Target::Node(node)) => found.push(node),
            Ok(Target::Missing(missing)) => errors.push(missing_error(&missing.was, &missing.id)),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        return Ok(found);
    }
    if global.json {
        #[derive(Serialize)]
        struct Said {
            schema: u32,
            command: &'static str,
            errors: Vec<serde_json::Value>,
        }
        out::json(&Said {
            schema: 1,
            command,
            errors: errors.iter().map(NodeError::json).collect(),
        })?;
    } else {
        errors.iter().for_each(NodeError::print);
        out::error("nothing was written");
    }
    Err(Failure::Reported)
}

/// `1 stop`, `3 stops`.
fn stops_words(n: usize) -> String {
    format!("{n} {}", if n == 1 { "stop" } else { "stops" })
}

/// A row for each of these stops of the tour, as `show` has one: the id, `◌` and the node's name, where it is, and `← s1 line 16` when there is a `from`.
fn plan_rows(tour: &Tour, ids: &[StopId]) -> String {
    let rows: Vec<(String, String, String)> = ids
        .iter()
        .filter_map(|id| tour.stop(*id))
        .map(|stop| {
            let mut tail = match (stop.from, stop.line) {
                (Some(from), Some(line)) => format!("← {from} line {line}"),
                (Some(from), None) => format!("← {from}"),
                _ => String::new(),
            };
            if let Some(same) = stop.same_as {
                if !tail.is_empty() {
                    tail.push_str("   ");
                }
                tail.push_str(&format!("= {same}"));
            }
            (stop.id.to_string(), view::stop_headline(stop), tail)
        })
        .collect();
    let id_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
    let headline_width = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    let mut text = String::new();
    for (id, headline, tail) in rows {
        let row = format!("  {id:<id_width$}  {headline:<headline_width$}  {tail}");
        text.push_str(row.trim_end());
        text.push('\n');
    }
    text
}

async fn start(global: &Global, args: &TourStartArgs) -> Result<(), Failure> {
    let params = StartParams {
        title: args.title.trim().to_string(),
        question: args.question.as_ref().map(|q| q.trim().to_string()),
        author: args.author.as_ref().map(|a| a.trim().to_string()),
        detail: detail_of(args.detail),
    };
    let problems = store::check_start(&params);
    if !problems.is_empty() {
        return Err(Failure::Usage(problems.join("\n  ")));
    }
    if args.plan.len() > MAX_STOPS {
        return Err(Failure::Usage(format!(
            "{} stops planned, the most one tour holds is {MAX_STOPS}. Plan fewer, and a second tour for the rest",
            args.plan.len()
        )));
    }
    // a bare `start` starts no driver; `--plan` finds every node first, before the tour is made (one unknown node: nothing is written)
    let opened = if args.plan.is_empty() && args.route.is_none() {
        None
    } else {
        Some(open_from(global).await?)
    };
    let alone;
    let project = match &opened {
        Some(opened) => &opened.project,
        None => {
            alone = open_without_driver(global)?;
            &alone
        }
    };
    let plan = match &opened {
        // `--route`: exactly the outline of `vnoc tour route`, one `plan` record
        Some(opened) if args.route.is_some() => {
            let lookup = Lookup::new(opened, 1);
            let text = args.route.as_deref().unwrap_or_default();
            let built = build_route(
                global,
                "tour start",
                opened,
                &lookup,
                text,
                args.depth.unwrap_or(1),
            )
            .await?;
            let nodes: Vec<&crate::index::Node> = built
                .route
                .stops
                .iter()
                .filter_map(|e| lookup.tree.find(&e.known.id))
                .collect();
            match planned_stops(&nodes, 0, None, None) {
                Ok(mut stops) if stops.len() == built.route.stops.len() => {
                    // ids s1… are the outline's numbers: `from` and `same_as` map to them, and so does `line`
                    let id = |n: usize| u32::try_from(n).ok().and_then(StopId::new);
                    for (stop, entry) in stops.iter_mut().zip(&built.route.stops) {
                        stop.from = entry.from.and_then(id);
                        stop.line = entry.line;
                        stop.same_as = entry.same_as.and_then(id);
                    }
                    stops
                }
                Ok(_) => {
                    return Err(Failure::Environment(
                        "a stop of the route is not in the index any more".to_string(),
                    ));
                }
                Err(refusals) => {
                    return refused_with_hint(
                        global,
                        "tour start",
                        None,
                        &refusals,
                        "nothing was written",
                    );
                }
            }
        }
        Some(opened) => {
            let lookup = Lookup::new(opened, args.plan.len());
            let nodes = find_all(global, "tour start", &lookup, &args.plan)?;
            match planned_stops(&nodes, 0, None, None) {
                Ok(stops) => stops,
                Err(refusals) => {
                    return refused_with_hint(
                        global,
                        "tour start",
                        None,
                        &refusals,
                        "nothing was written",
                    );
                }
            }
        }
        None => Vec::new(),
    };
    let planned: Vec<StopId> = plan.iter().map(|s| s.id).collect();
    let root = project.git_root().to_path_buf();
    let started = tokio::task::spawn_blocking(move || store::start_planned(&root, params, plan))
        .await
        .map_err(|e| Failure::Environment(e.to_string()))?
        .map_err(store_failure)?;
    let created = if started.created_vnoc {
        [".vnoc/vnoc.toml", ".vnoc/.gitignore"]
            .map(String::from)
            .to_vec()
    } else {
        Vec::new()
    };
    let rows = if planned.is_empty() {
        String::new()
    } else {
        let vnoc_root = store::vnoc_root(project.git_root());
        plan_rows(&read_tour(&vnoc_root, &started.id)?, &planned)
    };
    out::render(
        &Started {
            schema: 1,
            command: "tour start",
            tour: started.id.to_string(),
            folder: "private",
            path: doc_path(project, &started.path),
            created_vnoc: started.created_vnoc,
            created,
            planned: planned.iter().map(ToString::to_string).collect(),
            rows,
        },
        global,
    )
}

// --------------------------------------------------------------------------------------------------------------------- add

/// The text of a stop: a file, or standard input for `-`, as UTF-8. Over 16 MB it is not read further; `body::read` refuses anything over 64 KB
/// and says its size.
fn read_body(source: &str) -> Result<String, Failure> {
    let mut bytes = Vec::new();
    let read = if source == "-" {
        std::io::stdin()
            .lock()
            .take(BODY_READ_CAP + 1)
            .read_to_end(&mut bytes)
    } else {
        std::fs::File::open(source)
            .and_then(|file| file.take(BODY_READ_CAP + 1).read_to_end(&mut bytes))
    };
    read.map_err(|e| Failure::Usage(format!("could not read the text from {source}: {e}")))?;
    if bytes.len() as u64 > BODY_READ_CAP {
        return Err(Failure::Usage(format!(
            "the text is over {BODY_READ_CAP} bytes: the most one stop takes is {}",
            crate::tours::body::MAX_BODY_BYTES
        )));
    }
    String::from_utf8(bytes)
        .map_err(|_| Failure::Usage(format!("the text in {source} is not UTF-8")))
}

/// One block of a stop as `add` reports it: where it is and the first line it lights, so a wrong line number is seen at once.
#[derive(Serialize)]
struct AddedBlock {
    id: String,
    /// `parent` of a detail
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    lines: [u32; 2],
    /// the code of the block's first line, trimmed
    first_line: String,
}

#[derive(Serialize)]
struct Added {
    schema: u32,
    command: &'static str,
    tour: String,
    stop: String,
    /// its place on the route, from 1
    number: usize,
    kind: Option<String>,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[u32; 2]>,
    /// the intro, then every block, in reading order
    steps: Vec<String>,
    blocks: Vec<AddedBlock>,
    /// stops written, and stops still planned
    written: usize,
    planned: usize,
    /// the tour was pinned and is not any more
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    unpinned: bool,
    /// `--replace`: the stop was written again where it stood
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    replaced: bool,
    /// the stop was planned, and this call wrote it (the id and the place are the plan's, unless `--from` moved it)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    was_planned: bool,
    /// `--to`: blocks were added to this stop (and `under` this block)
    #[serde(skip_serializing_if = "Option::is_none")]
    added_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    under: Option<String>,
    #[serde(skip)]
    headline: String,
}

impl Render for Added {
    fn text(&self, _quiet: bool) -> String {
        let of = if self.planned == 0 {
            format!(
                "{} {} written",
                self.written,
                if self.written == 1 { "stop" } else { "stops" }
            )
        } else {
            format!(
                "{} of {} stops written",
                self.written,
                self.written + self.planned
            )
        };
        let mut text = if self.added_to.is_some() {
            let new = self.blocks.len();
            format!(
                "{} +{new} {}   stop {}   {}   {} in all\n",
                self.stop,
                if new == 1 { "block" } else { "blocks" },
                self.number,
                self.headline,
                steps_words(self.steps.len())
            )
        } else {
            format!(
                "{} {}   stop {}   {}   {}   · {of}\n",
                self.stop,
                if self.replaced {
                    "replaced"
                } else if self.was_planned {
                    "written (it was planned)"
                } else {
                    "written"
                },
                self.number,
                self.headline,
                steps_words(self.steps.len())
            )
        };
        let id_width = self.blocks.iter().map(|b| b.id.len()).max().unwrap_or(0);
        let lines_of = |b: &AddedBlock| {
            if b.lines[0] == b.lines[1] {
                b.lines[0].to_string()
            } else {
                format!("{}-{}", b.lines[0], b.lines[1])
            }
        };
        let lines_width = self
            .blocks
            .iter()
            .map(|b| lines_of(b).len())
            .max()
            .unwrap_or(0);
        for block in &self.blocks {
            let indent = if block.parent.is_some() { "    " } else { "  " };
            text.push_str(&format!(
                "{indent}{:<id_width$}  {:<lines_width$}   {}\n",
                block.id,
                lines_of(block),
                block.first_line,
            ));
        }
        if self.unpinned {
            text.push_str("not pinned any more: the tour changed after it was pinned\n");
        }
        text
    }
}

/// A Missing node is no node to write about: the code is gone.
fn missing_error(was: &str, id: &impl std::fmt::Display) -> NodeError {
    NodeError::new(
        "missing",
        format!(
            "a stop is about code that is there: {was} is Missing (its code is gone). Point its notes at where the code is now with `vnoc relink {id} <node>`"
        ),
    )
}

/// One refusal of a call, as `--json` has it: where in the text, and what is wrong.
#[derive(Serialize)]
struct RefusedProblem<'a> {
    #[serde(rename = "where")]
    place: &'a str,
    problem: &'a str,
}

/// A refused call as `--json` has it (`schema` and `command` first, as every command's value).
#[derive(Serialize)]
struct Refused<'a> {
    schema: u32,
    command: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tour: Option<String>,
    refused: Vec<RefusedProblem<'a>>,
}

/// What a refused call says: with `--json` the refusals as `{ "where", "problem" }` on standard output, else the problems under one line on
/// standard error. Nothing was written. Exit `2`.
fn refused_stop(
    global: &Global,
    command: &'static str,
    tour: &TourId,
    refusals: &[Refusal],
) -> Result<(), Failure> {
    refused_with_hint(
        global,
        command,
        Some(tour),
        refusals,
        "fix them, then send the whole stop again",
    )
}

fn refused_with_hint(
    global: &Global,
    command: &'static str,
    tour: Option<&TourId>,
    refusals: &[Refusal],
    hint: &str,
) -> Result<(), Failure> {
    if global.json {
        out::json(&Refused {
            schema: 1,
            command,
            tour: tour.map(ToString::to_string),
            refused: refusals
                .iter()
                .map(|r| RefusedProblem {
                    place: &r.place,
                    // the sentence carries its own place; the field already says where
                    problem: r
                        .problem
                        .strip_prefix(r.place.as_str())
                        .and_then(|rest| rest.strip_prefix(": "))
                        .unwrap_or(&r.problem),
                })
                .collect(),
        })?;
    } else {
        let items: Vec<String> = refusals.iter().map(|r| r.problem.clone()).collect();
        let problems = if items.len() == 1 {
            "1 problem".to_string()
        } else {
            format!("{} problems", items.len())
        };
        out::error(&format!("not written: {problems}"));
        out::hints(hint, &items, 0);
    }
    Err(Failure::Reported)
}

/// The refusal of a write on a tour the reader stopped (02 §5): the text, as the answer, on standard output, and exit `1`. `resume`, `edit`, `share`
/// and `delete` do not ask.
fn stopped(
    global: &Global,
    command: &'static str,
    vnoc_root: &Path,
    lock: &ToursLock,
    tour: &Tour,
) -> Result<(), Failure> {
    let Some(written) = store::reader_stopped(lock, vnoc_root, tour).map_err(store_failure)? else {
        return Ok(());
    };
    let message = view::stopped_text(&tour.id, written);
    if global.json {
        #[derive(Serialize)]
        struct Stopped<'a> {
            schema: u32,
            command: &'static str,
            tour: String,
            stopped: bool,
            written: usize,
            message: &'a str,
        }
        out::json(&Stopped {
            schema: 1,
            command,
            tour: tour.id.to_string(),
            stopped: true,
            written,
            message: &message,
        })?;
    } else {
        out::text(&message)?;
    }
    Err(Failure::Stopped)
}

/// The same refusal, asked before a command does anything slow (a driver, the index): takes the lock, reads the tour, and asks.
async fn early_stop(
    global: &Global,
    command: &'static str,
    vnoc_root: &Path,
    id: &TourId,
) -> Result<(), Failure> {
    let lock = take_lock(vnoc_root.to_path_buf()).await?;
    let tour = read_tour(vnoc_root, id)?;
    stopped(global, command, vnoc_root, &lock, &tour)
}

async fn add(global: &Global, args: &TourAddArgs) -> Result<(), Failure> {
    // clap's `requires` does not hold here (it lets `--under` through with a node and no `--to`)
    if args.under.is_some() && args.to.is_none() {
        return Err(Failure::Usage(
            "--under goes with --to: vnoc tour add <tour> --to <stop> --under <block>".to_string(),
        ));
    }
    // what needs no driver: the tour (in one place, and not stopped), and the text
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let tour_id = resolve(global, "tour add", &vnoc_root, &args.tour)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &tour_id) {
        return Err(store_failure(StoreError::InBoth(tour_id)));
    }
    early_stop(global, "tour add", &vnoc_root, &tour_id).await?;
    let body = read_body(&args.body_file)?;

    // the code as it is now
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let node = match &args.node {
        Some(text) => match lookup.resolve(text) {
            Ok(Target::Node(node)) => Some(node),
            Ok(Target::Missing(missing)) => {
                return refused_with(global, "tour add", missing_error(&missing.was, &missing.id));
            }
            Err(error) => return refused_with(global, "tour add", error),
        },
        None => None,
    };

    // the lock for the rest: the ids come from the file read under it, and the records go to the same file under it (01 §5)
    let lock = take_lock(vnoc_root.clone()).await?;
    let tour = read_tour(&vnoc_root, &tour_id)?;
    stopped(global, "tour add", &vnoc_root, &lock, &tour)?;
    let was_pinned = tour.pinned();
    let planned = {
        let index = opened.index.read().expect("the index lock is not poisoned");
        match (args.to, node) {
            (Some(to), _) => plan_more(
                &tour,
                &index,
                &opened.project,
                &lookup.tree,
                to,
                args.under,
                &body,
            ),
            (None, Some(node)) => plan_stop(
                &tour,
                &index,
                &opened.project,
                &lookup.tree,
                node,
                &body,
                AddOptions {
                    from: args.from,
                    line: args.line,
                    same_as: args.same_as,
                    replace: args.replace,
                },
            ),
            (None, None) => unreachable!("clap wants a node without --to"),
        }
    };
    let records = match planned {
        Ok(records) => records,
        Err(refusals) => return refused_stop(global, "tour add", &tour_id, &refusals),
    };
    store::append_locked(&lock, &vnoc_root, &tour_id, &records).map_err(store_failure)?;

    // what was written, as the replay has it now (the number is the place on the route)
    let stop_id = match (&records.first(), args.to) {
        (_, Some(to)) => to,
        (Some(Record::Stop(written)), None) => written.id,
        _ => {
            return Err(Failure::Environment(
                "a stop was planned with no stop record".to_string(),
            ));
        }
    };
    // a planned stop that this call wrote: it was on the route before, not written
    let was_planned = args.to.is_none()
        && args.replace.is_none()
        && tour.stop(stop_id).is_some_and(|s| !s.written);
    let after = read_tour(&vnoc_root, &tour_id)?;
    let Some(stop) = after.stop(stop_id) else {
        return Err(Failure::Environment(format!(
            "the stop {stop_id} was written and is not in the tour"
        )));
    };
    let text =
        source::read_text(&opened.project.git_root().join(stop.path.as_str())).unwrap_or_default();
    let blocks: Vec<AddedBlock> = records
        .iter()
        .filter_map(|record| match record {
            Record::Step(step) => Some(AddedBlock {
                id: step.id.to_string(),
                parent: step.parent.map(|p| p.to_string()),
                lines: [step.lines.first, step.lines.last],
                first_line: first_line(&text, step.lines.first),
            }),
            _ => None,
        })
        .collect();
    let size = after.size();
    out::render(
        &Added {
            schema: 1,
            command: "tour add",
            tour: tour_id.to_string(),
            stop: stop_id.to_string(),
            number: after.number(stop_id).unwrap_or(after.stops.len()),
            kind: stop.kind.as_ref().map(|k| k.as_str().to_string()),
            path: stop.path.to_string(),
            lines: stop.lines.map(|l| [l.first, l.last]),
            steps: steps_in_order(stop)
                .iter()
                .map(ToString::to_string)
                .collect(),
            blocks,
            written: size.written,
            planned: size.planned,
            unpinned: was_pinned,
            replaced: args.replace.is_some(),
            was_planned,
            added_to: args.to.map(|to| to.to_string()),
            under: args.under.map(|u| u.to_string()),
            headline: view::stop_headline(stop),
        },
        global,
    )
}

// ----------------------------------------------------------------------------------------------------------------------------------- route

/// A route and what the driver said when it was asked for it.
struct Built {
    route: route::Route,
    /// the driver has no call trees: the route is the node alone
    no_calls: bool,
}

/// The route of a node (02 §11): a function's from the driver's call tree (`calls::ask`, merged), a class's, a file's and a folder's from the index. The
/// tree is asked for `max(depth, 2)` levels, so the sizes of depths 1 and 2 are there whatever the depth. Reads only.
async fn build_route(
    global: &Global,
    command: &'static str,
    opened: &Opened,
    lookup: &Lookup<'_>,
    text: &str,
    depth: u32,
) -> Result<Built, Failure> {
    let node = match lookup.resolve(text) {
        Ok(Target::Node(node)) => node,
        Ok(Target::Missing(missing)) => {
            refused_with(global, command, missing_error(&missing.was, &missing.id))?;
            unreachable!("a refusal is a failure")
        }
        Err(error) => {
            refused_with(global, command, error)?;
            unreachable!("a refusal is a failure")
        }
    };
    let Some(root) = route::known_of(node) else {
        refused_with(
            global,
            command,
            NodeError::new(
                "root",
                "the project's root folder is not a stop: name a folder in it",
            ),
        )?;
        unreachable!("a refusal is a failure")
    };
    let levels = depth.max(2);
    let mut no_calls = false;
    let (item, how) = if node.kind == crate::index::Kind::Function {
        let tree = match calls::ask(
            &opened.project,
            &opened.index,
            &opened.host,
            &node.id,
            Some(levels),
        )
        .await
        {
            Ok(answer) => Some(calls::merge(&answer.root)),
            // a driver with no call trees: the node alone, and a line that says so
            Err(CallsError::MissingCapability(_)) => {
                no_calls = true;
                None
            }
            Err(CallsError::Invalid(message)) => return Err(Failure::NoSuchNode(message)),
            Err(CallsError::Internal(message)) => return Err(Failure::Environment(message)),
        };
        let known = |target: &vnoc_driver_protocol::CallTarget| {
            let location = vnoc_domain::LocationKey::new(
                opened.project.repo_path(target.file.as_deref()?).ok()?,
                target.qualname.clone(),
                target.occurrence,
            )
            .ok()?;
            lookup
                .tree
                .find(&location.to_string())
                .and_then(route::known_of)
        };
        let item = match &tree {
            Some(merged) => route::from_calls(merged, root, levels, &known),
            None => route::Item {
                known: root,
                line: None,
                children: Vec::new(),
            },
        };
        (item, route::How::Calls)
    } else {
        let item = route::from_tree(node, levels).ok_or_else(|| {
            Failure::Environment("the node is not a stop: name a folder in the project".to_string())
        })?;
        (item, route::How::Contents)
    };
    Ok(Built {
        route: route::propose(&item, how, depth),
        no_calls,
    })
}

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

async fn route(global: &Global, args: &TourRouteArgs) -> Result<(), Failure> {
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
    out::render(
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
    )
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

async fn plan(global: &Global, args: &TourPlanArgs) -> Result<(), Failure> {
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
    out::render(
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

/// Line `number` of `text`, trimmed, and cut at 72 characters.
fn first_line(text: &str, number: u32) -> String {
    let line = hash::lines_of(text, number, number)
        .first()
        .map_or("", |l| l.trim());
    if line.chars().count() > 72 {
        let cut: String = line.chars().take(71).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    }
}

// --------------------------------------------------------------------------------------------------------------------- finish

/// What `finish` did about IDs, for `--json`.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum IdsStep {
    /// the setting is `ask`: the question for the user, not done
    Ask { missing: usize, command: String },
    /// written, as `vnoc tour ids` does
    Written {
        stops: Vec<IdStop>,
        changed: Vec<String>,
        recorded: bool,
    },
}

#[derive(Serialize)]
struct Finished {
    schema: u32,
    command: &'static str,
    tour: String,
    /// the tour was finished before this call: nothing was written
    already: bool,
    stops: usize,
    steps: usize,
    /// planned stops that were never written and left the route
    left_the_route: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    ids: Option<IdsStep>,
    /// what was read as `ask` because it is not a value (`[tours]` in `vnoc.toml`), once
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    /// step 2: what became of the pin
    #[serde(skip_serializing_if = "Option::is_none")]
    pin: Option<PinOut>,
    /// the text of the IDs step: the line to ask the user, or what was written
    #[serde(skip)]
    ids_text: String,
    /// the text of the pin step: what was committed, the line to ask the user, or why it is not pinned
    #[serde(skip)]
    pin_text: String,
    /// `8a1c2e4`, when the tour is pinned now
    #[serde(skip)]
    pinned_at: Option<String>,
}

impl Render for Finished {
    fn text(&self, _quiet: bool) -> String {
        let stops = format!(
            "{} {}",
            self.stops,
            if self.stops == 1 { "stop" } else { "stops" }
        );
        let head = if self.already {
            "already finished"
        } else {
            "finished"
        };
        let mut text = format!("{head}: {stops}, {}", steps_words(self.steps));
        if self.left_the_route > 0 {
            text.push_str(&format!(
                " · {} planned {} the route",
                self.left_the_route,
                if self.left_the_route == 1 {
                    "stop left"
                } else {
                    "stops left"
                }
            ));
        }
        if let Some(at) = &self.pinned_at {
            text.push_str(&format!(" · pinned at {at}"));
        }
        text.push('\n');
        text.push_str(&self.ids_text);
        text.push_str(&self.pin_text);
        for note in &self.notes {
            text.push_str(&format!("note: {note}\n"));
        }
        text
    }
}

async fn finish(global: &Global, args: &TourFinishArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let tour_id = resolve(global, "tour finish", &vnoc_root, &args.tour)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &tour_id) {
        return Err(store_failure(StoreError::InBoth(tour_id)));
    }
    let lock = take_lock(vnoc_root.clone()).await?;
    let tour = read_tour(&vnoc_root, &tour_id)?;
    stopped(global, "tour finish", &vnoc_root, &lock, &tour)?;
    let shown = view::tour_view(&tour, Where::Private);
    // TR14b: a tour with no stop written (none at all, or planned only) has nothing to finish; an agent that chained `add` and `finish` in one call
    // finished an empty tour when the `add` was refused. The request's mistake, exit `2`, and nothing is written
    if shown.written == 0 {
        return Err(Failure::Usage(
            "no stop is written: add one first. Nothing was finished".to_string(),
        ));
    }
    let already = tour.state == crate::tours::State::Finished;
    if !already {
        let end = Record::End(EndRecord { at: time::now() });
        store::append_locked(&lock, &vnoc_root, &tour_id, &[end]).map_err(store_failure)?;
    }
    // the tours lock is let go: the IDs step takes the project's write lock, and then the tours lock again inside each edit (that order only)
    drop(lock);

    // step 1 of 02 §5: the IDs. A finished tour is not offered them again by the setting; a flag asks for them whatever
    let by_flag = args.ids || args.no_ids;
    let step = ids_after_end(
        global,
        &project,
        &vnoc_root,
        &tour,
        (args.ids, args.no_ids),
        !already || by_flag,
        None,
    )
    .await?;

    // step 2 of 02 §5: the pin, always tried. A finished tour is not offered a commit again by the setting; the flag asks whatever
    let pinned = if !already || args.commit {
        let read = settings::read(&vnoc_root).map_err(settings_failure)?;
        let commit = finishing::effective(read.commit.value, args.commit, args.no_commit);
        let ask = Ask::Finish {
            commit,
            message: args.message.as_deref(),
        };
        // the code is read again when the IDs step wrote into it: an index built before that is a moment old
        let mut lazy = Lazy::default();
        Some(pin_tour(global, &mut lazy, &project, &vnoc_root, &tour_id, &ask).await?)
    } else {
        None
    };
    let (pin_out, pin_text, pinned_at) = match pinned {
        Some(p) => {
            let mut text = p.lines.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            (Some(p.out), text, p.at)
        }
        None => (None, String::new(), None),
    };
    out::render(
        &Finished {
            schema: 1,
            command: "tour finish",
            tour: tour_id.to_string(),
            already,
            stops: shown.written,
            steps: shown.steps,
            left_the_route: if already { 0 } else { shown.planned },
            ids: step.ids,
            notes: step.notes,
            pin: pin_out,
            ids_text: step.text,
            pin_text,
            pinned_at,
        },
        global,
    )
}

/// What the IDs step after `end` did: for the answer, as data and as text.
struct IdsAfterEnd {
    ids: Option<IdsStep>,
    text: String,
    notes: Vec<String>,
}

/// Step 1 of 02 §5, for `finish` and `import`: by `[tours] ids` and the two flags (`--ids`, `--no-ids`), the IDs are written, or the line for the agent to ask the
/// user is said, or nothing. `run` is false for a tour that was finished already (the setting does not offer again). `opened` is the index when the caller has
/// one; else it is built, and only for a tour with a stop that had no ID when it was written. No driver is a note, not a failure.
async fn ids_after_end(
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
fn project_writes_ids(project: &crate::project::Project) -> Result<bool, Failure> {
    act::project_writes_ids(project).map_err(|e| act_failure(e, project))
}

/// A real check of the tour against the code as it is now, and where its stops and blocks are (what a pin records).
fn placed_check(opened: &Opened, tour: &Tour, len: u64) -> (Recorded, check::Placed) {
    act::placed_check(&opened.project, &opened.index, tour, len)
}

/// The stops of the tour with what there is to do for each, from the index as it is now.
fn picks_of(
    opened: &Opened,
    tour: &Tour,
    named: &[crate::tours::StopId],
) -> Vec<(crate::tours::StopId, crate::tours::ids::Pick)> {
    act::picks_of(&opened.project, &opened.index, tour, named)
}

/// The pin of one tour (01 §8, 02 §5 and §8): `act::pin_begin` asks what needs no code, the index is opened for the rest, `act::pin_finish` does it.
async fn pin_tour(
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
async fn write_ids(
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
async fn can_pin_words(
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

async fn pin_command(global: &Global, args: &TourPinArgs) -> Result<(), Failure> {
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
    out::render(
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

/// A refusal with the nearest names under it ("did you mean"): one sentence, and the list.
fn no_such(
    global: &Global,
    command: &'static str,
    why: &'static str,
    said: String,
    items: Vec<String>,
) -> Result<(), Failure> {
    let mut error = NodeError::new(why, said);
    if !items.is_empty() {
        error.message = format!("{} · did you mean: {}", error.message, items.join(" · "));
        error.hints = Some(Hints {
            lead: "did you mean".to_string(),
            items,
            more: 0,
        });
    }
    refused_with(global, command, error)
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

async fn drop_id(global: &Global, args: &TourDropArgs) -> Result<(), Failure> {
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
    out::render(
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

async fn resume(global: &Global, args: &TourResumeArgs) -> Result<(), Failure> {
    let (_project, w) = writing(global, "tour resume", &args.tour).await?;
    let stopped = matches!(w.tour.state, State::Stopping | State::Stopped);
    if stopped {
        store::append_locked(&w.lock, &w.vnoc_root, &w.id, &[Record::Resume(Bare {})])
            .map_err(store_failure)?;
    }
    out::render(
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

async fn halt(global: &Global, args: &TourResumeArgs) -> Result<(), Failure> {
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
    out::render(
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

async fn edit(global: &Global, args: &TourEditArgs) -> Result<(), Failure> {
    let title = args.title.as_deref().map(str::trim);
    let question = args.question.as_deref().map(str::trim);
    let problems = store::check_edit(title, question);
    if !problems.is_empty() {
        return Err(Failure::Usage(problems.join("\n  ")));
    }
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour edit", &vnoc_root, &args.tour)?;
    act::edit(&vnoc_root, &id, title, question, args.detail.map(detail_of))
        .map_err(|e| act_failure(e, &project))?;
    out::render(
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

async fn share(global: &Global, args: &TourShareArgs) -> Result<(), Failure> {
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
    out::render(
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

async fn ids(global: &Global, args: &TourIdsArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let vnoc_root = store::vnoc_root(opened.project.git_root());
    let id = resolve(global, "tour ids", &vnoc_root, &args.tour)?;
    let written = write_ids(global, &opened, &vnoc_root, &id, &args.stops).await?;
    out::render(&written, global)
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

async fn settings(global: &Global, args: &TourSettingsArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let choice = |arg: super::args::ChoiceArg| match arg {
        super::args::ChoiceArg::Always => Choice::Always,
        super::args::ChoiceArg::Ask => Choice::Ask,
        super::args::ChoiceArg::Never => Choice::Never,
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
    out::render(&shown, global)
}

// -------------------------------------------------------------------------------------------------------------- export, import

/// A stop's name as the document says it, for a stop that is not found now: the name it was written with, in the opened folder's terms (the path is the
/// repo's; a project opened below its git root shows paths without the prefix).
fn written_name(project: &crate::project::Project, stop: &crate::tours::TourStop) -> String {
    let name = doc::node_name(stop);
    match project.prefix() {
        "" => name,
        prefix => name
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix('/'))
            .map(str::to_string)
            .unwrap_or(name),
    }
}

async fn export(global: &Global, args: &TourExportArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour export", &vnoc_root, &args.tour)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &id) {
        return Err(store_failure(StoreError::InBoth(id)));
    }
    let tour = read_tour(&vnoc_root, &id)?;
    // the name each stop has now where it is found (by its ID, else by its name); where it is not, or there is no driver, the name it was written with
    let mut now: BTreeMap<crate::tours::StopId, String> = tour
        .stops
        .iter()
        .filter(|s| s.written)
        .map(|s| (s.id, written_name(&project, s)))
        .collect();
    match open_from(global).await {
        Ok(opened) => {
            let index = opened.index.read().expect("the index lock is not poisoned");
            let tree = index.tree(&project.name(), project.prefix());
            for stop in tour.stops.iter().filter(|s| s.written) {
                if let Some(node) = find(&index, &project, &tree, stop) {
                    now.insert(stop.id, node.location());
                }
            }
        }
        Err(Failure::Environment(_)) => {}
        Err(e) => return Err(e),
    }
    out::raw(&doc::print(&doc::nest(&tour, &now, args.flat)))
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

async fn import(global: &Global, args: &TourImportArgs) -> Result<(), Failure> {
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

    let (tour_id, kept, replaced) = match replace_id {
        Some(id) => {
            // the lock for the rest: the ids come from the file read under it, and the new file is written under the same lock (01 §5)
            let lock = take_lock(vnoc_root.clone()).await?;
            let old = read_tour(&vnoc_root, &id)?;
            if matches!(old.state, State::Writing | State::Stopping) {
                return Err(store_failure(StoreError::Refused(format!(
                    "the tour {id} is being written ({}): stop it or finish it first",
                    old.state
                ))));
            }
            let planned = {
                let index = opened.index.read().expect("the index lock is not poisoned");
                crate::tours::add::plan_tour(
                    &index,
                    &opened.project,
                    &resolve_node,
                    &flat,
                    Some(&old),
                )
            };
            let mut records = match planned {
                Ok(records) => records,
                Err(refusals) => {
                    return refused_with_hint(global, "tour import", Some(&id), &refusals, hint);
                }
            };
            records.push(Record::End(EndRecord { at: time::now() }));
            store::replace_locked(&lock, &vnoc_root, &id, &records).map_err(store_failure)?;
            drop(lock);
            let kept_ids: Vec<crate::tours::StopId> = records
                .iter()
                .filter_map(|r| match r {
                    Record::Stop(stop) if old.stop(stop.id).is_some_and(|o| o.written) => {
                        Some(stop.id)
                    }
                    _ => None,
                })
                .collect();
            let gone = old
                .stops
                .iter()
                .filter(|s| s.written && !kept_ids.contains(&s.id))
                .count();
            (id, Some((kept_ids.len(), gone)), true)
        }
        None => {
            let planned = {
                let index = opened.index.read().expect("the index lock is not poisoned");
                crate::tours::add::plan_tour(&index, &opened.project, &resolve_node, &flat, None)
            };
            let mut records = match planned {
                Ok(records) => records,
                Err(refusals) => {
                    return refused_with_hint(global, "tour import", None, &refusals, hint);
                }
            };
            records.push(Record::End(EndRecord { at: time::now() }));
            let started = store::start(
                project.git_root(),
                StartParams {
                    title: flat.title.trim().to_string(),
                    question: flat.question.as_ref().map(|q| q.trim().to_string()),
                    author: flat.author.clone(),
                    detail: flat.detail.unwrap_or(Detail::Normal),
                },
            )
            .map_err(store_failure)?;
            store::append(&vnoc_root, &started.id, &records).map_err(store_failure)?;
            (started.id, None, false)
        }
    };

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
    out::render(
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

async fn delete(global: &Global, args: &TourDeleteArgs) -> Result<(), Failure> {
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
    out::render(
        &Deleted {
            schema: 1,
            command: "tour delete",
            tours,
        },
        global,
    )
}

// ------------------------------------------------------------------------------------------------------------- checking

/// The index and the drivers, opened the first time a command needs them: a tour whose record holds needs neither (03 §3).
#[derive(Default)]
struct Lazy(Option<Opened>);

impl Lazy {
    async fn get(&mut self, global: &Global) -> Result<&Opened, Failure> {
        if self.0.is_none() {
            self.0 = Some(open_from(global).await?);
        }
        Ok(self.0.as_ref().expect("opened just now"))
    }
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
        let id_width = self.tours.iter().map(|t| t.id.len()).max().unwrap_or(0);
        let word_width = self
            .tours
            .iter()
            .filter(|t| t.readable)
            .map(|t| t.word.len())
            .max()
            .unwrap_or(0);
        let mut text = String::new();
        for tour in &self.tours {
            let row = if tour.readable {
                format!(
                    "{:<id_width$}  {:<word_width$}  {}",
                    tour.id, tour.word, tour.lines[0]
                )
            } else {
                format!("{:<id_width$}  {}", tour.id, tour.word)
            };
            text.push_str(row.trim_end());
            text.push('\n');
            for line in tour.lines.iter().skip(usize::from(tour.readable)) {
                text.push_str(&format!("  {line}\n"));
            }
        }
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
            text.push_str(&said);
            text.push('\n');
        }
        text
    }
}

/// The tours a command names (an id, or the start of one), or every tour; one row for each id (the project's copy of a tour that is in both folders).
fn chosen(rows: Vec<Row>, wanted: &[TourId]) -> Vec<Row> {
    let mut seen = std::collections::BTreeSet::new();
    rows.into_iter()
        .filter(|r| (wanted.is_empty() || wanted.contains(&r.id)) && seen.insert(r.id.clone()))
        .collect()
}

async fn check(global: &Global, args: &TourCheckArgs) -> Result<(), Failure> {
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
        view::list_text(&self.tours)
    }
}

async fn list(global: &Global, args: &TourListArgs) -> Result<(), Failure> {
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

async fn show(global: &Global, args: &TourShowArgs) -> Result<(), Failure> {
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
            out::render(
                &Shown {
                    schema: 1,
                    command: "tour show",
                    readable: true,
                    tour: tour_view,
                    text,
                },
                global,
            )
        }
        Err(unreadable) => out::render(
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

async fn where_command(global: &Global, args: &TourWhereArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = match &args.tour {
        Some(text) => resolve(global, "tour where", &vnoc_root, text)?,
        None => match last_moved(&vnoc_root) {
            Some(id) => id,
            None => {
                return out::render(
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
    out::render(
        &WhereOut {
            schema: 1,
            command: "tour where",
            moved_by: None,
            nothing_to_go_back_to: false,
            view,
            text,
        },
        global,
    )
}

async fn go_command(global: &Global, args: &TourGoArgs) -> Result<(), Failure> {
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
    out::render(
        &WhereOut {
            schema: 1,
            command: "tour go",
            moved_by: Some(args.r#move.clone()),
            nothing_to_go_back_to: gone.nothing_to_go_back_to,
            view,
            text,
        },
        global,
    )
}
