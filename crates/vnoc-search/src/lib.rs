//! V-NOC's search: which nodes a few typed words find, and in what order (`docs/20-search`).
//!
//! The crate takes plain lists and gives plain results. It knows no index, no `RepoPath`, no driver and no editor, and it does no I/O, so any
//! editor, the CLI or a web page that can produce the lists gets the same search, and it builds for `wasm32` where no native program runs.
//!
//! * [`Corpus`]: the nodes in reading order with their names split into words, built once and searched many times.
//! * [`parse`] and [`Query`]: what the user typed, as words, phrases and filters (`query.rs`).
//! * [`run`]: the nodes the query finds, best first, as [`Results`].
//!
//! A node's name, qualified name and path are matched in six tiers (`names.rs`), and a hit carries the ranges of its name to draw bold, in
//! UTF-16 code units as LSP's are (`utf16.rs`). The words of every doc are indexed with the corpus and ranked with BM25 (`docs.rs`).

mod docs;
mod names;
mod nearest;
mod query;
mod run;
mod utf16;
mod words;

use std::sync::Arc;

pub use docs::DocIndex;
pub use nearest::{Scope, nearest, nearest_in};
pub use query::{KindFilter, Query, parse};
pub use run::{matches, run};

/// What a node is. A method is a [`Kind::Function`] with `in_class` set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Folder,
    File,
    Class,
    Function,
}

/// A node of the project, as the search needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeEntry {
    pub id: String,
    pub kind: Kind,
    /// `parse_order`, or `orders.py` for a file.
    pub name: String,
    /// `OrderService.parse_order`; none for files and folders.
    pub qualname: Option<String>,
    /// Relative to the opened folder.
    pub path: String,
    pub line: Option<u32>,
    /// A function inside a class: a method.
    pub in_class: bool,
    /// How many docs the node has.
    pub docs: usize,
}

/// A doc of a node: its title and body are searched (`DocIndex`), and a hit is shown with a snippet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocEntry {
    /// Index into the nodes; none for a node that is not in the tree (see `missing`).
    pub node: Option<usize>,
    pub node_id: String,
    pub node_name: String,
    pub node_kind: Option<Kind>,
    pub node_path: String,
    /// The node is in the core's Missing list. Only reported, never used to rank or filter: a doc with no `node` and `missing` false is one whose
    /// code was not read (ignored, outside the opened folder, past the cap).
    pub missing: bool,
    pub file: String,
    pub doc_path: String,
    pub title: String,
    pub body: String,
}

/// The nodes and docs to search. Built once per change, kept, and searched on every key: the words of every name are split here, not per query.
pub struct Corpus {
    nodes: Vec<NodeEntry>,
    prepared: Vec<names::Prepared>,
    docs: Vec<DocEntry>,
    doc_index: Arc<DocIndex>,
}

impl Corpus {
    /// The nodes stay in the order given; that is the order of reading, and the caller's to get right. Builds the doc index of `docs`; a caller that
    /// keeps one across changes of the nodes uses [`Corpus::with_doc_index`].
    pub fn new(nodes: Vec<NodeEntry>, docs: Vec<DocEntry>) -> Corpus {
        let index = Arc::new(DocIndex::new(&docs));
        Corpus::with_doc_index(nodes, docs, index)
    }

    /// As [`Corpus::new`], with a doc index built earlier by [`DocIndex::new`]. `docs` must be the docs it was built from: the same text in the same
    /// order, though their node fields may differ (a doc is joined to whichever node it belongs to now). A different number of docs is a bug of the
    /// caller's: a debug build panics, and a release build builds a fresh index rather than read past the end of the old one.
    pub fn with_doc_index(
        nodes: Vec<NodeEntry>,
        docs: Vec<DocEntry>,
        index: Arc<DocIndex>,
    ) -> Corpus {
        debug_assert_eq!(
            docs.len(),
            index.len(),
            "the doc index was built from another list of docs"
        );
        let doc_index = reconciled(&docs, index);
        let prepared = nodes.iter().map(names::Prepared::new).collect();
        Corpus {
            nodes,
            prepared,
            docs,
            doc_index,
        }
    }
    /// The nodes in the order given; a [`NodeHit`]'s `index` points into them.
    pub fn nodes(&self) -> &[NodeEntry] {
        &self.nodes
    }

