//! `vnoc code <node>…` (03 §3): only the code of a function or a class, with line numbers.

use serde::Serialize;

use super::args::CodeArgs;
use super::nodes::{Answer, NodeError, Target, each, file_of, id_of, lines_of, location_of};
use super::out::Render;
use super::project::open_from;
use super::show::numbered;
use super::{Failure, Global, style};
use crate::index::Kind;
use crate::source;

pub async fn run(global: &Global, args: &CodeArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    each("code", global, &args.nodes.nodes, &opened, &|lookup,
                                                       target|
     -> Result<
        CodeAnswer,
        NodeError,
    > {
        let node = match target {
            Target::Missing(missing) => {
                return Err(NodeError::new(
                    "missing",
                    format!(
                        "{} is Missing: there is no code (vnoc show {} reads its docs)",
                        missing.was, missing.id
                    ),
                ));
            }
            Target::Node(node) => node,
        };
        let location = location_of(node);
        let (Some(range), Some((a, b))) = (node.range, lines_of(node)) else {
            let what = if node.kind == Kind::File {
                "a file"
            } else {
                "a folder"
            };
            return Err(NodeError::new(
                "no_code",
                format!("{location} is {what}: use vnoc show for a file or a folder"),
            ));
        };
        let excerpt =
            source::excerpt(&file_of(lookup, node), &range, args.context).map_err(|e| {
                NodeError::new(
                    "unreadable",
                    format!("{} could not be read: {e}", node.path),
                )
            })?;
        Ok(CodeAnswer {
            kind: if node.kind == Kind::Class {
                "class"
            } else {
                "function"
            },
            location,
            path: node.path.clone(),
            qualname: node.qualname.clone(),
            lines: [a, b],
            node_id: id_of(node).map(|id| id.to_string()),
            code: Lines {
                first: excerpt.first,
                lines: excerpt.lines,
            },
            // with several nodes each starts with where it is
            headed: lookup.count > 1,
        })
    })
}

#[derive(Serialize)]
pub struct Lines {
    first: u32,
    lines: Vec<String>,
}

/// What `code` says of one argument.
#[derive(Serialize)]
pub struct CodeAnswer {
    kind: &'static str,
    location: String,
    path: String,
    qualname: Option<String>,
    /// the def's own lines, whatever `--context` adds around them
    lines: [u32; 2],
    node_id: Option<String>,
    code: Lines,
    #[serde(skip)]
    headed: bool,
}

impl Render for CodeAnswer {
    fn text(&self, _quiet: bool) -> String {
        let mut text = String::new();
        if self.headed {
            let [a, b] = self.lines;
            text.push_str(&format!(
                "{} {}  {}\n",
                style::paint(style::dim(), "──"),
                style::paint(style::subject(), &self.location),
                style::paint(style::dim(), format!("lines {a}-{b}"))
            ));
        }
        text.push_str(&numbered(self.code.first, &self.code.lines, ""));
        text
    }
}

impl Answer for CodeAnswer {
    fn id(&self) -> String {
        self.location.clone()
    }

    fn paths(&self) -> Vec<String> {
        vec![self.path.clone()]
    }
}
