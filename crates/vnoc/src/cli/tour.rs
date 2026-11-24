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
    TourExportArgs, TourFinishArgs, TourIdsArgs, TourImportArgs, TourListArgs, TourPinArgs,
    TourResumeArgs, TourSettingsArgs, TourShareArgs, TourShowArgs, TourStartArgs,
};
use super::nodes::{Hints, Lookup, NodeError, Target, doc_path};
use super::out::{self, Render};
use super::project::{Opened, open_from, open_without_driver};
use super::write::{attach_failure, marker_words, take_lock as take_write_lock};
use super::{Failure, Global};
use crate::source;
use crate::tours::add::{
    AddOptions, block_of, did_you_mean_stops, nested_in, plan_more, plan_stop,
};
use crate::tours::check::{self, Answer, Code, Mode, Recorded, Source};
use crate::tours::doc;
use crate::tours::find::find;
use crate::tours::finish::{self as finishing, Step};
use crate::tours::on::{self, Map, Query};
use crate::tours::pin::{self, Changed, Pinnable, Reason};
use crate::tours::record::{
    Bare, Commit, Detail, DropRecord, EndRecord, MetaRecord, Record, StepId, StopId,
};
use crate::tours::rules::Refusal;
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
        TourCommand::Finish(args) => finish(global, args).await,
        TourCommand::Pin(args) => pin_command(global, args).await,
        TourCommand::List(args) => list(global, args).await,
        TourCommand::Show(args) => show(global, args).await,
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

