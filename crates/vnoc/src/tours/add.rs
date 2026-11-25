//! One `vnoc tour add`: the text, the node it is about and the tour so far in; every record the call writes out, or every refusal of it
//! (`docs/26-tours/02-commands.md` §4, `03-core.md` §7). Nothing is written here: [`plan_stop`] reads the node's file and nothing else, and the
//! command appends what it returns under the tours lock.
//!
//! The order of the refusals is the order the agent can fix them in: the shape of the text (`body::read`), then the text against the node
//! (`rules::check`), then what only the tour and the index know: `--from`, `--line`, `--same-as`, the 40 stops, the ids.

use vnoc_domain::{LocationKey, NodeId, NodeKind};

use super::body::{self, Body, BodyBlock};
use super::doc::{Flat, FlatBlock, FlatStop};
use super::find::{find, place_of};
use super::fold::{Tour, TourBlock, TourStop, steps_in_order};
use super::hash;
use super::record::{
    Lines, Next, PlanRecord, PlanStop, Record, StepId, StepRecord, StopId, StopRecord, TourRecord,
};
use super::rules::{self, NodeLines, Refusal};
use super::store;
use crate::index::{Index, Kind, Node};
use crate::project::Project;
use crate::source;

/// The most stops one tour holds (01 §10): the design's limit.
pub const MAX_STOPS: usize = 40;

/// What the call said besides the text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AddOptions {
    /// `--from`: the stop this one was reached from
    pub from: Option<StopId>,
    /// `--line`: the line of that stop's node where the call is
    pub line: Option<u32>,
    /// `--same-as`: the stop that already explained this node
    pub same_as: Option<StopId>,
    /// `--replace`: the stop this one takes the place of (its id, its place on the route)
    pub replace: Option<StopId>,
}

fn refusal(place: impl Into<String>, problem: impl Into<String>) -> Refusal {
    Refusal {
        place: place.into(),
        problem: problem.into(),
    }
}

/// What the refusals call a node: a def's qualname, a file's path, a folder's path with a `/`.
pub fn name_of(node: &Node) -> String {
    match (&node.qualname, node.kind) {
        (Some(qualname), _) => qualname.clone(),
        (None, Kind::Folder) => format!("{}/", node.path.trim_end_matches('/')),
        (None, _) => node.path.clone(),
    }
}

/// The same for a stop of the tour.
fn stop_name(stop: &TourStop) -> String {
    if !stop.qualname.is_empty() {
        stop.qualname.clone()
    } else if stop.kind == Some(NodeKind::Folder) {
        format!("{}/", stop.path.as_str().trim_end_matches('/'))
    } else {
        stop.path.to_string()
    }
}

/// The stops of the tour nearest in number to `wanted`, as a "did you mean": at most three.
pub fn did_you_mean_stops(tour: &Tour, wanted: StopId) -> Vec<StopId> {
    let mut ids: Vec<StopId> = tour.stops.iter().map(|s| s.id).collect();
    ids.sort_by_key(|id| (id.number().abs_diff(wanted.number()), id.number()));
    ids.truncate(3);
    ids
}

/// "`--from s9`: this tour has no stop s9 · did you mean: s3 · s2", or says the tour has no stops yet.
fn no_such_stop(tour: &Tour, flag: &str, wanted: StopId) -> Refusal {
    let near = did_you_mean_stops(tour, wanted);
    let problem = if near.is_empty() {
        format!("{flag} {wanted}: this tour has no stops yet")
    } else {
        let near: Vec<String> = near.iter().map(ToString::to_string).collect();
        format!(
            "{flag} {wanted}: this tour has no stop {wanted} · did you mean: {}",
            near.join(" · ")
        )
    };
    refusal(flag, problem)
}

/// A node's lines now, 1-based as `vnoc code` prints them: a def's from its range, a file's `1` to its last line (a file with no line counts as
/// one, so that the lines are never empty), `None` for a folder. The file is read for its length.
fn lines_now(project: &Project, node: &Node) -> Result<Option<(u32, u32)>, String> {
    match (node.kind, node.range) {
        (Kind::Folder, _) => Ok(None),
        (_, Some(range)) => Ok(Some((range.start.line + 1, range.end.line + 1))),
        (_, None) => {
            let text = read_file(project, node)?;
            let total = source::split_lines(&text).len() as u32;
            Ok(Some((1, total.max(1))))
        }
    }
}

fn read_file(project: &Project, node: &Node) -> Result<String, String> {
    source::read_text(&project.open_dir().join(&node.path))
        .map_err(|e| format!("{} could not be read: {e}", node.path))
}

pub(crate) fn node_kind(kind: Kind) -> NodeKind {
    match kind {
        Kind::Folder => NodeKind::Folder,
        Kind::File => NodeKind::File,
        Kind::Class => NodeKind::Class,
        Kind::Function => NodeKind::Function,
    }
}

/// The ID texts a line of this node's code may be a marker by (01 §7): the `ID:` text of the node itself and of every def inside it, as the driver
/// reported them (`Def.id`), and for a file its `FileID`. The tree is the nesting: the defs inside a def's lines are its descendants.
pub(super) fn marker_ids(index: &Index, node: &Node) -> Vec<String> {
    fn walk(index: &Index, node: &Node, ids: &mut Vec<String>) {
        if node.qualname.is_some()
            && let Some((path, qualname, occurrence)) = place_of(node)
            && let Ok(key) = LocationKey::new(path, qualname, occurrence)
            && let Some(id) = index.def_at(&key).and_then(|def| def.id)
        {
            ids.push(id);
        }
        for child in &node.children {
            walk(index, child, ids);
        }
    }
    let mut ids = Vec::new();
    if node.kind == Kind::File
        && let Some((path, _, _)) = place_of(node)
        && let Some(id) = index.file_id_text(&path)
    {
        ids.push(id.to_string());
    }
    walk(index, node, &mut ids);
    ids
}

/// The id of the node a stop or a node has, to tell "the same node": the stop's `node`, and the tree node's `node_id`.
fn node_id_of(node: &Node) -> Option<NodeId> {
    node.node_id
        .as_deref()
        .and_then(|id| NodeId::parse(id).ok())
}

/// Whether `stop` is about `node`: the same node ID, or the same path, qualname and occurrence (01 §6).
fn same_node(stop: &TourStop, node: &Node) -> bool {
    if let (Some(a), Some(b)) = (stop.node, node_id_of(node)) {
        return a == b;
    }
    place_of(node).is_some_and(|(path, qualname, occurrence)| {
        stop.path == path && stop.qualname == qualname && stop.occurrence == occurrence
    })
}

/// Whether `stop` is nested in `outer`: its `from`, or its `from`'s, and so on, is `outer`.
pub fn nested_in(tour: &Tour, stop: StopId, outer: StopId) -> bool {
    let mut at = tour.stop(stop).and_then(|s| s.from);
    // a route has at most 40 stops: a chain longer than that is a cycle of a hand-edited file
    for _ in 0..=tour.stops.len() {
        match at {
            Some(id) if id == outer => return true,
            Some(id) => at = tour.stop(id).and_then(|s| s.from),
            None => return false,
        }
    }
    false
}

/// `--line` against the node of the `--from` stop as it is now.
fn check_line(
    out: &mut Vec<Refusal>,
    index: &Index,
    project: &Project,
    tree: &Node,
    from: &TourStop,
    line: u32,
) {
    let flag = format!("--line {line}");
    let Some(found) = find(index, project, tree, from) else {
        out.push(refusal(
            "--line",
            format!(
                "{flag}: {} is not in the code now, so its lines are not known",
                stop_name(from)
            ),
        ));
        return;
    };
    match lines_now(project, found) {
        Ok(None) => out.push(refusal(
            "--line",
            format!("{flag}: {} is a folder: it has no lines", name_of(found)),
        )),
        Ok(Some((first, last))) if line < first || line > last => out.push(refusal(
            "--line",
            format!("{flag} is outside {} ({first}-{last})", name_of(found)),
        )),
        Ok(Some(_)) => {}
        Err(problem) => out.push(refusal("--line", problem)),
    }
}

/// Whether `node` is inside `outer`, and not `outer` itself: a file or folder in a folder, a def in a file, a def in a class (nested defs too).
fn inside(outer: &Node, node: &Node) -> bool {
    outer.id != node.id && outer.find(&node.id).is_some()
}

/// The sentence of `--from` with no `--line` on a node that is not inside the `--from` stop's node (TR14e): the one thing it says is how to say what
/// the agent meant. `what` is how the stop is named, `node` the new stop's node.
fn not_inside_sentence(what: &str, node: &str) -> String {
    format!(
        "{what} does not contain {node}: --from without --line is for something inside that stop. \
         If it is called there, add --line <the line of the call>; otherwise leave --from out."
    )
}

/// The same for a document, which has `from` and `line` as keys, and a label for a stop (`what` is the label and the node's name).
fn not_inside_document(label: &str, outer: &str, node: &str) -> String {
    format!(
        "{label} ({outer}) does not contain {node}: from without line is for something inside that stop. \
         If it is called there, give a line (the line of the call); otherwise leave from out."
    )
}

/// `--from` with no `--line`: the new stop's `node` must be inside the `--from` stop's node as it is now. A stop that is not found now contains nothing.
fn check_inside(
    out: &mut Vec<Refusal>,
    index: &Index,
    project: &Project,
    tree: &Node,
    from: &TourStop,
    node: &Node,
) {
    if find(index, project, tree, from).is_some_and(|outer| inside(outer, node)) {
        return;
    }
    out.push(refusal(
        "--from",
        not_inside_sentence(
            &format!("{} ({})", from.id, stop_name(from)),
            &name_of(node),
        ),
    ));
}

