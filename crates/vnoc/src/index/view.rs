//! The rows of `vnoc tree` (docs/21-agent-cli/03-read-commands.md §4): the tree `Index::tree` gives, cut to a depth, narrowed to some kinds and
//! to the nodes a query finds, and limited to a number of rows.
//!
//! Pure: a [`Node`], some options, and two lookups for what the tree's `Node` does not carry (a def's parameters, a doc's title). The tree does
//! not grow for them and nothing here reads the index, the disk or a query: `--where` arrives as a set of ids that was computed outside.

use std::collections::HashSet;

use serde::Serialize;

use super::tree::{Kind, Node};

/// What `--kind` names. `Function` is every function, methods included; `Method` is a function whose parent is a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindSel {
    Folder,
    File,
    Class,
    Function,
    Method,
}

pub struct TreeOptions<'a> {
    /// how many levels below the start: `Some(0)` is the start alone, `None` is all of them
    pub depth: Option<usize>,
    /// only these kinds get a row (their ancestors do too, so it reads as a tree); `None` is every kind
    pub kinds: Option<&'a [KindSel]>,
    /// only these ids get a row (and their ancestors); `None` is every node
    pub keep: Option<&'a HashSet<String>>,
    /// at most this many rows
    pub limit: usize,
    /// no row for a node that is only an ancestor of a wanted one: what a flat listing wants (the matches, one per row), where a tree wants the
    /// ancestors to read as a tree. It is the same set of rows when no kind or query narrows the tree.
    pub only_wanted: bool,
    /// the kind of the start's parent, to tell a method from a function when the start is one
    pub parent_kind: Option<Kind>,
}

/// A lookup of something the tree's nodes do not carry.
pub type Lookup<'a, T> = &'a dyn Fn(&Node) -> T;

/// What the tree's nodes do not carry, asked only for the rows that are given.
#[derive(Default)]
pub struct Lookups<'a> {
    /// a function's parameters (`--sig`)
    pub params: Option<Lookup<'a, Option<Vec<String>>>>,
    /// the titles of a node's docs (`--docs`)
    pub titles: Option<Lookup<'a, Vec<String>>>,
}

/// How much is below a node that the depth cut hid.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub folders: usize,
    pub files: usize,
    pub classes: usize,
    pub functions: usize,
    /// docs on the nodes below, not on the node itself
    pub docs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    pub depth: usize,
    pub kind: Kind,
    /// a function in a class
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub method: bool,
    /// the row is there only because something below it is wanted (the kinds or the query did not pick it)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub context: bool,
    pub name: String,
    pub location: String,
    /// 1-based, both ends included
    pub lines: Option<[u32; 2]>,
    pub docs: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub titles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
    /// what is below, when it is not shown: the depth cut it, or it is the start folder or file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
}

pub struct View {
    pub rows: Vec<Row>,
    /// how many rows the limit left out
    pub cut: usize,
}

/// The rows of the tree below `start`, in the tree's own order (folders first, then files, defs in source order), `start` first.
pub fn tree_view(start: &Node, options: &TreeOptions, lookups: &Lookups) -> View {
    let mut rows = rows_of(start, 0, options.parent_kind, options, lookups);
    let cut = rows.len().saturating_sub(options.limit);
    rows.truncate(options.limit);
    View { rows, cut }
}

/// The rows of `node` and what is under it. Empty when neither the node nor anything under it is wanted.
fn rows_of(
    node: &Node,
    depth: usize,
    parent_kind: Option<Kind>,
    options: &TreeOptions,
    lookups: &Lookups,
) -> Vec<Row> {
    let below = options.depth.is_none_or(|limit| depth < limit);
    let children: Vec<Row> = if below {
        node.children
            .iter()
            .flat_map(|child| rows_of(child, depth + 1, Some(node.kind), options, lookups))
            .collect()
    } else {
        Vec::new()
    };
    let itself = wanted(node, parent_kind, options);
    if !itself && (children.is_empty() || options.only_wanted) {
        return children;
    }
    let method = node.kind == Kind::Function && parent_kind == Some(Kind::Class);
    let cut_here = !below && !node.children.is_empty();
    let start_container = depth == 0 && matches!(node.kind, Kind::Folder | Kind::File);
    let counts = (cut_here || (start_container && !node.children.is_empty()))
        .then(|| counts_below(node))
        .filter(|c| *c != Counts::default());
    let row = Row {
        depth,
        kind: node.kind,
        method,
        context: !itself,
        name: node.name.clone(),
        location: node.location(),
        lines: node.range.map(|r| [r.start.line + 1, r.end.line + 1]),
        docs: node.docs,
        titles: match lookups.titles {
            Some(titles) if node.docs > 0 => titles(node),
            _ => Vec::new(),
        },
        params: match lookups.params {
            Some(params) if node.kind == Kind::Function => params(node),
            _ => None,
        },
        counts,
    };
    std::iter::once(row).chain(children).collect()
}

