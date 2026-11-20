//! `vnoc doc list | read | path` (03 §3): the docs of nodes.
//!
//! **A node named by its ID needs no index and so no driver**: its folder in `.vnoc/` is read directly (`project::open_without_driver`). Any other
//! way of naming a node goes through the index, as `show` does. A doc is text someone else wrote: it is printed as it is, never rewritten (01 §3).

use std::collections::BTreeMap;

use serde::Serialize;
use vnoc_domain::NodeId;

use super::args::{DocCommand, DocFileArgs, DocListArgs};
use super::nodes::{
    Answer, Doc, Lookup, NodeError, Target, docs_by_id, each, emit, expand, id_of, kind_word,
    lines_of, shown,
};
use super::out::{self, Render};
use super::project::{Opened, open_from, open_without_driver};
use super::show::{head_line, missing_line};
use super::{Failure, Global, style};
use crate::index::Node;
use crate::node_ref::{self, NodeRef};
use crate::userdata::{UserData, after_frontmatter};

pub async fn run(global: &Global, command: &DocCommand) -> Result<(), Failure> {
    match command {
        DocCommand::List(args) => list(global, args).await,
        DocCommand::Read(args) => {
            file_command(
                global,
                &args.file,
                args.where_.as_deref(),
                "doc read",
                View::Read,
            )
            .await
        }
        DocCommand::Path(args) => file_command(global, args, None, "doc path", View::Path).await,
        DocCommand::Tag(args) => super::tags::tag(global, args).await,
        DocCommand::New(args) => super::write::doc_new(global, args).await,
    }
}

/// `body` without its first line when that is the doc's own `# title`: the `── <title>` header says it already, and every note would pay for
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
    /// the def's lines (1-based, both ends included), with `doc read` of a node the index knows: the same as `show`'s
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[u32; 2]>,
    docs: Vec<Doc>,
    /// the texts, with `doc read`
    #[serde(skip_serializing_if = "Option::is_none", rename = "bodies")]
    bodies: Option<Vec<DocBody>>,
    #[serde(skip)]
    view: Option<View>,
    /// `--where` narrowed the docs: an empty list says "no docs match", not "no docs"
    #[serde(skip)]
    narrowed: bool,
}

impl DocsAnswer {
    fn node(kind: &str, location: String, node_id: Option<String>, docs: Vec<Doc>) -> DocsAnswer {
        DocsAnswer {
            kind: Some(kind.to_string()),
            location: Some(location),
            missing: false,
            was: None,
            node_id,
            lines: None,
            docs,
            bodies: None,
            view: None,
            narrowed: false,
        }
    }

