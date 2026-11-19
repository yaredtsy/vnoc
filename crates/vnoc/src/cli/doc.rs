//! `vnoc doc list | read | path` (03 §3): the docs of nodes.
//!
//! **A node named by its ID needs no index and so no driver**: its folder in `.vnoc/` is read directly (`project::open_without_driver`). Any other
//! way of naming a node goes through the index, as `show` does. A doc is text someone else wrote: it is printed as it is, never rewritten (01 §3).

use std::collections::BTreeMap;

use serde::Serialize;
use vnoc_domain::NodeId;

use super::args::{DocCommand, DocFileArgs, DocListArgs};
use super::nodes::{
    Answer, Doc, Lookup, NodeError, Target, docs_by_id, each, emit, expand, id_of, kind_word, shown,
};
use super::out::Render;
use super::project::{Opened, open_from, open_without_driver};
use super::show::{head_line, missing_line};
use super::{Failure, Global, style};
use crate::index::Node;
use crate::node_ref::{self, NodeRef};
use crate::userdata::{UserData, after_frontmatter};

pub async fn run(global: &Global, command: &DocCommand) -> Result<(), Failure> {
    match command {
        DocCommand::List(args) => list(global, args).await,
        DocCommand::Read(args) => file_command(global, args, "doc read", View::Read).await,
        DocCommand::Path(args) => file_command(global, args, "doc path", View::Path).await,
        DocCommand::New(args) => super::write::doc_new(global, args).await,
    }
}

/// `body` without its first line when that is the doc's own `# title`: the `── <title>  <path>` header says it already, and every note would pay for
/// it twice. The blank lines before it and right after it go with it. Any other first line is printed as it is, and so is everything after it:
/// nothing else is cut, and a line is never rewritten.
fn without_title<'a>(body: &'a str, title: &str) -> &'a str {
    let rest = body.trim_start_matches(['\n', '\r', ' ', '\t']);
    let line_end = rest.find('\n').map_or(rest.len(), |at| at + 1);
    let (first, after) = rest.split_at(line_end);
    match first.strip_prefix("# ") {
        Some(heading) if heading.trim() == title => after.trim_start_matches(['\n', '\r']),
        _ => body,
    }
}

/// Which of the three the text of an answer is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Read,
    Path,
}

/// A doc with its text, for `doc read`.
#[derive(Serialize)]
pub struct DocBody {
    title: String,
    path: String,
    /// the text after the frontmatter, as it is in the file
    body: String,
}

/// A node and its docs: what `doc list` and `doc path` answer for it, and `doc read` with the text.
#[derive(Serialize)]
pub struct DocsAnswer {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    missing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    was: Option<String>,
    node_id: Option<String>,
    docs: Vec<Doc>,
    /// the texts, with `doc read`
    #[serde(skip_serializing_if = "Option::is_none", rename = "bodies")]
    bodies: Option<Vec<DocBody>>,
    #[serde(skip)]
    view: Option<View>,
}

impl DocsAnswer {
    fn node(kind: &str, location: String, node_id: Option<String>, docs: Vec<Doc>) -> DocsAnswer {
        DocsAnswer {
            kind: Some(kind.to_string()),
            location: Some(location),
            missing: false,
            was: None,
            node_id,
            docs,
            bodies: None,
            view: None,
        }
    }

    fn missing(was: String, node_id: String, docs: Vec<Doc>) -> DocsAnswer {
        DocsAnswer {
            kind: None,
            location: None,
            missing: true,
            was: Some(was),
            node_id: Some(node_id),
            docs,
            bodies: None,
            view: None,
        }
    }

