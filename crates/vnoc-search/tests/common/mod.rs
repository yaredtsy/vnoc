//! Helpers the test files share: a corpus from a few lines, and a hit read back as its name and tier.

#![allow(dead_code)]

use vnoc_search::{Corpus, DocEntry, Kind, Limits, NodeEntry, Results, parse, run};

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
    corpus(tree_nodes())
}

/// The nodes of [`tree`], for a corpus that has docs too.
pub fn tree_nodes() -> Vec<NodeEntry> {
    vec![
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
    ]
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

/// The place in [`tree_nodes`] of the node called `name`.
pub fn at(name: &str) -> usize {
    tree_nodes().iter().position(|n| n.name == name).unwrap()
}

/// A doc of the tree's node called `name`.
pub fn doc_on(name: &str, file: &str, title: &str, body: &str) -> DocEntry {
    let nodes = tree_nodes();
    let at = nodes.iter().position(|n| n.name == name).unwrap();
    let node = &nodes[at];
    DocEntry {
        node: Some(at),
        node_id: node.id.clone(),
        node_name: node.name.clone(),
        node_kind: Some(node.kind),
        node_path: node.path.clone(),
        missing: false,
        file: file.to_string(),
        doc_path: format!(".vnoc/nodes/{file}"),
        title: title.to_string(),
        body: body.to_string(),
    }
}

/// A doc of a node nothing in the code matches any more: `path` and `kind` are what its `node.toml` says.
pub fn doc_missing(
    path: &str,
    kind: Option<Kind>,
    file: &str,
    title: &str,
    body: &str,
) -> DocEntry {
    DocEntry {
        node: None,
        node_id: format!("missing:{path}"),
        node_name: path.rsplit('/').next().unwrap().to_string(),
        node_kind: kind,
        node_path: path.to_string(),
        missing: true,
        file: file.to_string(),
        doc_path: format!(".vnoc/nodes/{file}"),
        title: title.to_string(),
        body: body.to_string(),
    }
}

/// The tree with these docs.
pub fn with_docs(docs: Vec<DocEntry>) -> Corpus {
    Corpus::new(tree_nodes(), docs)
}

/// The files of the docs a query found, in order.
pub fn found(corpus: &Corpus, query: &str) -> Vec<String> {
    let results = search(corpus, query);
    results
        .docs
        .iter()
        .map(|h| corpus.docs()[h.index].file.clone())
        .collect()
}

/// `["a", "b"]` as owned strings.
pub fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The `i`th of a sorted run of words made of letters only: `aaaa`, `aaab`, … A digit would be a word of its own, as in names.
pub fn letters(first: char, i: usize) -> String {
    let at = |n: usize| char::from(b'a' + (n % 26) as u8);
    format!("{first}{}{}{}", at(i / 676), at(i / 26), at(i))
}

/// The text of `range`, a pair of UTF-16 offsets, in `text`.
pub fn slice16(text: &str, range: (u32, u32)) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16(&units[range.0 as usize..range.1 as usize]).unwrap()
}
