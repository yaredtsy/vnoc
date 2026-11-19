//! `vnoc tree [<node>…]` (03 §4): the structure of the project as rows, printed. The rows are `index::tree_view`'s; this file resolves the
//! starts (as `show` does), computes the `--where` set, and prints. Two spaces a level, no box drawing.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::json;
use vnoc_domain::LocationKey;
use vnoc_search::matches;

use super::args::{Depth, KindArg, TreeArgs};
use super::nodes::{Lookup, NodeError, Target, expand, id_of, kind_word};
use super::out::{self, Render};
use super::project::open_from;
use super::{Failure, Format, Global, style};
use crate::index::{Counts, Kind, KindSel, Lookups, Node, Row, TreeOptions, tree_view};

pub async fn run(global: &Global, args: &TreeArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let starts = if args.nodes.is_empty() {
        Vec::new()
    } else {
        expand(&args.nodes)?
    };
    let lookup = Lookup::new(&opened, starts.len());

    let kinds: Vec<KindSel> = args.kind.iter().map(|k| select(*k)).collect();
    let keep = args
        .where_
        .as_deref()
        .map(|query| wanted_ids(&lookup, query));
    // a filter asks which nodes, not how deep: with --where or --kind the depth is all, unless it is given
    let filtered = keep.is_some() || !kinds.is_empty();
    let depth = match args.depth {
        Some(Depth::Levels(n)) => Some(n),
        Some(Depth::All) => None,
        None if filtered => None,
        None => Some(2),
    };

    let index = &opened.index;
    let params = |node: &Node| {
        let key: LocationKey = node.id.parse().ok()?;
        index.read().unwrap().def_at(&key).map(|def| def.params)
    };
    let titles = |node: &Node| {
        lookup
            .docs_of(id_of(node))
            .into_iter()
            .map(|d| d.title)
            .collect::<Vec<_>>()
    };
    let lookups = Lookups {
        params: args
            .sig
            .then_some(&params as &dyn Fn(&Node) -> Option<Vec<String>>),
        titles: args
            .docs
            .then_some(&titles as &dyn Fn(&Node) -> Vec<String>),
    };
    let style = Style {
        flat: args.flat,
        lines: args.lines,
        filtered,
    };

    // where each start is: the root when none is named
    let mut failed = false;
    let mut remaining = args.limit;
    let mut printed = false;
    let mut all = Tree::new(style);
    // one start: its rows (printed at once in text; kept for the one JSON value otherwise)
    let mut one = |start: &Node| -> Result<Tree, Failure> {
        let options = TreeOptions {
            depth,
            kinds: (!kinds.is_empty()).then_some(&kinds[..]),
            keep: keep.as_ref(),
            limit: remaining,
            only_wanted: style.flat,
            parent_kind: lookup.tree.parent_kind_of(&start.id),
        };
        let view = tree_view(start, &options, &lookups);
        remaining = remaining.saturating_sub(view.rows.len());
        let part = Tree {
            rows: view.rows,
            cut: view.cut,
            ..Tree::new(style)
        };
        if global.json {
            return Ok(part);
        }
        if let Some(format) = global.format {
            let lines: String = part
                .rows
                .iter()
                .map(|row| match format {
                    Format::Id => format!("{}\n", row.location),
                    Format::Path => format!("{}\n", file_of(row)),
                })
                .collect();
            out::text(&lines)?;
        } else {
            let text = part.text(global.quiet);
            out::text(&if printed { format!("\n{text}") } else { text })?;
            printed = true;
        }
        Ok(part)
    };

    let merge = |all: &mut Tree, part: Tree| {
        all.rows.extend(part.rows);
        all.cut += part.cut;
    };
    if starts.is_empty() {
        let part = one(&lookup.tree)?;
        merge(&mut all, part);
    }
    for arg in &starts {
        match lookup.resolve(arg) {
            Ok(Target::Node(node)) => {
                let part = one(node)?;
                merge(&mut all, part);
            }
            Ok(Target::Missing(missing)) => {
                failed = true;
                let error = NodeError::new(
                    "missing",
                    format!(
                        "{} is Missing: there is no tree (vnoc show {} reads its docs)",
                        missing.was, missing.id
                    ),
                );
                all.report(arg, error, global);
            }
            Err(error) => {
                failed = true;
                all.report(arg, error, global);
            }
        }
    }
    if global.json {
        out::json(&all)?;
    }
    if failed {
        Err(Failure::Reported)
    } else {
        Ok(())
    }
}

fn select(kind: KindArg) -> KindSel {
    match kind {
        KindArg::Folder => KindSel::Folder,
        KindArg::File => KindSel::File,
        KindArg::Class => KindSel::Class,
        KindArg::Function => KindSel::Function,
        KindArg::Method => KindSel::Method,
    }
}

/// The ids of the tree's nodes the query finds, by name or by one of their docs: doc 20's `matches`, asked once for every id.
fn wanted_ids(lookup: &Lookup, text: &str) -> HashSet<String> {
    let query = vnoc_search::parse(text);
    for note in &query.notes {
        out::note(note);
    }
    let mut ids: Vec<&str> = Vec::new();
    let mut stack = vec![&lookup.tree];
    while let Some(node) = stack.pop() {
        // the opened folder itself is not in the corpus
        if node.id != "/" {
            ids.push(&node.id);
        }
        stack.extend(&node.children);
    }
    let hits = matches(lookup.corpus(), &query, None, &ids);
    ids.into_iter()
        .zip(hits)
        .filter(|(_, hit)| *hit)
        .map(|(id, _)| id.to_string())
        .collect()
}

