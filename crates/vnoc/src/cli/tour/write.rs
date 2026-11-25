use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::cli::args::{TourAddArgs, TourFinishArgs, TourStartArgs};
use crate::cli::nodes::{Lookup, Target, doc_path};
use crate::cli::out::{self, Render};
use crate::cli::project::{open_from, open_without_driver};
use crate::cli::{Failure, Global};
use crate::source;
use crate::tours::act::{Ask, PinOut};
use crate::tours::add::{AddOptions, MAX_STOPS, plan_more, plan_stop, planned_stops};
use crate::tours::finish::{self as finishing};
use crate::tours::record::{EndRecord, Record, StopId};
use crate::tours::rules::Refusal;
use crate::tours::settings::{self};
use crate::tours::store::{self, Located, StartParams, StoreError, Where};
use crate::tours::{TourId, steps_in_order, time, view};

use super::act::{ids_after_end, pin_tour, read_tour};
use super::{
    BODY_READ_CAP, IdsStep, Lazy, build_route, detail_of, find_all, first_line, missing_error,
    plan_rows, refused_with, refused_with_hint, resolve, settings_failure, steps_words, stopped,
    stops_words, store_failure, take_lock,
};

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

pub(super) async fn start(global: &Global, args: &TourStartArgs) -> Result<(), Failure> {
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

pub(super) async fn add(global: &Global, args: &TourAddArgs) -> Result<(), Failure> {
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

pub(super) async fn finish(global: &Global, args: &TourFinishArgs) -> Result<(), Failure> {
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
