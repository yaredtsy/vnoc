//! Arguments → nodes (docs/21-agent-cli/02-naming-a-node.md): every command that takes `<node>` goes through here.
//!
//! The parse and the lookup are `node_ref`'s, the same two functions the search box uses; this file adds what only a command line has: the current
//! folder, `-` for standard input, several nodes in one call with each error printed in its place, and the sentences of §4.

use std::cell::OnceCell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Value, json};
use vnoc_domain::NodeId;
use vnoc_search::{Corpus, DocIndex};

use super::out::Render;
use super::project::Opened;
use super::{Failure, Format, Global, out};
use crate::index::{Index, Node};
use crate::node_ref::{self, Found, NodeRef, Why, is_absolute};
use crate::project::Project;
use crate::search::{self, CorpusSlot};
use crate::suggest::{Suggestion, suggest};
use crate::userdata::UserData;

/// An argument that named nothing, or nothing that has what the command needs: the case (`why`, a stable word for `--json`) and the sentence.
#[derive(Debug)]
pub struct NodeError {
    pub why: &'static str,
    /// one line, as `--json` has always had it: the sentence, then the hints after ` · ` (the text output does not print this: it puts them under
    /// the sentence)
    pub message: String,
    /// the sentence alone
    said: String,
    pub hints: Option<Hints>,
}

/// A list a sentence is followed by: what the name may have meant, what was found, what the file has, the files a name is. One per line in the text.
#[derive(Debug)]
pub struct Hints {
    /// `did you mean`, `found`, `main.py has:`, `name one of`
    pub lead: String,
    /// as they can be pasted back as an argument
    pub items: Vec<String>,
    /// how many more there were than are shown
    pub more: usize,
}

impl NodeError {
    /// A sentence with nothing after it.
    pub fn new(why: &'static str, message: impl Into<String>) -> NodeError {
        let message = message.into();
        NodeError {
            why,
            said: message.clone(),
            message,
            hints: None,
        }
    }

    /// A sentence and a list under it. `joined` is how the one-line `message` introduces the list (` · did you mean: `).
    fn with_hints(why: &'static str, said: String, joined: &str, hints: Hints) -> NodeError {
        let mut line = format!("{said}{joined}{}", hints.items.join(" · "));
        if hints.more > 0 {
            line.push_str(&format!(" · and {} more", hints.more));
        }
        NodeError {
            why,
            said,
            message: line,
            hints: Some(hints),
        }
    }

    /// The error as `--json` has it: `message` is one line, and `hints` (an addition) is the list.
    pub fn json(&self) -> Value {
        let mut value = json!({ "why": self.why, "message": self.message });
        if let Some(hints) = &self.hints {
            value["hints"] = json!(hints.items);
        }
        value
    }

    /// The error on standard error: the sentence, and the list one per line under it.
    pub fn print(&self) {
        out::error(&self.said);
        if let Some(hints) = &self.hints {
            out::hints(&hints.lead, &hints.items, hints.more);
        }
    }
}

/// A doc of a node: its title and its file, as [`doc_path`] prints it.
#[derive(Serialize)]
pub struct Doc {
    pub title: String,
    pub path: String,
    /// the file's name in the node's folder (`why-we-parse-twice.md`): what `doc read <node> <file>` takes. Not part of the JSON shape.
    #[serde(skip)]
    pub file: String,
    /// where the file is, for reading it
    #[serde(skip)]
    pub abs: PathBuf,
}

/// A node whose code is gone: what its record says, and its docs.
pub struct Missing {
    pub id: NodeId,
    /// where its `node.toml` says it was, as a location relative to the root
    pub was: String,
    /// the file part of `was`
    pub path: String,
    pub docs: Vec<Doc>,
}

pub enum Target<'t> {
    Node(&'t Node),
    Missing(Missing),
}

/// What a command that takes nodes answers for one of them: a value that renders as text and as JSON (`Render`), and says what `--format id` and
/// `--format path` print for it.
pub trait Answer: Render {
    /// the location, or the ID of a Missing node
    fn id(&self) -> String;
    /// the files `--format path` lists: the source file of a node, the files of a node's docs
    fn paths(&self) -> Vec<String>;
}

