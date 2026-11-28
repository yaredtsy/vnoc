//! A route proposed for a tour (`docs/26-tours/02-commands.md` §11): the stops a person would read to understand a node, from what it calls (a function)
//! or what it holds (a class, a file, a folder), in reading order, with how big each depth of it is. **Pure**: it is given the merged call tree (from
//! [`crate::calls::merge`]) or the index's tree, and how to name a target; the driver is asked by the caller.
//!
//! The rules (the table "Decided for TR12"):
//!
//! - depth 0 is the node alone; reading order is depth first, the children in the order they are given (`merge`'s: the order of their first call);
//! - a node already on the route becomes a stop with `same_as` the first stop on it, and is **not walked again** (a second call, a recursive call);
//! - a call the index does not know is neither a stop nor walked ([`from_calls`] leaves it out);
//! - `line` is the first call site that is in the parent stop's file, as a 1-based line; none for containment;
//! - the outline is cut at [`MAX_STOPS`] stops in reading order; a size that would pass it says so.
//!
//! Minutes are a guess that is the same guess every time: [`STEPS_PER_STOP`] steps of [`SECONDS_PER_STEP`] seconds, rounded up to whole minutes.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};
use vnoc_domain::NodeKind;
use vnoc_driver_protocol::CallTarget;

use super::add::{MAX_STOPS, node_kind};
use super::find::place_of;
use crate::calls::{self, CallsError, Merged};
use crate::driver_host::DriverHost;
use crate::index::{Index, Kind, Node};
use crate::project::Project;

/// The guess of 02 §11: two steps for a stop (its intro and one block), twenty seconds a step.
pub const STEPS_PER_STOP: usize = 2;
pub const SECONDS_PER_STEP: usize = 20;

/// The deepest `--depth` takes.
pub const MAX_DEPTH: u32 = 6;

/// Which kind of route: what a function calls, or what a class, a file or a folder holds. Only the words of the size lines differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    Calls,
    Contents,
}

/// A node as a stop is made of it: what `add` and `show` say of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// the tree's id of the node: what finds it again
    pub id: String,
    /// as a person names it (`path::qualname`, a path), relative to the opened folder: what `add` and `plan` take
    pub node: String,
    pub name: String,
    pub kind: NodeKind,
    /// the stop's repo path (a folder with no `/` at the end)
    pub path: String,
    /// the first line of a def, 1-based
    pub first: Option<u32>,
}

/// What a tree node is as a stop, or `None` for the opened folder itself (it is not a stop).
pub fn known_of(node: &Node) -> Option<Known> {
    let (path, qualname, _) = place_of(node)?;
    // named as `show` names a stop: a def's qualname, a file's name, a folder's name with a `/`
    let last = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    let name = if !qualname.is_empty() {
        qualname
    } else if node.kind == Kind::Folder {
        format!("{last}/")
    } else {
        last.to_string()
    };
    Some(Known {
        id: node.id.clone(),
        node: node.location(),
        name,
        kind: node_kind(node.kind),
        path: path.to_string(),
        first: node.range.map(|r| r.start.line + 1),
    })
}

/// A node of the tree to walk: the stop it would be, the line it is called on, and what is under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub known: Known,
    /// a line of the parent's node: where this is called (calls only)
    pub line: Option<u32>,
    pub children: Vec<Item>,
}

/// The tree of what `root` calls, down to `levels` below it: a call `known` does not name is left out, with everything under it. `line` is the first
/// site of the call that is in the parent's file.
pub fn from_calls(
    root: &Merged,
    root_known: Known,
    levels: u32,
    known: &dyn Fn(&CallTarget) -> Option<Known>,
) -> Item {
    let children = if levels == 0 {
        Vec::new()
    } else {
        root.children
            .iter()
            .filter_map(|child| {
                let named = known(&child.target)?;
                let line = child
                    .call_sites
                    .iter()
                    .find(|site| site.file.is_some() && site.file == root.target.file)
                    .map(|site| site.range.start.line + 1);
                let mut item = from_calls(child, named, levels - 1, known);
                item.line = line;
                Some(item)
            })
            .collect()
    };
    Item {
        known: root_known,
        line: None,
        children,
    }
}

