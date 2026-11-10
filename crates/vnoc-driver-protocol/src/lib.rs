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
