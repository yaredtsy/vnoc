//! `vnoc show <node>…` (03 §3): where a node is, the docs attached to it, and its code. Over `node_ref`, the tree and `source::excerpt`.
//!
//! Each node's answer is a value ([`NodeView`] or [`MissingView`]) that serializes to the JSON shape and renders as text; nothing is formatted
//! while it is computed (03 §11).

use serde::Serialize;

use super::args::ShowArgs;
use super::nodes::{
    Answer, Doc, Lookup, Missing, NodeError, Target, each, file_of, id_of, kind_word, lines_of,
    location_of,
};
use super::out::Render;
use super::project::open_from;
use super::{Failure, Global};
use crate::index::Node;
use crate::source;

/// Code longer than this is cut unless `--full`.
const LONGEST: usize = 120;

pub async fn run(global: &Global, args: &ShowArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    each("show", global, &args.nodes.nodes, &opened, &|lookup,
                                                       target|
     -> Result<
        ShowAnswer,
        NodeError,
    > {
        match target {
            Target::Missing(missing) => Ok(ShowAnswer::Missing(missing_view(missing))),
            Target::Node(node) => node_view(lookup, node, args).map(ShowAnswer::Node),
        }
    })
}

/// What `show` says of one argument.
#[derive(Serialize)]
#[serde(untagged)]
pub enum ShowAnswer {
    Node(NodeView),
    Missing(MissingView),
}

/// The code of a def, as much as is shown.
#[derive(Serialize)]
pub struct Code {
    /// the 1-based number of `lines[0]`
    pub first: u32,
    pub lines: Vec<String>,
    /// how many lines were left out at the end (`0` when none)
    pub more: usize,
}

#[derive(Serialize)]
pub struct Child {
    kind: &'static str,
    name: String,
    location: String,
    lines: Option<[u32; 2]>,
    docs: usize,
}

#[derive(Serialize)]
pub struct NodeView {
    kind: &'static str,
    location: String,
    path: String,
    qualname: Option<String>,
    lines: Option<[u32; 2]>,
    node_id: Option<String>,
    docs: Vec<Doc>,
    code: Option<Code>,
    /// one level of what a file or a folder holds, instead of code
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<Child>>,
}

#[derive(Serialize)]
pub struct MissingView {
    missing: bool,
    was: String,
    node_id: String,
    docs: Vec<Doc>,
    #[serde(skip)]
    path: String,
}

fn pair(lines: Option<(u32, u32)>) -> Option<[u32; 2]> {
    lines.map(|(a, b)| [a, b])
}

pub fn missing_view(missing: Missing) -> MissingView {
    MissingView {
        missing: true,
        was: missing.was,
        node_id: missing.id.to_string(),
        docs: missing.docs,
        path: missing.path,
    }
}

fn node_view(lookup: &Lookup, node: &Node, args: &ShowArgs) -> Result<NodeView, NodeError> {
    let code = match node.range {
        Some(range) if !args.no_code => {
            let excerpt =
                source::excerpt(&file_of(lookup, node), &range, 0).map_err(|e| NodeError {
                    why: "unreadable",
                    message: format!("{} could not be read: {e}", node.path),
                })?;
            let cut = if args.full {
                0
            } else {
                excerpt.lines.len().saturating_sub(LONGEST)
            };
            let shown = excerpt.lines.len() - cut;
            Some(Code {
                first: excerpt.first,
                lines: excerpt.lines[..shown].to_vec(),
                more: cut,
            })
        }
        _ => None,
    };
    let children =
        (!args.no_code && node.range.is_none() && !node.children.is_empty()).then(|| {
            node.children
                .iter()
                .map(|child| Child {
                    kind: kind_word(child.kind),
                    name: child.name.clone(),
                    location: location_of(child),
                    lines: pair(lines_of(child)),
                    docs: child.docs,
                })
                .collect()
        });
    Ok(NodeView {
        kind: kind_word(node.kind),
        location: location_of(node),
        path: node.path.clone(),
        qualname: node.qualname.clone(),
        lines: pair(lines_of(node)),
        node_id: id_of(node).map(|id| id.to_string()),
        docs: lookup.docs_of(id_of(node)),
        code,
        children,
    })
}

/// `docs (N)`, each doc, and the hint to read them (`--quiet` leaves the hint out).
pub fn docs_block(docs: &[Doc], read: &str, quiet: bool) -> String {
    let mut text = format!("docs ({})\n", docs.len());
    for doc in docs {
        text.push_str(&format!("  {}  {}\n", doc.title, doc.path));
    }
    if !docs.is_empty() && !quiet {
        text.push_str(&format!("  → vnoc doc read {read}\n"));
    }
    text
}

/// The lines as `  138  def …`, the numbers right-aligned to the widest.
pub fn numbered(first: u32, lines: &[String], indent: &str) -> String {
    let last = first as usize + lines.len().saturating_sub(1);
    let width = last.to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(n, line)| format!("{indent}{:>width$}  {line}\n", first as usize + n))
        .collect()
}

impl Render for ShowAnswer {
    fn text(&self, quiet: bool) -> String {
        match self {
            ShowAnswer::Node(view) => view.text(quiet),
            ShowAnswer::Missing(view) => view.text(quiet),
        }
    }
}

impl Render for NodeView {
    fn text(&self, quiet: bool) -> String {
        let mut head = format!("{}  {}", self.kind, self.location);
        if let Some([a, b]) = self.lines {
            head.push_str(&format!("   lines {a}-{b}"));
        }
        if let Some(id) = &self.node_id {
            head.push_str(&format!("   ID {id}"));
        }
        let mut text = format!("{head}\n{}", docs_block(&self.docs, &self.location, quiet));
        if let Some(code) = &self.code {
            text.push_str("code\n");
            text.push_str(&numbered(code.first, &code.lines, "  "));
            if code.more > 0 {
                let more = code.more;
                let lines = if more == 1 { "line" } else { "lines" };
                text.push_str(&if quiet {
                    format!("  … {more} more {lines}\n")
                } else {
                    format!(
                        "  … {more} more {lines} (vnoc code {}, or --full)\n",
                        self.location
                    )
                });
            }
        } else if let Some(children) = &self.children {
            text.push_str(&format!("contains ({})\n", children.len()));
            for child in children {
                text.push_str(&format!("  {}  {}", child.kind, child.name));
                if let Some([a, b]) = child.lines {
                    text.push_str(&format!("  lines {a}-{b}"));
                }
                text.push_str(&format!("  docs {}\n", child.docs));
            }
        }
        text
    }
}

impl Render for MissingView {
    fn text(&self, quiet: bool) -> String {
        let mut text = format!(
            "MISSING  was {}  ID {}  docs {}\n",
            self.was,
            self.node_id,
            self.docs.len()
        );
        // the ID reads it: a Missing node has no location any more
        if !self.docs.is_empty() {
            text.push_str(&docs_block(&self.docs, &self.node_id, quiet));
        }
        text
    }
}

impl Answer for ShowAnswer {
    fn id(&self) -> String {
        match self {
            ShowAnswer::Node(view) => view.location.clone(),
            ShowAnswer::Missing(view) => view.node_id.clone(),
        }
    }

    fn paths(&self) -> Vec<String> {
        vec![match self {
            ShowAnswer::Node(view) => view.path.clone(),
            ShowAnswer::Missing(view) => view.path.clone(),
        }]
    }
}
