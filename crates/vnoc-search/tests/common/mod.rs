//! Helpers the test files share: a corpus from a few lines, and a hit read back as its name and tier.

#![allow(dead_code)]

use vnoc_search::{Corpus, Kind, Limits, NodeEntry, Results, parse, run};

/// A node with only what the tests care about; the id is the path and the name.
pub fn node(
    kind: Kind,
    path: &str,
    name: &str,
    qualname: Option<&str>,
    in_class: bool,
) -> NodeEntry {
    NodeEntry {
        id: format!("{path}::{name}"),
        kind,
        name: name.to_string(),
        qualname: qualname.map(str::to_string),
        path: path.to_string(),
        line: None,
        in_class,
        docs: 0,
    }
}

pub fn file(path: &str) -> NodeEntry {
    node(
        Kind::File,
        path,
        path.rsplit('/').next().unwrap(),
        None,
        false,
    )
}

pub fn folder(path: &str) -> NodeEntry {
    node(
        Kind::Folder,
        path,
        path.rsplit('/').next().unwrap(),
        None,
        false,
    )
}

/// A function or class of `path` whose qualified name is its dotted name.
pub fn def(kind: Kind, path: &str, qualname: &str) -> NodeEntry {
    let name = qualname.rsplit('.').next().unwrap();
    node(kind, path, name, Some(qualname), false)
}

pub fn func(path: &str, name: &str) -> NodeEntry {
    def(Kind::Function, path, name)
}

pub fn method(path: &str, qualname: &str) -> NodeEntry {
    NodeEntry {
        in_class: true,
        ..def(Kind::Function, path, qualname)
    }
}

//   billing/ ├ invoice.py ├ total   ├ bill_all      ├ Invoice (class) ├ pay
//            └ rebilling.py └ rebill
//   shop/cart.py ├ add_item  └ billing_note
pub fn tree() -> Corpus {
    corpus(vec![
        folder("billing"),
        file("billing/invoice.py"),
        func("billing/invoice.py", "total"),
        func("billing/invoice.py", "bill_all"),
        def(Kind::Class, "billing/invoice.py", "Invoice"),
        method("billing/invoice.py", "Invoice.pay"),
        file("billing/rebilling.py"),
        func("billing/rebilling.py", "rebill"),
        folder("shop"),
        file("shop/cart.py"),
        func("shop/cart.py", "add_item"),
        func("shop/cart.py", "billing_note"),
    ])
}

pub fn corpus(nodes: Vec<NodeEntry>) -> Corpus {
    Corpus::new(nodes, Vec::new())
}

pub fn search(corpus: &Corpus, query: &str) -> Results {
    run(corpus, &parse(query), None, Limits::default())
}

/// `(name, tier)` of every hit.
pub fn rows(corpus: &Corpus, results: &Results) -> Vec<(String, u8)> {
    results
        .nodes
        .iter()
        .map(|h| (corpus.nodes()[h.index].name.clone(), h.tier))
        .collect()
}

pub fn ask(corpus: &Corpus, query: &str) -> Vec<(String, u8)> {
    rows(corpus, &search(corpus, query))
}

/// `("name", tier)` pairs written out.
pub fn want(rows: &[(&str, u8)]) -> Vec<(String, u8)> {
    rows.iter().map(|&(n, t)| (n.to_string(), t)).collect()
}
