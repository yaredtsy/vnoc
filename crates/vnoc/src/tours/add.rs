//! One `vnoc tour add`: the text, the node it is about and the tour so far in; every record the call writes out, or every refusal of it
//! (`docs/26-tours/02-commands.md` §4, `03-core.md` §7). Nothing is written here: [`plan_stop`] reads the node's file and nothing else, and the
//! command appends what it returns under the tours lock.
//!
//! The order of the refusals is the order the agent can fix them in: the shape of the text (`body::read`), then the text against the node
//! (`rules::check`), then what only the tour and the index know: `--from`, `--line`, `--same-as`, the 40 stops, the ids.

use vnoc_domain::{LocationKey, NodeId, NodeKind};

use super::body::{self, BodyBlock};
use super::find::{find, place_of};
use super::fold::{Tour, TourStop};
use super::hash;
use super::record::{Lines, Record, StepId, StepRecord, StopId, StopRecord};
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

fn node_kind(kind: Kind) -> NodeKind {
    match kind {
        Kind::Folder => NodeKind::Folder,
        Kind::File => NodeKind::File,
        Kind::Class => NodeKind::Class,
        Kind::Function => NodeKind::Function,
    }
}

/// The ID texts a line of this node's code may be a marker by (01 §7): the `ID:` text of the node itself and of every def inside it, as the driver
/// reported them (`Def.id`), and for a file its `FileID`. The tree is the nesting: the defs inside a def's lines are its descendants.
fn marker_ids(index: &Index, node: &Node) -> Vec<String> {
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
    let from = options.from.and_then(|id| tour.stop(id));
    if let Some(id) = options.from
        && from.is_none()
    {
        refusals.push(no_such_stop(tour, "--from", id));
    }
    if let (Some(from), Some(line)) = (from, options.line) {
        check_line(&mut refusals, index, project, tree, from, line);
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
    if tour.stops.len() >= MAX_STOPS {
        refusals.push(refusal(
            "tour",
            format!(
                "this tour has {} stops, the most one tour holds. Finish it and start a second tour",
                tour.stops.len()
            ),
        ));
    }
    let stop_id = store::next_stop_id(tour);
    if stop_id.is_none() {
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
        from: options.from,
        line: options.line,
        same_as: options.same_as,
        text: body.intro.clone(),
    })];

    // one block id after the other, from the first this stop has not used
    let mut used = tour.highest_block(stop_id);
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
    for top in &body.blocks {
        let id = next_id()?;
        records.push(step(top, id, None)?);
        for detail in &top.details {
            let detail_id = next_id()?;
            records.push(step(detail, detail_id, Some(id))?);
        }
    }
    Ok(records)
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

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
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
                },
            )
            .unwrap();
        let stop = stop_of(&records);
        assert_eq!(
            (stop.from, stop.line, stop.same_as),
            (Some(s1), Some(7), Some(s2))
        );
        // `--from` alone is containment: no line
        let records = f
            .plan(
                &tour,
                "a.py::Box",
                "A class.\n",
                AddOptions {
                    from: Some(s1),
                    ..AddOptions::default()
                },
            )
            .unwrap();
        assert_eq!(
            (stop_of(&records).from, stop_of(&records).line),
            (Some(s1), None)
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
        let refused = f
            .plan(
                &tour_of(&lines),
                "a.py::helper",
                "Intro.\n",
                AddOptions::default(),
            )
            .unwrap_err();
        assert_eq!(refused.len(), 1);
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
}
