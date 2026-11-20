//! Editor ↔ core messages (plan_vs/06-core/02-editor-protocol.md). LSP-framed JSON-RPC; every method is a custom `vnoc/*` message.

use async_lsp::lsp_types::notification::Notification;
use async_lsp::lsp_types::request::Request;
use serde::{Deserialize, Serialize};
use vnoc_driver_protocol::{CallMark, DefKind, Range};

use crate::index::{Kind, Node};

/// `vnoc/ping`: is the core alive, which one is it.
///
/// It takes no parameters, and clients send that three ways: absent, `null`, and `[]` (vscode-languageclient sends `[]`). So the
/// params type is "anything" and is ignored: a request that takes nothing must not fail on how a client says so.
pub enum Ping {}

#[derive(Debug, Serialize, Deserialize)]
pub struct PingResult {
    pub pong: bool,
    pub pid: u32,
    pub version: String,
}

impl Request for Ping {
    type Params = serde_json::Value;
    type Result = PingResult;
    const METHOD: &'static str = "vnoc/ping";
}

/// `vnoc/structure`: the folders, files, classes and functions under the opened folder (the one named in `initializationOptions.root`).
/// Answers once the first build is done. Takes no parameters (see `Ping`).
pub enum Structure {}

#[derive(Debug, Serialize, Deserialize)]
pub struct FailedFile {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stats {
    pub folders: usize,
    pub files: usize,
    pub classes: usize,
    pub functions: usize,
}

/// A documented node nothing in the code matches any more: shown under Needs Attention. The anchor is as `node.toml` says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingNode {
    /// the permanent ID
    pub id: String,
    /// `function`, `class`, `file`, `folder`, a value a newer V-NOC wrote, or `null` when `node.toml` does not say
    pub kind: Option<String>,
    pub path: String,
    pub qualname: String,
    /// how many docs are attached
    pub docs: usize,
}

/// A documented node whose `ID:` line was lost: `node.toml` says the ID is in the code, and the code has none, so the node was found only by its name
/// and path. Writing the ID back (`vnoc.writeId` with `key`) makes the doc follow the code again. Not a problem of the files and not Missing: the
/// node is in the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LostMarker {
    /// the permanent ID
    pub id: String,
    /// the node's id in the tree, which `vnoc/writeId` takes: `path::qualname[#n]` for a def, the path for a file or a folder
    pub key: String,
    pub kind: Kind,
    /// as `node.toml` says it, and so as the tree's ids start: at the git root
    pub path: String,
    /// empty for a file or a folder
    pub qualname: String,
}

/// Something the user should look at: where to go when they click it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub text: String,
    /// absolute path
    pub file: Option<String>,
    /// 0-based line in `file`
    pub line: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructureResult {
    /// the index's revision this answer was made from
    pub revision: u64,
    /// absolute path of the git root. Ids and `Node::path` below it are explained in `index::Node`: a node's `id` starts at this root.
    pub git_root: String,
    pub root: Node,
    pub stats: Stats,
    /// more files than the cap: the rest were not read
    pub truncated: bool,
    /// files the driver could not outline (the rest of the structure is still there)
    pub failed: Vec<FailedFile>,
    /// which optional methods the driver that read the code has (`"callTree"`, …): the editor shows a command only when its method is there
    pub capabilities: std::collections::BTreeMap<String, bool>,
    /// documented nodes that are not in the code now, sorted by path
    pub missing: Vec<MissingNode>,
    /// broken `.vnoc/` files and `.vnoc-id` files, and an ID written in two places
    pub problems: Vec<Problem>,
    /// nodes found only by their name although `node.toml` says the ID is in the code, sorted by path. An addition: a client that does not know
    /// the field ignores it, and one reading an older core's answer gets `[]`.
    #[serde(default)]
    pub lost_markers: Vec<LostMarker>,
}

impl Request for Structure {
    type Params = serde_json::Value;
    type Result = StructureResult;
    const METHOD: &'static str = "vnoc/structure";
}

/// `vnoc/search`: which nodes a few typed words find, best first (`docs/20-search/04-core.md` §3). Waits for the first build. The words are
/// matched by `vnoc-search`; this file only says what goes over the wire.
pub enum Search {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    /// how many node rows at most; 200 when absent
    #[serde(default)]
    pub nodes: Option<usize>,
    /// how many doc rows at most; 50 when absent
    #[serde(default)]
    pub docs: Option<usize>,
}

