//! Arguments → nodes (docs/21-agent-cli/02-naming-a-node.md): every command that takes `<node>` goes through here.
//!
//! The parse and the lookup are `node_ref`'s, the same two functions the search box uses; this file adds what only a command line has: the current
//! folder, `-` for standard input, several nodes in one call with each error printed in its place, and the sentences of §4.

use std::cell::OnceCell;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use vnoc_domain::NodeId;
use vnoc_search::{Corpus, DocIndex};

use super::project::Opened;
use super::{Failure, Format, Global, out};
use crate::index::{Index, Node};
use crate::node_ref::{self, Found, NodeRef, Why, is_absolute};
use crate::search::{self, CorpusSlot};
use crate::suggest::suggest;
use crate::userdata::UserData;

/// An argument that named nothing, or nothing that has what the command needs: the case (`why`, a stable word for `--json`) and the sentence.
#[derive(Debug)]
pub struct NodeError {
    pub why: &'static str,
    pub message: String,
}

/// A doc of a node: its title, and its file relative to the git root (`.vnoc/nodes/…`).
pub struct Doc {
    pub title: String,
    pub path: String,
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

/// What one command makes of one node, in each of the shapes it can be printed in.
pub struct Rendered {
    pub text: String,
    pub json: Value,
    /// for `--format id`: the location, or the ID of a Missing node
    pub id: String,
    /// for `--format path`: the source file
    pub path: String,
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
            return Err(NodeError {
                why: "empty",
                message: "an empty reference names no node".into(),
            });
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
                        let why = index.why_not(project, r);
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

    /// A node of the tree, unless the reference is a line past the end of its file.
    fn found(&self, index: &Index, r: &NodeRef, id: &str) -> Option<Result<Target<'_>, NodeError>> {
        let node = self.tree.find(id)?;
        if matches!(r, NodeRef::FileLine { .. })
            && node.kind == crate::index::Kind::File
            && let past @ Why::PastEnd { .. } = index.why_not(&self.opened.project, r)
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
        if let Some(joined) = joined {
            let why = index.why_not(project, &joined);
            if matches!(
                why,
                Why::NoDef { .. } | Why::Failed { .. } | Why::PastEnd { .. }
            ) {
                return (why, joined);
            }
        }
        (index.why_not(project, &written), written)
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
        let Some(id) = id else { return Vec::new() };
        let git_root = self.opened.project.git_root();
        UserData::new(git_root)
            .docs_of(&id)
            .into_iter()
            .map(|doc| Doc {
                title: doc.title,
                path: doc
                    .path
                    .strip_prefix(git_root)
                    .unwrap_or(&doc.path)
                    .to_string_lossy()
                    .replace('\\', "/"),
            })
            .collect()
    }

    /// The corpus for "did you mean", made on the first miss.
    fn corpus(&self) -> &Corpus {
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
        let (slug, message) = match why {
            Why::NoDef { path, name } if name.is_empty() => {
                ("no_def", format!("no node at {path}"))
            }
            Why::NoDef { path, name } => {
                let hints = self.locations(&r.clone().with_path(path.clone()), 3);
                let said = format!("no node at {path}::{name}");
                ("no_def", with_hints(said, " · did you mean: ", &hints))
            }
            Why::NoFile(text) => {
                // `OrderService.parse_order` parses as a path: with no folder in it, it may be a name
                let hints = match r {
                    NodeRef::Path(_) if !text.contains('/') && !text.contains('\\') => {
                        self.locations(r, 5)
                    }
                    _ => Vec::new(),
                };
                if hints.is_empty() {
                    ("no_file", format!("no file {text}"))
                } else {
                    let said = format!("\"{text}\" names no file");
                    ("no_file", with_hints(said, " · found: ", &hints))
                }
            }
            Why::NotRead(path) => (
                "not_read",
                format!(
                    "{path} is not read by V-NOC (it reads Python files that git does not ignore)"
                ),
            ),
            Why::Failed { path, message } => {
                ("failed", format!("{path} could not be read: {message}"))
            }
            Why::Ambiguous { name, files } => {
                let shown: Vec<&str> = files.iter().take(5).map(String::as_str).collect();
                let more = match files.len() - shown.len() {
                    0 => String::new(),
                    n => format!(" · and {n} more"),
                };
                (
                    "ambiguous",
                    format!(
                        "{name} names {} files: {}{more}",
                        files.len(),
                        shown.join(" · ")
                    ),
                )
            }
            Why::PastEnd { path, lines } => (
                "past_end",
                format!(
                    "{path} has {lines} line{}",
                    if *lines == 1 { "" } else { "s" }
                ),
            ),
            Why::NoId(id) => ("no_id", format!("no node has the ID {id}")),
            Why::Outside(path) => (
                "outside",
                format!("{path} is outside this project (root: {root})"),
            ),
        };
        NodeError { why: slug, message }
    }

    /// Up to `limit` locations `suggest` gives for `r`, each as it can be pasted back (the path relative to the root).
    fn locations(&self, r: &NodeRef, limit: usize) -> Vec<String> {
        let corpus = self.corpus();
        suggest(corpus, r, limit)
            .into_iter()
            .map(|at| {
                let node = &corpus.nodes()[at];
                // a node's id is `<repo path>::<qualname>#n`: what follows the `::` is the part that does not depend on the root
                match node.id.split_once("::") {
                    Some((_, rest)) => format!("{}::{rest}", node.path),
                    None => node.path.clone(),
                }
            })
            .collect()
    }
}

fn with_hints(said: String, lead: &str, hints: &[String]) -> String {
    if hints.is_empty() {
        said
    } else {
        format!("{said}{lead}{}", hints.join(" · "))
    }
}

/// A repo path as shown below the opened folder.
fn shown(prefix: &str, repo_path: &str) -> String {
    if prefix.is_empty() {
        return repo_path.to_string();
    }
    repo_path
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(repo_path)
        .to_string()
}

/// The location a node is named by in every answer: `path::qualname#n` for a def, the path for a file or a folder; relative to the root.
pub fn location_of(node: &Node) -> String {
    match node.id.split_once("::") {
        Some((_, rest)) => format!("{}::{rest}", node.path),
        None => node.path.clone(),
    }
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
pub fn each(
    command: &'static str,
    global: &Global,
    nodes: &[String],
    opened: &Opened,
    one: &dyn Fn(&Lookup, Target) -> Result<Rendered, NodeError>,
) -> Result<(), Failure> {
    let args = expand(nodes)?;
    let lookup = Lookup::new(opened, args.len());
    let (mut failed, mut printed) = (false, false);
    let mut results: Vec<Value> = Vec::new();
    for arg in &args {
        match lookup.resolve(arg).and_then(|target| one(&lookup, target)) {
            Ok(done) if global.json => {
                let mut value = done.json;
                value["arg"] = json!(arg);
                results.push(value);
            }
            Ok(done) => {
                let text = match global.format {
                    Some(Format::Id) => format!("{}\n", done.id),
                    Some(Format::Path) => format!("{}\n", done.path),
                    // a blank line between answers
                    None if printed => format!("\n{}", done.text),
                    None => done.text,
                };
                out::text(&text)?;
                printed = true;
            }
            Err(e) => {
                failed = true;
                if global.json {
                    results.push(
                        json!({ "arg": arg, "error": { "why": e.why, "message": e.message } }),
                    );
                } else {
                    out::error(&e.message);
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