/// The records of one `add`, or every refusal of the call together (D9).
///
/// `node` is a node of `tree` (the index's tree: `index.tree(..)`), as `Lookup::resolve` found it. `tour` is the tour as it was **just now**: the
/// ids come from it, so the caller reads it under the lock and writes under the same lock (01 §5).
pub fn plan_stop(
    tour: &Tour,
    index: &Index,
    project: &Project,
    tree: &Node,
    node: &Node,
    body_text: &str,
    options: AddOptions,
) -> Result<Vec<Record>, Vec<Refusal>> {
    let Some((path, qualname, occurrence)) = place_of(node) else {
        return Err(vec![refusal(
            "node",
            "the project's root folder is not a stop: name a folder in it",
        )]);
    };
    // the text, as far as it can be read, and what is wrong with its shape
    let (body, mut refusals) = body::read(body_text);

    let kind = node_kind(node.kind);
    let file_text = if node.kind == Kind::Folder {
        None
    } else {
        match read_file(project, node) {
            Ok(text) => Some(text),
            Err(problem) => return Err(vec![refusal("node", problem)]),
        }
    };
    let lines = match lines_now(project, node) {
        Ok(lines) => lines,
        Err(problem) => return Err(vec![refusal("node", problem)]),
    };
    let (first, last) = lines.unwrap_or((1, 1));

    // the text against the node
    refusals.extend(rules::check(
        &body,
        &NodeLines {
            name: name_of(node),
            first,
            last,
            kind: kind.clone(),
        },
        tour.detail,
        rules::Options {
            same_as: options.same_as.is_some(),
        },
    ));

    // what only the tour and the index know
    let replaced = options.replace.and_then(|id| tour.stop(id));
    if let Some(id) = options.replace
        && replaced.is_none()
    {
        refusals.push(no_such_stop(tour, "--replace", id));
    }
    let from = options.from.and_then(|id| tour.stop(id));
    if let Some(id) = options.from
        && from.is_none()
    {
        refusals.push(no_such_stop(tour, "--from", id));
    }
    if let (Some(from), Some(line)) = (from, options.line) {
        check_line(&mut refusals, index, project, tree, from, line);
    }
    if let Some(replace) = options.replace {
        // the stop cannot be reached from itself, or from a stop that is nested in it
        if let Some(id) = options.from
            && (id == replace || nested_in(tour, id, replace))
        {
            refusals.push(refusal(
                "--from",
                format!(
                    "--from {id}: {replace} is being replaced, and {id} is {replace} or inside it"
                ),
            ));
        }
        if options.same_as == Some(replace) {
            refusals.push(refusal(
                "--same-as",
                format!("--same-as {replace}: a stop is not the same as itself"),
            ));
        }
    }
    if let Some(id) = options.same_as {
        match tour.stop(id) {
            None => refusals.push(no_such_stop(tour, "--same-as", id)),
            Some(stop) if !stop.written => refusals.push(refusal(
                "--same-as",
                format!("{id} is planned, not written: --same-as names a stop that is written"),
            )),
            Some(stop) if !same_node(stop, node) => refusals.push(refusal(
                "--same-as",
                format!("{id} is {}, not {}", stop_name(stop), name_of(node)),
            )),
            Some(_) => {}
        }
    }
    // a planned stop on this node, not written: this call fills it (02 §4). Not with `--replace`, which is a stop already written
    let filling = match options.replace {
        None => planned_for(tour, index, project, tree, node),
        Some(_) => None,
    };
    // `--from` with no line is containment. A planned stop filled with the `--from` it was planned with keeps the plan's line: no new claim
    if let (Some(from), None) = (from, options.line)
        && !filling.is_some_and(|plan| plan.from == Some(from.id) && plan.line.is_some())
    {
        check_inside(&mut refusals, index, project, tree, from, node);
    }
    if let (Some(id), Some(filled)) = (options.from, filling)
        && (id == filled.id || nested_in(tour, id, filled.id))
    {
        refusals.push(refusal(
            "--from",
            format!(
                "--from {id}: {} is the planned stop this call fills, and {id} is {} or inside it",
                filled.id, filled.id
            ),
        ));
    }
    // a replacement and a planned stop that is filled are no new stops: they take the id and the place of the one they replace
    if options.replace.is_none() && filling.is_none() && tour.stops.len() >= MAX_STOPS {
        refusals.push(refusal(
            "tour",
            format!(
                "this tour has {} stops, the most one tour holds. Finish it and start a second tour",
                tour.stops.len()
            ),
        ));
    }
    let stop_id = match (options.replace, filling) {
        (Some(id), _) => replaced.map(|_| id),
        (None, Some(filled)) => Some(filled.id),
        (None, None) => store::next_stop_id(tour),
    };
    if stop_id.is_none() && options.replace.is_none() {
        refusals.push(refusal(
            "tour",
            "this tour has no stop ids left: start a second tour",
        ));
    }
    let (Some(stop_id), true) = (stop_id, refusals.is_empty()) else {
        return Err(refusals);
    };

    // the records
    let text = file_text.as_deref().unwrap_or("");
    let markers = marker_ids(index, node);
    let marker_refs: Vec<&str> = markers.iter().map(String::as_str).collect();
    let hash_of =
        |first: u32, last: u32| hash::of(&hash::lines_of(text, first, last), &marker_refs);

    // the call's `--from` and `--line`, else the stop's own
    let (from, line) = match (options.replace, options.from, replaced, filling) {
        (Some(_), None, Some(old), _) => (old.from, old.line),
        // a planned stop: the call's `--from` wins (the fold moves the stop when it is another than the plan's), and so does its `--line`; what the
        // call does not say is the plan's. The plan's line was a line of the plan's `--from`: it does not carry over to another stop
        (None, Some(call), _, Some(plan)) => (
            Some(call),
            options
                .line
                .or_else(|| (plan.from == Some(call)).then_some(plan.line).flatten()),
        ),
        (None, None, _, Some(plan)) => (plan.from, plan.line),
        _ => (options.from, options.line),
    };
    let has_hash = matches!(node.kind, Kind::Function | Kind::Class);
    let mut records = vec![Record::Stop(StopRecord {
        id: stop_id,
        path,
        qualname,
        occurrence: std::num::NonZeroU32::new(occurrence).expect("an occurrence is counted from 1"),
        kind,
        node: node_id_of(node),
        lines: lines.and_then(|(a, b)| Lines::new(a, b)),
        hash: has_hash.then(|| hash_of(first, last)),
        from,
        line,
        same_as: options.same_as,
        text: body.intro.clone(),
    })];

    records.extend(steps_of(
        &body,
        stop_id,
        tour.highest_block(stop_id),
        None,
        &hash_of,
    )?);
    Ok(records)
}

/// The first planned stop on the route, in route order, whose name found now by [`find`] is `node`: what an `add` on that node fills (02 §4). `None`
/// when no planned stop is on it; a planned stop whose name no longer resolves is never the one.
pub fn planned_for<'t>(
    tour: &'t Tour,
    index: &Index,
    project: &Project,
    tree: &Node,
    node: &Node,
) -> Option<&'t TourStop> {
    tour.stops
        .iter()
        .filter(|s| !s.written)
        .find(|s| find(index, project, tree, s).is_some_and(|found| found.id == node.id))
}

/// One `plan` record: `nodes` planned in the order given, with the next ids, each `from` the stop `from` (and at `line` of its node: one node then), placed
/// as `add` places new stops (02 §11). Every refusal of the call together, and nothing written. `from` may be a planned stop: a plan can be a tree. A
/// node already written or planned in this tour may be planned again (a second visit). The 40-stop limit counts planned stops, and a call that would
/// pass it is refused whole.
pub fn plan_route(
    tour: &Tour,
    index: &Index,
    project: &Project,
    tree: &Node,
    nodes: &[&Node],
    from: Option<StopId>,
    line: Option<u32>,
) -> Result<Vec<Record>, Vec<Refusal>> {
    let mut refusals = Vec::new();
    let from_stop = from.and_then(|id| tour.stop(id));
    if let Some(id) = from
        && from_stop.is_none()
    {
        refusals.push(no_such_stop(tour, "--from", id));
    }
    if let (Some(stop), Some(line)) = (from_stop, line) {
        if stop.written {
            check_line(&mut refusals, index, project, tree, stop, line);
        } else {
            refusals.push(refusal(
                "--line",
                format!(
                    "{} is planned, not written: --line needs a stop that is written",
                    stop.id
                ),
            ));
        }
    }
    if let (Some(stop), None) = (from_stop, line) {
        // `--from` with no line: every planned node is inside that stop's node; one refusal per node that is not
        for node in nodes {
            check_inside(&mut refusals, index, project, tree, stop, node);
        }
    }
    let total = tour.stops.len() + nodes.len();
    if total > MAX_STOPS {
        refusals.push(refusal(
            "tour",
            if tour.stops.len() >= MAX_STOPS {
                format!(
                    "this tour has {} stops, the most one tour holds. Finish it and start a second tour",
                    tour.stops.len()
                )
            } else {
                format!(
                    "planning {} more stops would give this tour {total} stops, the most one tour holds is {MAX_STOPS}. Finish it and start a second tour",
                    nodes.len()
                )
            },
        ));
    }
    let planned = planned_stops(nodes, tour.highest_stop(), from, line);
    match (planned, refusals.is_empty()) {
        (Ok(stops), true) => Ok(vec![Record::Plan(PlanRecord { stops })]),
        (Ok(_), false) => Err(refusals),
        (Err(more), _) => {
            refusals.extend(more);
            Err(refusals)
        }
    }
}

/// The `plan` entries for `nodes`: ids from `highest + 1` in order, each with the node's name as `find` will look for it again.
pub fn planned_stops(
    nodes: &[&Node],
    highest: u32,
    from: Option<StopId>,
    line: Option<u32>,
) -> Result<Vec<PlanStop>, Vec<Refusal>> {
    let mut stops = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let Some((path, qualname, occurrence)) = place_of(node) else {
            return Err(vec![refusal(
                "node",
                "the project's root folder is not a stop: name a folder in it",
            )]);
        };
        let id = highest
            .checked_add(i as u32 + 1)
            .and_then(StopId::new)
            .ok_or_else(|| {
                vec![refusal(
                    "tour",
                    "this tour has no stop ids left: start a second tour",
                )]
            })?;
        stops.push(PlanStop {
            id,
            path,
            qualname,
            occurrence: std::num::NonZeroU32::new(occurrence)
                .expect("an occurrence is counted from 1"),
            from,
            line,
            same_as: None,
        });
    }
    Ok(stops)
}