/// A doc file as every command prints it: relative to the root when `.vnoc/` is under the root (the ordinary case: the root is the git root), and
/// **absolute** when it is not. With `--root` below the git root, `.vnoc/` is outside the root, and a path relative to the git root would be
/// relative to nothing the agent has.
pub fn doc_path(project: &Project, doc: &Path) -> String {
    let path = doc.strip_prefix(project.open_dir()).unwrap_or(doc);
    path.to_string_lossy().replace('\\', "/")
}

/// The project as a list of nodes to look things up in: the tree, built once, and the corpus for "did you mean", built only on a miss.
pub struct Lookup<'a> {
    pub opened: &'a Opened,
    pub tree: Node,
    /// how many arguments the command was given (after `-` was read)
    pub count: usize,
    /// the current folder below the root, `/`-separated; empty when it is the root (or not under it)
    rel_cwd: String,
    corpus: OnceCell<Arc<Corpus>>,
    corpus_slot: CorpusSlot,
    doc_slot: Mutex<Option<(u64, Arc<DocIndex>)>>,
}

impl<'a> Lookup<'a> {
    pub fn new(opened: &'a Opened, count: usize) -> Lookup<'a> {
        let project = &opened.project;
        let tree = opened
            .index
            .read()
            .unwrap()
            .tree(&project.name(), project.prefix());
        let rel_cwd = std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                cwd.strip_prefix(project.open_dir())
                    .ok()
                    .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            })
            .unwrap_or_default();
        Lookup {
            opened,
            tree,
            count,
            rel_cwd,
            corpus: OnceCell::new(),
            corpus_slot: Mutex::new(None),
            doc_slot: Mutex::new(None),
        }
    }

    /// The node `arg` names. A relative path is tried from the current folder first and then from the root; each try is `node_ref`'s own.
    pub fn resolve(&self, arg: &str) -> Result<Target<'_>, NodeError> {
        let Some(written) = node_ref::parse(arg) else {
            return Err(NodeError::new("empty", "an empty reference names no node"));
        };
        let joined = self.joined(&written);
        let project = &self.opened.project;
        {
            let index = self.opened.index.read().unwrap();
            for r in joined.iter().chain(std::iter::once(&written)) {
                match index.resolve_ref(project, r) {
                    Found::Node(id) => {
                        if let Some(done) = self.found(&index, r, &id) {
                            return done;
                        }
                        // not in the tree: a file the driver failed on
                        let why = index.why_not(project, r).unwrap_or_else(|| unexplained(r));
                        drop(index);
                        return Err(self.explain(&why, r));
                    }
                    Found::Missing(id) => return Ok(Target::Missing(self.missing(&index, id))),
                    Found::Nothing => {}
                }
            }
        }
        let (why, used) = self.why(joined, written);
        Err(self.explain(&why, &used))
    }

    /// A `find` query that is one reference, made relative to the root through the current folder when that names something there: `charge.py:12`
    /// from `billing/` is `billing/charge.py:12`. It is `resolve`'s own order (the current folder first, then as written) and its own `joined`;
    /// a query of words, or one the current folder does not change, comes back as it was.
    pub fn relative_to_cwd(&self, text: &str) -> String {
        let project = &self.opened.project;
        let joined = vnoc_search::parse(text)
            .reference
            .as_deref()
            .and_then(node_ref::parse)
            .and_then(|written| self.joined(&written));
        let Some(joined) = joined else {
            return text.to_string();
        };
        let index = self.opened.index.read().unwrap();
        if matches!(index.resolve_ref(project, &joined), Found::Nothing) {
            return text.to_string();
        }
        match joined {
            NodeRef::Location { path, rest } => format!("{path}::{rest}"),
            NodeRef::FileLine { path, line } => format!("{path}:{line}"),
            NodeRef::Path(path) => path,
            NodeRef::Id(id) => id.to_string(),
        }
    }

    /// A node of the tree, unless the reference is a line past the end of its file.
    fn found(&self, index: &Index, r: &NodeRef, id: &str) -> Option<Result<Target<'_>, NodeError>> {
        let node = self.tree.find(id)?;
        if matches!(r, NodeRef::FileLine { .. })
            && node.kind == crate::index::Kind::File
            && let Some(past @ Why::PastEnd { .. }) = index.why_not(&self.opened.project, r)
        {
            return Some(Err(self.explain(&past, r)));
        }
        Some(Ok(Target::Node(node)))
    }

    /// The reference made relative to the root through the current folder, when that is a different reference: `billing/` and `charge.py:12` is
    /// `billing/charge.py:12`. `..` goes up and never above the root; an absolute path and an ID have no current folder.
    fn joined(&self, r: &NodeRef) -> Option<NodeRef> {
        let path = r
            .path()
            .filter(|p| !self.rel_cwd.is_empty() && !is_absolute(p))?;
        let mut parts: Vec<&str> = self.rel_cwd.split('/').collect();
        for part in path.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                other => parts.push(other),
            }
        }
        Some(r.clone().with_path(parts.join("/")))
    }

    /// The reason, and the reference it was asked of: the joined one when its file was found (the current folder is tried first), else as written.
    fn why(&self, joined: Option<NodeRef>, written: NodeRef) -> (Why, NodeRef) {
        let index = self.opened.index.read().unwrap();
        let project = &self.opened.project;
        if let Some(joined) = joined
            && let Some(why) = index.why_not(project, &joined)
            && matches!(
                why,
                Why::NoDef { .. } | Why::Failed { .. } | Why::PastEnd { .. }
            )
        {
            return (why, joined);
        }
        let why = index
            .why_not(project, &written)
            .unwrap_or_else(|| unexplained(&written));
        (why, written)
    }

    /// A node whose code is gone, by its ID: what its record says and its docs.
    pub fn describe_missing(&self, id: NodeId) -> Missing {
        self.missing(&self.opened.index.read().unwrap(), id)
    }

    fn missing(&self, index: &Index, id: NodeId) -> Missing {
        let project = &self.opened.project;
        let (was, path) = match index.record_toml(&id).and_then(|toml| toml.location().ok()) {
            Some(place) => {
                let path = shown(project.prefix(), place.path().as_str());
                let was = if place.qualname().is_empty() {
                    path.clone()
                } else {
                    let nth = if place.occurrence() > 1 {
                        format!("#{}", place.occurrence())
                    } else {
                        String::new()
                    };
                    format!("{path}::{}{nth}", place.qualname())
                };
                (was, path)
            }
            None => ("(not known)".to_string(), String::new()),
        };
        Missing {
            was,
            path,
            docs: self.docs_of(Some(id)),
            id,
        }
    }

    /// The docs of the node with this permanent ID.
    pub fn docs_of(&self, id: Option<NodeId>) -> Vec<Doc> {
        id.map(|id| docs_by_id(&self.opened.project, &id))
            .unwrap_or_default()
    }

    /// The corpus for "did you mean", made on the first miss.
    pub fn corpus(&self) -> &Corpus {
        self.corpus.get_or_init(|| {
            search::prepare(
                &self.opened.project,
                &self.opened.index,
                &self.corpus_slot,
                &self.doc_slot,
                "",
            )
            .corpus
        })
    }

    /// The sentence of 02 §4 for `why`. Every hint in it can be pasted back as the argument.
    fn explain(&self, why: &Why, r: &NodeRef) -> NodeError {
        let root = self.opened.project.open_dir().display();
        let plain = |slug: &'static str, message: String| NodeError::new(slug, message);
        match why {
            Why::NoDef { path, name } if name.is_empty() => {
                plain("no_def", format!("no node at {path}"))
            }
            Why::NoDef { path, name } => {
                let said = format!("no node at {path}::{name}");
                let asked = r.clone().with_path(path.clone());
                match self.suggestion(&asked, 3) {
                    // what the named file has: the sentence says whose
                    (Suggestion::Has(_), items, more) => NodeError::with_hints(
                        "no_def",
                        said,
                        &format!(" · {} has: ", base_name(path)),
                        Hints {
                            lead: format!("{} has:", base_name(path)),
                            items,
                            more,
                        },
                    ),
                    (Suggestion::Nothing, _, _) => plain("no_def", said),
                    (_, items, more) => NodeError::with_hints(
                        "no_def",
                        said,
                        " · did you mean: ",
                        Hints {
                            lead: "did you mean".into(),
                            items,
                            more,
                        },
                    ),
                }
            }
            Why::NoFile(text) => {
                // `OrderService.parse_order` parses as a path: with no folder in it, it may be a name
                let found = match r {
                    NodeRef::Path(_) if !text.contains('/') && !text.contains('\\') => {
                        Some(self.suggestion(r, 5))
                    }
                    _ => None,
                };
                match found {
                    Some((Suggestion::Found(_) | Suggestion::Near(_), items, more)) => {
                        NodeError::with_hints(
                            "no_file",
                            format!("\"{text}\" names no file"),
                            " · found: ",
                            Hints {
                                lead: "found".into(),
                                items,
                                more,
                            },
                        )
                    }
                    _ => plain("no_file", format!("no file {text}")),
                }
            }
            Why::NotRead(path) => plain(
                "not_read",
                format!(
                    "{path} is not read by V-NOC (it reads Python files that git does not ignore)"
                ),
            ),
            Why::Failed { path, message } => {
                plain("failed", format!("{path} could not be read: {message}"))
            }
            Why::Ambiguous { name, files } => {
                let shown: Vec<String> = files.iter().take(5).cloned().collect();
                NodeError::with_hints(
                    "ambiguous",
                    format!("{name} names {} files", files.len()),
                    ": ",
                    Hints {
                        lead: "name one of".into(),
                        more: files.len() - shown.len(),
                        items: shown,
                    },
                )
            }
            Why::PastEnd { path, lines } => plain(
                "past_end",
                format!(
                    "{path} has {lines} line{}",
                    if *lines == 1 { "" } else { "s" }
                ),
            ),
            Why::NoId(id) => plain("no_id", format!("no node has the ID {id}")),
            Why::Outside(path) => plain(
                "outside",
                format!("{path} is outside this project (root: {root})"),
            ),
        }
    }

    /// What `suggest` gives for `r`, as locations that can be pasted back (the path relative to the root): the shown ones, and how many more there are.
    /// A file's own defs (`Has`) are all shown; the others are cut at `shown`.
    fn suggestion(&self, r: &NodeRef, shown: usize) -> (Suggestion, Vec<String>, usize) {
        let corpus = self.corpus();
        let found = suggest(corpus, r, 50);
        let all: Vec<String> = found
            .indexes()
            .iter()
            .map(|&at| {
                let node = &corpus.nodes()[at];
                // a node's id is `<repo path>::<qualname>#n`: what follows the `::` is the part that does not depend on the root
                match node.id.split_once("::") {
                    Some((_, rest)) => format!("{}::{rest}", node.path),
                    None => node.path.clone(),
                }
            })
            .collect();
        let keep = if matches!(found, Suggestion::Has(_)) {
            all.len()
        } else {
            shown.min(all.len())
        };
        let more = all.len() - keep;
        (found, all.into_iter().take(keep).collect(), more)
    }
}

