//! "Did you mean": the nodes a name that found nothing was probably meant for (`docs/21-agent-cli/02-naming-a-node.md` §4).
//!
//! Search has no typo tolerance (doc 20), so this is a few passes over the corpus, and **the file the name was given with is looked at first**. When
//! the reference names a file the index has:
//!
//! 1. **in that file**: the word pass (a name given without its class: `orders/service.py::parse_order` is `OrderService.parse_order`), then
//!    `nearest_in` the file with no length floor (`man` is one letter from `main`);
//! 2. **in other files, strong matches only**: a def whose last name is the name given (the right name in the wrong file), or within `nearest`'s edits
//!    with its floor. Never a word hit: a name that only *contains* the letters (`man` in `DriverManager`) is no suggestion;
//! 3. else, if the file has at most [`FEW`] defs, **all of them** (what the file has, which is more use than nothing);
//! 4. else nothing.
//!
//! A qualified name with **no** file (`OrderService.parse_order`) is a search for a name: the project-wide word pass, then `nearest`. Only classes and
//! functions are suggested.
//!
//! The query is **built as a value**, never formatted into a string and parsed: a path with a space in it would not survive that.

use vnoc_search::{Corpus, Kind, KindFilter, Limits, Query, Scope, nearest, nearest_in, run};

use crate::node_ref::NodeRef;

/// Shorter names are too near to too many others across a project: no suggestion from there, as `nearest` has it.
const SHORTEST: usize = 3;

/// A file with this many defs or fewer is listed whole when nothing is near the name.
pub const FEW: usize = 10;

/// What a name that found nothing was probably meant for. Each is indexes into [`Corpus::nodes`]; the command says them in its own words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suggestion {
    /// `did you mean`: near the name given, the named file's own first
    Near(Vec<usize>),
    /// a name with no file, looked up as a name in the whole project: `found`
    Found(Vec<usize>),
    /// nothing is near, and the file is small: every def it has, in reading order (`main.py has: …`)
    Has(Vec<usize>),
    Nothing,
}

impl Suggestion {
    /// The indexes, whatever kind it is.
    pub fn indexes(&self) -> &[usize] {
        match self {
            Suggestion::Near(at) | Suggestion::Found(at) | Suggestion::Has(at) => at,
            Suggestion::Nothing => &[],
        }
    }
}

/// What `r` was probably meant for, at most `limit` (`Has` is the whole small file, whatever the limit). `r`'s path is what the file is looked for by,
/// as the corpus has it (relative to the opened folder: a command makes it so first). A reference with no name to look for (an ID, a file and a line, a
/// path with a folder in it) gets nothing.
pub fn suggest(corpus: &Corpus, r: &NodeRef, limit: usize) -> Suggestion {
    let Some((name, file)) = asked(r) else {
        return Suggestion::Nothing;
    };
    if name.is_empty() || limit == 0 {
        return Suggestion::Nothing;
    }
    let long_enough = name.chars().count() >= SHORTEST;
    // a file the index has: it has a node (the file's own, or a def)
    let file = file.filter(|f| corpus.nodes().iter().any(|n| n.path == *f));
    let Some(file) = file else {
        // a name asked for as a name: the whole project
        if !long_enough {
            return Suggestion::Nothing;
        }
        let found = words(corpus, &name, None, limit);
        let found = if found.is_empty() {
            nearest(corpus, &name, None, limit)
        } else {
            found
        };
        return if found.is_empty() {
            Suggestion::Nothing
        } else {
            Suggestion::Found(found)
        };
    };

    // 1. in the file
    if long_enough {
        let found = words(corpus, &name, Some(file), limit);
        if !found.is_empty() {
            return Suggestion::Near(found);
        }
    }
    let found = nearest_in(corpus, &name, Scope::File(file), false, limit);
    if !found.is_empty() {
        return Suggestion::Near(found);
    }
    // 2. in other files: the same name, or a few letters away; never a word that only contains it
    let mut strong: Vec<usize> = corpus
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            is_def(n.kind) && n.path != file && last_name(&n.name).eq_ignore_ascii_case(&name)
        })
        .map(|(at, _)| at)
        .collect();
    for at in nearest_in(corpus, &name, Scope::Elsewhere(file), true, limit) {
        if !strong.contains(&at) {
            strong.push(at);
        }
    }
    strong.truncate(limit);
    if !strong.is_empty() {
        return Suggestion::Near(strong);
    }
    // 3. what a small file has
    let defs: Vec<usize> = corpus
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, n)| is_def(n.kind) && n.path == file)
        .map(|(at, _)| at)
        .collect();
    if !defs.is_empty() && defs.len() <= FEW {
        return Suggestion::Has(defs);
    }
    Suggestion::Nothing
}

