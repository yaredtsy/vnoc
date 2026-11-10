//! Editor ↔ core messages (plan_vs/06-core/02-editor-protocol.md). LSP-framed JSON-RPC; every method is a custom `vnoc/*` message.

use async_lsp::lsp_types::notification::Notification;
use async_lsp::lsp_types::request::Request;
use serde::{Deserialize, Serialize};

use crate::index::Node;

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
}

impl Request for Structure {
    type Params = serde_json::Value;
    type Result = StructureResult;
    const METHOD: &'static str = "vnoc/structure";
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

/// `vnoc/attach`: attach a new doc to a function or class (`04-storage/03-writing-safely.md` §3).
///
/// The editor sends the document as it is now (`version` and `text`, unsaved changes included): the ID line is planned on that text.
/// The core asks the editor to apply the edit (`workspace/applyEdit`, refused if the document changed meanwhile), then writes `.vnoc/`.
pub enum Attach {}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachParams {
    /// the node's `id` from `vnoc/structure`: `path::qualname` or `path::qualname#n`
    pub key: String,
    pub title: String,
    /// the answer to the once-per-project question: write an ID into the code? Only used for the first attach in a project.
    #[serde(default)]
    pub markers: Option<bool>,
    pub document: Document,
}

#[derive(Debug, Serialize, Deserialize)]
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
}

impl Request for Attach {
    type Params = AttachParams;
    type Result = AttachResult;
    const METHOD: &'static str = "vnoc/attach";
}
