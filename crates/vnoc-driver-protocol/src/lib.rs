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
    pub const PLAN_FILE_ID: &str = "vnoc/planFileId";
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
    /// the ID text found in the file's `# FileID:` comment, as written (the core classifies it)
    #[serde(default)]
    pub file_id: Option<String>,
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

/// `vnoc/planFileId` (capability `ids.file`): plan the edit that puts a `FileID` comment at the top of `text`, the editor's buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFileIdParams {
    pub text: String,
    /// echoed back; the editor drops a plan made for an older version of the document
    pub version: i64,
    /// the ID text to write
    pub id: String,
}

/// One edit (or none, when the file already has a `FileID`), in the coordinates of the text the driver was given.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanFileIdResult {
    pub version: i64,
    pub edits: Vec<TextEdit>,
    /// what was inserted; `span` is in the coordinates of the **new** text
    pub written: Option<Written>,
    /// why nothing was planned (the file already has a `FileID`)
    #[serde(default)]
    pub skipped: Option<String>,
    /// blake3 of the new text, lowercase hex
    pub text_hash: String,
}

/// The def a call tree starts at: named by `qualname` and which same-named definition it is (`occurrence`, 1-based), as in a location key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallEntry {
    pub qualname: String,
    pub occurrence: u32,
}

/// `vnoc/callTree` (capability `callTree`): the calls made by one function, or by a class's `__init__`. `root` is the project folder,
/// `file` the entry's file relative to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeParams {
    pub root: String,
    pub file: String,
    pub entry: CallEntry,
    /// shortens the tree only: nodes deeper than this are marked `cut`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallMark {
    /// the callee is already above it on this path: shown, not entered
    Recursive,
    /// its calls are not in the tree: the depth or the node budget ran out
    Cut,
    /// the analysis gave up on part of it (a limit, the deadline)
    Incomplete,
}

/// The def a node stands for, named the way the outline names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallTarget {
    /// relative to `root`, `/` separators; `null` when the def is outside `root`
    pub file: Option<String>,
    pub qualname: String,
    pub occurrence: u32,
    pub kind: DefKind,
    /// the ID text as written in the docstring, without a `FunctionSchema/` / `ClassSchema/` prefix; `null` when it has none
    pub id: Option<String>,
}

/// One call site: the tree is not merged, so one node per call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallNode {
    pub target: CallTarget,
    /// where the call is written; `null` on the root
    pub call_site: Option<Range>,
    /// the file `call_site` is in: relative to `root`, `/` separators. Absent: the parent node's `target.file` (a driver from before 7.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_site_file: Option<String>,
    #[serde(default)]
    pub marks: Vec<CallMark>,
    #[serde(default)]
    pub children: Vec<CallNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeResult {
    pub root: CallNode,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = r#"{ "file": "a.py", "qualname": "f", "occurrence": 1, "kind": "function", "id": null }"#;

    #[test]
    fn a_call_node_from_a_driver_before_7_7_has_no_site_file() {
        let node: CallNode = serde_json::from_str(&format!(r#"{{ "target": {TARGET}, "call_site": null }}"#)).unwrap();
        assert_eq!(node.call_site_file, None);
    }

    #[test]
    fn a_site_file_is_written_when_there_is_one_and_left_out_when_there_is_not() {
        let node: CallNode = serde_json::from_str(&format!(r#"{{ "target": {TARGET}, "call_site": null }}"#)).unwrap();
        assert!(serde_json::to_value(&node).unwrap().get("call_site_file").is_none(), "None adds no key");
        let named = CallNode { call_site_file: Some("pkg/base.py".to_string()), ..node };
        assert_eq!(serde_json::to_value(&named).unwrap()["call_site_file"], "pkg/base.py");
    }
}
