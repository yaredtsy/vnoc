//! Rust types for the core ↔ driver wire. **The definition is the wire spec** (`plan_vs/07-drivers/01-driver-protocol.md`,
//! VS-D68); these types follow it and may lag it. A driver is free to use any language or transport library.
//!
//! Readers ignore fields they do not know, so every type here simply omits `deny_unknown_fields`.
//!
//! Positions are **0-based line, UTF-16 `character`**, as in LSP and VS Code.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Sent in `initialize`. A breaking change to the wire bumps it; additive changes do not.
pub const PROTOCOL_VERSION: u32 = 1;

/// Request names.
pub mod method {
    pub const OUTLINE: &str = "vnoc/outline";
    pub const PLAN_IDS: &str = "vnoc/planIds";
    pub const CALL_TREE: &str = "vnoc/callTree";
    pub const EXPAND: &str = "vnoc/expand";
    pub const ENVIRONMENT_INPUTS: &str = "vnoc/environmentInputs";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol: u32,
    pub name: String,
    pub version: String,
    /// Which optional methods the driver implements (`"outline"`, `"ids.plan"`, …). Absent = false.
    #[serde(default)]
    pub capabilities: BTreeMap<String, bool>,
}

/// Give `text` (an unsaved buffer) **or** `root` and `file` (a path the driver reads).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutlineParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefKind {
    Function,
    Class,
}

/// One def or class. `qualname` is the language's own name and **opaque to the core** (Python: `__qualname__`, with `<locals>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Def {
    pub qualname: String,
    pub name: String,
    pub kind: DefKind,
    /// the enclosing def's qualname, `""` at module level
    pub parent: String,
    #[serde(default)]
    pub parent_kind: Option<DefKind>,
    pub range: Range,
    #[serde(default)]
    pub params: Vec<String>,
    /// the ID text found in the code, as written (the core classifies it)
    #[serde(default)]
    pub id: Option<String>,
    /// a parse error inside this def's lines
    #[serde(default)]
    pub syntax_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxError {
    pub range: Range,
    pub message: String,
}

/// Defs are **flat, in source order, parents before children**.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutlineResult {
    pub defs: Vec<Def>,
    #[serde(default)]
    pub syntax_errors: Vec<SyntaxError>,
}

/// One ID the core wants written: **the core picks the ID, the driver picks the place**.
/// The def is named by `qualname` and which same-named definition it is (`occurrence`, 1-based), as in a location key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanId {
    pub qualname: String,
    pub occurrence: u32,
    /// the ID text to write
    pub id: String,
    /// replace an ID the def already has (the driver may refuse: protocol 1 drivers need not support it)
    #[serde(default)]
    pub replace_existing: bool,
}

/// `vnoc/planIds` (capability `ids.plan`): plan the edits that put these IDs into `text`, the editor's buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanIdsParams {
    pub text: String,
    /// echoed back; the editor drops a plan made for an older version of the document
    pub version: i64,
    pub ids: Vec<PlanId>,
}

/// An edit in the coordinates of the text the driver was given (all edits of one plan apply against that same text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    #[serde(rename = "newText")]
    pub new_text: String,
}

/// What was inserted for one ID. `span` is in the coordinates of the **new** text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Written {
    pub id: String,
    pub qualname: String,
    pub occurrence: u32,
    pub span: Range,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub qualname: String,
    pub occurrence: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanIdsResult {
    pub version: i64,
    pub edits: Vec<TextEdit>,
    pub written: Vec<Written>,
    /// asked for, but no ID could go there (a syntax error inside the def, an ID already there)
    pub skipped: Vec<Skipped>,
    /// asked for, but no such def in this text
    pub unmatched: Vec<PlanId>,
    /// blake3 of the new text, lowercase hex: "this is exactly what I predicted"
    pub text_hash: String,
}