/// One step record for each block of the text, in reading order (a block, then its details): ids from the first block number the stop has not
/// used, `parent` for the details (and for the top blocks too when `under` is given).
fn steps_of(
    body: &body::Body,
    stop_id: StopId,
    highest_used: u32,
    under: Option<StepId>,
    hash_of: &dyn Fn(u32, u32) -> hash::Hash,
) -> Result<Vec<Record>, Vec<Refusal>> {
    let mut used = highest_used;
    let mut next_id = || -> Result<StepId, Vec<Refusal>> {
        used = used.checked_add(1).ok_or_else(|| {
            vec![refusal(
                "tour",
                "this stop has no block ids left: start a second tour",
            )]
        })?;
        StepId::block(stop_id, used).ok_or_else(|| vec![refusal("tour", "no block id")])
    };
    let step = |block: &BodyBlock, id: StepId, parent: Option<StepId>| {
        let lines = Lines::new(block.lines.0, block.lines.1).ok_or_else(|| {
            vec![refusal(
                format!("block {}", block.number),
                format!(
                    "block {}: lines {}-{} are not lines",
                    block.number, block.lines.0, block.lines.1
                ),
            )]
        })?;
        Ok::<Record, Vec<Refusal>>(Record::Step(StepRecord {
            id,
            stop: stop_id,
            parent,
            lines,
            hash: hash_of(block.lines.0, block.lines.1),
            title: block.title.clone(),
            text: block.text.clone(),
        }))
    };
    let mut records = Vec::new();
    for top in &body.blocks {
        let id = next_id()?;
        records.push(step(top, id, under)?);
        for detail in &top.details {
            let detail_id = next_id()?;
            records.push(step(detail, detail_id, Some(id))?);
        }
    }
    Ok(records)
}

/// A block of a stop by its id, and how deep it is (0 at the top of the stop).
pub fn block_of(stop: &TourStop, id: StepId) -> Option<(&TourBlock, usize)> {
    fn walk(blocks: &[TourBlock], id: StepId, depth: usize) -> Option<(&TourBlock, usize)> {
        blocks.iter().find_map(|b| {
            if b.id == id {
                Some((b, depth))
            } else {
                walk(&b.details, id, depth + 1)
            }
        })
    }
    walk(&stop.blocks, id, 0)
}

/// The blocks to add to a stop that is written (`add --to`), or every refusal of the call: the text is blocks only (`##`, and `###` details of them);
/// with `under` the `##` blocks are details of that block, which must be at the top of the stop. Checked against the stop's node now and against
/// the blocks the stop has (as they are found now): inside the node, inside `under`, not over a sibling, and at most 12 in all, details counted.
/// No disk write; the records are the new steps only.
pub fn plan_more(
    tour: &Tour,
    index: &Index,
    project: &Project,
    tree: &Node,
    stop_id: StopId,
    under: Option<StepId>,
    body_text: &str,
) -> Result<Vec<Record>, Vec<Refusal>> {
    let Some(stop) = tour.stop(stop_id) else {
        return Err(vec![no_such_stop(tour, "--to", stop_id)]);
    };
    if !stop.written {
        return Err(vec![refusal(
            "--to",
            format!(
                "{stop_id} is planned, not written: --to adds blocks to a stop that is written"
            ),
        )]);
    }
    let (body, mut refusals) = body::read_as(
        body_text,
        if under.is_some() {
            body::Text::Details
        } else {
            body::Text::Blocks
        },
    );
    let Some(node) = find(index, project, tree, stop) else {
        return Err(vec![refusal(
            "--to",
            format!(
                "{stop_id}'s node, {}, is not in the code now: replace the stop (add --replace {stop_id}) instead",
                stop_name(stop)
            ),
        )]);
    };
    let text = if node.kind == Kind::Folder {
        String::new()
    } else {
        read_file(project, node).map_err(|p| vec![refusal("node", p)])?
    };
    let (first, last) = lines_now(project, node)
        .map_err(|p| vec![refusal("node", p)])?
        .unwrap_or((1, 1));
    refusals.extend(rules::check(
        &body,
        &NodeLines {
            name: name_of(node),
            first,
            last,
            kind: node_kind(node.kind),
        },
        tour.detail,
        rules::Options {
            same_as: stop.same_as.is_some(),
        },
    ));

    let markers = marker_ids(index, node);
    let marker_refs: Vec<&str> = markers.iter().map(String::as_str).collect();
    // where the stop's blocks are now
    let now: std::collections::BTreeMap<StepId, (u32, u32)> =
        crate::tours::check::compare(stop, node, &text, &marker_refs)
            .blocks
            .iter()
            .map(|b| (b.id, b.now))
            .collect();

    // --under: a block of this stop, at its top
    let mut region: Option<(StepId, (u32, u32))> = None;
    if let Some(id) = under {
        match block_of(stop, id) {
            _ if id.stop() != stop_id || id.is_intro() => refusals.push(refusal(
                "--under",
                format!("--under {id} is not a block of {stop_id}"),
            )),
            None => {
                let have: Vec<String> = stop
                    .blocks
                    .iter()
                    .map(|b| b.id.to_string())
                    .collect();
                refusals.push(refusal(
                    "--under",
                    format!(
                        "--under {id}: {stop_id} has no such block{}",
                        if have.is_empty() {
                            " (it has no blocks)".to_string()
                        } else {
                            format!(" · its blocks: {}", have.join(", "))
                        }
                    ),
                ));
            }
            Some((_, depth)) if depth > 0 => refusals.push(refusal(
                "--under",
                format!("--under {id} is a detail: details nest one level, so it takes none. Add to its block instead"),
            )),
            Some(_) => region = now.get(&id).map(|r| (id, *r)),
        }
    }

    // the siblings of the new blocks: the details of `under`, or the top of the stop
    let sibling_ids: Vec<StepId> = match under {
        Some(id) => block_of(stop, id)
            .map(|(block, _)| block.details.iter().map(|d| d.id).collect())
            .unwrap_or_default(),
        None => stop.blocks.iter().map(|b| b.id).collect(),
    };
    let siblings: Vec<(StepId, (u32, u32))> = sibling_ids
        .into_iter()
        .filter_map(|id| now.get(&id).map(|r| (id, *r)))
        .collect();
    for block in &body.blocks {
        let (a, b) = block.lines;
        if let Some((id, (from, to))) = region
            && (a < from || b > to)
        {
            refusals.push(refusal(
                format!("block {}", block.number),
                format!(
                    "block {} ({a}-{b}) is not inside {id} ({from}-{to})",
                    block.number
                ),
            ));
        }
        if let Some((id, (from, _))) = siblings
            .iter()
            .find(|(_, (from, to))| a <= *to && *from <= b)
        {
            let line = a.max(*from);
            refusals.push(refusal(
                format!("block {}", block.number),
                format!("block {} and {id} both light line {line}", block.number),
            ));
        }
    }

    // at most 12 blocks in a stop, the ones there counted (rules refuses a text that is over by itself)
    let existing = steps_in_order(stop).len() - 1;
    let new = body
        .blocks
        .iter()
        .map(|b| 1 + b.details.len())
        .sum::<usize>();
    if new <= 12 && existing + new > 12 {
        refusals.push(refusal(
            "tour",
            format!(
                "this stop has {} blocks, the most one stop holds is 12 (details count). Make the rest a second stop",
                existing + new
            ),
        ));
    }
    if !refusals.is_empty() {
        return Err(refusals);
    }
    let hash_of =
        |first: u32, last: u32| hash::of(&hash::lines_of(&text, first, last), &marker_refs);
    steps_of(&body, stop_id, tour.highest_block(stop_id), under, &hash_of)
}

// ------------------------------------------------------------------------------------------------------------------------ a whole tour

