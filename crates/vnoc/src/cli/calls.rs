//! `vnoc calls <node> [--depth N]` (03 §7): what a function calls, as `calls::tree_for` merges it (the same function `vnoc/callTree` answers with),
//! printed as an indented tree. Callees only: the core has no callers.

use serde::Serialize;
use serde_json::json;
use vnoc_driver_protocol::CallMark;

use super::args::CallsArgs;
use super::nodes::{Lookup, Target, id_of, kind_word, lines_of};
use super::out::{self, Render};
use super::project::open_from;
use super::show::head_line;
use super::{Failure, Global};
use crate::calls::{self, CallsError};
use crate::index::Kind;
use crate::protocol::{CallTreeNode, CallTreeResult};

pub async fn run(global: &Global, args: &CallsArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let node = match lookup.resolve(&args.node) {
        Ok(Target::Node(node)) => node,
        Ok(Target::Missing(missing)) => {
            return refused(
                global,
                "missing",
                format!(
                    "calls start at a function or a class: {} is Missing (its code is gone)",
                    missing.was
                ),
            );
        }
        Err(error) => return refused(global, error.why, error.message),
    };
    if matches!(node.kind, Kind::File | Kind::Folder) {
        return refused(
            global,
            "not_a_def",
            format!(
                "calls start at a function or a class: {} is a {}",
                node.location(),
                kind_word(node.kind)
            ),
        );
    }
    let result = calls::tree_for(
        &opened.project,
        &opened.index,
        &opened.host,
        &node.id,
        Some(args.depth),
    )
    .await
    .map_err(|e| match e {
        CallsError::Invalid(message) => Failure::NoSuchNode(message),
        CallsError::MissingCapability(message) | CallsError::Internal(message) => {
            Failure::Environment(message)
        }
    })?;
    let answer = Calls {
        schema: 1,
        command: "calls",
        depth: args.depth,
        head: head_line(
            kind_word(node.kind),
            &node.location(),
            lines_of(node).map(|(a, b)| [a, b]),
            id_of(node).map(|id| id.to_string()).as_deref(),
        ),
        rows: rows_of(&result.root.children, &lookup),
        depth_asked: args.depth,
        result,
    };
    out::render(&answer, global)
}

/// An argument that cannot start a call tree: the sentence, and `--json` has it in the value (exit `2`, as `show` does for a node it cannot find).
fn refused(global: &Global, why: &'static str, message: String) -> Result<(), Failure> {
    if global.json {
        out::json(&json!({
            "schema": 1,
            "command": "calls",
            "error": { "why": why, "message": message },
        }))?;
        Err(Failure::Reported)
    } else {
        Err(Failure::NoSuchNode(message))
    }
}

/// What `calls` answers: the protocol's tree with the flags it was asked with, and the rows its text is made of.
#[derive(Serialize)]
struct Calls {
    schema: u32,
    command: &'static str,
    /// the depth the tree was asked for
    depth: u32,
    #[serde(flatten)]
    result: CallTreeResult,
    #[serde(skip)]
    head: String,
    #[serde(skip)]
    rows: Vec<Row>,
    #[serde(skip)]
    depth_asked: u32,
}

/// A row of the text: what to say of one node of the tree, and its children.
struct Row {
    name: String,
    /// the location below the root, or what to say when the code is not in the index
    place: String,
    times: u32,
    marks: Vec<CallMark>,
    /// the location to follow it from, when the tree stopped at it and the index has it
    deeper: Option<String>,
    children: Vec<Row>,
}

/// What a row says for a node of the tree: its name, and where it is. `known`: the location below the root, when the index has the node. A target
/// the index does not know is **outside the project** when the driver gave it no file, and **not in the index** when it did (an ignored file).
fn name_and_place(node: &CallTreeNode, known: Option<String>) -> (String, String) {
    match (known, &node.path) {
        (Some(location), _) => (node.name.clone(), location),
        (None, None) => (node.qualname.clone(), "(outside the project)".to_string()),
        (None, Some(path)) => (
            node.name.clone(),
            format!("{path}::{} (not in the index)", node.qualname),
        ),
    }
}