/// The tree of what `node` holds, down to `levels` below it: its children as the index has them (a folder's files and folders, a file's classes and
/// functions, a class's methods).
pub fn from_tree(node: &Node, levels: u32) -> Option<Item> {
    let known = known_of(node)?;
    let children = if levels == 0 {
        Vec::new()
    } else {
        node.children
            .iter()
            .filter_map(|child| from_tree(child, levels - 1))
            .collect()
    };
    Some(Item {
        known,
        line: None,
        children,
    })
}

/// One stop of the outline. `from` and `same_as` are numbers of the outline (`n`, from 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub n: usize,
    pub known: Known,
    pub from: Option<usize>,
    pub line: Option<u32>,
    pub same_as: Option<usize>,
}

impl Serialize for Entry {
    /// `{n, node, name, kind, path, first, from, line, same_as}`: every key there, `null` for what a stop has not
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut o = s.serialize_struct("Entry", 9)?;
        o.serialize_field("n", &self.n)?;
        o.serialize_field("node", &self.known.node)?;
        o.serialize_field("name", &self.known.name)?;
        o.serialize_field("kind", self.known.kind.as_str())?;
        o.serialize_field("path", &self.known.path)?;
        o.serialize_field("first", &self.known.first)?;
        o.serialize_field("from", &self.from)?;
        o.serialize_field("line", &self.line)?;
        o.serialize_field("same_as", &self.same_as)?;
        o.end()
    }
}

/// What one depth of the route adds up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Size {
    pub depth: u32,
    pub stops: usize,
    pub minutes: usize,
    /// the route at this depth passes [`MAX_STOPS`]: `stops` is the 40 it is cut at
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
}

impl Size {
    /// For a person, on one line: `9 stops · about 6 min` (`1 stop`, and `(cut)` when the route is cut).
    pub fn words(&self) -> String {
        format!(
            "{} {}{} · about {} min",
            self.stops,
            if self.stops == 1 { "stop" } else { "stops" },
            if self.cut { " (cut)" } else { "" },
            self.minutes
        )
    }
}

/// Whole minutes for this many stops: [`STEPS_PER_STOP`] steps of [`SECONDS_PER_STEP`] seconds each, rounded up (1 stop is `1`, 6 are `4`, 10 are `7`).
pub fn minutes(stops: usize) -> usize {
    (stops * STEPS_PER_STOP * SECONDS_PER_STEP).div_ceil(60)
}

/// A proposed route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub how: How,
    /// the node it is of, as a person names it
    pub node: String,
    /// its name, for the first size line
    pub name: String,
    pub depth: u32,
    /// every depth from 0 to `max(depth, 2)`
    pub sizes: Vec<Size>,
    /// the outline of `depth`
    pub stops: Vec<Entry>,
    /// the outline passes [`MAX_STOPS`] and ends there
    pub cut: bool,
}

/// The outline of the tree down to `depth`: depth first, a node already on it a `same_as` stop that is not walked, ended at [`MAX_STOPS`].
fn outline(root: &Item, depth: u32) -> (Vec<Entry>, bool) {
    struct Walk {
        stops: Vec<Entry>,
        at: HashMap<String, usize>,
        cut: bool,
    }
    fn visit(walk: &mut Walk, item: &Item, from: Option<usize>, levels: u32) {
        if walk.stops.len() >= MAX_STOPS {
            walk.cut = true;
            return;
        }
        let n = walk.stops.len() + 1;
        let same_as = walk.at.get(&item.known.id).copied();
        if same_as.is_none() {
            walk.at.insert(item.known.id.clone(), n);
        }
        walk.stops.push(Entry {
            n,
            known: item.known.clone(),
            from,
            line: item.line,
            same_as,
        });
        if same_as.is_some() || levels == 0 {
            return;
        }
        for child in &item.children {
            visit(walk, child, Some(n), levels - 1);
            if walk.cut {
                return;
            }
        }
    }
    let mut walk = Walk {
        stops: Vec::new(),
        at: HashMap::new(),
        cut: false,
    };
    visit(&mut walk, root, None, depth);
    (walk.stops, walk.cut)
}