/// `main.py` of `api/main.py`.
fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// What to say when a reference found nothing and the library has no reason: it cannot happen (a miss always has one), and a sentence is better
/// than a panic.
fn unexplained(r: &NodeRef) -> Why {
    Why::NoFile(r.path().unwrap_or_default().to_string())
}

/// A repo path as shown below the opened folder.
pub fn shown(prefix: &str, repo_path: &str) -> String {
    if prefix.is_empty() {
        return repo_path.to_string();
    }
    repo_path
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(repo_path)
        .to_string()
}

/// The docs in the folder of the node with this ID, read from the disk: no index is needed.
pub fn docs_by_id(project: &Project, id: &NodeId) -> Vec<Doc> {
    UserData::new(project.git_root())
        .docs_of(id)
        .into_iter()
        .map(|doc| Doc {
            title: doc.title,
            path: doc_path(project, &doc.path),
            file: doc.file,
            abs: doc.path,
        })
        .collect()
}

/// What a tree node is called in every answer.
pub fn kind_word(kind: crate::index::Kind) -> &'static str {
    match kind {
        crate::index::Kind::Folder => "folder",
        crate::index::Kind::File => "file",
        crate::index::Kind::Class => "class",
        crate::index::Kind::Function => "function",
    }
}

/// The location a node is named by in every answer: `path::qualname#n` for a def, the path for a file or a folder; relative to the root.
pub fn location_of(node: &Node) -> String {
    node.location()
}