/// Where a refusal of `rules::check` or `body` is, in the document: `intro` is the stop's `text`, `block 3` the third block in reading order, with its
/// `title` or `text`; and the sentence says it in the document's words too (`block 3` becomes `stops[0].blocks[1]`).
fn relabel(refusal: Refusal, stop_place: &str, blocks: &[String]) -> Refusal {
    let place = if refusal.place == "intro" {
        format!("{stop_place}.text")
    } else if let Some(rest) = refusal.place.strip_prefix("block ") {
        let (number, tail) = rest.split_once(' ').unwrap_or((rest, ""));
        match number
            .parse::<usize>()
            .ok()
            .and_then(|n| blocks.get(n.checked_sub(1)?))
        {
            Some(base) if tail.is_empty() => base.clone(),
            Some(base) => format!("{base}.{tail}"),
            None => stop_place.to_string(),
        }
    } else {
        stop_place.to_string()
    };
    let said = refusal
        .problem
        .strip_prefix(&format!("{}: ", refusal.place))
        .unwrap_or(&refusal.problem);
    // `block 3` inside the sentence is the block of the document too
    let mut text = String::new();
    let mut rest = said;
    while let Some(at) = rest.find("block ") {
        text.push_str(&rest[..at]);
        let after = &rest[at + 6..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        match digits
            .parse::<usize>()
            .ok()
            .and_then(|n| blocks.get(n.checked_sub(1)?))
        {
            Some(base) => {
                text.push_str(base);
                rest = &after[digits.len()..];
            }
            None => {
                text.push_str("block ");
                rest = after;
            }
        }
    }
    text.push_str(rest);
    Refusal {
        problem: format!("{place}: {text}"),
        place,
    }
}

/// The text of a stop of a document as a [`Body`] (the blocks in reading order: a top block, then its details), with the refusals of its shape. A
/// block is numbered by its place in that order, so `block 3` of a refusal is `blocks[2]` of the list this returns.
fn body_of(stop: &FlatStop, out: &mut Vec<Refusal>) -> (Body, Vec<String>) {
    let mut local = Vec::new();
    let intro = body::paragraph_text(&stop.text);
    if intro.is_empty() {
        local.push(refusal(
            "intro",
            "intro: there is none. Start with a paragraph on what this is for",
        ));
    }
    let mut body = Body {
        intro,
        blocks: Vec::new(),
    };
    // for each flat block, its index in `body.blocks` when it is a top block
    let mut tops: Vec<Option<usize>> = Vec::new();
    let mut places = Vec::new();
    for (i, flat) in stop.blocks.iter().enumerate() {
        let n = i + 1;
        places.push(flat.place.clone());
        if flat.title.trim().is_empty() || flat.title.contains(['\n', '\r']) {
            local.push(refusal(
                format!("block {n} title"),
                format!(
                    "block {n} title: {}",
                    if flat.title.trim().is_empty() {
                        "there is none"
                    } else {
                        "a title is one line"
                    }
                ),
            ));
        }
        let text = body::paragraph_text(&flat.text);
        if text.is_empty() {
            local.push(refusal(
                format!("block {n} text"),
                format!(
                    "block {n} text: there is none. Every block says why those lines are there"
                ),
            ));
        }
        let block = BodyBlock {
            number: n,
            lines: flat.lines,
            title: flat.title.trim().to_string(),
            text,
            details: Vec::new(),
        };
        match flat.parent {
            Some(parent) => {
                match tops.get(parent).copied().flatten() {
                    Some(top) => body.blocks[top].details.push(block),
                    None => body.blocks.push(block),
                }
                tops.push(None);
            }
            None => {
                body.blocks.push(block);
                tops.push(Some(body.blocks.len() - 1));
            }
        }
    }
    out.extend(local.into_iter().map(|r| relabel(r, &stop.place, &places)));
    (body, places)
}

/// A new tour's ids, or a replaced tour's: the number a stop or a block has, kept when `old` has it, else counted up from the highest ever used.
struct Numbers {
    next: u32,
}

impl Numbers {
    fn take(&mut self) -> Option<u32> {
        self.next = self.next.checked_add(1)?;
        Some(self.next)
    }
}

/// Every record of a whole tour from its document, or every refusal of the document, each with its place as written (`stops[0].stops[1].blocks[0].text:
/// 412 of 280 characters`); nothing is half-checked. The stops are in the route's order ([`super::doc::flatten`]).
///
/// With no `old` (a new tour) the ids are made in order, `s1`, `s2`, …, `s1.b1`, …: an `id` of the document is the author's label and counts only where a
/// `same_as`, `from` or `parent` names it. With `old` (`import --replace`) a stop or block whose id the tour has keeps it, the others get numbers above
/// the highest **ever used** in `old` (01 §5), and every hash is taken anew against the code now. The first record is then the new `tour` line: `old`'s
/// id and `at`, and its author unless the document gives one, `next` holding the highest numbers ever used.
///
/// `resolve` is the command's own way of reading a `node` (`file::Name`, `file:line`, an ID, a path): the sentence it gives is the refusal.
pub fn plan_tour<'t>(
    index: &Index,
    project: &Project,
    resolve: &dyn Fn(&str) -> Result<&'t Node, String>,
    flat: &Flat,
    old: Option<&Tour>,
) -> Result<Vec<Record>, Vec<Refusal>> {
    let mut refusals: Vec<Refusal> = Vec::new();
    let detail = flat
        .detail
        .or(old.map(|t| t.detail))
        .unwrap_or(super::record::Detail::Normal);

    // the tour's own words, as `start` checks them
    for problem in store::check_start(&store::StartParams {
        title: flat.title.clone(),
        question: flat.question.clone(),
        author: flat.author.clone(),
        detail,
    }) {
        let key = problem
            .strip_prefix("--")
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or("title")
            .to_string();
        let said = problem
            .strip_prefix(&format!("--{key} "))
            .unwrap_or(&problem)
            .to_string();
        refusals.push(refusal(key.clone(), format!("{key}: {said}")));
    }
    if flat.stops.len() > MAX_STOPS {
        refusals.push(refusal(
            "stops",
            format!(
                "stops: this document has {} stops, the most one tour holds is {MAX_STOPS}. Make it two tours",
                flat.stops.len()
            ),
        ));
    }

    // the ids of the stops
    let kept: Vec<Option<StopId>> = flat
        .stops
        .iter()
        .map(|s| {
            let id = s.id.as_deref().and_then(|l| StopId::parse(l).ok())?;
            old?.stop(id).filter(|o| o.written).map(|_| id)
        })
        .collect();
    let mut numbers = Numbers {
        next: old.map_or(0, Tour::highest_stop).max(
            kept.iter()
                .flatten()
                .map(|id| id.number())
                .max()
                .unwrap_or(0),
        ),
    };
    let mut stop_ids: Vec<StopId> = Vec::new();
    for (stop, keep) in flat.stops.iter().zip(&kept) {
        let id = match keep {
            Some(id) => Some(*id),
            None => numbers.take().and_then(StopId::new),
        };
        let Some(id) = id else {
            refusals.push(refusal(
                &stop.place,
                format!("{}: this tour has no stop ids left", stop.place),
            ));
            continue;
        };
        stop_ids.push(id);
    }

    // each stop against the code
    let mut nodes: Vec<Option<&'t Node>> = Vec::new();
    let mut records: Vec<Record> = Vec::new();
    // the highest block number used in each stop of this tour, new or kept
    let mut blocks_used: std::collections::BTreeMap<StopId, u32> =
        std::collections::BTreeMap::new();
    for (i, stop) in flat.stops.iter().enumerate() {
        let Some(&id) = stop_ids.get(i) else {
            nodes.push(None);
            continue;
        };
        let (body, _) = {
            let mut shape = Vec::new();
            let made = body_of(stop, &mut shape);
            refusals.extend(shape);
            made
        };
        let node = match resolve(&stop.node) {
            Ok(node) => Some(node),
            Err(sentence) => {
                refusals.push(refusal(
                    at_key(&stop.place, "node"),
                    format!("{}.node: {sentence}", stop.place),
                ));
                None
            }
        };
        nodes.push(node);
        let Some(node) = node else { continue };
        let Some((path, qualname, occurrence)) = place_of(node) else {
            refusals.push(refusal(
                at_key(&stop.place, "node"),
                format!(
                    "{}.node: the project's root folder is not a stop: name a folder in it",
                    stop.place
                ),
            ));
            continue;
        };
        let kind = node_kind(node.kind);
        let file_text = if node.kind == Kind::Folder {
            None
        } else {
            match read_file(project, node) {
                Ok(text) => Some(text),
                Err(problem) => {
                    refusals.push(refusal(
                        at_key(&stop.place, "node"),
                        format!("{}.node: {problem}", stop.place),
                    ));
                    continue;
                }
            }
        };
        let lines = match lines_now(project, node) {
            Ok(lines) => lines,
            Err(problem) => {
                refusals.push(refusal(
                    at_key(&stop.place, "node"),
                    format!("{}.node: {problem}", stop.place),
                ));
                continue;
            }
        };
        let (first, last) = lines.unwrap_or((1, 1));
        let places: Vec<String> = stop.blocks.iter().map(|b| b.place.clone()).collect();
        refusals.extend(
            rules::check(
                &body,
                &NodeLines {
                    name: name_of(node),
                    first,
                    last,
                    kind: kind.clone(),
                },
                detail,
                rules::Options {
                    same_as: stop.same_as.is_some(),
                },
            )
            .into_iter()
            .map(|r| relabel(r, &stop.place, &places)),
        );

        // how the stop was reached
        if let (Some(from), Some(line)) = (stop.from, stop.line)
            && let Some(Some(above)) = nodes.get(from)
        {
            match lines_now(project, above) {
                Ok(None) => refusals.push(refusal(
                    at_key(&stop.place, "line"),
                    format!(
                        "{}.line: {} is a folder: it has no lines",
                        stop.place,
                        name_of(above)
                    ),
                )),
                Ok(Some((a, b))) if line < a || line > b => refusals.push(refusal(
                    at_key(&stop.place, "line"),
                    format!(
                        "{}.line: {line} is outside {} ({a}-{b})",
                        stop.place,
                        name_of(above)
                    ),
                )),
                Ok(Some(_)) => {}
                Err(problem) => refusals.push(refusal(
                    at_key(&stop.place, "line"),
                    format!("{}.line: {problem}", stop.place),
                )),
            }
        }
        if let (Some(from), None) = (stop.from, stop.line)
            && let Some(Some(above)) = nodes.get(from)
            && !inside(above, node)
        {
            refusals.push(refusal(
                at_key(&stop.place, "from"),
                format!(
                    "{}.from: {}",
                    stop.place,
                    not_inside_document(
                        flat.stops[from].id.as_deref().unwrap_or("that stop"),
                        &name_of(above),
                        &name_of(node)
                    )
                ),
            ));
        }
        if let Some(same) = stop.same_as
            && let Some(Some(other)) = nodes.get(same)
            && other.id != node.id
        {
            refusals.push(refusal(
                at_key(&stop.place, "same_as"),
                format!(
                    "{}.same_as: {} is {}, not {}",
                    stop.place,
                    flat.stops[same].id.as_deref().unwrap_or("that stop"),
                    name_of(other),
                    name_of(node)
                ),
            ));
        }
        if !refusals.is_empty() {
            continue; // nothing is written: the records are only made once everything checked
        }

        // the records
        let text = file_text.as_deref().unwrap_or("");
        let markers = marker_ids(index, node);
        let marker_refs: Vec<&str> = markers.iter().map(String::as_str).collect();
        let hash_of =
            |first: u32, last: u32| hash::of(&hash::lines_of(text, first, last), &marker_refs);
        let has_hash = matches!(node.kind, Kind::Function | Kind::Class);
        records.push(Record::Stop(StopRecord {
            id,
            path,
            qualname,
            occurrence: std::num::NonZeroU32::new(occurrence)
                .expect("an occurrence is counted from 1"),
            kind,
            node: node_id_of(node),
            lines: lines.and_then(|(a, b)| Lines::new(a, b)),
            hash: has_hash.then(|| hash_of(first, last)),
            from: stop.from.and_then(|f| stop_ids.get(f).copied()),
            line: stop.line,
            same_as: stop.same_as.and_then(|s| stop_ids.get(s).copied()),
            text: body.intro.clone(),
        }));
        match block_records(stop, id, old, &hash_of) {
            Ok((steps, used)) => {
                blocks_used.insert(id, used);
                records.extend(steps);
            }
            Err(more) => refusals.extend(more),
        }
    }
    if !refusals.is_empty() {
        return Err(refusals);
    }

    // a replacement is a whole new file: its first line
    if let Some(old) = old {
        let stop_number = old
            .highest_stop()
            .max(stop_ids.iter().map(|s| s.number()).max().unwrap_or(0));
        let mut blocks = std::collections::BTreeMap::new();
        let all: std::collections::BTreeSet<StopId> = stop_ids
            .iter()
            .copied()
            .chain(old.stops.iter().map(|s| s.id))
            .collect();
        for id in all {
            let used = old
                .highest_block(id)
                .max(blocks_used.get(&id).copied().unwrap_or(0));
            if used > 0 {
                blocks.insert(id, used);
            }
        }
        records.insert(
            0,
            Record::Tour(TourRecord {
                schema: 1,
                id: old.id.clone(),
                title: flat.title.trim().to_string(),
                question: flat.question.as_ref().map(|q| q.trim().to_string()),
                author: flat.author.clone().or_else(|| old.author.clone()),
                at: old.at.clone(),
                detail,
                next: Some(Next {
                    stop: stop_number,
                    blocks,
                }),
            }),
        );
    }
    Ok(records)
}