/// What each depth from 0 to `last` adds up to. The tree must go at least that deep.
pub fn sizes(root: &Item, last: u32) -> Vec<Size> {
    (0..=last)
        .map(|d| {
            let (stops, cut) = outline(root, d);
            Size {
                depth: d,
                stops: stops.len(),
                minutes: minutes(stops.len()),
                cut,
            }
        })
        .collect()
}

/// The route of `root` at `depth`, with the sizes of every depth from 0 to `max(depth, 2)`. The tree must go at least that deep.
pub fn propose(root: &Item, how: How, depth: u32) -> Route {
    let sizes = sizes(root, depth.max(2));
    let (stops, cut) = outline(root, depth);
    Route {
        how,
        node: root.known.node.clone(),
        name: root.known.name.clone(),
        depth,
        sizes,
        stops,
        cut,
    }
}

/// Why the tree of a node could not be made.
#[derive(Debug)]
pub enum BuildError {
    /// the node is not one the driver can start a tree at (`vnoc/callTree`'s refusal)
    Invalid(String),
    Internal(String),
}

/// The tree of `node` for a route, `levels` deep: a function's from the driver's call tree (merged), a class's, a file's and a folder's from the index. The
/// one place `vnoc tour route` and `vnoc/tourAsk` both get it from. `bool`: the driver has no call trees, so the tree is the node alone. `root` is the node as
/// a stop ([`known_of`]); `tree` is the index's tree of the opened folder.
pub async fn build(
    project: &Project,
    index: &RwLock<Index>,
    host: &DriverHost,
    tree: &Node,
    node: &Node,
    root: Known,
    levels: u32,
) -> Result<(Item, How, bool), BuildError> {
    if node.kind != Kind::Function {
        let item = from_tree(node, levels).ok_or_else(|| {
            BuildError::Internal("the node is not a stop: name a folder in the project".to_string())
        })?;
        return Ok((item, How::Contents, false));
    }
    let mut no_calls = false;
    let merged = match calls::ask(project, index, host, &node.id, Some(levels)).await {
        Ok(answer) => Some(calls::merge(&answer.root)),
        // a driver with no call trees: the node alone, and a line that says so
        Err(CallsError::MissingCapability(_)) => {
            no_calls = true;
            None
        }
        Err(CallsError::Invalid(message)) => return Err(BuildError::Invalid(message)),
        Err(CallsError::Internal(message)) => return Err(BuildError::Internal(message)),
    };
    let known = |target: &CallTarget| {
        let location = vnoc_domain::LocationKey::new(
            project.repo_path(target.file.as_deref()?).ok()?,
            target.qualname.clone(),
            target.occurrence,
        )
        .ok()?;
        tree.find(&location.to_string()).and_then(known_of)
    };
    let item = match &merged {
        Some(merged) => from_calls(merged, root, levels, &known),
        None => Item {
            known: root,
            line: None,
            children: Vec::new(),
        },
    };
    Ok((item, How::Calls, no_calls))
}

fn icon(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Function => "ƒ",
        NodeKind::Class => "◆",
        NodeKind::File => "▪",
        NodeKind::Folder => "□",
        _ => "?",
    }
}

impl Route {
    /// What a depth adds, in the words the design offers (W13): `and what it calls`, `and what those call`, `and calls 3 deep`; for what a node holds,
    /// `and what is in it`, `and what is in those`, `and 3 deep`. Depth 0 is the node by its name.
    fn label(&self, depth: u32) -> String {
        match (self.how, depth) {
            (_, 0) => self.name.clone(),
            (How::Calls, 1) => "and what it calls".to_string(),
            (How::Calls, 2) => "and what those call".to_string(),
            (How::Calls, n) => format!("and calls {n} deep"),
            (How::Contents, 1) => "and what is in it".to_string(),
            (How::Contents, 2) => "and what is in those".to_string(),
            (How::Contents, n) => format!("and {n} deep"),
        }
    }