/// A def's lines as a person reads them: 1-based, both ends included.
pub fn lines_of(node: &Node) -> Option<(u32, u32)> {
    node.range.map(|r| (r.start.line + 1, r.end.line + 1))
}

/// The node's permanent ID, when it has one.
pub fn id_of(node: &Node) -> Option<NodeId> {
    node.node_id
        .as_deref()
        .and_then(|id| NodeId::parse(id).ok())
}

/// The arguments with `-` replaced by what standard input holds: one reference per line, blank lines skipped.
pub fn expand(args: &[String]) -> Result<Vec<String>, Failure> {
    let mut out = Vec::new();
    for arg in args {
        if arg != "-" {
            out.push(arg.clone());
            continue;
        }
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| Failure::Environment(format!("could not read standard input: {e}")))?;
        out.extend(
            input
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(String::from),
        );
    }
    Ok(out)
}

/// The absolute path of a node's file.
pub fn file_of(lookup: &Lookup, node: &Node) -> PathBuf {
    lookup.opened.project.open_dir().join(&node.path)
}

/// Runs `one` on each node the arguments name, and prints what it makes of each in the shape the flags ask for. An argument that names nothing
/// does not stop the others: its sentence goes to standard error **after** standard output was flushed, so a harness that merges the two streams
/// sees them in the order of the arguments; `--json` has one value with an entry for each argument and nothing on standard error. The exit is
/// `2` if any argument failed.
pub fn each<A: Answer>(
    command: &'static str,
    global: &Global,
    nodes: &[String],
    opened: &Opened,
    one: &dyn Fn(&Lookup, Target) -> Result<A, NodeError>,
) -> Result<(), Failure> {
    let args = expand(nodes)?;
    let lookup = Lookup::new(opened, args.len());
    emit(
        command,
        global,
        args.iter().map(|arg| {
            let answer = lookup.resolve(arg).and_then(|target| one(&lookup, target));
            (arg.as_str(), answer)
        }),
    )
}

