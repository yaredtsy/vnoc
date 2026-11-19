//! `vnoc code <node>…` (03 §3): only the code of a function or a class, with line numbers.

use serde_json::json;

use super::args::CodeArgs;
use super::nodes::{NodeError, Rendered, Target, each, file_of, id_of, lines_of, location_of};
use super::project::open_from;
use super::show::numbered;
use super::{Failure, Global};
use crate::source;

pub async fn run(global: &Global, args: &CodeArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    each(
        "code",
        global,
        &args.nodes.nodes,
        &opened,
        &|lookup, target| {
            let node = match target {
                Target::Missing(missing) => {
                    return Err(NodeError {
                        why: "missing",
                        message: format!(
                            "{} is Missing: there is no code (vnoc show {} reads its docs)",
                            missing.was, missing.id
                        ),
                    });
                }
                Target::Node(node) => node,
            };
            let location = location_of(node);
            let (Some(range), Some((a, b))) = (node.range, lines_of(node)) else {
                let what = if node.kind == crate::index::Kind::File {
                    "a file"
                } else {
                    "a folder"
                };
                return Err(NodeError {
                    why: "no_code",
                    message: format!("{location} is {what}: use vnoc show for a file or a folder"),
                });
            };
            let excerpt =
                source::excerpt(&file_of(lookup, node), &range, args.context).map_err(|e| {
                    NodeError {
                        why: "unreadable",
                        message: format!("{} could not be read: {e}", node.path),
                    }
                })?;
            let mut text = String::new();
            // with several nodes, each starts with where it is: the lines are the def's own, whatever `--context` adds
            if lookup.count > 1 {
                text.push_str(&format!("── {location}  lines {a}-{b}\n"));
            }
            text.push_str(&numbered(&excerpt, excerpt.lines.len(), ""));
            Ok(Rendered {
                text,
                json: json!({
                    "kind": if node.kind == crate::index::Kind::Class { "class" } else { "function" },
                    "location": location,
                    "path": node.path,
                    "qualname": node.qualname,
                    "lines": [a, b],
                    "node_id": id_of(node).map(|id| id.to_string()),
                    "code": { "first": excerpt.first, "lines": excerpt.lines },
                }),
                id: location,
                path: node.path.clone(),
            })
        },
    )
}