    /// The text of `route`: a line for each depth (what it adds, its stops, its minutes, and the flag that asks for it), then the outline: a row for each
    /// stop, indented by depth, with `line N` and `= k`.
    ///
    /// ```text
    /// checkout_order                           1 stop    about 1 min
    /// and what it calls                        6 stops   about 4 min      --depth 1
    ///
    ///  1  ƒ checkout_order      app/checkout.py:11
    ///  2    ƒ check_stock       app/inventory.py:12     line 13
    ///  4      ƒ check_stock     app/inventory.py:12     line 21   = 2
    /// ```
    pub fn text(&self) -> String {
        let mut text = String::new();
        let labels: Vec<String> = self.sizes.iter().map(|s| self.label(s.depth)).collect();
        let label_width = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let counts: Vec<String> = self
            .sizes
            .iter()
            .map(|s| {
                format!(
                    "{} {}{}",
                    s.stops,
                    if s.stops == 1 { "stop" } else { "stops" },
                    if s.cut { " (cut)" } else { "" }
                )
            })
            .collect();
        let count_width = counts.iter().map(|c| c.chars().count()).max().unwrap_or(0);
        for ((size, label), count) in self.sizes.iter().zip(&labels).zip(&counts) {
            let ask = if size.depth == 0 {
                String::new()
            } else {
                format!("      --depth {}", size.depth)
            };
            let row = format!(
                "{label:<label_width$}   {count:<count_width$}   about {} min{ask}",
                size.minutes
            );
            text.push_str(row.trim_end());
            text.push('\n');
        }
        text.push('\n');
        let depth_of = |e: &Entry| {
            let mut depth = 0;
            let mut at = e.from;
            while let Some(parent) = at {
                depth += 1;
                at = self.stops[parent - 1].from;
            }
            depth
        };
        let names: Vec<String> = self
            .stops
            .iter()
            .map(|e| {
                format!(
                    "{}{} {}",
                    "  ".repeat(depth_of(e)),
                    icon(&e.known.kind),
                    e.known.name
                )
            })
            .collect();
        let places: Vec<String> = self
            .stops
            .iter()
            .map(|e| match e.known.first {
                Some(first) => format!("{}:{first}", e.known.path),
                // a folder at the top says its name once
                None if e.known.kind == NodeKind::Folder
                    && format!("{}/", e.known.path) == e.known.name =>
                {
                    String::new()
                }
                None if e.known.kind == NodeKind::Folder => format!("{}/", e.known.path),
                None => e.known.path.clone(),
            })
            .collect();
        let tails: Vec<String> = self
            .stops
            .iter()
            .map(|e| {
                let mut tail = Vec::new();
                if let Some(line) = e.line {
                    tail.push(format!("line {line}"));
                }
                if let Some(same) = e.same_as {
                    tail.push(format!("= {same}"));
                }
                tail.join("   ")
            })
            .collect();
        let number_width = self.stops.len().to_string().len().max(2);
        let name_width = names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
        let place_width = places.iter().map(|p| p.chars().count()).max().unwrap_or(0);
        for (i, e) in self.stops.iter().enumerate() {
            let row = format!(
                "{:>number_width$}  {:<name_width$}  {:<place_width$}  {}",
                e.n, names[i], places[i], tails[i]
            );
            text.push_str(row.trim_end());
            text.push('\n');
        }
        if self.cut {
            text.push_str(&format!("… cut at {MAX_STOPS} stops\n"));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{DefKind, Position, Range};

    #[test]
    fn a_size_says_its_stops_and_minutes_on_one_line() {
        let size = |stops, cut| Size {
            depth: 1,
            stops,
            minutes: minutes(stops),
            cut,
        };
        assert_eq!(size(1, false).words(), "1 stop · about 1 min");
        assert_eq!(size(9, false).words(), "9 stops · about 6 min");
        assert_eq!(size(40, true).words(), "40 stops (cut) · about 27 min");
    }

    #[test]
    fn the_sizes_of_a_tree_are_the_ones_propose_gives() {
        let leaf = |id: &str| Item {
            known: known(id, NodeKind::Function),
            line: None,
            children: Vec::new(),
        };
        let mut root = leaf("a.py::f");
        root.children = vec![leaf("a.py::g"), leaf("a.py::h")];
        let all = sizes(&root, 6);
        assert_eq!(all.len(), 7);
        assert_eq!(
            all.iter().map(|s| s.stops).collect::<Vec<_>>(),
            [1, 3, 3, 3, 3, 3, 3]
        );
        assert_eq!(propose(&root, How::Calls, 1).sizes, all[..3].to_vec());
    }

    fn known(id: &str, kind: NodeKind) -> Known {
        let (path, name) = id.split_once("::").unwrap_or((id, id));
        Known {
            id: id.to_string(),
            node: id.to_string(),
            name: name.to_string(),
            kind,
            path: path.to_string(),
            first: Some(10),
        }
    }

    fn item(id: &str, line: Option<u32>, children: Vec<Item>) -> Item {
        Item {
            known: known(id, NodeKind::Function),
            line,
            children,
        }
    }

    fn ids(route: &Route) -> Vec<String> {
        route
            .stops
            .iter()
            .map(|e| {
                format!(
                    "{}:{}{}{}",
                    e.n,
                    e.known.id.rsplit("::").next().unwrap(),
                    e.from.map_or(String::new(), |f| format!("<{f}")),
                    e.same_as.map_or(String::new(), |s| format!("={s}")),
                )
            })
            .collect()
    }

    /// a calls b (line 3) and c (line 5); b calls d (line 2); c calls d (line 7) and a again (recursion), and e
    fn tree() -> Item {
        item(
            "a.py::a",
            None,
            vec![
                item(
                    "a.py::b",
                    Some(3),
                    vec![item(
                        "a.py::d",
                        Some(2),
                        vec![item("a.py::z", Some(1), vec![])],
                    )],
                ),
                item(
                    "a.py::c",
                    Some(5),
                    vec![
                        item("a.py::d", Some(7), vec![item("a.py::z", Some(1), vec![])]),
                        item("a.py::a", Some(8), vec![item("a.py::b", Some(3), vec![])]),
                        item("a.py::e", Some(9), vec![]),
                    ],
                ),
            ],
        )
    }

    #[test]
    fn depth_zero_is_the_node_alone_and_depth_one_its_calls_in_the_order_given() {
        let t = tree();
        assert_eq!(ids(&propose(&t, How::Calls, 0)), ["1:a"]);
        assert_eq!(ids(&propose(&t, How::Calls, 1)), ["1:a", "2:b<1", "3:c<1"]);
    }

    #[test]
    fn the_order_is_depth_first_and_a_node_met_again_is_a_same_as_stop_that_is_not_walked() {
        let t = tree();
        let r = propose(&t, How::Calls, 3);
        // d under c is `= 3`'s... d was first met under b (stop 3); under c it is a stop with same_as 3 and its `z` is not walked again.
        // The recursive call of a is `= 1`, and the `b` under it is not walked
        assert_eq!(
            ids(&r),
            [
                "1:a", "2:b<1", "3:d<2", "4:z<3", "5:c<1", "6:d<5=3", "7:a<5=1", "8:e<5"
            ]
        );
        // the `line` is each call's own
        let lines: Vec<Option<u32>> = r.stops.iter().map(|e| e.line).collect();
        assert_eq!(
            lines,
            [
                None,
                Some(3),
                Some(2),
                Some(1),
                Some(5),
                Some(7),
                Some(8),
                Some(9)
            ]
        );
    }

    #[test]
    fn the_sizes_are_for_every_depth_to_the_greater_of_the_depth_and_two_and_the_minutes_round_up()
    {
        let t = tree();
        let r = propose(&t, How::Calls, 0);
        assert_eq!(
            r.sizes
                .iter()
                .map(|s| (s.depth, s.stops))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 3), (2, 7)]
        );
        let r = propose(&t, How::Calls, 4);
        assert_eq!(r.sizes.len(), 5);
        assert_eq!(r.stops.len(), r.sizes[4].stops);
        // 1, 6 and 10 stops: 1, 4 and 7 minutes
        assert_eq!((minutes(1), minutes(6), minutes(10)), (1, 4, 7));
        assert_eq!((minutes(0), minutes(3), minutes(40)), (0, 2, 27));
    }