    /// The same node with its texts read, for `doc read`.
    fn read(mut self) -> DocsAnswer {
        let bodies = self
            .docs
            .iter()
            .map(|doc| DocBody {
                title: doc.title.clone(),
                path: doc.path.clone(),
                body: std::fs::read_to_string(&doc.abs)
                    .map(|text| {
                        without_title(after_frontmatter(&text).unwrap_or(&text), &doc.title)
                            .to_string()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        self.bodies = Some(bodies);
        self
    }

    /// Where the node is, as its header says it.
    fn header(&self) -> String {
        match (&self.was, &self.location, &self.kind) {
            (Some(was), _, _) => {
                missing_line(was, self.node_id.as_deref().unwrap_or(""), self.docs.len())
            }
            (None, Some(location), kind) => head_line(
                kind.as_deref().unwrap_or("node"),
                location,
                None,
                self.node_id.as_deref(),
            ),
            _ => String::new(),
        }
    }

    /// The node as the commands that take one name it again: its location, or its ID when it is Missing.
    fn name(&self) -> String {
        self.location
            .clone()
            .or_else(|| self.node_id.clone())
            .unwrap_or_default()
    }
}

impl Render for DocsAnswer {
    fn text(&self, _quiet: bool) -> String {
        match self.view.unwrap_or(View::List) {
            View::Path => self.docs.iter().map(|d| format!("{}\n", d.path)).collect(),
            View::Read => {
                let Some(bodies) = &self.bodies else {
                    return String::new();
                };
                if bodies.is_empty() {
                    return format!("no docs on {}\n", self.name());
                }
                bodies
                    .iter()
                    .map(|doc| {
                        let mut text = format!(
                            "{} {}  {}\n{}",
                            style::paint(style::dim(), "──"),
                            style::paint(style::title(), &doc.title),
                            style::paint(style::dim(), &doc.path),
                            doc.body
                        );
                        if !text.ends_with('\n') {
                            text.push('\n');
                        }
                        text
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            View::List => {
                let mut text = format!("{}\n", self.header());
                if self.docs.is_empty() {
                    text.push_str("  no docs\n");
                }
                for doc in &self.docs {
                    text.push_str(&doc.row());
                }
                text
            }
        }
    }
}

impl Answer for DocsAnswer {
    fn id(&self) -> String {
        self.name()
    }

    fn paths(&self) -> Vec<String> {
        self.docs.iter().map(|d| d.path.clone()).collect()
    }
}

fn of_target(lookup: &Lookup, target: Target) -> DocsAnswer {
    match target {
        Target::Node(node) => of_node(lookup, node),
        Target::Missing(missing) => {
            DocsAnswer::missing(missing.was, missing.id.to_string(), missing.docs)
        }
    }
}

fn of_node(lookup: &Lookup, node: &Node) -> DocsAnswer {
    DocsAnswer::node(
        kind_word(node.kind),
        node.location(),
        id_of(node).map(|id| id.to_string()),
        lookup.docs_of(id_of(node)),
    )
}

/// Every argument is an ID (`-` is read first): then no index is needed.
fn only_ids(args: &[String]) -> bool {
    !args.is_empty()
        && args
            .iter()
            .all(|arg| matches!(node_ref::parse(arg), Some(NodeRef::Id(_))))
}

/// A node known only by its record: where `node.toml` says it was, its kind, and the docs in its folder. `Err` when there is no such folder.
fn by_id(project: &crate::project::Project, arg: &str) -> Result<DocsAnswer, NodeError> {
    let Some(NodeRef::Id(id)) = node_ref::parse(arg) else {
        unreachable!("only IDs come here")
    };
    let Some(toml) = UserData::new(project.git_root()).node_toml(&id) else {
        return Err(NodeError::new("no_id", format!("no node has the ID {id}")));
    };
    let (location, kind) = anchor_of(project, &toml);
    Ok(DocsAnswer::node(
        &kind,
        location,
        Some(id.to_string()),
        docs_by_id(project, &id),
    ))
}

/// The location and the kind a record gives: not whether the code is still there (that needs the index).
fn anchor_of(project: &crate::project::Project, toml: &vnoc_domain::NodeToml) -> (String, String) {
    let kind = toml
        .kind()
        .map_or("node".to_string(), |k| k.as_str().to_string());
    let location = match toml.location() {
        Ok(place) => {
            let path = shown(project.prefix(), place.path().as_str());
            match (place.qualname(), place.occurrence()) {
                ("", _) => path,
                (qualname, 1) => format!("{path}::{qualname}"),
                (qualname, n) => format!("{path}::{qualname}#{n}"),
            }
        }
        Err(_) => "(not known)".to_string(),
    };
    (location, kind)
}

async fn list(global: &Global, args: &DocListArgs) -> Result<(), Failure> {
    if args.nodes.is_empty() {
        let opened = open_from(global).await?;
        let groups = whole_project(&opened);
        return print_all("doc list", global, groups);
    }
    let nodes = expand(&args.nodes)?;
    if only_ids(&nodes) {
        let project = open_without_driver(global)?;
        return emit(
            "doc list",
            global,
            nodes.iter().map(|arg| (arg.as_str(), by_id(&project, arg))),
        );
    }
    let opened = open_from(global).await?;
    each("doc list", global, &nodes, &opened, &|lookup, target| {
        Ok::<_, NodeError>(of_target(lookup, target))
    })
}

/// `doc read` and `doc path`: one node, and one of its docs when `file` is given.
async fn file_command(
    global: &Global,
    args: &DocFileArgs,
    command: &'static str,
    view: View,
) -> Result<(), Failure> {
    let nodes = expand(std::slice::from_ref(&args.node))?;
    let finish = |mut answer: DocsAnswer| -> Result<DocsAnswer, NodeError> {
        if let Some(file) = &args.file {
            let wanted = |doc: &Doc| doc.file == *file || doc.file == format!("{file}.md");
            if !answer.docs.iter().any(wanted) {
                let have: Vec<&str> = answer.docs.iter().map(|d| d.file.as_str()).collect();
                return Err(NodeError::new(
                    "no_doc",
                    format!(
                        "no doc named {file} on {}: it has {}",
                        answer.name(),
                        if have.is_empty() {
                            "no docs".to_string()
                        } else {
                            have.join(" · ")
                        }
                    ),
                ));
            }
            answer.docs.retain(wanted);
        }
        answer.view = Some(view);
        Ok(if view == View::Read {
            answer.read()
        } else {
            answer
        })
    };
    if only_ids(&nodes) {
        let project = open_without_driver(global)?;
        return emit(
            command,
            global,
            nodes
                .iter()
                .map(|arg| (arg.as_str(), by_id(&project, arg).and_then(&finish))),
        );
    }
    let opened = open_from(global).await?;
    each(command, global, &nodes, &opened, &|lookup, target| {
        finish(of_target(lookup, target))
    })
}

/// Every doc of the project, grouped by node: the nodes of the tree in its order, then the Missing ones, then those whose code was not read.
fn whole_project(opened: &Opened) -> Vec<DocsAnswer> {
    let project = &opened.project;
    let index = opened.index.read().unwrap();
    let texts = index.doc_texts();
    let mut by_node: BTreeMap<NodeId, Vec<Doc>> = BTreeMap::new();
    for (id, file, text) in texts.iter() {
        by_node.entry(*id).or_default().push(Doc {
            title: text.title.clone(),
            tags: text.tags.clone(),
            author: text.author.clone(),
            path: super::nodes::doc_path(project, &text.path),
            file: file.to_string(),
            abs: text.path.clone(),
        });
    }
    let tree = index.tree(&project.name(), project.prefix());
    let mut groups = Vec::new();
    let mut stack = vec![&tree];
    // depth first, in the tree's order
    while let Some(node) = stack.pop() {
        if let Some(docs) = id_of(node).and_then(|id| by_node.remove(&id)) {
            groups.push(DocsAnswer::node(
                kind_word(node.kind),
                node.location(),
                id_of(node).map(|id| id.to_string()),
                docs,
            ));
        }
        stack.extend(node.children.iter().rev());
    }
    for missing in index.missing() {
        let Ok(id) = NodeId::parse(&missing.id) else {
            continue;
        };
        if let Some(docs) = by_node.remove(&id) {
            let path = shown(project.prefix(), &missing.path);
            let was = if missing.qualname.is_empty() {
                path
            } else {
                format!("{path}::{}", missing.qualname)
            };
            groups.push(DocsAnswer::missing(was, missing.id.clone(), docs));
        }
    }
    // a node whose code was not read (outside the folder, ignored): its record says where it was
    for (id, docs) in by_node {
        let (location, kind) = match index.record_toml(&id) {
            Some(toml) => anchor_of(project, toml),
            None => ("(not known)".to_string(), "node".to_string()),
        };
        groups.push(DocsAnswer::node(
            &kind,
            location,
            Some(id.to_string()),
            docs,
        ));
    }
    groups
}

/// The groups of `doc list` with no node, as the flags ask: there is no argument to name them by, so none is in the JSON.
fn print_all(
    command: &'static str,
    global: &Global,
    groups: Vec<DocsAnswer>,
) -> Result<(), Failure> {
    if groups.is_empty() && !global.json && global.format.is_none() {
        return super::out::text("no docs\n");
    }
    emit(
        command,
        global,
        groups
            .into_iter()
            .map(|group| ("", Ok::<_, NodeError>(group))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_is_dropped_when_it_is_the_title_and_only_then() {
        let title = "Why we parse twice";
        // the heading and the blank line after it are not printed
        assert_eq!(
            without_title("# Why we parse twice\n\nText.\n", title),
            "Text.\n"
        );
        assert_eq!(
            without_title("# Why we parse twice\r\n\r\nText.\r\n", title),
            "Text.\r\n"
        );
        assert_eq!(
            without_title("\n\n# Why we parse twice\nText.\n", title),
            "Text.\n"
        );
        // the heading with spaces round it is still the title
        assert_eq!(
            without_title("# Why we parse twice  \nText.\n", title),
            "Text.\n"
        );
        // another heading, another first line, a deeper heading: printed as it is
        for body in [
            "# Another heading\n\nText.\n",
            "Intro first.\n\n# Why we parse twice\nText.\n",
            "## Why we parse twice\nText.\n",
            "#Why we parse twice\nText.\n",
            "Text only.\n",
            "",
        ] {
            assert_eq!(without_title(body, title), body, "{body:?}");
        }
        // only the title line goes: a second heading with the same text stays
        assert_eq!(
            without_title("# Why we parse twice\n\n# Why we parse twice\nx\n", title),
            "# Why we parse twice\nx\n"
        );
        // a title that is the whole doc
        assert_eq!(without_title("# Why we parse twice", title), "");
    }
}