fn is_def(kind: Kind) -> bool {
    matches!(kind, Kind::Class | Kind::Function)
}

/// `parse_order` of `OrderService.parse_order`. This is not a name to show (a node's shown name is its outline's `Def::name`): it is what a
/// name is **looked up** by, on both sides, the typed one and each node's own. A Bend `Nat.double` is looked up as `double`, as a Python
/// `C.double` is, and both find it.
fn last_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The name to look for (the last part of the qualified name) and the file it was asked in.
fn asked(r: &NodeRef) -> Option<(String, Option<&str>)> {
    match r {
        NodeRef::Location { path, rest } => {
            let name = rest
                .rsplit_once('#')
                .map_or(rest.as_str(), |(whole, _)| whole);
            let name = name.rsplit('.').next().unwrap_or(name);
            Some((
                name.to_string(),
                (!path.is_empty()).then_some(path.as_str()),
            ))
        }
        // `OrderService.parse_order`: a qualified name with no file parses as a path
        NodeRef::Path(text) if !text.contains('/') && !text.contains('\\') => {
            let name = text.rsplit('.').next().unwrap_or(text);
            Some((name.to_string(), None))
        }
        _ => None,
    }
}

/// `run` on one word, the classes and functions only, optionally inside one file.
fn words(corpus: &Corpus, name: &str, file: Option<&str>, limit: usize) -> Vec<usize> {
    let query = Query {
        words: vec![name.to_lowercase()],
        inside: file.map(inside).into_iter().collect(),
        kinds: Some(vec![KindFilter::Class, KindFilter::Function]),
        ..Query::default()
    };
    // no docs are wanted: `docs: 0`
    let results = run(
        corpus,
        &query,
        None,
        Limits {
            nodes: limit,
            docs: 0,
        },
    );
    results.nodes.into_iter().map(|hit| hit.index).collect()
}

