//! `vnoc find '<query>'` (03 §5): doc 20's search, printed. `search::prepare` and `vnoc_search::run` are what `vnoc/search` calls, so the order
//! cannot differ from the box's; this file adds the command's own limits, the join of each row to the tree (both lines, the ID), and the text.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Serialize;
use vnoc_search::Limits;

use super::args::FindArgs;
use super::nodes::{Lookup, doc_path, id_of, kind_word};
use super::out::{self, Render};
use super::project::open_from;
use super::show::{MissingView, missing_view};
use super::{Failure, Format, Global, style};
use crate::node_ref::Found;
use crate::protocol::SearchNode;
use crate::search::{self, CorpusSlot};

/// The command's own limits: a model reads every row, where the box's 200 and 50 suit a list a person scrolls (03 §5).
const NODES: usize = 20;
const DOCS: usize = 10;

pub async fn run(global: &Global, args: &FindArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let text = lookup.relative_to_cwd(&args.query.join(" "));
    let (corpus_slot, doc_slot): (CorpusSlot, Mutex<_>) = (Mutex::new(None), Mutex::new(None));
    let found = search::prepare(
        &opened.project,
        &opened.index,
        &corpus_slot,
        &doc_slot,
        &text,
    );
    let limits = match args.limit {
        Some(n) => Limits { nodes: n, docs: n },
        None => Limits {
            nodes: NODES,
            docs: DOCS,
        },
    };
    let results = vnoc_search::run(&found.corpus, &found.query, found.exact, limits);
    let answer = search::answer(found.revision, &found.corpus, &found.query, results);
    for note in &answer.notes {
        out::note(note);
    }

    let reference = match found.reference {
        Some(Found::Missing(id)) => Some(missing_view(lookup.describe_missing(id))),
        _ => None,
    };
    let nodes: Vec<NodeRow> = answer
        .nodes
        .into_iter()
        .map(|hit| {
            let node = lookup.tree.find(&hit.id);
            NodeRow {
                location: node.map_or_else(|| hit.path.clone(), |n| n.location()),
                lines: node
                    .and_then(|n| n.range)
                    .map(|r| [r.start.line + 1, r.end.line + 1]),
                node_id: node.and_then(id_of).map(|id| id.to_string()),
                hit,
            }
        })
        .collect();
    let docs: Vec<DocRow> = answer
        .docs
        .into_iter()
        .map(|doc| {
            let location = match &doc.node {
                Some(id) => lookup.tree.find(id).map(|n| n.location()),
                None => None,
            }
            .unwrap_or_else(|| anchor(&doc.node_path, &doc.node_name, doc.node_kind));
            DocRow {
                path: doc_path(&opened.project, std::path::Path::new(&doc.doc_path)),
                location,
                node: doc.node,
                node_id: doc.node_id,
                node_kind: doc.node_kind.map(kind_word),
                missing: doc.missing,
                file: doc.file,
                title: doc.title,
                title_match: doc.title_match,
                snippet: doc.snippet,
                snippet_match: doc.snippet_match,
                tags: doc.tags,
                author: doc.author,
            }
        })
        .collect();
    let find = Find {
        schema: 1,
        command: "find",
        reference,
        nodes: if args.docs { Vec::new() } else { nodes },
        nodes_total: answer.nodes_total,
        docs: if args.nodes { Vec::new() } else { docs },
        docs_total: answer.docs_total,
        notes: answer.notes,
        show_nodes: !args.docs,
        show_docs: !args.nodes,
    };
    match global.format {
        Some(format) => out::text(&find.lines(format)),
        None => out::render(&find, global),
    }
}

/// Where a node that is not in the tree is, as its record says: `path::name`, or the path for a file or a folder.
fn anchor(path: &str, name: &str, kind: Option<crate::index::Kind>) -> String {
    use crate::index::Kind;
    match kind {
        Some(Kind::File | Kind::Folder) => path.to_string(),
        _ if name.is_empty() => path.to_string(),
        _ => format!("{path}::{name}"),
    }
}