/// A node the query found. `id` is the tree's.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchNode {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    /// relative to the opened folder
    pub path: String,
    /// 0-based; `null` for a file or a folder
    pub line: Option<u32>,
    /// a function inside a class: a method
    pub in_class: bool,
    /// how many docs are attached
    pub docs: usize,
    /// 0 to 5, smaller is better (`docs/20-search/02-names.md`)
    pub tier: u8,
    /// the node a pasted reference named
    pub exact: bool,
    /// the parts of `name` to draw bold: `[start, end]` in UTF-16 units, sorted and merged
    #[serde(rename = "match")]
    pub ranges: Vec<(u32, u32)>,
}

/// A doc the query found (`docs/20-search/04-core.md` §3). A doc whose code is not in the tree has `node: null`; `missing` says whether its node is in the Missing list.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchDoc {
    /// the tree's node id; `null` for a Missing node
    pub node: Option<String>,
    pub node_id: String,
    pub node_name: String,
    pub node_kind: Option<Kind>,
    pub node_path: String,
    pub missing: bool,
    pub file: String,
    pub doc_path: String,
    pub title: String,
    pub title_match: Vec<(u32, u32)>,
    pub snippet: String,
    pub snippet_match: Vec<(u32, u32)>,
    /// the frontmatter's `tags:`, lower case, in the order written; empty when the doc has none
    #[serde(default)]
    pub tags: Vec<String>,
    /// the frontmatter's `author:`
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// the index's revision this answer was made from
    pub revision: u64,
    pub nodes: Vec<SearchNode>,
    /// how many nodes matched before the limit
    pub nodes_total: usize,
    pub docs: Vec<SearchDoc>,
    pub docs_total: usize,
    /// what was wrong with a filter in the query
    pub notes: Vec<String>,
}

impl Request for Search {
    type Params = SearchParams;
    type Result = SearchResult;
    const METHOD: &'static str = "vnoc/search";
}