    #[test]
    fn a_route_that_passes_forty_is_cut_in_reading_order_and_its_size_says_so() {
        let leaves: Vec<Item> = (0..45)
            .map(|n| item(&format!("a.py::f{n}"), Some(n + 1), vec![]))
            .collect();
        let t = item("a.py::root", None, leaves);
        let r = propose(&t, How::Calls, 1);
        assert_eq!(r.stops.len(), 40);
        assert!(r.cut);
        assert_eq!(r.stops[39].known.id, "a.py::f38");
        assert_eq!(
            r.sizes[1],
            Size {
                depth: 1,
                stops: 40,
                minutes: 27,
                cut: true
            }
        );
        assert!(!r.sizes[0].cut);
        let text = r.text();
        assert!(text.contains("40 stops (cut)"), "{text}");
        assert!(text.ends_with("… cut at 40 stops\n"), "{text}");
        // exactly forty is not cut
        let t = item(
            "a.py::root",
            None,
            (0..39)
                .map(|n| item(&format!("a.py::f{n}"), None, vec![]))
                .collect(),
        );
        let r = propose(&t, How::Calls, 1);
        assert_eq!((r.stops.len(), r.cut), (40, false));
    }

    #[test]
    fn the_text_has_a_size_line_for_each_depth_and_rows_indented_by_depth_with_line_and_same_as() {
        let t = item(
            "app/checkout.py::checkout_order",
            None,
            vec![
                item("app/inventory.py::check_stock", Some(13), vec![]),
                item(
                    "app/inventory.py::reserve_stock",
                    Some(16),
                    vec![item("app/inventory.py::check_stock", Some(21), vec![])],
                ),
            ],
        );
        let text = propose(&t, How::Calls, 2).text();
        let want = "\
checkout_order        1 stop    about 1 min
and what it calls     3 stops   about 2 min      --depth 1
and what those call   4 stops   about 3 min      --depth 2

 1  ƒ checkout_order   app/checkout.py:10
 2    ƒ check_stock    app/inventory.py:10  line 13
 3    ƒ reserve_stock  app/inventory.py:10  line 16
 4      ƒ check_stock  app/inventory.py:10  line 21   = 2
";
        assert_eq!(text, want);
    }