/// A path as a query holds it: lower case, `/`, no `./` in front and no `/` behind.
fn inside(path: &str) -> String {
    let path = path.replace('\\', "/").to_lowercase();
    let path = path.strip_prefix("./").unwrap_or(&path);
    path.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_ref;
    use vnoc_search::{Kind, NodeEntry};

    fn node(kind: Kind, qualname: &str, path: &str) -> NodeEntry {
        NodeEntry {
            id: format!("{path}::{qualname}"),
            kind,
            name: qualname.rsplit('.').next().unwrap().to_string(),
            qualname: Some(qualname.to_string()),
            path: path.to_string(),
            line: None,
            in_class: qualname.contains('.'),
            docs: 0,
        }
    }

    /// The names of `crates/vnoc/tests/sample/`, plus a folder with a space in its name.
    fn sample() -> Corpus {
        let f = |q, p| node(Kind::Function, q, p);
        Corpus::new(
            vec![
                node(Kind::Class, "OrderService", "orders/service.py"),
                f("OrderService.__init__", "orders/service.py"),
                f("OrderService.parse_order", "orders/service.py"),
                f("OrderService.place", "orders/service.py"),
                f("OrderService.cancel", "orders/service.py"),
                f("normalize", "orders/parse.py"),
                f("normalize", "orders/parse.py"),
                f("split_items", "orders/parse.py"),
                f("rate_for", "billing/tax.py"),
                f("apply_tax", "billing/tax.py"),
                f("retry_charge", "billing/charge.py"),
                f("retry_charge.<locals>.one_try", "billing/charge.py"),
                node(Kind::Class, "Invoice", "billing/invoice.py"),
                f("render_invoice", "billing/invoice.py"),
                f("parse_order", "my orders/legacy.py"),
                node(Kind::File, "service.py", "orders/service.py"),
            ],
            Vec::new(),
        )
    }

    /// `text` as the command line would hold it, and the qualified names it is given.
    fn suggested(corpus: &Corpus, text: &str) -> Vec<String> {
        let r = node_ref::parse(text).unwrap();
        suggest(corpus, &r, 5)
            .indexes()
            .iter()
            .copied()
            .map(|at| {
                let n = &corpus.nodes()[at];
                format!("{}::{}", n.path, n.qualname.as_deref().unwrap())
            })
            .collect()
    }

    #[test]
    fn a_method_given_without_its_class_is_found() {
        assert_eq!(
            suggested(&sample(), "orders/service.py::parse_order"),
            ["orders/service.py::OrderService.parse_order"]
        );
    }

    #[test]
    fn a_name_in_the_wrong_file_is_found_in_the_right_one() {
        assert_eq!(
            suggested(&sample(), "billing/tax.py::split_items"),
            ["orders/parse.py::split_items"]
        );
    }

    #[test]
    fn a_misspelt_name_finds_the_nearest() {
        let corpus = sample();
        // the file's own `parse_order`, and nothing from the other file: other files are asked only when the named one has nothing near
        assert_eq!(
            suggested(&corpus, "orders/service.py::OrderService.parse_ordr"),
            ["orders/service.py::OrderService.parse_order"]
        );
        assert_eq!(
            suggested(&corpus, "billing/charge.py::retry_chrage"),
            ["billing/charge.py::retry_charge"]
        );
        assert_eq!(
            suggested(&corpus, "billing/tax.py::aply_tax"),
            ["billing/tax.py::apply_tax"]
        );
    }

    #[test]
    fn the_named_file_is_asked_first_and_the_others_only_when_it_has_nothing_near() {
        let corpus = sample();
        // `parse_order` is in two files: the one named answers, and the other is not added to it
        assert_eq!(
            suggested(&corpus, "my orders/legacy.py::parse_ordr"),
            ["my orders/legacy.py::parse_order"]
        );
        // the file has nothing near: the other file's is the same name (a typo of it, with the floor)
        assert_eq!(
            suggested(&corpus, "billing/tax.py::parse_ordr"),
            [
                "orders/service.py::OrderService.parse_order",
                "my orders/legacy.py::parse_order"
            ]
        );
    }

    #[test]
    fn a_path_with_a_space_in_it_is_a_path() {
        // built as a value: a space would have split a query string into two words
        let corpus = sample();
        let r = NodeRef::Location {
            path: "my orders/legacy.py".into(),
            rest: "parse_order".into(),
        };
        let found = suggest(&corpus, &r, 5);
        assert_eq!(
            corpus.nodes()[found.indexes()[0]].path,
            "my orders/legacy.py"
        );
    }

    #[test]
    fn a_qualified_name_with_no_file_is_looked_up_by_its_last_part() {
        assert_eq!(
            suggested(&sample(), "OrderService.parse_order"),
            [
                "orders/service.py::OrderService.parse_order",
                "my orders/legacy.py::parse_order"
            ]
        );
        assert!(suggested(&sample(), "OrderService.nothing_like_it").is_empty());
    }

    #[test]
    fn short_names_and_references_with_no_name_get_nothing() {
        let corpus = sample();
        // `pl` is near nothing, and `service.py` is a small file: it is listed whole (below), not guessed at
        assert!(matches!(
            suggest(
                &corpus,
                &node_ref::parse("orders/service.py::pl").unwrap(),
                5
            ),
            Suggestion::Has(_)
        ));
        assert!(
            suggested(&corpus, "billing/tax.py::pl").len() == 2,
            "a small file lists what it has"
        );
        assert!(suggested(&corpus, "orders/service.py:12").is_empty());
        assert!(
            suggested(&corpus, "orders/nothing.py").is_empty(),
            "a path with a folder"
        );
        assert!(suggested(&corpus, "0Q8V5W4T2F7KX9M3B6D1RZ4H7K").is_empty());
        // `orders.pi` is a mistyped file: its last part, `pi`, is too short to be a name
        assert!(suggested(&corpus, "orders.pi").is_empty());
    }

    #[test]
    fn a_limit_of_zero_gives_none() {
        let r = node_ref::parse("orders/service.py::parse_order").unwrap();
        assert_eq!(suggest(&sample(), &r, 0), Suggestion::Nothing);
    }

    /// A project in which `api/main.py` has the one def `main`, as the user's `main.py` has, and which has `Manager` classes elsewhere.
    fn user_project() -> Corpus {
        let f = |q, p| node(Kind::Function, q, p);
        let mut nodes = vec![
            f("main", "api/main.py"),
            node(Kind::Class, "DriverManager", "core/driver.py"),
            node(Kind::Class, "SocketManager", "core/socket.py"),
            node(Kind::Class, "PerformanceTimer", "core/perf.py"),
            f("manage", "core/other.py"),
            f("Main", "web/app.py"),
        ];
        // a big file: eleven defs
        nodes.extend(
            (0..11).map(|n| node(Kind::Function, &format!("handler_{n}"), "web/handlers.py")),
        );
        Corpus::new(nodes, Vec::new())
    }

    #[test]
    fn a_short_misspelt_name_finds_the_files_one_def_and_nothing_that_only_contains_the_letters() {
        let corpus = user_project();
        assert_eq!(
            suggested(&corpus, "api/main.py::man"),
            ["api/main.py::main"]
        );
        // `Manager`, `manage` and `PerformanceTimer` are never suggested for it
        let all = suggested(&corpus, "api/main.py::man");
        assert!(all.iter().all(|s| s.starts_with("api/main.py")), "{all:?}");
        assert!(matches!(
            suggest(&corpus, &node_ref::parse("api/main.py::man").unwrap(), 3),
            Suggestion::Near(_)
        ));
    }

    #[test]
    fn the_same_name_in_another_file_is_a_strong_match_whatever_its_length_and_case() {
        let corpus = user_project();
        // `main` is in `api/main.py`, and `Main` in `web/app.py`: asked in a file that has neither
        assert_eq!(
            suggested(&corpus, "core/perf.py::main"),
            ["api/main.py::main", "web/app.py::Main"]
        );
        // a word that only contains it is not: `man` is inside `DriverManager`, `SocketManager` and `manage`
        assert!(
            suggested(&corpus, "core/perf.py::man")
                .iter()
                .all(|s| !s.contains("Manager") && !s.contains("manage"))
        );
    }

    #[test]
    fn a_small_file_is_listed_whole_when_nothing_is_near_and_a_big_one_is_not() {
        let corpus = user_project();
        let r = node_ref::parse("api/main.py::zzzzzz").unwrap();
        let Suggestion::Has(defs) = suggest(&corpus, &r, 3) else {
            panic!("a file with one def lists it");
        };
        assert_eq!(defs.len(), 1);
        assert_eq!(corpus.nodes()[defs[0]].qualname.as_deref(), Some("main"));
        // eleven defs is more than FEW: no hint at all
        assert_eq!(
            suggest(
                &corpus,
                &node_ref::parse("web/handlers.py::zzzzzz").unwrap(),
                3
            ),
            Suggestion::Nothing
        );
        // exactly ten is still listed, in reading order and in full whatever the limit
        let ten = Corpus::new(
            (0..10)
                .map(|n| node(Kind::Function, &format!("handler_{n}"), "ten.py"))
                .collect(),
            Vec::new(),
        );
        let Suggestion::Has(all) = suggest(&ten, &node_ref::parse("ten.py::zzzzzz").unwrap(), 3)
        else {
            panic!("ten defs are listed");
        };
        assert_eq!(all, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn a_name_with_no_file_is_found_not_guessed() {
        let corpus = user_project();
        assert!(matches!(
            suggest(&corpus, &node_ref::parse("DriverManagr").unwrap(), 5),
            Suggestion::Found(_)
        ));
        assert_eq!(
            suggest(&corpus, &node_ref::parse("nothing_like_it").unwrap(), 5),
            Suggestion::Nothing
        );
    }

    /// A Bend file: its defs are named as written, dots included (`Nat.double`), and none is under another.
    fn bend() -> Corpus {
        let def = |qualname: &str, path: &str| NodeEntry {
            id: format!("{path}::{qualname}"),
            kind: Kind::Function,
            name: qualname.to_string(),
            qualname: Some(qualname.to_string()),
            path: path.to_string(),
            line: None,
            in_class: false,
            docs: 0,
        };
        Corpus::new(
            vec![
                def("Nat.double", "lib.bend"),
                def("Nat.add", "lib.bend"),
                def("main", "main.bend"),
            ],
            Vec::new(),
        )
    }

    #[test]
    fn a_dotted_bend_name_is_found_when_it_is_misspelt_or_in_the_wrong_file() {
        let corpus = bend();
        // the name a user types is cut at its last dot to look it up, and a node's own name is cut the same way: the two agree
        assert_eq!(
            suggested(&corpus, "lib.bend::Nat.dubble"),
            ["lib.bend::Nat.double"]
        );
        assert_eq!(
            suggested(&corpus, "main.bend::Nat.double"),
            ["lib.bend::Nat.double"],
            "the right name in the wrong file"
        );
        assert_eq!(suggested(&corpus, "Nat.double"), ["lib.bend::Nat.double"]);
    }
}
