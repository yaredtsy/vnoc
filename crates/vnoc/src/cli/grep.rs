//! `vnoc grep '<regex>' [<path>…]` (03 §6): the library's `grep`, printed. The pattern is compiled first, so a bad one is exit `2` before anything
//! is built; the paths are resolved as node arguments are, so every form and every error is `show`'s.

use std::collections::HashSet;

use regex::RegexBuilder;
use serde::Serialize;
use serde_json::json;
use vnoc_domain::RepoPath;

use super::args::GrepArgs;
use super::nodes::{Lookup, NodeError, Target, doc_path, expand, kind_word};
use super::out::{self, Render};
use super::project::open_from;
use super::{Failure, Format, Global, style};
use crate::grep::{DocHits, HitLine, NodeHits, grep, grep_docs};

pub async fn run(global: &Global, args: &GrepArgs) -> Result<(), Failure> {
    let re = RegexBuilder::new(&args.pattern)
        .case_insensitive(args.ignore_case)
        .build()
        .map_err(|e| Failure::Usage(e.to_string()))?;
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 0);

    // where to look: the files and folders the arguments name, as a repo path each; none means every file
    let (mut scope, mut errors) = (Vec::new(), Vec::new());
    for arg in expand(&args.paths)? {
        match lookup.resolve(&arg) {
            Ok(Target::Node(node)) => {
                // a def is looked for in its whole file: its id is `path::qualname`, a file's or a folder's is the path
                let path = node.id.split("::").next().unwrap_or(&node.id);
                if let Ok(path) = RepoPath::new(path) {
                    scope.push(path);
                }
            }
            Ok(Target::Missing(missing)) => errors.push((
                arg,
                NodeError::new(
                    "missing",
                    format!("{} is Missing: there is no code to search", missing.was),
                ),
            )),
            Err(error) => errors.push((arg, error)),
        }
    }
    if !errors.is_empty() {
        // a path that names nothing is not widened to the whole project: nothing is searched
        if global.json {
            let list: Vec<_> = errors
                .iter()
                .map(|(arg, e)| json!({ "arg": arg, "error": e.json() }))
                .collect();
            out::json(&json!({ "schema": 1, "command": "grep", "errors": list }))?;
        } else {
            for (_, error) in &errors {
                error.print();
            }
        }
        return Err(Failure::Reported);
    }

    let index = opened.index.read().unwrap();
    let project = &opened.project;
    let style = Style { names: args.names };
    let answer = if args.docs {
        let found = grep_docs(project, &index, &re);
        let total = found.docs.len();
        let docs: Vec<DocRow> = found
            .docs
            .into_iter()
            .map(|doc| DocRow::of(doc, &lookup))
            .collect();
        Grep::docs(args, docs, total, found.hits, style)
    } else {
        let found = grep(project, &index, &re, &scope);
        let total = found.nodes.len();
        Grep::code(args, found.nodes, total, found.hits, style)
    };
    match global.format {
        Some(format) => out::text(&answer.lines(format)),
        None => out::render(&answer, global),
    }
}

/// How the text is laid out (not in the JSON).
#[derive(Clone, Copy)]
struct Style {
    names: bool,
}

/// A doc with hits, with its file as the commands print it.
#[derive(Serialize)]
struct DocRow {
    title: String,
    path: String,
    node_id: String,
    /// where its node is; `null` for a Missing node and one whose code was not read
    location: Option<String>,
    hits: usize,
    shown: Vec<HitLine>,
    more: usize,
}

impl DocRow {
    fn of(doc: DocHits, lookup: &Lookup) -> DocRow {
        DocRow {
            path: doc_path(&lookup.opened.project, &doc.path),
            title: doc.title,
            node_id: doc.node_id,
            location: doc.location,
            hits: doc.hits,
            shown: doc.shown,
            more: doc.more,
        }
    }
}

/// What `grep` answers: the nodes (or with `--docs` the docs) with hits, the first `limit` of them, and how many there are in all.
#[derive(Serialize)]
struct Grep {
    schema: u32,
    command: &'static str,
    pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nodes: Option<Vec<NodeHits>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docs: Option<Vec<DocRow>>,
    /// how many nodes (docs) have a hit, those cut by the limit too
    total: usize,
    /// how many hits there are in all
    hits: usize,
    /// how many nodes (docs) the limit left out
    cut: usize,
    #[serde(skip)]
    style: Style,
}

impl Grep {
    fn code(
        args: &GrepArgs,
        mut nodes: Vec<NodeHits>,
        total: usize,
        hits: usize,
        style: Style,
    ) -> Grep {
        nodes.truncate(args.limit);
        Grep {
            schema: 1,
            command: "grep",
            pattern: args.pattern.clone(),
            cut: total - nodes.len(),
            nodes: Some(nodes),
            docs: None,
            total,
            hits,
            style,
        }
    }

