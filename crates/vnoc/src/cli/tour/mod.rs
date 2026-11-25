//! `vnoc tour start | add | finish | check | list | show` (`docs/26-tours/02-commands.md` §2 – §6, §12): parse, call `tours/`, print.
//!
//! Every command here is a thin handler: the files, the rules and the text of a tour are `crates/vnoc/src/tours/`. Only `add` always reads the code; `check`, `list --on` and
//! `show --check` start a driver only when a tour's recorded check no longer holds, or a node is to be found. Exit codes as `02` §12: a refusal and a tour, stop or node that is not there are `2`, the lock and the
//! environment `3`.

mod act;
mod read;
mod write;

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::calls::{self, CallsError};
use crate::cli::args::{DetailArg, TourCommand};
use crate::cli::nodes::{Hints, Lookup, NodeError, Target};
use crate::cli::out::{self};
use crate::cli::project::{Opened, open_from};
use crate::cli::write::attach_failure;
use crate::cli::{Failure, Global};
use crate::tours::act::{ActError, IdStop};
use crate::tours::record::{Detail, StopId};
use crate::tours::route;
use crate::tours::rules::Refusal;
use crate::tours::settings::{self};
use crate::tours::store::{self, Pick, Row, StoreError};
use crate::tours::{Tour, TourId, hash, view};
use crate::userdata::ToursLock;

use act::{
    delete, drop_id, edit, export, halt, ids, import, pin_command, plan, resume, settings, share,
};
use read::{check, go_command, list, route, show, where_command};
use write::{add, finish, start};

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

/// `1 step`, `5 steps`.
fn steps_words(n: usize) -> String {
    format!("{n} {}", if n == 1 { "step" } else { "steps" })
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

/// What the IDs step after `end` did: for the answer, as data and as text.
struct IdsAfterEnd {
    ids: Option<IdsStep>,
    text: String,
    notes: Vec<String>,
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

/// The tours a command names (an id, or the start of one), or every tour; one row for each id (the project's copy of a tour that is in both folders).
fn chosen(rows: Vec<Row>, wanted: &[TourId]) -> Vec<Row> {
    let mut seen = std::collections::BTreeSet::new();
    rows.into_iter()
        .filter(|r| (wanted.is_empty() || wanted.contains(&r.id)) && seen.insert(r.id.clone()))
        .collect()
}