/// A node the query found: the protocol's row, and what the tree adds to it.
#[derive(Serialize)]
struct NodeRow {
    #[serde(flatten)]
    hit: SearchNode,
    location: String,
    /// 1-based, both ends included; `null` for a file or a folder
    lines: Option<[u32; 2]>,
    node_id: Option<String>,
}

/// A doc the query found: the protocol's row with the doc's file as the commands print it (relative to the root, absolute when `.vnoc/` is not
/// under it), and the node's location.
#[derive(Serialize)]
struct DocRow {
    /// the tree's id of its node; `null` when the node is Missing or its code was not read
    node: Option<String>,
    /// where its node is (or was)
    location: String,
    node_id: String,
    node_kind: Option<&'static str>,
    missing: bool,
    file: String,
    path: String,
    title: String,
    title_match: Vec<(u32, u32)>,
    snippet: String,
    snippet_match: Vec<(u32, u32)>,
    /// the doc's `tags:`; left out of `--json` when it has none
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    /// the doc's `author:`; left out of `--json` when it has none
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
}

/// What `find` answers.
#[derive(Serialize)]
struct Find {
    schema: u32,
    command: &'static str,
    /// the node a query that is an ID named, when its code is gone
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<MissingView>,
    nodes: Vec<NodeRow>,
    nodes_total: usize,
    docs: Vec<DocRow>,
    docs_total: usize,
    notes: Vec<String>,
    #[serde(skip)]
    show_nodes: bool,
    #[serde(skip)]
    show_docs: bool,
}

/// `text` with the parts `ranges` name (UTF-16 units of `text`, sorted and merged, as the search gives them) between `**`; in a terminal they are bold
/// yellow and the asterisks are not written (the one place the words differ: a pipe has no other way to show a hit).
fn highlight(text: &str, ranges: &[(u32, u32)]) -> String {
    let marks: Vec<u32> = ranges
        .iter()
        .filter(|(start, end)| start < end)
        .flat_map(|&(start, end)| [start, end])
        .collect();
    let painted = style::on(style::Stream::Out);
    let (mut out, mut at, mut next) = (String::new(), 0u32, 0usize);
    // marks come in pairs: the first opens a hit, the second closes it
    let mut marked = |out: &mut String, at: u32| {
        while next < marks.len() && marks[next] == at {
            match (painted, next % 2 == 0) {
                (false, _) => out.push_str("**"),
                (true, true) => out.push_str(&format!("{}", style::hit().render())),
                (true, false) => out.push_str(&format!("{}", style::hit().render_reset())),
            }
            next += 1;
        }
    };
    for c in text.chars() {
        marked(&mut out, at);
        out.push(c);
        at += c.len_utf16() as u32;
    }
    marked(&mut out, at);
    out
}

impl Find {
    /// `--format id`: the location of each node found, then of the node of each doc found, once each; `path`: the file of each node and the doc
    /// files. Nothing else, so it pipes into `vnoc show -` and `vnoc code -`.
    fn lines(&self, format: Format) -> String {
        let mut seen = HashSet::new();
        let mut text = String::new();
        let mut add = |line: String| {
            if seen.insert(line.clone()) {
                text.push_str(&line);
                text.push('\n');
            }
        };
        for row in &self.nodes {
            match format {
                Format::Id => add(row.location.clone()),
                Format::Path => add(row.hit.path.clone()),
            }
        }
        for row in &self.docs {
            match format {
                // a Missing node, or one whose code was not read, has no location to paste
                Format::Id if row.node.is_some() => add(row.location.clone()),
                Format::Id => {}
                Format::Path => add(row.path.clone()),
            }
        }
        text
    }

    fn group_header(name: &str, shown: usize, total: usize) -> String {
        let name = style::paint(style::dim(), name);
        if shown == total {
            format!("{name} ({})\n", style::num(total))
        } else {
            format!("{name} ({} of {})\n", style::num(shown), style::num(total))
        }
    }