/// The tour as it is now, under the lock the caller holds. A tour that cannot be read cannot be written to.
fn read_tour(vnoc_root: &Path, id: &TourId) -> Result<crate::tours::Tour, Failure> {
    let (_, folded) = store::read(vnoc_root, id).map_err(store_failure)?;
    folded.map_err(|e| {
        let said = format!("the tour {id} cannot be read: {e}");
        // a tour of a newer schema is for a newer V-NOC (the environment); a broken one is the request's
        if e.too_new {
            Failure::Environment(said)
        } else {
            Failure::Usage(said)
        }
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
}

impl Render for Started {
    fn text(&self, quiet: bool) -> String {
        let mut text = format!("started {}   {}   {}\n", self.tour, self.folder, self.path);
        if self.created_vnoc {
            text.push_str("created .vnoc/ (vnoc.toml, .gitignore)\n");
        }
        if !quiet {
            text.push_str(&format!(
                "next: vnoc tour add {} <node> --body-file -\n",
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

async fn start(global: &Global, args: &TourStartArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
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
    let root = project.git_root().to_path_buf();
    let started = tokio::task::spawn_blocking(move || store::start(&root, params))
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
    out::render(
        &Started {
            schema: 1,
            command: "tour start",
            tour: started.id.to_string(),
            folder: "private",
            path: doc_path(&project, &started.path),
            created_vnoc: started.created_vnoc,
            created,
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
                if self.replaced { "replaced" } else { "written" },
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
            added_to: args.to.map(|to| to.to_string()),
            under: args.under.map(|u| u.to_string()),
            headline: view::stop_headline(stop),
        },
        global,
    )
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
        if let Some(at) = &self.pinned_at {
            text.push_str(&format!(" · pinned at {at}"));
        }
        text.push('\n');
        if self.left_the_route > 0 {
            text.push_str(&format!(
                "{} planned {} never written and left the route\n",
                self.left_the_route,
                if self.left_the_route == 1 {
                    "stop was"
                } else {
                    "stops were"
                }
            ));
        }
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

/// How a pin is asked for.
enum Ask<'a> {
    /// `finish` and `import` (02 §5): files that are not committed are dealt with by `[tours] commit`, or by the flags of this call
    Finish {
        commit: Choice,
        message: Option<&'a str>,
    },
    /// `vnoc tour pin` (02 §8): files are committed only with `--commit`
    Pin {
        commit: bool,
        message: Option<&'a str>,
    },
}

/// What a commit made to pin did.
#[derive(Serialize)]
struct CommittedOut {
    commit: String,
    message: String,
    /// the tour's code files
    files: Vec<String>,
    /// the `node.toml` of the tour's IDs that were committed with them (TR9b), under the code files in the text
    #[serde(skip_serializing_if = "Vec::is_empty")]
    node_files: Vec<String>,
}

impl CommittedOut {
    /// `committed 2 files as 3c9e1f7   <message>` and the files under it, code first, then the node files of the IDs.
    fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "committed {} as {}   {}",
            count_files(self.files.len() + self.node_files.len()),
            &self.commit[..7],
            self.message
        )];
        lines.extend(
            self.files
                .iter()
                .chain(&self.node_files)
                .map(|f| format!("  {f}")),
        );
        lines
    }
}

/// What became of the pin of one tour, for `--json`.
#[derive(Serialize)]
struct PinOut {
    /// `pinned`, `ask` (the user is to be asked: `[tours] commit` is `ask`) or `not_pinned`
    state: &'static str,
    /// the pinned commit, all 40 characters
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    committed: Option<CommittedOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reasons: Vec<Reason>,
    /// for `ask`: the command that would do it
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    /// for `ask`: the `node.toml` files of the tour's IDs that `--commit` would commit with the code files (which are in `reasons`)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    node_files: Vec<String>,
    /// `--commit` was asked and was refused: nothing was committed
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    refused: bool,
}

/// What [`pin_tour`] did: the answer, the lines to print under it (no end of line), and the 7 characters of the commit when the tour is pinned now.
struct Pinned {
    out: PinOut,
    lines: Vec<String>,
    at: Option<String>,
}

fn short_commit(commit: &Commit) -> String {
    commit.as_str()[..7].to_string()
}

/// Not pinned, and why, worded for the way it was asked.
fn not_pinned(
    ask: &Ask<'_>,
    id: &TourId,
    pinnable: &Pinnable,
    committed: Option<CommittedOut>,
    node_files: &[Changed],
) -> Pinned {
    let Pinnable::No(reasons) = pinnable else {
        unreachable!("not_pinned is for a pin that cannot be made")
    };
    let of_this_tour = matches!(ask, Ask::Pin { .. });
    // a commit that was made stays made: it is said first, and the reason under it
    let mut lines: Vec<String> = committed
        .as_ref()
        .map(CommittedOut::lines)
        .unwrap_or_default();
    for reason in reasons {
        lines.extend(reason.lines(of_this_tour));
    }
    let mut out = PinOut {
        state: "not_pinned",
        commit: None,
        committed,
        reasons: reasons.clone(),
        command: None,
        node_files: Vec::new(),
        refused: false,
    };
    match ask {
        Ask::Finish { commit, .. } => {
            if let (Choice::Ask, Some(files)) = (commit, pinnable.only_uncommitted()) {
                let why = reasons[0].words();
                lines = crate::tours::finish::ask_commit_lines(id, &why, files, node_files);
                out.state = "ask";
                out.command = Some(format!("vnoc tour pin {id} --commit"));
                out.node_files = node_files.iter().map(|f| f.path.to_string()).collect();
            }
        }
        Ask::Pin { commit, .. } => {
            if *commit {
                // `--commit` is for files that are the one thing in the way; nothing was committed unless a commit was made
                out.refused = pinnable.only_uncommitted().is_none() && out.committed.is_none();
                if out.refused {
                    lines.push("nothing was committed: --commit is for files that are the one thing in the way".to_string());
                }
            } else if pinnable.only_uncommitted().is_some() {
                lines.push(format!(
                    "commit them, then run this again. Or: vnoc tour pin {id} --commit"
                ));
            }
        }
    }
    Pinned {
        out,
        lines,
        at: None,
    }
}

/// The pin of one tour (01 §8, 02 §5 and §8): finished, then what git says, then the check against the code as it is now, then HEAD or the reasons;
/// files that are not committed are committed first when `ask` says so. The tour is read here, after whatever the caller wrote before.
async fn pin_tour(
    global: &Global,
    lazy: &mut Lazy,
    project: &crate::project::Project,
    vnoc_root: &Path,
    id: &TourId,
    ask: &Ask<'_>,
) -> Result<Pinned, Failure> {
    let measured = store::read_measured(vnoc_root, id).map_err(store_failure)?;
    let tour = match &measured.tour {
        Ok(tour) => tour,
        Err(e) => return Err(Failure::Usage(format!("the tour {id} cannot be read: {e}"))),
    };
    let no = |reasons: Vec<Reason>| Pinnable::No(reasons);
    // what needs no look at the code
    let head = match pin::head(project, tour) {
        Ok(head) => head,
        Err(reasons) => return Ok(not_pinned(ask, id, &no(reasons), None, &[])),
    };
    let opened = match lazy.get(global).await {
        Ok(opened) => opened,
        Err(Failure::Environment(why)) => {
            return Ok(not_pinned(
                ask,
                id,
                &no(vec![Reason::NotChecked { why }]),
                None,
                &[],
            ));
        }
        Err(e) => return Err(e),
    };
    let (recorded, placed) = placed_check(opened, tour, measured.len);
    // what was just found is the record of this tour: the next look costs `stat`s
    check::record(vnoc_root, id, &recorded).map_err(store_failure)?;

    let mut pinnable = pin::can_pin(project, tour, head, &recorded, &placed);
    let wants_commit = match ask {
        Ask::Finish { commit, .. } => *commit == Choice::Always,
        Ask::Pin { commit, .. } => *commit,
    };
    let mut committed = None;
    // the node files of the tour's IDs go with the code files, and only when code is being committed: they do not decide the pin
    let node_files = if pinnable.only_uncommitted().is_some() {
        pin::node_files(project, tour)
    } else {
        Vec::new()
    };
    if wants_commit && let Some(files) = pinnable.only_uncommitted() {
        let message = match ask {
            Ask::Finish { message, .. } | Ask::Pin { message, .. } => message.map(String::from),
        }
        .unwrap_or_else(|| crate::tours::finish::default_message(&tour.title));
        let code_files: Vec<Changed> = files.to_vec();
        let all: Vec<Changed> = code_files.iter().chain(&node_files).cloned().collect();
        let done = pin::commit_files(project, &all, &message)
            .map_err(|e| Failure::Environment(e.to_string()))?;
        committed = Some(CommittedOut {
            commit: done.commit.as_str().to_string(),
            message,
            files: code_files.iter().map(|f| f.path.to_string()).collect(),
            node_files: node_files.iter().map(|f| f.path.to_string()).collect(),
        });
        // a hook that changed a file while the commit was made: the commit holds code the check never read. It stays; it is not what was checked
        let code_paths: Vec<_> = code_files.iter().map(|f| f.path.clone()).collect();
        let hooked = pin::changed_since_read(project, &placed.read, &code_paths);
        pinnable = match hooked.first() {
            Some(path) => {
                // git left the index with those files as they were before the hook: HEAD's, so the next `pin` finds them committed
                pin::settle_index(project, &hooked);
                Pinnable::No(vec![Reason::HookChanged {
                    path: path.clone(),
                    tour: id.to_string(),
                }])
            }
            // what is on disk is what HEAD holds now: asked again, with the new HEAD
            None => pin::can_pin(project, tour, done.commit, &recorded, &placed),
        };
    }
    match pinnable {
        Pinnable::No(_) => Ok(not_pinned(ask, id, &pinnable, committed, &node_files)),
        Pinnable::Yes { head } => {
            // the tour is as it was when it was read: a write in between would be a stop the pin never saw
            let lock = take_lock(vnoc_root.to_path_buf()).await?;
            let len = std::fs::metadata(&measured.path).map(|m| m.len()).ok();
            if len != Some(measured.len) {
                return Err(store_failure(StoreError::Refused(format!(
                    "the tour {id} changed while it was being pinned: run the command again"
                ))));
            }
            let record = pin::make(head.clone(), &placed);
            store::append_locked(&lock, vnoc_root, id, &[Record::Pin(record)])
                .map_err(store_failure)?;
            drop(lock);
            let at = short_commit(&head);
            let mut lines = committed
                .as_ref()
                .map(CommittedOut::lines)
                .unwrap_or_default();
            if matches!(ask, Ask::Pin { .. }) {
                lines.push(format!("pinned {id} at {at}"));
            }
            Ok(Pinned {
                out: PinOut {
                    state: "pinned",
                    commit: Some(head.as_str().to_string()),
                    committed,
                    reasons: Vec::new(),
                    command: None,
                    node_files: Vec::new(),
                    refused: false,
                },
                lines,
                at: Some(at),
            })
        }
    }
}

/// A real check of the tour against the code as it is now, and where its stops and blocks are (what a pin records).
fn placed_check(opened: &Opened, tour: &Tour, len: u64) -> (Recorded, check::Placed) {
    let index = opened.index.read().expect("the index lock is not poisoned");
    let tree = index.tree(&opened.project.name(), opened.project.prefix());
    let code = Code {
        index: &index,
        project: &opened.project,
        tree: &tree,
    };
    let mut read = |path: &Path| source::read_text(path);
    check::check_placed(&code, &mut read, tour, len)
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

fn count_files(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
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
        let holds = tour
            .pin
            .as_ref()
            .filter(|_| tour.pinned())
            .map(|pin| (pin.commit.clone(), pin::reachable(&project, &pin.commit)));
        // pinned, and the commit is behind HEAD: nothing to do
        if let Some((commit, true)) = holds {
            if named {
                tours.push(PinnedTour {
                    tour: row.id.to_string(),
                    already: true,
                    lines: vec![format!(
                        "{} is pinned at {} already",
                        row.id,
                        short_commit(&commit)
                    )],
                    out: PinOut {
                        state: "pinned",
                        commit: Some(commit.as_str().to_string()),
                        committed: None,
                        reasons: Vec::new(),
                        command: None,
                        node_files: Vec::new(),
                        refused: false,
                    },
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
    let (_project, w) = writing(global, "tour edit", &args.tour).await?;
    let meta = MetaRecord {
        title: title.map(String::from),
        question: question.map(String::from),
        detail: args.detail.map(detail_of),
    };
    store::append_locked(&w.lock, &w.vnoc_root, &w.id, &[Record::Meta(meta)])
        .map_err(store_failure)?;
    out::render(
        &Edited {
            schema: 1,
            command: "tour edit",
            tour: w.id.to_string(),
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

/// What `ids` says went with a stop.
#[derive(Serialize)]
struct IdStop {
    stop: String,
    /// `written`, `had`, `by_name` or `gone`
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    /// the file the ID line is in, or the `.vnoc-id` file of the folder
    #[serde(skip_serializing_if = "Option::is_none")]
    changed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    said: Option<String>,
}

#[derive(Serialize)]
struct IdsWritten {
    schema: u32,
    command: &'static str,
    tour: String,
    stops: Vec<IdStop>,
    /// every file written or made, once, in the order they were
    changed: Vec<String>,
    /// an `ids` record was written
    recorded: bool,
    /// the ID lines are in the code and the tour was told where they put its lines: the tours that follow the files' changes need no check
    #[serde(skip)]
    words: Vec<String>,
}

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

/// The project writes no ID lines: `[ids] default = "none"` in `.vnoc/vnoc.toml`. `Err` for a file that cannot be read.
fn project_writes_ids(project: &crate::project::Project) -> Result<bool, Failure> {
    Ok(crate::userdata::UserData::new(project.git_root())
        .settings()
        .map_err(super::write::settings_failure)?
        .markers)
}

async fn ids(global: &Global, args: &TourIdsArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let project = &opened.project;
    let vnoc_root = store::vnoc_root(project.git_root());
    let id = resolve(global, "tour ids", &vnoc_root, &args.tour)?;
    if let Located::Both { .. } = store::locate(&vnoc_root, &id) {
        return Err(store_failure(StoreError::InBoth(id)));
    }
    if !project_writes_ids(project)? {
        return Err(Failure::Usage(
            "this project writes no ID lines ([ids] default = \"none\" in .vnoc/vnoc.toml)".into(),
        ));
    }
    let written = write_ids(global, &opened, &vnoc_root, &id, &args.stops).await?;
    out::render(&written, global)
}

/// The stops of the tour with what there is to do for each, from the index as it is now. No lock is held: it is for counting.
fn picks_of(
    opened: &Opened,
    tour: &Tour,
    named: &[crate::tours::StopId],
) -> Vec<(crate::tours::StopId, crate::tours::ids::Pick)> {
    let index = opened.index.read().expect("the index lock is not poisoned");
    let tree = index.tree(&opened.project.name(), opened.project.prefix());
    let code = Code {
        index: &index,
        project: &opened.project,
        tree: &tree,
    };
    crate::tours::ids::pick(&code, tour, named)
}

/// Gives the stops' nodes an ID: the project's write lock, `ensure_node` for each node that has none (each marker's edit tells the tours where
/// their lines went), then the `ids` record under the tours lock. Neither lock is held by the caller.
async fn write_ids(
    global: &Global,
    opened: &Opened,
    vnoc_root: &Path,
    id: &TourId,
    named: &[crate::tours::StopId],
) -> Result<IdsWritten, Failure> {
    use crate::tours::ids::{self, Outcome};
    let project = &opened.project;
    // the project's write lock first, then (inside each marker's edit, and at the end) the tours lock: never the other way round
    let _one_writer = take_write_lock(project).await?;
    let tour = read_tour(vnoc_root, id)?;
    for wanted in named {
        if !tour.stop(*wanted).is_some_and(|s| s.written) {
            let near = did_you_mean_stops(&tour, *wanted);
            no_such(
                global,
                "tour ids",
                "no_stop",
                format!("no written stop {wanted} in this tour"),
                near.iter().map(ToString::to_string).collect(),
            )?;
            unreachable!("a refusal is a failure");
        }
    }
    let picks = picks_of(opened, &tour, named);
    let mut env = crate::identity::Env {
        project,
        index: &opened.index,
        host: &opened.host,
        edits: crate::identity::EditSink::Disk,
    };
    let done = ids::write(&mut env, &picks)
        .await
        .map_err(|e| attach_failure(e, project))?;

    // the tour as the edits left it (each marker's edit wrote a `rebase`), then the `ids` record, under the tours lock
    let lock = take_lock(vnoc_root.to_path_buf()).await?;
    let tour = read_tour(vnoc_root, id)?;
    let record = ids::record(&tour, &done);
    let recorded = record.is_some();
    if let Some(record) = record {
        store::append_locked(&lock, vnoc_root, id, &[Record::Ids(record)])
            .map_err(store_failure)?;
    }
    drop(lock);

    let mut stops = Vec::new();
    let mut words = Vec::new();
    let mut changed: Vec<String> = Vec::new();
    for ((stop, outcome), (_, pick)) in done.iter().zip(&picks) {
        let (state, node, file, said) = match outcome {
            Outcome::Had(node) => ("had", Some(node.to_string()), None, String::new()),
            Outcome::Written {
                id: node,
                path,
                kind,
                marker,
            } => {
                let said = match marker {
                    crate::identity::MarkerWritten::Line(line) => marker_words(*kind, *line),
                    crate::identity::MarkerWritten::FolderFile => {
                        "a .vnoc-id file was made".to_string()
                    }
                };
                ("written", Some(node.to_string()), Some(path.clone()), said)
            }
            Outcome::ByName { id: node, notice } => (
                "by_name",
                Some(node.to_string()),
                None,
                notice.clone().unwrap_or_else(|| {
                    "the ID is known but is not written in the code".to_string()
                }),
            ),
            Outcome::Gone => (
                "gone",
                None,
                None,
                "its node is not found in the code now: skipped".to_string(),
            ),
        };
        let what = match pick {
            ids::Pick::Needs { path, .. } => path.clone(),
            _ => String::new(),
        };
        if let Some(file) = &file
            && !changed.contains(file)
        {
            changed.push(file.clone());
        }
        words.push(match (state, what.is_empty()) {
            ("written", false) => format!("{what}   {said}"),
            ("had", _) => String::new(),
            _ => said.clone(),
        });
        stops.push(IdStop {
            stop: stop.to_string(),
            state,
            id: node,
            changed: file,
            said: (!said.is_empty()).then_some(said),
        });
    }
    Ok(IdsWritten {
        schema: 1,
        command: "tour ids",
        tour: id.to_string(),
        stops,
        changed,
        recorded,
        words,
    })
}

// ------------------------------------------------------------------------------------------------------------------ settings

#[derive(Serialize)]
struct SettingShown {
    value: Choice,
    /// `file` (`[tours]` in `.vnoc/vnoc.toml` says it) or `default`
    from: Origin,
}

#[derive(Serialize)]
struct SettingsShown {
    schema: u32,
    command: &'static str,
    ids: SettingShown,
    commit: SettingShown,
    /// the keys this call changed
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changed: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    problems: Vec<String>,
    /// `ids = "always"` in a project that writes no ID lines: it does nothing there
    #[serde(skip_serializing_if = "Option::is_none")]
    does_nothing: Option<String>,
}

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

fn settings_failure(e: settings::SettingsError) -> Failure {
    match e {
        settings::SettingsError::NoFile => Failure::Usage(e.to_string()),
        _ => Failure::Environment(e.to_string()),
    }
}

async fn settings(global: &Global, args: &TourSettingsArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut changed = Vec::new();
    if args.ids.is_some() || args.commit.is_some() {
        // the project's file: under the project's write lock, as everything else that writes `.vnoc/`
        let _one_writer = take_write_lock(&project).await?;
        let choice = |arg: super::args::ChoiceArg| match arg {
            super::args::ChoiceArg::Always => Choice::Always,
            super::args::ChoiceArg::Ask => Choice::Ask,
            super::args::ChoiceArg::Never => Choice::Never,
        };
        for (key, arg) in [(Key::Ids, args.ids), (Key::Commit, args.commit)] {
            if let Some(arg) = arg {
                settings::set(&vnoc_root, key, choice(arg)).map_err(settings_failure)?;
                changed.push(key.as_str());
            }
        }
    }
    let read = settings::read(&vnoc_root).map_err(settings_failure)?;
    let shown = |s: settings::Setting| SettingShown {
        value: s.value,
        from: s.from,
    };
    out::render(
        &SettingsShown {
            schema: 1,
            command: "tour settings",
            ids: shown(read.ids),
            commit: shown(read.commit),
            changed,
            problems: read.problems,
            does_nothing: settings::does_nothing_here(
                read.ids.value,
                project_writes_ids(&project)?,
            ),
        },
        global,
    )
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

/// A tour to find the freshness of: what the check is made true of is the file as it was **before** it was read.
struct Subject<'a> {
    id: &'a TourId,
    path: &'a Path,
    len: u64,
    tour: &'a Tour,
}

fn subject(row: &Row) -> Option<Subject<'_>> {
    row.tour.as_ref().ok().map(|tour| Subject {
        id: &row.id,
        path: &row.path,
        len: row.len,
        tour,
    })
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
    let mut done = BTreeMap::new();
    let mut need = Vec::new();
    for s in subjects {
        if done.contains_key(s.id) {
            continue;
        }
        if !mode.again
            && let Some(recorded) = check::recall(vnoc_root, project.git_root(), s.id, s.path)
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
            check::record(vnoc_root, s.id, &recorded).map_err(store_failure)?;
            done.insert(
                s.id.clone(),
                Answer {
                    from: Source::Check,
                    recorded,
                },
            );
        }
    }
    if need.is_empty() {
        return Ok(done);
    }
    let opened = lazy.get(global).await?;
    let index = opened.index.read().expect("the index lock is not poisoned");
    let tree = index.tree(&opened.project.name(), opened.project.prefix());
    let code = Code {
        index: &index,
        project: &opened.project,
        tree: &tree,
    };
    for s in need {
        let mut read = |path: &Path| source::read_text(path);
        let recorded = check::check(&code, &mut read, s.tour, s.len);
        check::record(vnoc_root, s.id, &recorded).map_err(store_failure)?;
        done.insert(
            s.id.clone(),
            Answer {
                from: Source::Check,
                recorded,
            },
        );
    }
    Ok(done)
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
            let error = NodeError::new(
                "root",
                "the project's root folder is every tour's: name a folder in it",
            );
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
    let mut rows = view::list_rows(&listed.tours, &known, args.check, &|commit| {
        pin::has_commit(&project, commit)
    });

    if let Some((_, query)) = &on_node {
        // by what the tours wrote: no check, no code read
        let tours: Vec<&Tour> = listed
            .tours
            .iter()
            .filter_map(|r| r.tour.as_ref().ok())
            .collect();
        let at_row: Vec<usize> = listed
            .tours
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tour.is_ok())
            .map(|(i, _)| i)
            .collect();
        let found: BTreeMap<usize, on::On> = Map::of(&tours)
            .on(query)
            .into_iter()
            .map(|on| (at_row[on.at], on))
            .collect();
        let kept: Vec<view::ListRowView> = rows
            .into_iter()
            .enumerate()
            .filter_map(|(i, mut row)| {
                row.on = Some(view::OnView::of(found.get(&i)?));
                Some(row)
            })
            .collect();
        rows = kept;
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

/// Puts the code of the pinned commit on each stop that a check found changed or gone (`show --as-written`, 03 §5): the stop's lines as the pin's
/// `where` has them, and the lines its blocks light there, exactly (a stop with no blocks lights all of it). A stop the commit cannot give (it is not
/// in this repository, or has no such file) is shown as one that is not pinned is.
fn written_code(
    project: &crate::project::Project,
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
            let mut tour_view = view::tour_view(tour, read.place);
            let holds = tour.pin.as_ref().filter(|_| tour.pinned());
            if let Some(pin) = holds {
                tour_view.pin_in_repository = Some(pin::has_commit(&project, &pin.commit));
            }
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