    #[test]
    fn containment_labels_and_folders_and_files_have_no_first_line() {
        let folder = Item {
            known: Known {
                id: "app".into(),
                node: "app".into(),
                name: "app/".into(),
                kind: NodeKind::Folder,
                path: "app".into(),
                first: None,
            },
            line: None,
            children: vec![Item {
                known: Known {
                    id: "app/a.py".into(),
                    node: "app/a.py".into(),
                    name: "a.py".into(),
                    kind: NodeKind::File,
                    path: "app/a.py".into(),
                    first: None,
                },
                line: None,
                children: vec![item("app/a.py::f", None, vec![])],
            }],
        };
        let r = propose(&folder, How::Contents, 2);
        let text = r.text();
        assert!(text.contains("and what is in it"), "{text}");
        assert!(text.contains("and what is in those"), "{text}");
        assert!(text.contains(" 1  □ app/\n"), "{text}");
        assert!(text.contains(" 2    ▪ a.py  app/a.py\n"), "{text}");
        assert_eq!(propose(&folder, How::Contents, 3).sizes.len(), 4);
        assert!(
            propose(&folder, How::Contents, 3)
                .text()
                .contains("and 3 deep")
        );
    }

    // ---- from_calls

    fn at(line: u32) -> Range {
        let p = |character| Position { line, character };
        Range {
            start: p(0),
            end: p(5),
        }
    }

    fn target(file: Option<&str>, qualname: &str) -> CallTarget {
        CallTarget {
            file: file.map(str::to_string),
            qualname: qualname.to_string(),
            occurrence: 1,
            kind: DefKind::Function,
            id: None,
        }
    }

    fn merged(file: &str, qualname: &str, sites: &[(&str, u32)], children: Vec<Merged>) -> Merged {
        Merged {
            target: target(Some(file), qualname),
            call_count: sites.len().max(1) as u32,
            call_sites: sites
                .iter()
                .map(|(f, l)| crate::calls::Site {
                    file: Some((*f).to_string()),
                    range: at(*l),
                })
                .collect(),
            marks: vec![],
            children,
        }
    }