/// Whether the options want this node itself (not only because of what is under it).
fn wanted(node: &Node, parent_kind: Option<Kind>, options: &TreeOptions) -> bool {
    let is = |sel: &KindSel| match sel {
        KindSel::Folder => node.kind == Kind::Folder,
        KindSel::File => node.kind == Kind::File,
        KindSel::Class => node.kind == Kind::Class,
        KindSel::Function => node.kind == Kind::Function,
        KindSel::Method => node.kind == Kind::Function && parent_kind == Some(Kind::Class),
    };
    options.kinds.is_none_or(|kinds| kinds.iter().any(is))
        && options.keep.is_none_or(|keep| keep.contains(&node.id))
}

/// What is under `node`, not counting `node` itself.
fn counts_below(node: &Node) -> Counts {
    let mut counts = Counts::default();
    for child in &node.children {
        let [folders, files, classes, functions] = child.counts();
        counts.folders += folders;
        counts.files += files;
        counts.classes += classes;
        counts.functions += functions;
        counts.docs += total_docs(child);
    }
    counts
}

fn total_docs(node: &Node) -> usize {
    node.docs + node.children.iter().map(total_docs).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{Position, Range};

    fn node(id: &str, kind: Kind, name: &str, docs: usize, children: Vec<Node>) -> Node {
        let at = |line| Position { line, character: 0 };
        let (path, qualname) = match id.split_once("::") {
            Some((path, qualname)) => (path.to_string(), Some(qualname.to_string())),
            // the opened folder itself has no path
            None if id == "/" => (String::new(), None),
            None => (id.to_string(), None),
        };
        Node {
            id: id.into(),
            kind,
            name: name.into(),
            path,
            qualname,
            range: matches!(kind, Kind::Class | Kind::Function).then(|| Range {
                start: at(3),
                end: at(9),
            }),
            syntax_error: false,
            node_id: None,
            docs,
            link: None,
            children,
        }
    }

    /// root / billing / charge.py (class Charger: method send, method retry; function top) and billing/tax.py (rate, with a nested def)
    fn tree() -> Node {
        let f = |id: &str, name: &str, docs| node(id, Kind::Function, name, docs, vec![]);
        node(
            "/",
            Kind::Folder,
            "repo",
            0,
            vec![node(
                "billing",
                Kind::Folder,
                "billing",
                1,
                vec![
                    node(
                        "billing/charge.py",
                        Kind::File,
                        "charge.py",
                        0,
                        vec![
                            node(
                                "billing/charge.py::Charger",
                                Kind::Class,
                                "Charger",
                                2,
                                vec![
                                    f("billing/charge.py::Charger.send", "send", 0),
                                    f("billing/charge.py::Charger.retry", "retry", 1),
                                ],
                            ),
                            f("billing/charge.py::top", "top", 0),
                        ],
                    ),
                    node(
                        "billing/tax.py",
                        Kind::File,
                        "tax.py",
                        0,
                        vec![node(
                            "billing/tax.py::rate",
                            Kind::Function,
                            "rate",
                            0,
                            vec![f("billing/tax.py::rate.<locals>.inner", "inner", 0)],
                        )],
                    ),
                ],
            )],
        )
    }

    fn options() -> TreeOptions<'static> {
        TreeOptions {
            depth: None,
            kinds: None,
            keep: None,
            limit: 200,
            only_wanted: false,
            parent_kind: None,
        }
    }

    fn names(view: &View) -> Vec<String> {
        view.rows
            .iter()
            .map(|r| format!("{}{}", "  ".repeat(r.depth), r.name))
            .collect()
    }

    fn run(options: &TreeOptions) -> View {
        tree_view(&tree(), options, &Lookups::default())
    }

    #[test]
    fn all_of_it_in_the_trees_order() {
        let view = run(&options());
        assert_eq!(
            names(&view),
            [
                "repo",
                "  billing",
                "    charge.py",
                "      Charger",
                "        send",
                "        retry",
                "      top",
                "    tax.py",
                "      rate",
                "        inner"
            ]
        );
        assert_eq!(view.cut, 0);
    }

    #[test]
    fn depth_counts_levels_below_the_start() {
        let none = run(&TreeOptions {
            depth: Some(0),
            ..options()
        });
        assert_eq!(names(&none), ["repo"]);
        let two = run(&TreeOptions {
            depth: Some(2),
            ..options()
        });
        assert_eq!(
            names(&two),
            ["repo", "  billing", "    charge.py", "    tax.py"]
        );
        // a start that is not the root: the depth counts from it
        let charge = tree().find("billing/charge.py").unwrap().clone();
        let one = tree_view(
            &charge,
            &TreeOptions {
                depth: Some(1),
                ..options()
            },
            &Lookups::default(),
        );
        assert_eq!(names(&one), ["charge.py", "  Charger", "  top"]);
        assert_eq!(one.rows[0].depth, 0);
    }

    #[test]
    fn a_cut_node_carries_the_counts_of_what_is_below_it() {
        let view = run(&TreeOptions {
            depth: Some(1),
            ..options()
        });
        let billing = &view.rows[1];
        assert_eq!(
            billing.counts,
            Some(Counts {
                folders: 0,
                files: 2,
                classes: 1,
                functions: 5,
                docs: 3
            })
        );
        // the start folder shows them too, as the sketch of 03 §4 has it, and a def that was not cut shows none
        assert!(view.rows[0].counts.is_some());
        let full = run(&options());
        assert!(
            full.rows[1..].iter().all(|r| r.counts.is_none()),
            "{:?}",
            full.rows
        );
        // a file cut by the depth: what is under it
        let files = run(&TreeOptions {
            depth: Some(2),
            ..options()
        });
        assert_eq!(
            files.rows[2].counts,
            Some(Counts {
                folders: 0,
                files: 0,
                classes: 1,
                functions: 3,
                docs: 3
            })
        );
        // nothing below: no counts
        let leaf = run(&TreeOptions {
            depth: Some(5),
            ..options()
        });
        assert!(leaf.rows.last().unwrap().counts.is_none());
    }

    #[test]
    fn kinds_keep_their_ancestors() {
        let kinds = [KindSel::Class];
        let view = run(&TreeOptions {
            kinds: Some(&kinds),
            ..options()
        });
        assert_eq!(
            names(&view),
            ["repo", "  billing", "    charge.py", "      Charger"]
        );
        // a class below the depth is out of reach: nothing is kept, and not even the start is a row
        let two = run(&TreeOptions {
            kinds: Some(&kinds),
            depth: Some(2),
            ..options()
        });
        assert!(two.rows.is_empty());
    }

    #[test]
    fn a_method_is_a_function_in_a_class_and_a_function_is_every_function() {
        let methods = [KindSel::Method];
        let view = run(&TreeOptions {
            kinds: Some(&methods),
            ..options()
        });
        assert_eq!(
            names(&view),
            [
                "repo",
                "  billing",
                "    charge.py",
                "      Charger",
                "        send",
                "        retry"
            ]
        );
        assert!(view.rows.iter().filter(|r| r.method).count() == 2);
        let functions = [KindSel::Function];
        let view = run(&TreeOptions {
            kinds: Some(&functions),
            ..options()
        });
        let kept: Vec<&str> = view
            .rows
            .iter()
            .filter(|r| r.kind == Kind::Function)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(kept, ["send", "retry", "top", "rate", "inner"]);
        // a start that is a method knows its parent by `parent_kind`
        let send = tree()
            .find("billing/charge.py::Charger.send")
            .unwrap()
            .clone();
        let alone = tree_view(
            &send,
            &TreeOptions {
                kinds: Some(&methods),
                parent_kind: Some(Kind::Class),
                ..options()
            },
            &Lookups::default(),
        );
        assert_eq!(names(&alone), ["send"]);
        assert!(alone.rows[0].method);
    }

    #[test]
    fn where_keeps_the_ids_it_is_given_and_their_ancestors() {
        let keep: HashSet<String> = [
            "billing/charge.py::top".to_string(),
            "billing/tax.py::rate".to_string(),
        ]
        .into();
        let view = run(&TreeOptions {
            keep: Some(&keep),
            ..options()
        });
        assert_eq!(
            names(&view),
            [
                "repo",
                "  billing",
                "    charge.py",
                "      top",
                "    tax.py",
                "      rate"
            ]
        );
        let nothing: HashSet<String> = HashSet::new();
        assert!(
            run(&TreeOptions {
                keep: Some(&nothing),
                ..options()
            })
            .rows
            .is_empty()
        );
    }

    #[test]
    fn undocumented_functions_in_place() {
        // `--where 'no:doc kind:fn'`: the set is computed outside; with the kind it is what `--flat` prints
        let keep: HashSet<String> = [
            "billing/charge.py::Charger.send",
            "billing/charge.py::top",
            "billing/tax.py::rate",
            "billing/tax.py::rate.<locals>.inner",
        ]
        .map(String::from)
        .into();
        let functions = [KindSel::Function];
        let view = run(&TreeOptions {
            keep: Some(&keep),
            kinds: Some(&functions),
            ..options()
        });
        let rows: Vec<&str> = view
            .rows
            .iter()
            .filter(|r| r.kind == Kind::Function)
            .map(|r| r.location.as_str())
            .collect();
        assert_eq!(
            rows,
            [
                "billing/charge.py::Charger.send",
                "billing/charge.py::top",
                "billing/tax.py::rate",
                "billing/tax.py::rate.<locals>.inner"
            ]
        );
    }

    #[test]
    fn only_wanted_leaves_out_the_rows_that_are_ancestors_and_nothing_else() {
        let keep: HashSet<String> = [
            "billing/charge.py::top".to_string(),
            "billing/tax.py::rate".to_string(),
        ]
        .into();
        let tree = run(&TreeOptions {
            keep: Some(&keep),
            ..options()
        });
        let context: Vec<&str> = tree
            .rows
            .iter()
            .filter(|r| r.context)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(context, ["repo", "billing", "charge.py", "tax.py"]);
        let flat = run(&TreeOptions {
            keep: Some(&keep),
            only_wanted: true,
            ..options()
        });
        let flat_names: Vec<&str> = flat.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(flat_names, ["top", "rate"]);
        assert!(flat.rows.iter().all(|r| !r.context));
        // the limit counts the rows that are given, not the ancestors that were left out
        let one = run(&TreeOptions {
            keep: Some(&keep),
            only_wanted: true,
            limit: 1,
            ..options()
        });
        assert_eq!((one.rows.len(), one.cut), (1, 1));
        // with nothing narrowing the tree every node is wanted, and it is the same rows
        let all = run(&TreeOptions {
            only_wanted: true,
            ..options()
        });
        assert_eq!(names(&all), names(&run(&options())));
    }

    #[test]
    fn the_limit_says_how_many_rows_it_left_out() {
        let view = run(&TreeOptions {
            limit: 4,
            ..options()
        });
        assert_eq!(view.rows.len(), 4);
        assert_eq!(view.cut, 6);
        let exact = run(&TreeOptions {
            limit: 10,
            ..options()
        });
        assert_eq!((exact.rows.len(), exact.cut), (10, 0));
        let none = run(&TreeOptions {
            limit: 0,
            ..options()
        });
        assert_eq!((none.rows.len(), none.cut), (0, 10));
    }

    #[test]
    fn rows_say_where_a_def_is_and_the_lookups_are_asked_only_for_rows_given() {
        let asked = std::cell::RefCell::new(Vec::new());
        let params = |n: &Node| {
            asked.borrow_mut().push(n.name.clone());
            Some(vec!["self".to_string(), "raw".to_string()])
        };
        let titles = |n: &Node| vec![format!("about {}", n.name)];
        let view = tree_view(
            &tree(),
            &TreeOptions {
                depth: Some(3),
                ..options()
            },
            &Lookups {
                params: Some(&params),
                titles: Some(&titles),
            },
        );
        let top = view.rows.iter().find(|r| r.name == "top").unwrap();
        assert_eq!(top.location, "billing/charge.py::top");
        assert_eq!(top.lines, Some([4, 10]), "1-based, both ends included");
        assert_eq!(
            top.params.as_deref(),
            Some(&["self".to_string(), "raw".to_string()][..])
        );
        assert!(top.titles.is_empty(), "no docs: no titles asked for");
        let charger = view.rows.iter().find(|r| r.name == "Charger").unwrap();
        assert_eq!(charger.titles, ["about Charger"]);
        assert!(charger.params.is_none(), "a class has no signature row");
        // `inner` is below the depth: never asked for
        assert!(
            !asked.borrow().contains(&"inner".to_string()),
            "{:?}",
            asked.borrow()
        );
        assert_eq!(view.rows[0].location, ".");
    }

    #[test]
    fn parent_kind_of_finds_the_kind_of_the_parent() {
        let tree = tree();
        assert_eq!(
            tree.parent_kind_of("billing/charge.py::Charger.send"),
            Some(Kind::Class)
        );
        assert_eq!(
            tree.parent_kind_of("billing/tax.py::rate.<locals>.inner"),
            Some(Kind::Function)
        );
        assert_eq!(tree.parent_kind_of("billing"), Some(Kind::Folder));
        assert_eq!(tree.parent_kind_of("/"), None);
        assert_eq!(tree.parent_kind_of("nope"), None);
    }
}
