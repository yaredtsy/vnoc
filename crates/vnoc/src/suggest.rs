//! "Did you mean": the nodes a name that found nothing was probably meant for (`docs/21-agent-cli/02-naming-a-node.md` §4).
//!
//! Search has no typo tolerance (doc 20), so this is two passes. First `run` on the last part of the name, inside the file it was given with and
//! then anywhere: it finds a name given without its class (`orders/service.py::parse_order` is `OrderService.parse_order`). If that finds nothing,
//! `nearest` finds the names a few letters away (`parse_ordr`). Only classes and functions are suggested.
//!
//! The query is **built as a value**, never formatted into a string and parsed: a path with a space in it would not survive that.

use vnoc_search::{Corpus, KindFilter, Limits, Query, nearest, run};

use crate::node_ref::NodeRef;

/// Shorter names are too near to too many others: no suggestion, as `nearest` has it.
const SHORTEST: usize = 3;

/// The corpus nodes (indexes into [`Corpus::nodes`]) `r` was probably meant for, at most `limit`. `r`'s path is what the file is looked for by, as the
/// corpus has it (relative to the opened folder: a command makes it so first). A reference with no name to look for (an ID, a file and a line, a
/// path with a folder in it) gets none.
pub fn suggest(corpus: &Corpus, r: &NodeRef, limit: usize) -> Vec<usize> {
    let Some((name, file)) = asked(r) else {
        return Vec::new();
    };
    if name.chars().count() < SHORTEST || limit == 0 {
        return Vec::new();
    }
    if let Some(file) = file {
        let found = words(corpus, &name, Some(file), limit);
        if !found.is_empty() {
            return found;
        }
    }
    let found = words(corpus, &name, None, limit);
    if !found.is_empty() {
        return found;
    }
    nearest(corpus, &name, file, limit)
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
            .into_iter()
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
        // the file's own `parse_order` first, then the one in the other file
        assert_eq!(
            suggested(&corpus, "orders/service.py::OrderService.parse_ordr"),
            [
                "orders/service.py::OrderService.parse_order",
                "my orders/legacy.py::parse_order"
            ]
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
    fn the_files_own_names_come_before_the_others() {
        let corpus = sample();
        // `parse_order` is in two files: the one named comes first
        let in_legacy = suggested(&corpus, "my orders/legacy.py::parse_ordr");
        assert_eq!(in_legacy[0], "my orders/legacy.py::parse_order");
        assert!(in_legacy.contains(&"orders/service.py::OrderService.parse_order".to_string()));
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
        assert_eq!(corpus.nodes()[found[0]].path, "my orders/legacy.py");
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
        assert!(suggested(&corpus, "orders/service.py::pl").is_empty());
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
        assert!(suggest(&sample(), &r, 0).is_empty());
    }
}