    fn docs(
        args: &GrepArgs,
        mut docs: Vec<DocRow>,
        total: usize,
        hits: usize,
        style: Style,
    ) -> Grep {
        docs.truncate(args.limit);
        Grep {
            schema: 1,
            command: "grep",
            pattern: args.pattern.clone(),
            cut: total - docs.len(),
            nodes: None,
            docs: Some(docs),
            total,
            hits,
            style,
        }
    }

    /// `--format id`: the location of each node (of the node a doc is on, when the tree has it); `path`: the file of each, once.
    fn lines(&self, format: Format) -> String {
        let mut seen = HashSet::new();
        let mut text = String::new();
        let mut add = |line: &str| {
            if seen.insert(line.to_string()) {
                text.push_str(line);
                text.push('\n');
            }
        };
        for node in self.nodes.iter().flatten() {
            add(match format {
                Format::Id => &node.location,
                Format::Path => &node.path,
            });
        }
        for doc in self.docs.iter().flatten() {
            match (format, &doc.location) {
                (Format::Id, Some(location)) => add(location),
                (Format::Id, None) => add(&doc.node_id),
                (Format::Path, _) => add(&doc.path),
            }
        }
        text
    }
}

fn hits_word(n: usize) -> String {
    style::count(n, "hit", "hits")
}

/// `  44  text`, the numbers right-aligned to the widest of these lines.
fn numbered(lines: &[HitLine]) -> String {
    let width = lines
        .iter()
        .map(|l| l.line.to_string().len())
        .max()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            let number = style::paint(style::dim(), format!("{:>width$}", l.line));
            format!("  {number}  {}\n", l.text)
        })
        .collect()
}

fn more_hits(more: usize) -> String {
    if more == 0 {
        String::new()
    } else {
        let said = format!(
            "… {} more {}",
            style::grouped(more),
            if more == 1 { "hit" } else { "hits" }
        );
        format!("  {}\n", style::paint(style::dim(), said))
    }
}

impl Render for Grep {
    fn text(&self, quiet: bool) -> String {
        let mut text = String::new();
        for node in self.nodes.iter().flatten() {
            text.push_str(&format!(
                "{}  {}",
                style::paint(style::dim(), kind_word(node.kind)),
                style::paint(style::location(), &node.location)
            ));
            if let Some([a, b]) = node.lines {
                text.push_str(&format!(
                    "   {}",
                    style::paint(style::dim(), format!("lines {a}-{b}"))
                ));
            }
            if node.docs > 0 {
                text.push_str(&format!("   · {}", style::count(node.docs, "doc", "docs")));
            }
            text.push_str(&format!("   {}", hits_word(node.hits)));
            if node.outside_def {
                text.push_str(&format!(
                    "   {}",
                    style::paint(style::dim(), "(outside any def)")
                ));
            }
            text.push('\n');
            if !self.style.names {
                text.push_str(&numbered(&node.shown));
                text.push_str(&more_hits(node.more));
            }
        }
        for doc in self.docs.iter().flatten() {
            let on = match &doc.location {
                Some(location) => style::paint(style::location(), location),
                None => format!("{} {}", style::paint(style::dim(), "ID"), doc.node_id),
            };
            // the doc's path is a long fact of its own (03 §11): on the line under the row
            text.push_str(&format!(
                "{}  {}  {} {on}   {}\n  {}\n",
                style::paint(style::dim(), "doc"),
                style::paint(style::title(), &doc.title),
                style::paint(style::dim(), "on"),
                hits_word(doc.hits),
                style::paint(style::dim(), &doc.path)
            ));
            if !self.style.names {
                text.push_str(&numbered(&doc.shown));
                text.push_str(&more_hits(doc.more));
            }
        }
        if self.total == 0 {
            return "no matches\n".to_string();
        }
        let what = if self.docs.is_some() { "docs" } else { "nodes" };
        if self.cut > 0 {
            let cut = style::grouped(self.cut);
            let said = if quiet {
                format!("… {cut} more {what}")
            } else {
                format!(
                    "… {cut} more {what} (narrow with a path or the pattern, or --limit N; {} in all)",
                    style::grouped(self.total)
                )
            };
            text.push_str(&format!("{}\n", style::paint(style::dim(), said)));
        }
        if !quiet {
            let one = if what == "nodes" { "node" } else { "doc" };
            // the summary stands off from the rows by one blank line (03 §11)
            text.push('\n');
            text.push_str(&format!(
                "{} · {}\n",
                style::count(self.total, one, what),
                hits_word(self.hits)
            ));
        }
        text
    }
}
