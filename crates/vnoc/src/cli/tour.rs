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
    TourFinishArgs, TourIdsArgs, TourListArgs, TourResumeArgs, TourSettingsArgs, TourShareArgs,
    TourShowArgs, TourStartArgs,
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
use crate::tours::finish::{self as finishing, Step};
use crate::tours::on::{self, Map, Query};
use crate::tours::record::{Bare, Detail, DropRecord, EndRecord, MetaRecord, Record};
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
        TourCommand::List(args) => list(global, args).await,
        TourCommand::Show(args) => show(global, args).await,
        TourCommand::Check(args) => check(global, args).await,
        TourCommand::Drop(args) => drop_id(global, args).await,
        TourCommand::Resume(args) => resume(global, args).await,
        TourCommand::Edit(args) => edit(global, args).await,
        TourCommand::Ids(args) => ids(global, args).await,
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
    tour: String,
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
    if global.json {
        out::json(&Refused {
            schema: 1,
            command,
            tour: tour.to_string(),
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
        out::hints("fix them, then send the whole stop again", &items, 0);
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
    /// the text of the IDs step: the line to ask the user, or what was written
    #[serde(skip)]
    ids_text: String,
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
        let mut text = format!("{head}: {stops}, {}\n", steps_words(self.steps));
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
    let read = settings::read(&vnoc_root).map_err(settings_failure)?;
    let choice = finishing::effective(read.ids.value, args.ids, args.no_ids);
    let by_flag = args.ids || args.no_ids;
    let mut ids = None;
    let mut ids_text = String::new();
    let mut notes = read.problems;
    // a stop whose node had an ID when it was written has nothing to give: no index is built for a tour with no such stop
    let could = tour.stops.iter().any(|s| s.written && s.node.is_none());
    if (!already || by_flag) && choice != Choice::Never && could && project_writes_ids(&project)? {
        // the tour is finished already: no driver to count with is a note, not a failure of `finish`
        let opened = match open_from(global).await {
            Ok(opened) => Some(opened),
            Err(Failure::Environment(why)) => {
                notes.push(format!(
                    "could not tell which stops have no ID, so none were written or offered: {why}"
                ));
                None
            }
            Err(e) => return Err(e),
        };
        let missing = opened
            .as_ref()
            .map_or(0, |o| finishing::missing(&picks_of(o, &tour, &[])));
        match finishing::step(choice, true, missing) {
            Step::Nothing => {}
            Step::Ask { missing } => {
                let line = finishing::ask_line(&tour_id, missing);
                ids_text = format!("{line}\n");
                ids = Some(IdsStep::Ask {
                    missing,
                    command: format!("vnoc tour ids {tour_id}"),
                });
            }
            Step::Write => {
                let opened = opened
                    .as_ref()
                    .expect("a stop is missing an ID only when the index was built");
                let written = write_ids(global, opened, &vnoc_root, &tour_id, &[]).await?;
                ids_text = written.text(false);
                ids = Some(IdsStep::Written {
                    stops: written.stops,
                    changed: written.changed,
                    recorded: written.recorded,
                });
            }
        }
    }
    out::render(
        &Finished {
            schema: 1,
            command: "tour finish",
            tour: tour_id.to_string(),
            already,
            stops: shown.written,
            steps: shown.steps,
            left_the_route: if already { 0 } else { shown.planned },
            ids,
            notes,
            ids_text,
        },
        global,
    )
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
            lines,
            word: view::check_word(changed, gone),
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
    let mut rows = view::list_rows(&listed.tours, &known, args.check);

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
            if args.check {
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