    pub fn docs(&self) -> &[DocEntry] {
        &self.docs
    }

    /// The index into [`Corpus::nodes`] of the node with this id, by a scan: one lookup per query.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
}

/// `index` when it covers as many docs as there are, else a fresh one built from them.
fn reconciled(docs: &[DocEntry], index: Arc<DocIndex>) -> Arc<DocIndex> {
    if docs.len() == index.len() {
        index
    } else {
        Arc::new(DocIndex::new(docs))
    }
}

/// How many of each kind of result to give back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub nodes: usize,
    pub docs: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            nodes: 200,
            docs: 50,
        }
    }
}

/// The answer: the best of the nodes and docs found, and how many there were before the limits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Results {
    pub nodes: Vec<NodeHit>,
    pub nodes_total: usize,
    pub docs: Vec<DocHit>,
    pub docs_total: usize,
}

/// A node the query found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeHit {
    /// Into [`Corpus::nodes`].
    pub index: usize,
    /// 0 to 5, smaller is better: name starts with it, name holds it, its letters start the name's words, qualified name holds it, path starts
    /// with it, path holds it.
    pub tier: u8,
    /// Words of the name passed over by the fuzzy match; 0 unless the tier is 2.
    pub skipped: u32,
    /// The node a pasted reference named, put first.
    pub exact: bool,
    /// The parts of the name to draw bold: sorted, merged where they touch, in UTF-16 units.
    pub ranges: Vec<(u32, u32)>,
}

/// A doc the query found (03).
#[derive(Clone, Debug, PartialEq)]
pub struct DocHit {
    /// Into [`Corpus::docs`].
    pub index: usize,
    /// BM25 over the words of the title (counted three times) and the body; bigger is better.
    pub score: f64,
    /// The hits in the title, to draw bold: sorted, merged where they touch, in UTF-16 units.
    pub title_ranges: Vec<(u32, u32)>,
    /// About 120 chars of the body around the first hit, in one line, with `…` where it was cut; the start of the body when the title holds every
    /// hit; empty when the body is.
    pub snippet: String,
    /// The hits inside the snippet, in UTF-16 units of the snippet as it is here (a leading `…` is one unit).
    pub snippet_ranges: Vec<(u32, u32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> NodeEntry {
        NodeEntry {
            id: id.to_string(),
            kind: Kind::File,
            name: id.to_string(),
            qualname: None,
            path: id.to_string(),
            line: None,
            in_class: false,
            docs: 0,
        }
    }

    fn doc(body: &str) -> DocEntry {
        DocEntry {
            node: None,
            node_id: String::new(),
            node_name: String::new(),
            node_kind: None,
            node_path: String::new(),
            missing: false,
            file: String::new(),
            doc_path: String::new(),
            title: String::new(),
            body: body.to_string(),
        }
    }

    #[test]
    fn position_is_the_index_of_the_node_with_that_id() {
        let corpus = Corpus::new(vec![node("a"), node("b"), node("c")], Vec::new());
        assert_eq!(corpus.position("a"), Some(0));
        assert_eq!(corpus.position("c"), Some(2));
        assert_eq!(corpus.position("A"), None, "ids are compared as written");
        assert_eq!(corpus.position("d"), None);
        assert_eq!(Corpus::new(Vec::new(), Vec::new()).position("a"), None);
    }

    #[test]
    fn an_index_for_another_number_of_docs_is_replaced_not_used() {
        let docs = vec![doc("one"), doc("two")];
        let kept = Arc::new(DocIndex::new(&docs));
        assert!(Arc::ptr_eq(&reconciled(&docs, kept.clone()), &kept));
        for other in [&docs[..1], &[doc("a"), doc("b"), doc("c")][..], &[][..]] {
            let got = reconciled(other, kept.clone());
            assert!(!Arc::ptr_eq(&got, &kept));
            assert_eq!(got.len(), other.len());
        }
    }
}
