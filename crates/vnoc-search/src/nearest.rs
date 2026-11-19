//! The names closest to one that was typed wrong (`docs/21-agent-cli/02-naming-a-node.md` §4), for a command's "did you mean".
//!
//! This is **not** search: `run` has no typo tolerance on purpose (doc 20), so the box stays as it is, and this is used only after a name found
//! nothing. How many edits count depends on the length of the name given: 3 letters or fewer allow none (a short name is too near to too many; inside
//! one file, [`nearest_in`] drops that floor), 4 allow one, 5 or more allow two. An edit is a letter dropped, added or changed, or two neighbouring letters swapped (optimal string alignment).
//! Case is ignored. Only the last part of a qualified name is compared, on both sides (`parse_ordr` against `parse_order`, never against
//! `OrderService.parse_order`).

use crate::{Corpus, Kind, NodeEntry};

/// The classes and functions whose name is within the allowed edits of `name`, as indexes into [`Corpus::nodes`]: the nodes of `in_file` (a
/// path as the nodes have it) first, then the closer, then reading order; at most `limit`. A file or a folder is named by a path, so it is never
/// a suggestion here.
pub fn nearest(corpus: &Corpus, name: &str, in_file: Option<&str>, limit: usize) -> Vec<usize> {
    find(corpus, name, true, |_| true, in_file, limit)
}

/// Where [`nearest_in`] looks: one file's own nodes, or every node except that file's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope<'a> {
    File(&'a str),
    Elsewhere(&'a str),
}

/// The same, inside a [`Scope`], and with or without the length floor. `floor: true` is `nearest`'s rule (3 letters or fewer allow no edit: a short
/// name is too near to too many names in a whole project). `floor: false` is for **one file**, which has few names to be near to: 1 edit up to 4
/// letters (a name of 1 to 4 letters, `man` for `main`), 2 from 5; only an empty name gets nothing. Closer first, then reading order.
pub fn nearest_in(
    corpus: &Corpus,
    name: &str,
    scope: Scope<'_>,
    floor: bool,
    limit: usize,
) -> Vec<usize> {
    match scope {
        Scope::File(file) => find(corpus, name, floor, |node| node.path == file, None, limit),
        Scope::Elsewhere(file) => find(corpus, name, floor, |node| node.path != file, None, limit),
    }
}

fn find(
    corpus: &Corpus,
    name: &str,
    floor: bool,
    keep: impl Fn(&NodeEntry) -> bool,
    in_file: Option<&str>,
    limit: usize,
) -> Vec<usize> {
    let given = letters(last_part(name));
    let allowed = match (given.len(), floor) {
        (0, _) | (1..=3, true) => return Vec::new(),
        (1..=4, _) => 1,
        _ => 2,
    };
    let mut found: Vec<(bool, usize, usize)> = corpus
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.kind, Kind::Class | Kind::Function) && keep(node))
        .filter_map(|(at, node)| {
            let edits = edits_within(&given, &letters(last_part(&node.name)), allowed)?;
            Some((in_file != Some(node.path.as_str()), edits, at))
        })
        .collect();
    found.sort();
    found.into_iter().take(limit).map(|(_, _, at)| at).collect()
}

/// `parse_order` of `OrderService.parse_order`, and of `parse_order#2`.
fn last_part(name: &str) -> &str {
    let name = name.rsplit_once('#').map_or(name, |(whole, n)| {
        if n.bytes().all(|b| b.is_ascii_digit()) {
            whole
        } else {
            name
        }
    });
    name.rsplit('.').next().unwrap_or(name)
}