    fn missing(was: String, node_id: String, docs: Vec<Doc>) -> DocsAnswer {
        DocsAnswer {
            kind: None,
            location: None,
            missing: true,
            was: Some(was),
            node_id: Some(node_id),
            lines: None,
            docs,
            bodies: None,
            view: None,
            narrowed: false,
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
                self.lines,
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
                    let match_ = if self.narrowed { " match" } else { "" };
                    return format!("no docs on {}{match_}\n", self.name());
                }
                // the node's header first, as `show` prints it (a Missing node: its MISSING line), then the notes
                let notes = bodies
                    .iter()
                    .map(|doc| {
                        let mut text = format!(
                            "{} {}\n   {}\n{}",
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
                    .join("\n");
                format!("{}\n{notes}", self.header())
            }
            View::List => {
                let mut text = format!("{}\n", self.header());
                if self.docs.is_empty() {
                    text.push_str(if self.narrowed {
                        "  no docs match\n"
                    } else {
                        "  no docs\n"
                    });
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

fn of_target(lookup: &Lookup, target: Target, with_lines: bool) -> DocsAnswer {
    match target {
        Target::Node(node) => {
            let mut answer = of_node(lookup, node);
            // only `doc read` names the node's lines, as its header does (`show`'s); `doc list` and `doc path` stay as they were
            if !with_lines {
                answer.lines = None;
            }
            answer
        }
        Target::Missing(missing) => {
            DocsAnswer::missing(missing.was, missing.id.to_string(), missing.docs)
        }
    }
}

fn of_node(lookup: &Lookup, node: &Node) -> DocsAnswer {
    let mut answer = DocsAnswer::node(
        kind_word(node.kind),
        node.location(),
        id_of(node).map(|id| id.to_string()),
        lookup.docs_of(id_of(node)),
    );
    answer.lines = lines_of(node).map(|(a, b)| [a, b]);
    answer
}

/// Every argument is an ID (`-` is read first): then no index is needed.
pub(super) fn only_ids(args: &[String]) -> bool {
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
    let narrow_by = narrowing(args.where_.as_deref())?;
    if args.nodes.is_empty() {
        let opened = open_from(global).await?;
        let mut groups = whole_project(&opened);
        if let Some(text) = narrow_by {
            let narrow = Narrow::new(&Lookup::new(&opened, 0), text);
            for group in &mut groups {
                narrow.apply(&mut group.docs);
            }
            groups.retain(|group| !group.docs.is_empty());
            return print_all("doc list", global, groups, true);
        }
        return print_all("doc list", global, groups, false);
    }
    let nodes = expand(&args.nodes)?;
    if let Some(text) = narrow_by {
        // the query is answered by the index: the doc texts and the nodes they belong to
        let opened = open_from(global).await?;
        let narrow = std::cell::OnceCell::new();
        return each("doc list", global, &nodes, &opened, &|lookup, target| {
            let mut answer = of_target(lookup, target, false);
            narrow
                .get_or_init(|| Narrow::new(lookup, text))
                .apply(&mut answer.docs);
            answer.narrowed = true;
            Ok::<_, NodeError>(answer)
        });
    }
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
        Ok::<_, NodeError>(of_target(lookup, target, false))
    })
}

/// `doc read` and `doc path`: one node, and one of its docs when `file` is given.
async fn file_command(
    global: &Global,
    args: &DocFileArgs,
    narrow_by: Option<&str>,
    command: &'static str,
    view: View,
) -> Result<(), Failure> {
    let narrow_by = narrowing(narrow_by)?;
    let nodes = expand(std::slice::from_ref(&args.node))?;
    let narrow = std::cell::OnceCell::new();
    let finish =
        |mut answer: DocsAnswer, lookup: Option<&Lookup>| -> Result<DocsAnswer, NodeError> {
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
            if let (Some(text), Some(lookup)) = (narrow_by, lookup) {
                narrow
                    .get_or_init(|| Narrow::new(lookup, text))
                    .apply(&mut answer.docs);
                answer.narrowed = true;
            }
            answer.view = Some(view);
            Ok(if view == View::Read {
                answer.read()
            } else {
                answer
            })
        };
    // a node named by its ID needs no index, unless `--where` asks the index which docs match
    if only_ids(&nodes) && narrow_by.is_none() {
        let project = open_without_driver(global)?;
        return emit(
            command,
            global,
            nodes.iter().map(|arg| {
                (
                    arg.as_str(),
                    by_id(&project, arg).and_then(|answer| finish(answer, None)),
                )
            }),
        );
    }
    let opened = open_from(global).await?;
    each(command, global, &nodes, &opened, &|lookup, target| {
        finish(of_target(lookup, target, view == View::Read), Some(lookup))
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

/// The docs of the ONE node `arg` names, and the project they are in: a node named by its ID is read from `.vnoc/` with no driver, any other name goes
/// through the index. The inner `Err` is a node that cannot be found, with the sentence every command gives.
pub(super) async fn node_docs(
    global: &Global,
    arg: &str,
) -> Result<
    (
        crate::project::Project,
        Result<(String, Vec<Doc>), NodeError>,
    ),
    Failure,
> {
    if only_ids(std::slice::from_ref(&arg.to_string())) {
        let project = open_without_driver(global)?;
        let answer = by_id(&project, arg).map(|a| (a.name(), a.docs));
        return Ok((project, answer));
    }
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let answer = lookup.resolve(arg).map(|target| {
        let a = of_target(&lookup, target, false);
        (a.name(), a.docs)
    });
    Ok((opened.project.clone(), answer))
}

/// The one doc of `docs` that `file` names, or the node's only doc when no file is given. A node with none, or with several and no file, is an error
/// that says what it has.
pub(super) fn choose_doc(name: &str, docs: Vec<Doc>, file: Option<&str>) -> Result<Doc, NodeError> {
    let have = |docs: &[Doc]| -> String {
        if docs.is_empty() {
            "no docs".to_string()
        } else {
            docs.iter()
                .map(|d| d.file.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        }
    };
    match (file, docs.len()) {
        (Some(file), _) => {
            let wanted = |doc: &Doc| doc.file == file || doc.file == format!("{file}.md");
            match docs.iter().position(wanted) {
                Some(at) => Ok(docs.into_iter().nth(at).expect("found above")),
                None => Err(NodeError::new(
                    "no_doc",
                    format!("no doc named {file} on {name}: it has {}", have(&docs)),
                )),
            }
        }
        (None, 1) => Ok(docs.into_iter().next().expect("one")),
        (None, 0) => Err(NodeError::new("no_doc", format!("no docs on {name}"))),
        (None, n) => Err(NodeError::new(
            "no_doc",
            format!(
                "{name} has {n} docs: name one of {} (the file name goes after the node)",
                have(&docs)
            ),
        )),
    }
}

/// `--where` as given: nothing, or a query that says something.
fn narrowing(text: Option<&str>) -> Result<Option<&str>, Failure> {
    match text {
        Some(query) if query.trim().is_empty() => Err(Failure::Usage(
            "--where needs a query: `tag:plan`, `author:claude-code`, words from the text".into(),
        )),
        other => Ok(other),
    }
}

/// What `--where` keeps (03 §3): the docs `vnoc_search::run` finds for the query with **no node rows and no limit**, by their file, and where each
/// stands in `run`'s order. The query is doc 20's, unchanged (Q9). Only what is about a doc narrows by itself: `tag:`, `author:` and words in the text;
/// `in:` and `kind:` narrow by the doc's node (on a node given by name they are already answered), and `has:doc` and `no:doc`, which are about nodes,
/// are a note and are ignored.
struct Narrow {
    rank: std::collections::HashMap<String, usize>,
}

impl Narrow {
    fn new(lookup: &Lookup, text: &str) -> Narrow {
        let mut query = vnoc_search::parse(text);
        for note in &query.notes {
            out::note(note);
        }
        if query.docs.take().is_some() {
            out::note("`has:doc` and `no:doc` are about nodes, not docs: ignored here");
        }
        let about_docs =
            query.has_doc_filters() || !query.words.is_empty() || !query.phrases.is_empty();
        if !about_docs && !query.keeps_nothing {
            out::note(
                "nothing in this query is about a doc: add `tag:`, `author:` or words from the text (`kind:` and `in:` alone select no doc)",
            );
        }
        let corpus = lookup.corpus();
        let limits = vnoc_search::Limits {
            nodes: 0,
            docs: usize::MAX / 2,
        };
        let found = vnoc_search::run(corpus, &query, None, limits);
        let rank = found
            .docs
            .iter()
            .enumerate()
            .map(|(at, hit)| (corpus.docs()[hit.index].doc_path.clone(), at))
            .collect();
        Narrow { rank }
    }

    /// Keeps the docs the query found, in `run`'s order.
    fn apply(&self, docs: &mut Vec<Doc>) {
        let at = |doc: &Doc| self.rank.get(doc.abs.to_string_lossy().as_ref()).copied();
        docs.retain(|doc| at(doc).is_some());
        docs.sort_by_key(|doc| at(doc));
    }
}

/// The groups of `doc list` with no node, as the flags ask: there is no argument to name them by, so none is in the JSON.
fn print_all(
    command: &'static str,
    global: &Global,
    groups: Vec<DocsAnswer>,
    narrowed: bool,
) -> Result<(), Failure> {
    if groups.is_empty() && !global.json && global.format.is_none() {
        return super::out::text(if narrowed {
            "no docs match\n"
        } else {
            "no docs\n"
        });
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