fn rows_of(nodes: &[CallTreeNode], lookup: &Lookup) -> Vec<Row> {
    nodes
        .iter()
        .map(|node| {
            let known = node
                .key
                .as_deref()
                .and_then(|key| lookup.tree.find(key))
                .map(|found| found.location());
            let (name, place) = name_and_place(node, known.clone());
            Row {
                name,
                place,
                times: node.call_count,
                marks: node.marks.clone(),
                deeper: if node.marks.contains(&CallMark::Cut) {
                    known
                } else {
                    None
                },
                children: rows_of(&node.children, lookup),
            }
        })
        .collect()
}

fn write_rows(text: &mut String, rows: &[Row], level: usize, depth: u32) {
    let indent = "  ".repeat(level);
    for row in rows {
        text.push_str(&format!("{indent}{}  {}", row.name, row.place));
        if row.times > 1 {
            text.push_str(&format!("  ×{}", row.times));
        }
        for (mark, word) in [
            (CallMark::Recursive, "recursive"),
            (CallMark::Likely, "likely"),
            (CallMark::Incomplete, "incomplete"),
        ] {
            if row.marks.contains(&mark) {
                text.push_str(&format!("  ({word})"));
            }
        }
        text.push('\n');
        write_rows(text, &row.children, level + 1, depth);
        if let Some(location) = &row.deeper {
            text.push_str(&format!(
                "{indent}  … (deeper: vnoc calls {location} --depth {depth})\n"
            ));
        }
    }
}

impl Render for Calls {
    fn text(&self, _quiet: bool) -> String {
        let mut text = format!("{}\n", self.head);
        if self.rows.is_empty() {
            text.push_str("  (no calls)\n");
        }
        write_rows(&mut text, &self.rows, 1, self.depth_asked);
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::DefKind;

    fn node(path: Option<&str>, qualname: &str) -> CallTreeNode {
        CallTreeNode {
            key: None,
            name: qualname.rsplit('.').next().unwrap().to_string(),
            qualname: qualname.to_string(),
            path: path.map(str::to_string),
            kind: DefKind::Function,
            call_count: 1,
            call_sites: vec![],
            marks: vec![],
            children: vec![],
        }
    }

    fn row(name: &str, place: &str, times: u32, marks: &[CallMark], children: Vec<Row>) -> Row {
        Row {
            name: name.into(),
            place: place.into(),
            times,
            marks: marks.to_vec(),
            deeper: marks.contains(&CallMark::Cut).then(|| place.to_string()),
            children,
        }
    }

    #[test]
    fn a_target_the_index_lacks_is_outside_the_project_or_not_in_the_index() {
        let outside = node(None, "json.dumps");
        assert_eq!(
            name_and_place(&outside, None),
            (
                "json.dumps".to_string(),
                "(outside the project)".to_string()
            )
        );
        let ignored = node(Some("build/gen.py"), "make");
        assert_eq!(
            name_and_place(&ignored, None),
            (
                "make".to_string(),
                "build/gen.py::make (not in the index)".to_string()
            )
        );
        let known = node(Some("a.py"), "A.f");
        assert_eq!(
            name_and_place(&known, Some("a.py::A.f".into())),
            ("f".to_string(), "a.py::A.f".to_string())
        );
    }

    #[test]
    fn rows_have_two_spaces_a_level_times_marks_and_a_way_down_for_a_cut() {
        let rows = vec![
            row(
                "helper",
                "calc.py::helper",
                3,
                &[],
                vec![row("leaf", "calc.py::leaf", 1, &[CallMark::Cut], vec![])],
            ),
            row("json.dumps", "(outside the project)", 1, &[], vec![]),
            row("loop", "calc.py::loop", 1, &[CallMark::Recursive], vec![]),
            row(
                "guess",
                "m.py::guess",
                2,
                &[CallMark::Likely, CallMark::Incomplete],
                vec![],
            ),
        ];
        let mut text = String::new();
        write_rows(&mut text, &rows, 1, 4);
        let expected = [
            "  helper  calc.py::helper  ×3",
            "    leaf  calc.py::leaf",
            "      … (deeper: vnoc calls calc.py::leaf --depth 4)",
            "  json.dumps  (outside the project)",
            "  loop  calc.py::loop  (recursive)",
            "  guess  m.py::guess  ×2  (likely)  (incomplete)",
        ]
        .map(|line| format!("{line}\n"))
        .concat();
        assert_eq!(text, expected);
    }
}