fn letters(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// The edit distance of `a` and `b` (optimal string alignment), when it is at most `allowed`.
fn edits_within(a: &[char], b: &[char], allowed: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > allowed {
        return None;
    }
    // three rows of the table are enough: the one being made, the one before, and the one before that (for a swap)
    let mut before: Vec<usize> = Vec::new();
    let mut last: Vec<usize> = (0..=b.len()).collect();
    for (i, &x) in a.iter().enumerate() {
        let mut row = vec![i + 1];
        for (j, &y) in b.iter().enumerate() {
            let mut best = (last[j] + usize::from(x != y))
                .min(last[j + 1] + 1)
                .min(row[j] + 1);
            if i > 0 && j > 0 && x == b[j - 1] && a[i - 1] == y {
                best = best.min(before[j - 1] + 1);
            }
            row.push(best);
        }
        before = std::mem::replace(&mut last, row);
    }
    let edits = last[b.len()];
    (edits <= allowed).then_some(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeEntry;

    fn entry(kind: Kind, qualname: &str, path: &str) -> NodeEntry {
        let name = qualname.rsplit('.').next().unwrap().to_string();
        NodeEntry {
            id: format!("{path}::{qualname}"),
            kind,
            name,
            qualname: Some(qualname.to_string()),
            path: path.to_string(),
            line: None,
            in_class: false,
            docs: 0,
        }
    }

    fn corpus(names: &[(&str, &str)]) -> Corpus {
        Corpus::new(
            names
                .iter()
                .map(|(path, qualname)| entry(Kind::Function, qualname, path))
                .collect(),
            Vec::new(),
        )
    }

    /// The qualified names `nearest` gives, in order.
    fn names(corpus: &Corpus, name: &str, in_file: Option<&str>) -> Vec<String> {
        nearest(corpus, name, in_file, 10)
            .into_iter()
            .map(|at| corpus.nodes()[at].qualname.clone().unwrap())
            .collect()
    }

    #[test]
    fn one_letter_dropped_added_or_changed() {
        let c = corpus(&[("a.py", "parse_order")]);
        for typo in ["parse_ordr", "parse_orderr", "parse_ordar", "arse_order"] {
            assert_eq!(names(&c, typo, None), ["parse_order"], "{typo}");
        }
    }

    #[test]
    fn two_neighbours_swapped_are_one_edit() {
        let c = corpus(&[("a.py", "render")]);
        assert_eq!(names(&c, "rednrer", None), ["render"], "two edits");
        assert_eq!(names(&c, "rendre", None), ["render"], "a swap");
        // a swap is one edit, so a 4-letter name allows it
        let four = corpus(&[("a.py", "load")]);
        assert_eq!(names(&four, "laod", None), ["load"]);
    }

    #[test]
    fn five_letters_or_more_allow_two_edits_and_not_three() {
        let c = corpus(&[("a.py", "parse_order")]);
        assert_eq!(names(&c, "pars_ordr", None), ["parse_order"], "two dropped");
        assert!(names(&c, "pars_ord", None).is_empty(), "three dropped");
    }

    #[test]
    fn four_letters_allow_one_edit_only() {
        let c = corpus(&[("a.py", "load")]);
        assert_eq!(
            names(&c, "lod", None),
            Vec::<String>::new(),
            "3 letters given: none"
        );
        assert_eq!(names(&c, "lead", None), ["load"], "one changed");
        assert!(names(&c, "lend", None).is_empty(), "two changed");
        assert_eq!(names(&c, "loads", None), ["load"], "one added");
    }

    #[test]
    fn a_name_of_three_letters_or_fewer_is_never_given_a_suggestion() {
        let c = corpus(&[("a.py", "get"), ("a.py", "set"), ("a.py", "run")]);
        for name in ["gt", "get", "gat", "ru", ""] {
            assert!(names(&c, name, None).is_empty(), "{name:?}");
        }
    }

    #[test]
    fn case_is_ignored() {
        let c = corpus(&[("a.py", "OrderService")]);
        assert_eq!(names(&c, "orderservise", None), ["OrderService"]);
        assert_eq!(names(&c, "ORDERSERVICE", None), ["OrderService"]);
    }

    #[test]
    fn the_last_part_of_a_qualified_name_is_compared_on_both_sides() {
        let c = corpus(&[("a.py", "OrderService.parse_order")]);
        assert_eq!(
            names(&c, "Other.parse_ordr", None),
            ["OrderService.parse_order"]
        );
        assert_eq!(
            names(&c, "parse_ordr#2", None),
            ["OrderService.parse_order"]
        );
    }

    #[test]
    fn the_files_own_nodes_come_first_then_the_closer_then_reading_order() {
        let c = corpus(&[
            ("a.py", "parse_ordex"), // 1 edit, in another file, first in reading order
            ("b.py", "parse_ordxx"), // 2 edits, this file
            ("b.py", "parse_ordek"), // 1 edit, this file
            ("c.py", "parse_ordey"), // 1 edit, another file
        ]);
        assert_eq!(
            names(&c, "parse_order", Some("b.py")),
            ["parse_ordek", "parse_ordxx", "parse_ordex", "parse_ordey"]
        );
        assert_eq!(
            names(&c, "parse_order", None),
            ["parse_ordex", "parse_ordek", "parse_ordey", "parse_ordxx"]
        );
        assert_eq!(nearest(&c, "parse_order", None, 2).len(), 2);
    }

    #[test]
    fn files_and_folders_are_not_suggested() {
        let nodes = vec![
            entry(Kind::File, "orders.py", "orders.py"),
            entry(Kind::Folder, "orders_", "orders_"),
        ];
        let c = Corpus::new(nodes, Vec::new());
        assert!(nearest(&c, "orders.pi", None, 5).is_empty());
    }

    #[test]
    fn inside_one_file_a_short_name_has_no_floor() {
        let c = corpus(&[
            ("a.py", "main"),
            ("b.py", "main"),
            ("a.py", "manager"),
            ("a.py", "xyz"),
        ]);
        let in_a = |name: &str, floor: bool| -> Vec<String> {
            nearest_in(&c, name, Scope::File("a.py"), floor, 10)
                .into_iter()
                .map(|at| c.nodes()[at].qualname.clone().unwrap())
                .collect()
        };
        // `man` is a letter from `main`; `manager` is four letters further, and only this file is looked at
        assert_eq!(in_a("man", false), ["main"]);
        assert_eq!(in_a("xy", false), ["xyz"], "two letters, one edit");
        assert_eq!(
            in_a("q", false),
            Vec::<String>::new(),
            "one letter is one edit from a 1-2 letter name, and there is none"
        );
        assert_eq!(
            in_a("", false),
            Vec::<String>::new(),
            "an empty name gets nothing"
        );
        // with the floor it is `nearest`'s rule: three letters or fewer allow none
        assert_eq!(in_a("man", true), Vec::<String>::new());
        // 4 letters: one edit; 5 or more: two
        assert_eq!(in_a("mian", false), ["main"]);
        assert_eq!(in_a("manage", false), ["manager"]);
    }

    #[test]
    fn elsewhere_leaves_the_named_file_out() {
        let c = corpus(&[
            ("a.py", "parse_order"),
            ("b.py", "parse_ordr"),
            ("c.py", "parse_order"),
        ]);
        let names = |scope| -> Vec<usize> { nearest_in(&c, "parse_order", scope, true, 10) };
        assert_eq!(names(Scope::Elsewhere("a.py")), [2, 1]);
        assert_eq!(names(Scope::File("a.py")), [0]);
    }
}
