//! Editor ↔ core messages (plan_vs/06-core/02-editor-protocol.md). LSP-framed JSON-RPC; every method is a custom `vnoc/*` request.

use async_lsp::lsp_types::request::Request;
use serde::{Deserialize, Serialize};

use crate::structure::Node;

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

/// `vnoc/structure`: the folders, files, classes and functions under `root`.
pub enum Structure {}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructureParams {
    /// absolute path of the opened folder
    pub root: String,
}

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
    pub ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StructureResult {
    pub root: Node,
    pub stats: Stats,
    /// more files than the cap: the rest were not read
    pub truncated: bool,
    /// files the driver could not outline (the rest of the structure is still there)
    pub failed: Vec<FailedFile>,
}

impl Request for Structure {
    type Params = StructureParams;
    type Result = StructureResult;
    const METHOD: &'static str = "vnoc/structure";
}
