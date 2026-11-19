//! `vnoc show <node>…` (03 §3): where a node is, the docs attached to it, and its code. Over `node_ref`, the tree and `source::excerpt`.

use serde_json::{Value, json};

use super::args::ShowArgs;
use super::nodes::{
    Doc, Lookup, Missing, NodeError, Rendered, Target, each, file_of, id_of, lines_of, location_of,
};
use super::project::open_from;
use super::{Failure, Global};
use crate::index::{Kind, Node};
use crate::source::{self, Excerpt};

/// Code longer than this is cut unless `--full`.
const LONGEST: usize = 120;

pub async fn run(global: &Global, args: &ShowArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    each(
        "show",
        global,
        &args.nodes.nodes,
        &opened,
        &|lookup, target| match target {
            Target::Missing(missing) => Ok(missing_view(&missing, global.quiet)),
            Target::Node(node) => node_view(lookup, node, args, global.quiet),
        },
    )
}

fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Folder => "folder",
        Kind::File => "file",
        Kind::Class => "class",
        Kind::Function => "function",
    }
}

fn docs_json(docs: &[Doc]) -> Value {
    docs.iter()
        .map(|d| json!({ "title": d.title, "path": d.path }))
        .collect()
}

/// `docs (N)`, each doc, and the hint to read them.
fn docs_block(docs: &[Doc], read: &str, quiet: bool) -> String {
    let mut text = format!("docs ({})\n", docs.len());
    for doc in docs {
        text.push_str(&format!("  {}  {}\n", doc.title, doc.path));
    }
    if !docs.is_empty() && !quiet {
        text.push_str(&format!("  → vnoc doc read {read}\n"));
    }
    text
}

fn missing_view(missing: &Missing, quiet: bool) -> Rendered {
    let mut text = format!(
        "MISSING  was {}  ID {}  docs {}\n",
        missing.was,
        missing.id,
        missing.docs.len()
    );
    // the ID reads it: a Missing node has no location any more
    if !missing.docs.is_empty() {
        text.push_str(&docs_block(&missing.docs, &missing.id.to_string(), quiet));
    }
    Rendered {
        text,
        json: json!({
            "missing": true,
            "was": missing.was,
            "node_id": missing.id.to_string(),
            "docs": docs_json(&missing.docs),
        }),
        id: missing.id.to_string(),
        path: missing.path.clone(),
    }
}

/// The lines as `  138  def …`, the numbers right-aligned to the widest.
pub fn numbered(excerpt: &Excerpt, shown: usize, indent: &str) -> String {
    let last = excerpt.first as usize + shown.saturating_sub(1);
    let width = last.to_string().len();
    excerpt
        .lines
        .iter()
        .take(shown)
        .enumerate()
        .map(|(n, line)| format!("{indent}{:>width$}  {line}\n", excerpt.first as usize + n))
        .collect()
}

/// The code of a def, cut to [`LONGEST`] lines unless `full`. The second is how many lines were left out.
fn code_of(lookup: &Lookup, node: &Node, full: bool) -> Result<(Excerpt, usize), NodeError> {
    let range = node.range.expect("a def has a range");
    let excerpt = source::excerpt(&file_of(lookup, node), &range, 0).map_err(|e| NodeError {
        why: "unreadable",
        message: format!("{} could not be read: {e}", node.path),
    })?;
    let cut = if full {
        0
    } else {
        excerpt.lines.len().saturating_sub(LONGEST)
    };
    Ok((excerpt, cut))
}

fn node_view(
    lookup: &Lookup,
    node: &Node,
    args: &ShowArgs,
    quiet: bool,
) -> Result<Rendered, NodeError> {
    let location = location_of(node);
    let id = id_of(node);
    let docs = lookup.docs_of(id);
    let lines = lines_of(node);
    let is_def = node.range.is_some();

    let mut head = format!("{}  {location}", kind_word(node.kind));
    if let Some((a, b)) = lines {
        head.push_str(&format!("   lines {a}-{b}"));
    }
    if let Some(id) = id {
        head.push_str(&format!("   ID {id}"));
    }
    let mut text = format!("{head}\n{}", docs_block(&docs, &location, quiet));
    let mut json = json!({
        "kind": kind_word(node.kind),
        "location": location,
        "path": node.path,
        "qualname": node.qualname,
        "lines": lines.map(|(a, b)| json!([a, b])),
        "node_id": id.map(|id| id.to_string()),
        "docs": docs_json(&docs),
        "code": Value::Null,
    });

    if !args.no_code && is_def {
        let (excerpt, cut) = code_of(lookup, node, args.full)?;
        let shown = excerpt.lines.len() - cut;
        text.push_str("code\n");
        text.push_str(&numbered(&excerpt, shown, "  "));
        if cut > 0 {
            let lines = if cut == 1 { "line" } else { "lines" };
            text.push_str(&if quiet {
                format!("  … {cut} more {lines}\n")
            } else {
                format!("  … {cut} more {lines} (vnoc code {location}, or --full)\n")
            });
        }
        json["code"] = json!({
            "first": excerpt.first,
            "lines": &excerpt.lines[..shown],
            "more": cut,
        });
    } else if !args.no_code && !node.children.is_empty() {
        // a file or a folder: one level of what it holds instead of code
        text.push_str(&format!("contains ({})\n", node.children.len()));
        let mut children = Vec::new();
        for child in &node.children {
            let mut row = format!("  {}  {}", kind_word(child.kind), child.name);
            if let Some((a, b)) = lines_of(child) {
                row.push_str(&format!("  lines {a}-{b}"));
            }
            row.push_str(&format!("  docs {}\n", child.docs));
            text.push_str(&row);
            children.push(json!({
                "kind": kind_word(child.kind),
                "name": child.name,
                "location": location_of(child),
                "lines": lines_of(child).map(|(a, b)| json!([a, b])),
                "docs": child.docs,
            }));
        }
        json["children"] = Value::Array(children);
    }
    Ok(Rendered {
        text,
        json,
        id: location,
        path: node.path.clone(),
    })
}