/// Prints one answer per argument, as they come: see [`each`]. The answers are computed as the iterator is walked, so each is printed before the
/// next one is looked for, and an error lands between the answers on either side of it.
pub fn emit<'a, A: Answer>(
    command: &'static str,
    global: &Global,
    answers: impl Iterator<Item = (&'a str, Result<A, NodeError>)>,
) -> Result<(), Failure> {
    let (mut failed, mut printed) = (false, false);
    let mut results: Vec<Value> = Vec::new();
    for (arg, answer) in answers {
        match answer {
            Ok(done) if global.json => {
                let mut value = serde_json::to_value(&done).expect("an answer is JSON");
                if !arg.is_empty() {
                    value["arg"] = json!(arg);
                }
                results.push(value);
            }
            Ok(done) => {
                let text = match global.format {
                    Some(Format::Id) => format!("{}\n", done.id()),
                    Some(Format::Path) => done.paths().iter().map(|p| format!("{p}\n")).collect(),
                    // a blank line between answers
                    None if printed => format!("\n{}", done.text(global.quiet)),
                    None => done.text(global.quiet),
                };
                out::text(&text)?;
                printed = true;
            }
            Err(e) => {
                failed = true;
                if global.json {
                    results.push(json!({ "arg": arg, "error": e.json() }));
                } else {
                    e.print();
                }
            }
        }
    }
    if global.json {
        out::json(&json!({ "schema": 1, "command": command, "results": results }))?;
    }
    if failed {
        Err(Failure::Reported)
    } else {
        Ok(())
    }
}
