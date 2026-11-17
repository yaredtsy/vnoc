//! V-NOC's search: which nodes a few typed words find, and in what order (`docs/20-search`).
//!
//! The crate takes plain lists and gives plain results. It knows no index, no `RepoPath`, no driver and no editor, and it does no I/O, so any
//! editor, the CLI or a web page that can produce the lists gets the same search, and it builds for `wasm32` where no native program runs.
//!
//! * [`Corpus`]: the nodes in reading order with their names split into words, built once and searched many times.
//! * [`parse`] and [`Query`]: the words the user typed, lower-cased.
//! * [`run`]: the nodes the words find, best first, as [`Results`].
//!
//! A node's name, qualified name and path are matched in six tiers (`names.rs`), and a hit carries the ranges of its name to draw bold, in
//! UTF-16 code units as LSP's are (`utf16.rs`). Doc text arrives in a later step: [`DocEntry`] and [`DocHit`] are declared and unused.

mod names;
mod run;
mod utf16;
mod words;

pub use run::run;

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

/// A doc of a node. Declared now; nothing reads it until the doc text is searched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocEntry {
    /// Index into the nodes; none for a node that is missing.
    pub node: Option<usize>,
    pub node_id: String,
    pub node_name: String,
    pub node_kind: Option<Kind>,
    pub node_path: String,
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
}

impl Corpus {
    /// The nodes stay in the order given; that is the order of reading, and the caller's to get right.
    pub fn new(nodes: Vec<NodeEntry>, docs: Vec<DocEntry>) -> Corpus {
        let prepared = nodes.iter().map(names::Prepared::new).collect();
        Corpus {
            nodes,
            prepared,
            docs,
        }
    }

    /// The nodes in the order given; a [`NodeHit`]'s `index` points into them.
    pub fn nodes(&self) -> &[NodeEntry] {
        &self.nodes
    }

    pub fn docs(&self) -> &[DocEntry] {
        &self.docs
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

/// What the user typed, as words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// Lower-case, none empty.
    pub words: Vec<String>,
}

/// Splits `query` at white space and lower-cases it.
pub fn parse(query: &str) -> Query {
    Query {
        words: query.split_whitespace().map(str::to_lowercase).collect(),
    }
}

/// The answer: the best of the nodes and docs found, and how many there were before the limits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// A doc the query found. Empty until the doc text is searched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocHit {}