/// The source file of a row: the part of its location before `::`.
fn file_of(row: &Row) -> &str {
    row.location.split("::").next().unwrap_or(&row.location)
}

/// How the text of a tree is laid out (none of it is in the JSON).
#[derive(Clone, Copy)]
struct Style {
    flat: bool,
    lines: bool,
    /// a filter was given: an empty answer says no node matches
    filtered: bool,
}

/// What `tree` answers: the rows in order, flat with their depth, and how many the limit left out.
#[derive(Serialize)]
struct Tree {
    schema: u32,
    command: &'static str,
    rows: Vec<Row>,
    cut: usize,
    /// arguments that named nothing, when some did not (the text has them on standard error, in place)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<serde_json::Value>,
    #[serde(skip)]
    style: Style,
}

impl Tree {
    fn new(style: Style) -> Tree {
        Tree {
            schema: 1,
            command: "tree",
            rows: Vec::new(),
            cut: 0,
            errors: Vec::new(),
            style,
        }
    }

    /// An argument that named nothing: its sentence on standard error in place, or an entry of `errors` with `--json`.
    fn report(&mut self, arg: &str, error: NodeError, global: &Global) {
        if global.json {
            self.errors
                .push(json!({ "arg": arg, "error": error.json() }));
        } else {
            error.print();
        }
    }
}

/// `1 file`, `3 files`: the number grouped by thousands, and bold in a terminal.
fn plural(n: usize, one: &str, many: &str) -> String {
    style::count(n, one, many)
}

/// `4 files · 3 classes · 31 functions · 6 docs inside`, with no part that is 0. The docs are those of the nodes **below**, so they are not read as
/// the node's own, which a row says with `· 2 docs` (a node's own docs follow the counts: `1 class · 3 functions · 2 docs`).
fn counts_text(c: &Counts) -> String {
    let docs = (c.docs > 0).then(|| format!("{} inside", plural(c.docs, "doc", "docs")));
    [
        (c.folders, "folder", "folders"),
        (c.files, "file", "files"),
        (c.classes, "class", "classes"),
        (c.functions, "function", "functions"),
    ]
    .iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, one, many)| plural(*n, one, many))
    .chain(docs)
    .collect::<Vec<_>>()
    .join(" · ")
}

impl Tree {
    fn row_text(&self, row: &Row) -> String {
        // the row's parts, after the name: what is below it, its lines, then its own docs. One rule for the separators: a part after the first is
        // introduced by ` · `, and the node's own docs always are (`· 2 docs`, also when they are the only part)
        let mut parts: Vec<String> = Vec::new();
        if let Some(counts) = &row.counts {
            parts.push(counts_text(counts));
        }
        if self.style.lines
            && let Some([a, b]) = row.lines
        {
            parts.push(style::paint(style::dim(), format!("{a}-{b}")));
        }
        let params = row
            .params
            .as_ref()
            .map(|p| format!("({})", p.join(", ")))
            .unwrap_or_default();
        let mut tail = parts.join(" · ");
        if row.docs > 0 {
            let own = plural(row.docs, "doc", "docs");
            tail.push_str(&if tail.is_empty() {
                format!("· {own}")
            } else {
                format!(" · {own}")
            });
        }
        if !tail.is_empty() {
            tail.insert_str(0, "  ");
        }
        if self.style.flat {
            // the location, then what else there is: no indent, so each line stands alone
            let sig = if params.is_empty() {
                String::new()
            } else {
                format!("  {params}")
            };
            let titles = if row.titles.is_empty() {
                String::new()
            } else {
                format!(": {}", row.titles.join(" | "))
            };
            return format!(
                "{}  {}{sig}{tail}{titles}\n",
                style::paint(style::dim(), kind_word(row.kind)),
                style::paint(style::location(), &row.location)
            );
        }
        let indent = "  ".repeat(row.depth);
        let label = match row.kind {
            Kind::Folder => format!("{}/", row.name),
            Kind::File => row.name.clone(),
            Kind::Class => format!("{} {}", style::paint(style::dim(), "class"), row.name),
            Kind::Function => format!("{}{params}", row.name),
        };
        let mut text = format!("{indent}{label}{tail}\n");
        for title in &row.titles {
            text.push_str(&format!(
                "{indent}  {} {}\n",
                style::paint(style::dim(), "doc:"),
                style::paint(style::title(), title)
            ));
        }
        text
    }
}

impl Render for Tree {
    fn text(&self, quiet: bool) -> String {
        let mut text: String = self.rows.iter().map(|row| self.row_text(row)).collect();
        if self.rows.is_empty() && self.cut == 0 && self.style.filtered {
            text.push_str("no nodes match\n");
        }
        if self.cut > 0 {
            let rows = if self.cut == 1 { "row" } else { "rows" };
            let cut = style::grouped(self.cut);
            let said = if quiet {
                format!("… {cut} more {rows}")
            } else {
                format!("… {cut} more {rows} (narrow with a path, --depth or --where)")
            };
            text.push_str(&format!("{}\n", style::paint(style::dim(), said)));
        }
        text
    }
}