    fn more(shown: usize, total: usize, quiet: bool) -> String {
        if shown >= total || quiet {
            return String::new();
        }
        format!(
            "  {}\n",
            style::paint(
                style::dim(),
                format!(
                    "… {} more (narrow the query, or --limit N)",
                    style::grouped(total - shown)
                )
            )
        )
    }
}

impl Render for Find {
    fn text(&self, quiet: bool) -> String {
        let mut text = String::new();
        if let Some(reference) = &self.reference {
            text.push_str(&reference.text(quiet));
        }
        if self.show_nodes && !self.nodes.is_empty() {
            text.push_str(&Find::group_header(
                "nodes",
                self.nodes.len(),
                self.nodes_total,
            ));
            for row in &self.nodes {
                text.push_str(&format!(
                    "  {}  {}",
                    style::paint(style::dim(), kind_word(row.hit.kind)),
                    style::paint(style::location(), &row.location)
                ));
                if let Some([a, b]) = row.lines {
                    text.push_str(&format!(
                        "   {}",
                        style::paint(style::dim(), format!("lines {a}-{b}"))
                    ));
                }
                if let Some(id) = &row.node_id {
                    text.push_str(&format!("   {} {id}", style::paint(style::dim(), "ID")));
                }
                if row.hit.docs > 0 {
                    text.push_str(&format!(
                        "   · {}",
                        style::count(row.hit.docs, "doc", "docs")
                    ));
                }
                text.push('\n');
            }
            text.push_str(&Find::more(self.nodes.len(), self.nodes_total, quiet));
        }
        if self.show_docs && !self.docs.is_empty() {
            text.push_str(&Find::group_header(
                "docs",
                self.docs.len(),
                self.docs_total,
            ));
            for row in &self.docs {
                let missing = if row.missing {
                    format!("{} ", style::paint(style::alert(), "MISSING"))
                } else {
                    String::new()
                };
                let tags = if row.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        "  {}",
                        style::paint(style::dim(), format!("[{}]", row.tags.join(", ")))
                    )
                };
                text.push_str(&format!(
                    "  {}{tags}  {} {missing}{}\n",
                    style::paint(style::title(), &row.title),
                    style::paint(style::dim(), "on"),
                    style::paint(style::location(), &row.location)
                ));
                if !row.snippet.is_empty() {
                    text.push_str(&format!(
                        "    {}\n",
                        highlight(&row.snippet, &row.snippet_match)
                    ));
                }
                text.push_str(&format!("    {}\n", style::paint(style::dim(), &row.path)));
            }
            text.push_str(&Find::more(self.docs.len(), self.docs_total, quiet));
        }
        let rows = (self.show_nodes && !self.nodes.is_empty())
            || (self.show_docs && !self.docs.is_empty());
        if !rows && self.reference.is_none() {
            text.push_str("no matches\n");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_are_put_between_stars_in_utf16_units() {
        assert_eq!(
            highlight("the gateway drops", &[(4, 11)]),
            "the **gateway** drops"
        );
        assert_eq!(highlight("retry", &[(0, 5)]), "**retry**");
        assert_eq!(
            highlight("a retry b retry", &[(2, 7), (10, 15)]),
            "a **retry** b **retry**"
        );
        // a letter outside the BMP is two units: the range after it is shifted by two, not one
        assert_eq!(highlight("😀 retry", &[(3, 8)]), "😀 **retry**");
        assert_eq!(highlight("plain", &[]), "plain");
        assert_eq!(
            highlight("plain", &[(2, 2)]),
            "plain",
            "an empty range marks nothing"
        );
    }

    #[test]
    fn the_header_says_the_total_only_when_rows_were_cut() {
        assert_eq!(Find::group_header("nodes", 3, 3), "nodes (3)\n");
        assert_eq!(Find::group_header("nodes", 20, 143), "nodes (20 of 143)\n");
        assert_eq!(
            Find::more(20, 143, false),
            "  … 123 more (narrow the query, or --limit N)\n"
        );
        assert_eq!(Find::more(20, 143, true), "");
        assert_eq!(Find::more(3, 3, false), "");
    }
}