/// `vnoc/searchCards`: of these node ids, which the query finds (`docs/20-search/04-core.md` §5): by name, or because a doc of the node is a doc hit, as the
/// unlimited `vnoc/search` would say. The canvas asks it for the cards it shows, to dim the rest. Waits for the first build.
pub enum SearchCards {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchCardsParams {
    pub query: String,
    /// the tree's node ids (a structure card's id, a call card's target key)
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchCardsResult {
    /// the index's revision this answer was made from (the same one `vnoc/search` says)
    pub revision: u64,
    /// the ids the query finds: a subset of the asked ones, in the order given
    pub hits: Vec<String>,
}

impl Request for SearchCards {
    type Params = SearchCardsParams;
    type Result = SearchCardsResult;
    const METHOD: &'static str = "vnoc/searchCards";
}

/// `vnoc/filesChanged` (editor → core): these files were created, changed or deleted. Absolute paths. The core finds out what
/// is new or gone by itself; the list says which existing files changed content.
pub enum FilesChanged {}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilesChangedParams {
    pub paths: Vec<String>,
}

impl Notification for FilesChanged {
    type Params = FilesChangedParams;
    const METHOD: &'static str = "vnoc/filesChanged";
}

/// `vnoc/rebuild` (editor → core): read every file again. The escape hatch behind the sidebar's Refresh button.
pub enum Rebuild {}

impl Notification for Rebuild {
    type Params = serde_json::Value;
    const METHOD: &'static str = "vnoc/rebuild";
}

/// `vnoc/indexChanged` (core → editor): the index has a new revision (or its first one); ask for `vnoc/structure` again.
pub enum IndexChanged {}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexChangedParams {
    pub revision: u64,
}

impl Notification for IndexChanged {
    type Params = IndexChangedParams;
    const METHOD: &'static str = "vnoc/indexChanged";
}

/// `vnoc/projectInfo`: what the editor needs to know before asking the user about a first attach. Takes no parameters (see `Ping`).
pub enum ProjectInfo {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfoResult {
    /// the project already has a `.vnoc/` (something was attached before): the first-attach question was asked
    pub initialized: bool,
    /// attaching writes an ID into the code (`[ids] default` in `vnoc.toml`)
    pub markers: bool,
}

impl Request for ProjectInfo {
    type Params = serde_json::Value;
    type Result = ProjectInfoResult;
    const METHOD: &'static str = "vnoc/projectInfo";
}

/// `vnoc/attach`: attach a new doc to a function, class, file or folder (`04-storage/03-writing-safely.md` §3).
///
/// For a function, class or file the editor sends the document as it is now (`version` and `text`, unsaved changes included): the ID
/// is planned on that text. The core asks the editor to apply the edit (`workspace/applyEdit`, refused if the document changed
/// meanwhile), then writes `.vnoc/`. A folder has no document: its ID goes into a `.vnoc-id` file, which the core writes itself.
pub enum Attach {}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachParams {
    /// the node's `id` from `vnoc/structure`: `path::qualname` or `path::qualname#n` for a function or class, the repo path for a file or folder
    pub key: String,
    pub title: String,
    /// the answer to the once-per-project question: write an ID into the code? Only used for the first attach in a project.
    #[serde(default)]
    pub markers: Option<bool>,
    /// required for a function, class or file; a folder has none
    #[serde(default)]
    pub document: Option<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub uri: async_lsp::lsp_types::Url,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachResult {
    /// the node's permanent ID
    pub id: String,
    /// absolute path of the new doc
    pub doc: String,
    /// there is an ID line in the code: the doc follows the function through renames and moves
    pub id_in_code: bool,
    /// something the user should hear (the ID could not be written, so the doc is attached by name)
    pub notice: Option<String>,
    /// the body began with its own `# <title>` line and it was left out (the heading is V-NOC's); absent otherwise
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub title_line_dropped: bool,
}

impl Request for Attach {
    type Params = AttachParams;
    type Result = AttachResult;
    const METHOD: &'static str = "vnoc/attach";
}

/// `vnoc/docs`: the docs attached to a node, for the doc picker.
pub enum Docs {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocsParams {
    /// the node's permanent ID (`Node::node_id`)
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocInfo {
    pub file: String,
    pub title: String,
    /// absolute path of the Markdown file
    pub path: String,
    /// the frontmatter's `tags:`, lower case, in the order written; empty when the doc has none
    #[serde(default)]
    pub tags: Vec<String>,
    /// the frontmatter's `author:`
    #[serde(default)]
    pub author: Option<String>,
}

impl Request for Docs {
    type Params = DocsParams;
    type Result = Vec<DocInfo>;
    const METHOD: &'static str = "vnoc/docs";
}

/// `vnoc/relink`: a Missing node's docs go to another place in the code (`docs/11` §3). The ID is written there first (as attach does),
/// then `node.toml` is moved. Refused, with the reason, when the place has another node's ID, another node resolves there, or the kind differs.
pub enum Relink {}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelinkParams {
    /// the permanent ID of the Missing node
    pub id: String,
    /// the new place: a node's `id` from `vnoc/structure` (`path::qualname[#n]` for a function or class, the repo path for a file or folder)
    pub to: String,
    /// the open document of the new place, as for `vnoc/attach`: needed for a function, class or file when the ID is to be written
    #[serde(default)]
    pub document: Option<Document>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Written {
    /// the ID is in the code at the new place: the docs follow the code from now on
    Id,
    /// only the stored place was moved (IDs are off, or the ID could not be written)
    AnchorOnly,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelinkResult {
    pub written: Written,
    /// something the user should hear (the ID could not be written)
    pub notice: Option<String>,
}

impl Request for Relink {
    type Params = RelinkParams;
    type Result = RelinkResult;
    const METHOD: &'static str = "vnoc/relink";
}

/// `vnoc/writeId`: a node found by name gets its ID written into the code again (`docs/11` §4): a deleted `ID:` line, or an ID that could not be written
/// at attach. Refused, with the reason, when the node has no docs, already has its ID in the code, or the project does not write IDs.
pub enum WriteId {}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteIdParams {
    /// the node's `id` from `vnoc/structure` (`path::qualname[#n]` for a function or class, the repo path for a file or folder)
    pub key: String,
    /// the open document of the node, as for `vnoc/attach`: needed for a function, class or file
    #[serde(default)]
    pub document: Option<Document>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteIdResult {
    /// the ID is in the code now
    pub written: bool,
    /// something the user should hear (the ID could not be written)
    pub notice: Option<String>,
}

impl Request for WriteId {
    type Params = WriteIdParams;
    type Result = WriteIdResult;
    const METHOD: &'static str = "vnoc/writeId";
}

/// `vnoc/locate`: where a function or class is in a document's current text (`docs/17-edit-in-canvas.md` §6). The editor asks it when a draft saves and the
/// document has changed since the draft started: the function may have moved. The core outlines the text it is given (the driver's `vnoc/outline` takes `text`),
/// and finds the node by its ID first, then by its qualname, as `identity::resolve` does. Nothing is cached. `null` when the text has no such node.
pub enum Locate {}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocateParams {
    /// the node's `id` from `vnoc/structure`: `path::qualname[#n]` of a function or class
    pub key: String,
    /// the open document of the node's file, as for `vnoc/attach`; its `text` is what is outlined
    pub document: Document,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Located {
    pub range: Range,
}

impl Request for Locate {
    type Params = LocateParams;
    type Result = Option<Located>;
    const METHOD: &'static str = "vnoc/locate";
}

/// `vnoc/trash`: Remove, step 1 (`docs/11` §5). Writes the entry's `trashed.toml` and says where the node folder moves; the editor moves it.
/// Refused unless the node is in the index's Missing list now: the core holds the rule, not only the menu.
pub enum Trash {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashParams {
    /// the permanent ID of the Missing node
    pub id: String,
}

/// Where a node folder moves from and to (absolute paths): the editor does the move.
#[derive(Debug, Serialize, Deserialize)]
pub struct MovePlan {
    pub from: String,
    pub to: String,
}

impl Request for Trash {
    type Params = TrashParams;
    type Result = MovePlan;
    const METHOD: &'static str = "vnoc/trash";
}

/// `vnoc/trashList`: the removed nodes that can be restored, newest first. Empty entries are removed by the call.
pub enum TrashList {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashItem {
    pub entry: String,
    /// the permanent ID
    pub id: String,
    /// UTC, `2026-11-11T10:30:00Z`
    pub removed_at: String,
    pub docs: usize,
    /// where the node was (its qualname, or its path for a file or folder): what the user recognises it by
    pub name: String,
}

impl Request for TrashList {
    type Params = serde_json::Value;
    type Result = Vec<TrashItem>;
    const METHOD: &'static str = "vnoc/trashList";
}

/// `vnoc/trashInfo`: what Empty Trash needs to ask and to delete: where the trash is, how many removed nodes it holds and how big it is. Empty entries
/// are removed by the call, so they are not counted.
pub enum TrashInfo {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashInfoResult {
    /// absolute path of the trash folder
    pub folder: String,
    pub entries: usize,
    pub bytes: u64,
}

impl Request for TrashInfo {
    type Params = serde_json::Value;
    type Result = TrashInfoResult;
    const METHOD: &'static str = "vnoc/trashInfo";
}

/// `vnoc/restorePlan`: Restore, step 2. Where the entry's node folder goes back to, worked out from its ID; refused when a node folder is there.
pub enum RestorePlan {}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestorePlanParams {
    pub entry: String,
}

impl Request for RestorePlan {
    type Params = RestorePlanParams;
    type Result = MovePlan;
    const METHOD: &'static str = "vnoc/restorePlan";
}

/// `vnoc/callTree`: what a function calls (or what a class's `__init__` calls), as a tree, with calls to one function merged (`docs/14` §6).
/// Waits for the first build. A request that is cancelled (`$/cancelRequest`) cancels the driver's work too.
pub enum CallTree {}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallTreeParams {
    /// the node's `id` from `vnoc/structure`: `path::qualname[#n]`, as `vnoc/attach` takes it. A function or a class.
    pub key: String,
}

/// `vnoc/hasCalls`: how many call cards opening this function would show, without sending the tree (`docs/16-ui-redesign/05-calls-as-nodes.md` §3.1).
/// One key per request, so each is scheduled and cancelled on its own. The driver is asked for a tree one level deep and the answer goes through the
/// same merge as `vnoc/callTree`; its marks never leave the core. Waits for the first build.
pub enum HasCalls {}

#[derive(Debug, Serialize, Deserialize)]
pub struct HasCallsParams {
    /// as in `vnoc/callTree`
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HasCallsResult {
    /// the index's revision the answer is from (the editor compares it with the structure's, within one run of the core)
    pub revision: u64,
    /// how many nodes opening the function would show: the children of its merged tree
    pub count: u32,
}

impl Request for HasCalls {
    type Params = HasCallsParams;
    type Result = HasCallsResult;
    const METHOD: &'static str = "vnoc/hasCalls";
}

/// One place a node is called.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSite {
    /// relative to the opened folder, as `CallTreeNode.path`; `null` when the file is not under it
    pub path: Option<String>,
    /// the range's fields sit beside `path`, so a site still reads as a range (`site.start.line`)
    #[serde(flatten)]
    pub range: Range,
}

/// One function or class in the tree; calls to it from one place are merged into one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallTreeNode {
    /// the node's `id` from `vnoc/structure`, when the index has it; `null` for code the index does not know (a library, an ignored file)
    pub key: Option<String>,
    /// the last part of `qualname`
    pub name: String,
    pub qualname: String,
    /// relative to the opened folder, as in `vnoc/structure`; `null` when the code is not under it
    pub path: Option<String>,
    pub kind: DefKind,
    /// the calls that reach this node, once per copy of a merged parent; not the number of places it is written (that is `call_sites`); 1 for the root
    pub call_count: u32,
    /// where it is called, each with the file it is written in, by file and then in source order; empty for the root
    pub call_sites: Vec<CallSite>,
    pub marks: Vec<CallMark>,
    pub children: Vec<CallTreeNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallTreeResult {
    /// the index's revision the keys were mapped against; an answer older than the structure the editor shows is dropped
    pub revision: u64,
    pub root: CallTreeNode,
}

impl Request for CallTree {
    type Params = CallTreeParams;
    type Result = CallTreeResult;
    const METHOD: &'static str = "vnoc/callTree";
}