    fn named(target: &CallTarget) -> Option<Known> {
        let file = target.file.as_deref()?;
        // `lib.py` is not in the index
        if file == "lib.py" {
            return None;
        }
        Some(known(
            &format!("{file}::{}", target.qualname),
            NodeKind::Function,
        ))
    }

    #[test]
    fn a_function_that_calls_another_twice_is_one_stop_with_the_first_line() {
        // merge gave one node with two sites (0-based lines 4 and 9): the first is line 5
        let root = merged(
            "a.py",
            "main",
            &[],
            vec![merged(
                "a.py",
                "helper",
                &[("a.py", 4), ("a.py", 9)],
                vec![],
            )],
        );
        let item = from_calls(&root, known("a.py::main", NodeKind::Function), 2, &named);
        let r = propose(&item, How::Calls, 1);
        assert_eq!(ids(&r), ["1:main", "2:helper<1"]);
        assert_eq!(r.stops[1].line, Some(5));
    }

    #[test]
    fn a_call_the_index_does_not_know_is_no_stop_and_is_not_walked_and_a_site_in_another_file_gives_no_line()
     {
        let root = merged(
            "a.py",
            "main",
            &[],
            vec![
                merged(
                    "lib.py",
                    "dumps",
                    &[("a.py", 2)],
                    vec![merged("a.py", "callback", &[("lib.py", 7)], vec![])],
                ),
                // a def of another file, called from `b.py` (a merge by ID can put two parents in two files): the site is not in main's file
                merged("b.py", "other", &[("b.py", 3), ("a.py", 6)], vec![]),
                merged("b.py", "third", &[("b.py", 3)], vec![]),
            ],
        );
        let item = from_calls(&root, known("a.py::main", NodeKind::Function), 2, &named);
        let r = propose(&item, How::Calls, 2);
        assert_eq!(ids(&r), ["1:main", "2:other<1", "3:third<1"]);
        // `other` has a site in main's file (a.py, line 7): the first of those; `third` has none there
        assert_eq!(r.stops[1].line, Some(7));
        assert_eq!(r.stops[2].line, None);
    }

    #[test]
    fn a_recursive_call_is_a_same_as_stop_and_the_calls_under_it_are_not_walked() {
        // main -> fact -> fact (the driver shows it, marked recursive, with the calls it would make)
        let root = merged(
            "a.py",
            "main",
            &[],
            vec![merged(
                "a.py",
                "fact",
                &[("a.py", 1)],
                vec![merged(
                    "a.py",
                    "fact",
                    &[("a.py", 4)],
                    vec![merged("a.py", "mul", &[("a.py", 4)], vec![])],
                )],
            )],
        );
        let item = from_calls(&root, known("a.py::main", NodeKind::Function), 3, &named);
        let r = propose(&item, How::Calls, 3);
        assert_eq!(ids(&r), ["1:main", "2:fact<1", "3:fact<2=2"]);
        assert_eq!(r.stops[2].line, Some(5));
    }

    #[test]
    fn a_tree_is_only_built_as_deep_as_asked() {
        let root = merged(
            "a.py",
            "main",
            &[],
            vec![merged(
                "a.py",
                "b",
                &[("a.py", 1)],
                vec![merged("a.py", "c", &[("a.py", 2)], vec![])],
            )],
        );
        let item = from_calls(&root, known("a.py::main", NodeKind::Function), 1, &named);
        assert_eq!(ids(&propose(&item, How::Calls, 6)), ["1:main", "2:b<1"]);
    }

    #[test]
    fn an_entry_is_json_with_every_key_and_null_for_what_it_has_not() {
        let r = propose(&tree(), How::Calls, 1);
        let json = serde_json::to_value(&r.stops[1]).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"n": 2, "node": "a.py::b", "name": "b", "kind": "function",
                "path": "a.py", "first": 10, "from": 1, "line": 3, "same_as": null})
        );
    }
}