fn at_key(place: &str, key: &str) -> String {
    format!("{place}.{key}")
}

/// The `step` records of one stop of a document, with the highest block number used under it. A block whose id the old stop has keeps it; the others
/// are counted up from the highest ever used under that stop.
fn block_records(
    stop: &FlatStop,
    id: StopId,
    old: Option<&Tour>,
    hash_of: &dyn Fn(u32, u32) -> hash::Hash,
) -> Result<(Vec<Record>, u32), Vec<Refusal>> {
    let old_stop = old.and_then(|t| t.stop(id)).filter(|s| s.written);
    let kept: Vec<Option<StepId>> = stop
        .blocks
        .iter()
        .map(|b| {
            let step = b.id.as_deref().and_then(|l| StepId::parse(l).ok())?;
            let old_stop = old_stop?;
            (step.stop() == id && !step.is_intro() && block_of(old_stop, step).is_some())
                .then_some(step)
        })
        .collect();
    let mut numbers = Numbers {
        next: old.map_or(0, |t| t.highest_block(id)).max(
            kept.iter()
                .flatten()
                .filter_map(|s| s.block_number())
                .max()
                .unwrap_or(0),
        ),
    };
    let mut ids: Vec<StepId> = Vec::new();
    for (block, keep) in stop.blocks.iter().zip(&kept) {
        let step = match keep {
            Some(step) => Some(*step),
            None => numbers.take().and_then(|n| StepId::block(id, n)),
        };
        let Some(step) = step else {
            return Err(vec![refusal(
                &block.place,
                format!("{}: this stop has no block ids left", block.place),
            )]);
        };
        ids.push(step);
    }
    let mut records = Vec::new();
    for (i, block) in stop.blocks.iter().enumerate() {
        let FlatBlock {
            lines, title, text, ..
        } = block;
        records.push(Record::Step(StepRecord {
            id: ids[i],
            stop: id,
            parent: block.parent.map(|p| ids[p]),
            lines: Lines::new(lines.0, lines.1).ok_or_else(|| {
                vec![refusal(
                    at_key(&block.place, "lines"),
                    format!(
                        "{}.lines: {}-{} are not lines",
                        block.place, lines.0, lines.1
                    ),
                )]
            })?,
            hash: hash_of(lines.0, lines.1),
            title: title.trim().to_string(),
            text: body::paragraph_text(text),
        }));
    }
    let used = ids
        .iter()
        .filter_map(|s| s.block_number())
        .max()
        .unwrap_or(0);
    Ok((records, used.max(numbers.next)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;
    use crate::tours::record::print_line;
    use std::path::PathBuf;
    use vnoc_domain::RepoPath;
    use vnoc_driver_protocol::{Def, DefKind, Position, Range};

    const ALPHA_ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
    const ONE_ID: &str = "3M1X9C7B5N2V8K4J6H0G1F3D5S";

    /// The text of `a.py`: `alpha` (1-9, an ID on line 2), `helper` (11-12), `Box` (14-24) with `one` (15-17, an ID on line 16) and `two` (18-24).
    const A_PY: &str = "def alpha(x):\n    \"\"\"ID: 0Q8V5W4T2F7KX9M3B6D1RZ4H7K\"\"\"\n    a = x + 1\n    b = a * 2\n    c = helper(b)\n    d = c - 1\n    e = d + 1\n    f = e + 1\n    return f\n\ndef helper(y):\n    return y\n\nclass Box:\n    def one(self):\n        \"\"\"ID: 3M1X9C7B5N2V8K4J6H0G1F3D5S\"\"\"\n        return 1\n    def two(self):\n        a = 1\n        b = 2\n        c = 3\n        d = 4\n        e = 5\n        return a\n";

    fn def(
        qualname: &str,
        parent: &str,
        kind: DefKind,
        from: u32,
        to: u32,
        id: Option<&str>,
    ) -> Def {
        Def {
            qualname: qualname.to_string(),
            name: qualname.rsplit('.').next().unwrap().to_string(),
            kind,
            parent: parent.to_string(),
            parent_kind: None,
            range: Range {
                start: Position {
                    line: from,
                    character: 0,
                },
                end: Position {
                    line: to,
                    character: 0,
                },
            },
            params: vec![],
            id: id.map(String::from),
            syntax_error: false,
        }
    }

    /// A project folder with `a.py` and `sub/z.py` on disk, and an index that outlines them.
    struct Fixture {
        project: Project,
        index: Index,
        tree: Node,
        dir: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Fixture {
            let dir = std::env::temp_dir().join(format!("vnoc-add-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            crate::testdir::remove_at_end(&dir);
            std::fs::create_dir_all(dir.join("sub")).unwrap();
            std::fs::write(dir.join("a.py"), A_PY).unwrap();
            std::fs::write(dir.join("sub/z.py"), "def z():\n    return 1\n").unwrap();
            let dir = dunce::canonicalize(dir).unwrap();
            let project = Project::open(&dir).unwrap();
            let index = Index::for_tests(
                vec![
                    (
                        RepoPath::new("a.py").unwrap(),
                        vec![
                            def("alpha", "", DefKind::Function, 0, 8, Some(ALPHA_ID)),
                            def("helper", "", DefKind::Function, 10, 11, None),
                            def("Box", "", DefKind::Class, 13, 23, None),
                            def("Box.one", "Box", DefKind::Function, 14, 16, Some(ONE_ID)),
                            def("Box.two", "Box", DefKind::Function, 17, 23, None),
                            def("Box.two.inner", "Box.two", DefKind::Function, 19, 20, None),
                        ],
                    ),
                    (
                        RepoPath::new("sub/z.py").unwrap(),
                        vec![def("z", "", DefKind::Function, 0, 1, None)],
                    ),
                ],
                vec![],
                vec![RepoPath::new("sub").unwrap()],
            );
            let tree = index.tree(&project.name(), project.prefix());
            Fixture {
                project,
                index,
                tree,
                dir,
            }
        }

        fn node(&self, id: &str) -> &Node {
            self.tree.find(id).unwrap_or_else(|| panic!("no node {id}"))
        }

        fn plan(
            &self,
            tour: &Tour,
            node: &str,
            body: &str,
            options: AddOptions,
        ) -> Result<Vec<Record>, Vec<Refusal>> {
            plan_stop(
                tour,
                &self.index,
                &self.project,
                &self.tree,
                self.node(node),
                body,
                options,
            )
        }

        fn hash_of(&self, path: &str, first: u32, last: u32, markers: &[&str]) -> String {
            let text = std::fs::read_to_string(self.dir.join(path)).unwrap();
            hash::of(&hash::lines_of(&text, first, last), markers).to_string()
        }
    }

    const HEAD: &str =
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;

    fn tour_of(lines: &[String]) -> Tour {
        let mut text = format!("{HEAD}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        fold(&text).unwrap()
    }

    fn empty() -> Tour {
        tour_of(&[])
    }

    /// A stop line for `s<n>` on `a.py::alpha`, with `extra` keys before `text`.
    fn stop_line(n: u32, qualname: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"stop","id":"s{n}","path":"a.py","qualname":"{qualname}","kind":"function","lines":[1,9]{extra},"text":"t"}}"#
        )
    }

    fn texts(refusals: &[Refusal]) -> Vec<&str> {
        refusals.iter().map(|r| r.problem.as_str()).collect()
    }

    fn stop_of(records: &[Record]) -> &StopRecord {
        match &records[0] {
            Record::Stop(stop) => stop,
            other => panic!("not a stop: {other:?}"),
        }
    }

    fn steps_of(records: &[Record]) -> Vec<&StepRecord> {
        records
            .iter()
            .filter_map(|r| match r {
                Record::Step(step) => Some(step),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_function_stop_is_one_stop_record_then_a_step_per_block_in_reading_order() {
        let f = Fixture::new("function");
        let body = "alpha adds, doubles, and calls the helper.\n\n## 3-5 Compute\nIt builds up the value.\n\n### 5 Call the helper\nOne call out.\n\n## 6-9 Finish\nIt winds down.\n";
        let records = f
            .plan(&empty(), "a.py::alpha", body, AddOptions::default())
            .unwrap();
        assert_eq!(records.len(), 4);
        let stop = stop_of(&records);
        assert_eq!(stop.id.to_string(), "s1");
        assert_eq!(stop.path.as_str(), "a.py");
        assert_eq!(stop.qualname, "alpha");
        assert_eq!(stop.occurrence.get(), 1);
        assert_eq!(stop.kind, NodeKind::Function);
        assert_eq!(stop.lines, Lines::new(1, 9));
        assert_eq!(stop.text, "alpha adds, doubles, and calls the helper.");
        assert_eq!(
            stop.node, None,
            "the index has no ID for it: only the code does"
        );
        assert_eq!((stop.from, stop.line, stop.same_as), (None, None, None));
        // the marker line (line 2) is left out of the hash
        assert_eq!(
            stop.hash.unwrap().to_string(),
            f.hash_of("a.py", 1, 9, &[ALPHA_ID])
        );
        let steps = steps_of(&records);
        let shown: Vec<(String, Option<String>, u32, u32, &str)> = steps
            .iter()
            .map(|s| {
                (
                    s.id.to_string(),
                    s.parent.map(|p| p.to_string()),
                    s.lines.first,
                    s.lines.last,
                    s.title.as_str(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("s1.b1".into(), None, 3, 5, "Compute"),
                (
                    "s1.b2".into(),
                    Some("s1.b1".into()),
                    5,
                    5,
                    "Call the helper"
                ),
                ("s1.b3".into(), None, 6, 9, "Finish"),
            ]
        );
        assert_eq!(steps[1].text, "One call out.");
        assert_eq!(steps[0].hash.to_string(), f.hash_of("a.py", 3, 5, &[]));
        assert_eq!(steps[2].hash.to_string(), f.hash_of("a.py", 6, 9, &[]));
        assert!(steps.iter().all(|s| s.stop == stop.id));
    }

    #[test]
    fn the_hash_with_the_id_line_equals_the_hash_of_the_same_code_without_it() {
        let f = Fixture::new("marker");
        let intro = "alpha, in one paragraph.\n";
        let with_marker = f
            .plan(&empty(), "a.py::alpha", intro, AddOptions::default())
            .unwrap();
        // the same function with its ID line taken out of the file and of the index
        let without: String = A_PY
            .lines()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, l)| format!("{l}\n"))
            .collect();
        let want = hash::of(&hash::lines_of(&without, 1, 8), &[]);
        assert_eq!(stop_of(&with_marker).hash, Some(want));
    }

    #[test]
    fn a_class_stop_leaves_out_the_marker_line_of_a_def_inside_it() {
        let f = Fixture::new("class");
        let records = f
            .plan(&empty(), "a.py::Box", "A box.\n", AddOptions::default())
            .unwrap();
        let stop = stop_of(&records);
        assert_eq!(stop.kind, NodeKind::Class);
        assert_eq!(stop.lines, Lines::new(14, 24));
        assert_eq!(
            stop.hash.unwrap().to_string(),
            f.hash_of("a.py", 14, 24, &[ONE_ID]),
            "line 16 holds the ID of Box.one"
        );
        assert_ne!(
            stop.hash.unwrap().to_string(),
            f.hash_of("a.py", 14, 24, &[]),
            "and it would count without the rule"
        );
    }

    #[test]
    fn a_file_has_its_lines_and_no_hash_and_its_blocks_are_hashed() {
        let f = Fixture::new("file");
        let body = "The file.\n\n## 11-12 The helper\nIt helps.\n";
        let records = f
            .plan(&empty(), "a.py", body, AddOptions::default())
            .unwrap();
        let stop = stop_of(&records);
        assert_eq!(stop.kind, NodeKind::File);
        assert_eq!(
            (stop.qualname.as_str(), stop.lines),
            ("", Lines::new(1, 24))
        );
        assert_eq!(stop.hash, None);
        assert_eq!(
            steps_of(&records)[0].hash.to_string(),
            f.hash_of("a.py", 11, 12, &[])
        );
    }

    #[test]
    fn a_folder_has_no_lines_and_no_hash_and_takes_no_blocks() {
        let f = Fixture::new("folder");
        let records = f
            .plan(&empty(), "sub", "A folder.\n", AddOptions::default())
            .unwrap();
        let stop = stop_of(&records);
        assert_eq!(stop.kind, NodeKind::Folder);
        assert_eq!(
            (stop.path.as_str(), stop.lines, stop.hash),
            ("sub", None, None)
        );
        let refused = f
            .plan(
                &empty(),
                "sub",
                "A folder.\n\n## 1-2 A block\nNo.\n",
                AddOptions::default(),
            )
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["sub/ is a folder: it has no lines to light"]
        );
    }

    #[test]
    fn the_second_def_of_a_name_keeps_its_occurrence() {
        let dir = std::env::temp_dir().join(format!("vnoc-add-twice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("t.py"),
            "def f():\n    return 1\n\ndef f():\n    return 2\n",
        )
        .unwrap();
        let dir = dunce::canonicalize(dir).unwrap();
        let project = Project::open(&dir).unwrap();
        let index = Index::for_tests(
            vec![(
                RepoPath::new("t.py").unwrap(),
                vec![
                    def("f", "", DefKind::Function, 0, 1, None),
                    def("f", "", DefKind::Function, 3, 4, None),
                ],
            )],
            vec![],
            vec![],
        );
        let tree = index.tree(&project.name(), project.prefix());
        let node = tree.find("t.py::f#2").unwrap();
        let records = plan_stop(
            &empty(),
            &index,
            &project,
            &tree,
            node,
            "The second f.\n",
            AddOptions::default(),
        )
        .unwrap();
        assert_eq!(stop_of(&records).occurrence.get(), 2);
        assert_eq!(stop_of(&records).lines, Lines::new(4, 5));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_ids_go_on_from_the_highest_ever_used_and_a_new_stop_has_no_block_used() {
        let f = Fixture::new("ids");
        // s7 was dropped, and its blocks with it
        let tour = tour_of(&[
            stop_line(2, "helper", ""),
            r#"{"type":"drop","id":"s7"}"#.to_string(),
        ]);
        let body = "alpha.\n\n## 3-5 A\nx.\n\n## 6-9 B\ny.\n";
        let records = f
            .plan(&tour, "a.py::alpha", body, AddOptions::default())
            .unwrap();
        assert_eq!(stop_of(&records).id.to_string(), "s8");
        let ids: Vec<String> = steps_of(&records)
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(ids, ["s8.b1", "s8.b2"]);
    }

    #[test]
    fn from_line_and_same_as_are_written_on_the_stop() {
        let f = Fixture::new("links");
        let tour = tour_of(&[
            stop_line(1, "alpha", ""),
            stop_line(2, "helper", r#","from":"s1","line":5"#),
        ]);
        let s1 = StopId::parse("s1").unwrap();
        let s2 = StopId::parse("s2").unwrap();
        let records = f
            .plan(
                &tour,
                "a.py::helper",
                "Called again.\n",
                AddOptions {
                    from: Some(s1),
                    line: Some(7),
                    same_as: Some(s2),
                    ..AddOptions::default()
                },
            )
            .unwrap();
        let stop = stop_of(&records);
        assert_eq!(
            (stop.from, stop.line, stop.same_as),
            (Some(s1), Some(7), Some(s2))
        );
        // `--from` alone is containment: no line (the file `a.py`, s3, has `Box` in it)
        let tour = tour_of(&[
            stop_line(1, "alpha", ""),
            stop_line(2, "helper", r#","from":"s1","line":5"#),
            stop_line(3, "", ""),
        ]);
        let s3 = StopId::parse("s3").unwrap();
        let records = f
            .plan(
                &tour,
                "a.py::Box",
                "A class.\n",
                AddOptions {
                    from: Some(s3),
                    ..AddOptions::default()
                },
            )
            .unwrap();
        assert_eq!(
            (stop_of(&records).from, stop_of(&records).line),
            (Some(s3), None)
        );
    }

    /// `--from` with no `--line` is taken only for a node inside the `--from` stop's node: a file or a folder in a folder, a def in a file, a def in a
    /// class, a def in a def (TR14e).
    #[test]
    fn from_with_no_line_is_for_what_is_inside_that_stop() {
        let f = Fixture::new("contain");
        let stop = |n: u32, path: &str, qualname: &str, kind: &str| {
            format!(
                r#"{{"type":"stop","id":"s{n}","path":"{path}","qualname":"{qualname}","kind":"{kind}","text":"t"}}"#
            )
        };
        let tour = tour_of(&[
            stop(1, "sub", "", "folder"),
            stop(2, "a.py", "", "file"),
            stop(3, "a.py", "Box", "class"),
            stop(4, "a.py", "Box.two", "function"),
            stop(5, "a.py", "helper", "function"),
        ]);
        let from = |n: u32| AddOptions {
            from: Some(StopId::new(n).unwrap()),
            ..AddOptions::default()
        };
        // inside: a file in a folder, a def in a file, a def in a class, a nested def in a def, and in a class and a file too
        for (stop, node) in [
            (1, "sub/z.py"),
            (1, "sub/z.py::z"),
            (2, "a.py::helper"),
            (2, "a.py::Box.two.inner"),
            (3, "a.py::Box.one"),
            (3, "a.py::Box.two.inner"),
            (4, "a.py::Box.two.inner"),
        ] {
            let records = f
                .plan(&tour, node, "Intro.\n", from(stop))
                .unwrap_or_else(|r| {
                    panic!("s{stop} contains {node}: {:?}", texts(&r));
                });
            assert_eq!(stop_of(&records).line, None, "{node}");
        }
        // not inside: its own node, a parent, a sibling, a node in another folder or file
        for (stop, node, outer) in [
            (4, "a.py::Box.two", "Box.two"),
            (4, "a.py::Box", "Box.two"),
            (4, "a.py::Box.one", "Box.two"),
            (5, "a.py::Box", "helper"),
            (3, "a.py::helper", "Box"),
            (1, "a.py", "sub/"),
            (2, "sub/z.py", "a.py"),
            (2, "a.py", "a.py"),
        ] {
            let refused = f.plan(&tour, node, "Intro.\n", from(stop)).unwrap_err();
            let name = node.rsplit("::").next().unwrap();
            assert_eq!(
                texts(&refused),
                [format!(
                    "s{stop} ({outer}) does not contain {name}: --from without --line is for something inside that stop. \
                          If it is called there, add --line <the line of the call>; otherwise leave --from out."
                )],
                "s{stop} {node}"
            );
            assert_eq!(refused[0].place, "--from");
        }
        // with a line it is the call that is said, and nothing is asked of containment
        let with_line = AddOptions {
            line: Some(11),
            ..from(5)
        };
        assert!(f.plan(&tour, "a.py::Box", "Intro.\n", with_line).is_ok());
    }

    /// A planned stop filled with another `--from` is a new claim and is checked; with the `--from` it was planned with, the plan's line carries over.
    #[test]
    fn a_planned_stop_filled_with_another_from_is_checked_like_any() {
        let f = Fixture::new("contain-plan");
        let tour = tour_of(&[
            stop_line(1, "alpha", ""),
            stop_line(2, "", ""),
            r#"{"type":"plan","stops":[{"id":"s3","path":"a.py","qualname":"helper","from":"s1","line":5},{"id":"s4","path":"a.py","qualname":"Box","from":"s2"}]}"#
                .to_string(),
        ]);
        let from = |n: u32| AddOptions {
            from: Some(StopId::new(n).unwrap()),
            ..AddOptions::default()
        };
        // planned from s1 at line 5, filled with s1 and no line: the plan's line carries over
        assert_eq!(
            stop_of(&f.plan(&tour, "a.py::helper", "x.\n", from(1)).unwrap()).line,
            Some(5)
        );
        // another `--from` and no line: `helper` is in the file s2
        assert_eq!(
            stop_of(&f.plan(&tour, "a.py::helper", "x.\n", from(2)).unwrap()).line,
            None
        );
        // and a `--from` that does not contain it is refused
        let refused = f.plan(&tour, "a.py::Box", "x.\n", from(1)).unwrap_err();
        assert_eq!(
            texts(&refused),
            [not_inside_sentence("s1 (alpha)", "Box").as_str()]
        );
    }

    /// The refusals only the tour and the index know, each by itself.
    #[test]
    fn each_refusal_of_the_tour_and_the_index_has_its_sentence() {
        let f = Fixture::new("refusals");
        let planned = r#"{"type":"plan","stops":[{"id":"s3","path":"a.py","qualname":"helper"}]}"#;
        let tour = tour_of(&[
            stop_line(1, "alpha", ""),
            stop_line(2, "helper", r#","from":"s1","line":5"#),
            planned.to_string(),
        ]);
        let id = |n: u32| StopId::new(n).unwrap();
        let options = |from: Option<u32>, line: Option<u32>, same_as: Option<u32>| AddOptions {
            from: from.map(id),
            line,
            same_as: same_as.map(id),
            ..AddOptions::default()
        };
        let cases: Vec<(&str, AddOptions, &str, &str)> = vec![
            (
                "a.py::helper",
                options(Some(9), None, None),
                "--from",
                "--from s9: this tour has no stop s9 · did you mean: s3 · s2 · s1",
            ),
            (
                "a.py::helper",
                options(Some(1), Some(99), None),
                "--line",
                "--line 99 is outside alpha (1-9)",
            ),
            (
                "a.py::helper",
                options(Some(1), Some(0), None),
                "--line",
                "--line 0 is outside alpha (1-9)",
            ),
            (
                "a.py::Box",
                options(None, None, Some(1)),
                "--same-as",
                "s1 is alpha, not Box",
            ),
            (
                "a.py::helper",
                options(None, None, Some(3)),
                "--same-as",
                "s3 is planned, not written: --same-as names a stop that is written",
            ),
            (
                "a.py::helper",
                options(None, None, Some(8)),
                "--same-as",
                "--same-as s8: this tour has no stop s8 · did you mean: s3 · s2 · s1",
            ),
        ];
        for (node, options, place, sentence) in cases {
            let refused = f.plan(&tour, node, "Intro.\n", options).unwrap_err();
            assert_eq!(texts(&refused), [sentence], "{node} {options:?}");
            assert_eq!(refused[0].place, place);
        }
    }

    #[test]
    fn a_line_of_a_stop_that_is_a_folder_or_is_gone_is_refused() {
        let f = Fixture::new("line-of");
        let folder =
            r#"{"type":"stop","id":"s1","path":"sub","qualname":"","kind":"folder","text":"t"}"#;
        let gone = r#"{"type":"stop","id":"s2","path":"a.py","qualname":"nowhere","kind":"function","lines":[1,9],"text":"t"}"#;
        let tour = tour_of(&[folder.to_string(), gone.to_string()]);
        let at = |from: u32| AddOptions {
            from: StopId::new(from),
            line: Some(3),
            same_as: None,
            ..AddOptions::default()
        };
        let refused = f
            .plan(&tour, "a.py::helper", "Intro.\n", at(1))
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["--line 3: sub/ is a folder: it has no lines"]
        );
        let refused = f
            .plan(&tour, "a.py::helper", "Intro.\n", at(2))
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["--line 3: nowhere is not in the code now, so its lines are not known"]
        );
    }

    #[test]
    fn a_line_in_a_file_is_inside_its_length_and_the_stop_found_by_name_is_the_one_checked() {
        let f = Fixture::new("line-file");
        let file = r#"{"type":"stop","id":"s1","path":"a.py","qualname":"","kind":"file","lines":[1,24],"text":"t"}"#;
        let tour = tour_of(&[file.to_string()]);
        let at = |line: u32| AddOptions {
            from: StopId::new(1),
            line: Some(line),
            same_as: None,
            ..AddOptions::default()
        };
        assert!(f.plan(&tour, "a.py::helper", "Intro.\n", at(24)).is_ok());
        let refused = f
            .plan(&tour, "a.py::helper", "Intro.\n", at(25))
            .unwrap_err();
        assert_eq!(texts(&refused), ["--line 25 is outside a.py (1-24)"]);
    }

    #[test]
    fn every_refusal_of_one_call_comes_together_with_the_text_first() {
        let f = Fixture::new("together");
        let tour = tour_of(&[stop_line(1, "alpha", "")]);
        // a text with no intro, a block with no text, a block outside the node, and a --from that is not a stop
        let body = "## 40-44 Outside\nx.\n\n## 3-4 Empty\n";
        let refused = f
            .plan(
                &tour,
                "a.py::alpha",
                body,
                AddOptions {
                    from: StopId::new(5),
                    ..AddOptions::default()
                },
            )
            .unwrap_err();
        let said = texts(&refused);
        assert_eq!(said.len(), 4, "{said:?}");
        assert!(said[0].starts_with("intro: there is none"), "{said:?}");
        assert!(
            said[1].starts_with("block 2 text: there is none"),
            "{said:?}"
        );
        assert!(
            said[2].starts_with("block 1: lines 40-44 are outside alpha (1-9)"),
            "{said:?}"
        );
        assert!(said[3].starts_with("--from s5"), "{said:?}");
    }

    #[test]
    fn the_forty_first_stop_is_refused_and_a_full_tour_counts_planned_stops() {
        let f = Fixture::new("forty");
        let lines: Vec<String> = (1..=40).map(|n| stop_line(n, "alpha", "")).collect();
        let tour = tour_of(&lines);
        let refused = f
            .plan(&tour, "a.py::helper", "Intro.\n", AddOptions::default())
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["this tour has 40 stops, the most one tour holds. Finish it and start a second tour"]
        );
        // 39 written and one planned is full too
        let mut lines: Vec<String> = (1..=39).map(|n| stop_line(n, "alpha", "")).collect();
        lines.push(
            r#"{"type":"plan","stops":[{"id":"s40","path":"a.py","qualname":"helper"}]}"#
                .to_string(),
        );
        let full = tour_of(&lines);
        let refused = f
            .plan(&full, "a.py::alpha", "Intro.\n", AddOptions::default())
            .unwrap_err();
        assert_eq!(refused.len(), 1);
        // a call on the planned node fills the planned stop: it takes the id s40 and adds no stop
        let records = f
            .plan(&full, "a.py::helper", "Intro.\n", AddOptions::default())
            .unwrap();
        assert!(matches!(&records[0], Record::Stop(s) if s.id == StopId::new(40).unwrap()));
        // 39 in all still has room
        let lines: Vec<String> = (1..=39).map(|n| stop_line(n, "alpha", "")).collect();
        assert!(
            f.plan(
                &tour_of(&lines),
                "a.py::helper",
                "Intro.\n",
                AddOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn stop_ids_that_ran_out_refuse_and_never_reuse_a_number() {
        let f = Fixture::new("ran-out");
        let tour = tour_of(&[r#"{"type":"drop","id":"s4294967295"}"#.to_string()]);
        let refused = f
            .plan(&tour, "a.py::helper", "Intro.\n", AddOptions::default())
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["this tour has no stop ids left: start a second tour"]
        );
    }

    #[test]
    fn the_root_folder_is_not_a_stop_and_a_file_that_cannot_be_read_is_said() {
        let f = Fixture::new("root");
        let refused = f
            .plan(&empty(), "/", "Intro.\n", AddOptions::default())
            .unwrap_err();
        assert_eq!(
            texts(&refused),
            ["the project's root folder is not a stop: name a folder in it"]
        );
        std::fs::remove_file(f.dir.join("a.py")).unwrap();
        let refused = f
            .plan(&empty(), "a.py::helper", "Intro.\n", AddOptions::default())
            .unwrap_err();
        assert!(
            texts(&refused)[0].starts_with("a.py could not be read"),
            "{refused:?}"
        );
    }

    #[test]
    fn a_text_over_64_kb_is_one_refusal_and_nothing_else() {
        let f = Fixture::new("big");
        let body = "x".repeat(70_000);
        let refused = f
            .plan(&empty(), "a.py::helper", &body, AddOptions::default())
            .unwrap_err();
        assert_eq!(
            texts(&refused)[0],
            "the text is 70000 bytes: the most one stop takes is 65536"
        );
    }

    #[test]
    fn what_plan_stop_wrote_replays_as_the_stop_it_planned() {
        let f = Fixture::new("replay");
        let body = "alpha.\n\n## 3-5 A\nx.\n\n### 4 B\ny.\n";
        let records = f
            .plan(&empty(), "a.py::alpha", body, AddOptions::default())
            .unwrap();
        let mut text = format!("{HEAD}\n");
        for record in &records {
            text.push_str(&print_line(record));
        }
        let tour = fold(&text).unwrap();
        assert!(tour.problems.is_empty(), "{:?}", tour.problems);
        let stop = &tour.stops[0];
        assert_eq!(stop.blocks.len(), 1);
        assert_eq!(stop.blocks[0].details.len(), 1);
    }

    // ---- --replace, --to, --under

    const H: &str = "0000000000000001";

    fn step(stop: u32, n: u32, parent: Option<&str>, a: u32, b: u32) -> String {
        let parent = parent.map_or(String::new(), |p| format!(r#","parent":"{p}""#));
        format!(
            r#"{{"type":"step","id":"s{stop}.b{n}","stop":"s{stop}"{parent},"lines":[{a},{b}],"hash":"{H}","title":"T","text":"t"}}"#
        )
    }

    /// `s1` on `alpha` (1-9) with `s1.b1` 3-5, its detail `s1.b2` 4-4, and `s1.b3` 7-9; `s2` on `helper` reached from `s1` line 5.
    fn tour_with_blocks() -> Tour {
        tour_of(&[
            stop_line(1, "alpha", ""),
            step(1, 1, None, 3, 5),
            step(1, 2, Some("s1.b1"), 4, 4),
            step(1, 3, None, 7, 9),
            stop_line(2, "helper", r#","from":"s1","line":5"#),
        ])
    }

    fn ids(records: &[Record]) -> Vec<String> {
        records
            .iter()
            .map(|r| match r {
                Record::Stop(s) => s.id.to_string(),
                Record::Step(s) => s.id.to_string(),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_replacement_keeps_the_id_takes_new_block_ids_above_the_highest_used_and_keeps_from_and_line()
     {
        let f = Fixture::new("replace");
        let tour = tour_with_blocks();
        let s1 = StopId::parse("s1").unwrap();
        let s2 = StopId::parse("s2").unwrap();
        let replace = |id: StopId, node: &str, body: &str, extra: AddOptions| {
            f.plan(
                &tour,
                node,
                body,
                AddOptions {
                    replace: Some(id),
                    ..extra
                },
            )
        };
        // s1 again, on the same node: its id, blocks from b4 (b1 to b3 were used)
        let records = replace(
            s1,
            "a.py::alpha",
            "New intro.\n\n## 3-4 A\nx.\n\n## 6-9 B\ny.\n",
            AddOptions::default(),
        )
        .unwrap();
        assert_eq!(ids(&records), ["s1", "s1.b4", "s1.b5"]);
        assert_eq!(stop_of(&records).text, "New intro.");
        // a node that is another: the same id, the new node
        let records = replace(s1, "a.py::helper", "Helper now.\n", AddOptions::default()).unwrap();
        let stop = stop_of(&records);
        assert_eq!(
            (stop.id, stop.qualname.as_str(), stop.lines),
            (s1, "helper", Lines::new(11, 12))
        );
        // no `--from`: the stop's own `from` and `line`; one given: the call's, and its line (or none)
        let records = replace(s2, "a.py::helper", "Again.\n", AddOptions::default()).unwrap();
        assert_eq!(
            (stop_of(&records).from, stop_of(&records).line),
            (Some(s1), Some(5))
        );
        let records = replace(
            s2,
            "a.py::helper",
            "Again.\n",
            AddOptions {
                from: Some(s1),
                line: Some(7),
                ..AddOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            (stop_of(&records).from, stop_of(&records).line),
            (Some(s1), Some(7))
        );
        // `--from` and no line: the call's own, and containment: `alpha` does not contain `helper`
        let refused = replace(
            s2,
            "a.py::helper",
            "Again.\n",
            AddOptions {
                from: Some(s1),
                ..AddOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            texts(&refused),
            [not_inside_sentence("s1 (alpha)", "helper").as_str()]
        );
        // a replacement is no new stop: a full tour can be mended
        let lines: Vec<String> = (1..=40).map(|n| stop_line(n, "alpha", "")).collect();
        let full = tour_of(&lines);
        assert!(
            f.plan(
                &full,
                "a.py::helper",
                "x.\n",
                AddOptions {
                    replace: Some(s1),
                    ..AddOptions::default()
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn a_replacement_that_cannot_be_is_refused_with_its_reason() {
        let f = Fixture::new("replace-refused");
        let tour = tour_with_blocks();
        let id = |n| StopId::new(n).unwrap();
        let refused = |options: AddOptions| {
            texts(&f.plan(&tour, "a.py::helper", "x.\n", options).unwrap_err())
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            refused(AddOptions {
                replace: Some(id(9)),
                ..AddOptions::default()
            }),
            ["--replace s9: this tour has no stop s9 · did you mean: s2 · s1"]
        );
        // from itself, or from a stop inside it
        assert_eq!(
            refused(AddOptions {
                replace: Some(id(1)),
                from: Some(id(1)),
                line: Some(1),
                ..AddOptions::default()
            }),
            ["--from s1: s1 is being replaced, and s1 is s1 or inside it"]
        );
        assert_eq!(
            refused(AddOptions {
                replace: Some(id(1)),
                from: Some(id(2)),
                line: Some(11),
                ..AddOptions::default()
            }),
            ["--from s2: s1 is being replaced, and s2 is s1 or inside it"]
        );
        assert_eq!(
            refused(AddOptions {
                replace: Some(id(2)),
                same_as: Some(id(2)),
                ..AddOptions::default()
            }),
            ["--same-as s2: a stop is not the same as itself"]
        );
        assert!(nested_in(&tour, id(2), id(1)));
        assert!(!nested_in(&tour, id(1), id(2)));
        assert!(!nested_in(&tour, id(1), id(1)));
    }

    fn more(
        f: &Fixture,
        tour: &Tour,
        stop: u32,
        under: Option<&str>,
        body: &str,
    ) -> Result<Vec<Record>, Vec<String>> {
        plan_more(
            tour,
            &f.index,
            &f.project,
            &f.tree,
            StopId::new(stop).unwrap(),
            under.map(|u| StepId::parse(u).unwrap()),
            body,
        )
        .map_err(|r| r.into_iter().map(|r| r.problem).collect())
    }

    #[test]
    fn blocks_added_to_a_stop_come_after_its_blocks_with_ids_above_the_highest_used() {
        let f = Fixture::new("more");
        let tour = tour_with_blocks();
        let records = more(
            &f,
            &tour,
            1,
            None,
            "## 1-2 Top\nIt opens.\n\n### 2-2 Inner\nIn it.\n",
        )
        .unwrap();
        assert_eq!(ids(&records), ["s1.b4", "s1.b5"]);
        let steps = steps_of(&records);
        assert_eq!(
            (steps[0].parent, steps[0].lines),
            (None, Lines::new(1, 2).unwrap())
        );
        assert_eq!(steps[1].parent, Some(StepId::parse("s1.b4").unwrap()));
        assert_eq!(
            steps[0].hash.to_string(),
            f.hash_of("a.py", 1, 2, &[ALPHA_ID])
        );
        assert!(
            records.iter().all(|r| matches!(r, Record::Step(_))),
            "steps only: the stop is as it was"
        );
    }

    #[test]
    fn blocks_added_are_checked_against_the_node_and_the_blocks_the_stop_has() {
        let f = Fixture::new("more-refused");
        let tour = tour_with_blocks();
        let cases: Vec<(&str, &str)> = vec![
            ("## 4-6 Over\nx.\n", "block 1 and s1.b1 both light line 4"),
            (
                "## 1-2 A\nx.\n\n## 2-3 B\ny.\n",
                "blocks 1 and 2 both light line 2",
            ),
            (
                "## 8-8 In the last\nx.\n",
                "block 1 and s1.b3 both light line 8",
            ),
            (
                "## 1-30 Too far\nx.\n",
                "block 1: lines 1-30 are outside alpha (1-9)",
            ),
            (
                "An intro.\n\n## 1-2 A\nx.\n",
                "intro: --to adds blocks: leave out the first paragraph, start with a ## heading",
            ),
        ];
        for (body, said) in cases {
            let refused = more(&f, &tour, 1, None, body).unwrap_err();
            assert!(
                refused.iter().any(|r| r.contains(said)),
                "{body}: {refused:?}"
            );
        }
        // a stop that is planned, not written, and one that is not there
        let planned = tour_of(&[
            r#"{"type":"plan","stops":[{"id":"s1","path":"a.py","qualname":"alpha"}]}"#.to_string(),
        ]);
        assert_eq!(
            more(&f, &planned, 1, None, "## 1-2 A\nx.\n").unwrap_err(),
            ["s1 is planned, not written: --to adds blocks to a stop that is written"]
        );
        assert!(
            more(&f, &tour, 7, None, "## 1-2 A\nx.\n").unwrap_err()[0]
                .starts_with("--to s7: this tour has no stop s7")
        );
        // a stop whose node is gone: replace it
        let gone = tour_of(&[stop_line(1, "nowhere", "")]);
        assert!(
            more(&f, &gone, 1, None, "## 1-2 A\nx.\n").unwrap_err()[0]
                .contains("is not in the code now: replace the stop (add --replace s1)")
        );
        // a stop that is one sentence and a short one take no blocks
        let same = tour_of(&[
            stop_line(1, "alpha", ""),
            stop_line(2, "alpha", r#","same_as":"s1""#),
        ]);
        assert!(
            more(&f, &same, 2, None, "## 1-2 A\nx.\n").unwrap_err()[0]
                .contains("a --same-as stop is one sentence")
        );
        let short = tour_of(&[r#"{"type":"stop","id":"s1","path":"a.py","qualname":"helper","kind":"function","lines":[11,12],"text":"t"}"#.to_string()]);
        assert!(
            more(&f, &short, 1, None, "## 11-11 A\nx.\n").unwrap_err()[0]
                .contains("helper is 2 lines")
        );
    }

    #[test]
    fn at_most_twelve_blocks_in_a_stop_with_the_ones_there_counted() {
        let f = Fixture::new("more-twelve");
        let mut lines = vec![stop_line(1, "alpha", "")];
        // twelve blocks there: six blocks and a detail each
        for n in 0..6u32 {
            lines.push(step(1, 1 + 2 * n, None, 3, 9));
            lines.push(step(
                1,
                2 + 2 * n,
                Some(&format!("s1.b{}", 1 + 2 * n)),
                3,
                3,
            ));
        }
        let full = tour_of(&lines);
        let refused = more(&f, &full, 1, None, "## 1-2 A\nx.\n").unwrap_err();
        assert!(
            refused.iter().any(|r| r == "this stop has 13 blocks, the most one stop holds is 12 (details count). Make the rest a second stop"),
            "{refused:?}"
        );
        // eleven there: one more is the twelfth
        let eleven = tour_of(&lines[..12]);
        assert_eq!(
            more(&f, &eleven, 1, None, "## 1-2 A\nx.\n").unwrap().len(),
            1
        );
    }

    #[test]
    fn details_added_under_a_block_are_inside_it_and_beside_its_details_and_never_three_deep() {
        let f = Fixture::new("under");
        let tour = tour_with_blocks();
        // s1.b1 is 3-5 with a detail at 4: lines 3 and 5 are free
        let records = more(
            &f,
            &tour,
            1,
            Some("s1.b1"),
            "## 3-3 First\nx.\n\n## 5-5 Last\ny.\n",
        )
        .unwrap();
        let steps = steps_of(&records);
        assert_eq!(ids(&records), ["s1.b4", "s1.b5"]);
        assert!(
            steps
                .iter()
                .all(|s| s.parent == Some(StepId::parse("s1.b1").unwrap()))
        );
        let cases: Vec<(&str, &str, &str)> = vec![
            (
                "s1.b1",
                "## 4-4 On the detail\nx.\n",
                "block 1 and s1.b2 both light line 4",
            ),
            (
                "s1.b1",
                "## 6-7 Outside it\nx.\n",
                "block 1 (6-7) is not inside s1.b1 (3-5)",
            ),
            (
                "s1.b1",
                "## 3-3 A\nx.\n\n### 3-3 Deeper\ny.\n",
                "details nest one level",
            ),
            (
                "s1.b2",
                "## 4-4 A\nx.\n",
                "--under s1.b2 is a detail: details nest one level",
            ),
            (
                "s1.b9",
                "## 4-4 A\nx.\n",
                "--under s1.b9: s1 has no such block · its blocks: s1.b1, s1.b3",
            ),
            (
                "s2.b1",
                "## 4-4 A\nx.\n",
                "--under s2.b1 is not a block of s1",
            ),
            ("s1", "## 4-4 A\nx.\n", "--under s1 is not a block of s1"),
        ];
        for (under, body, said) in cases {
            let refused = more(&f, &tour, 1, Some(under), body).unwrap_err();
            assert!(
                refused.iter().any(|r| r.contains(said)),
                "{under} {body}: {refused:?}"
            );
        }
    }
}
