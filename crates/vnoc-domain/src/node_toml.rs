//! `node.toml`: one per node, in `.vnoc/nodes/<shard>/<id>/`. It is the node's **anchor** (where it is, so a fresh clone can
//! find it again) and the node's state.
//!
//! # The format (final from the MVP on)
//!
//! ```toml
//! path       = "src/orders.py"             # anchor: the repo path, as git records it
//! qualname   = "OrderService.parse_order"  # anchor: Python's __qualname__ (with <locals>)
//! id_in_code = "yes"                       # "yes" | "no" | "never"; the MVP always writes "yes"
//! status     = "active"                    # "active" | "archived"
//! # archived_at = 2026-11-06               # a date; written when the node is archived
//! # occurrence = 2                         # only when it is not 1: which same-named definition of the file
//! ```
//!
//! **The rules that make later changes migration-free:**
//! 1. **Readers ignore fields they do not know**, and **writers keep them**: this file is edited in place with `toml_edit`,
//!    so unknown keys, comments and the order of keys survive a read-modify-write.
//! 2. The MVP **writes only the fields it uses** (plus `id_in_code`, which a later mode reads).
//! 3. A *known* field with a value this version does not know (`id_in_code = "sometimes"`) is read as [`IdInCode::Other`] and
//!    **kept untouched** unless the code deliberately sets it.
//! 4. There is **no version field here**: adding a field is not a format change. Only `vnoc.toml` carries `format`.

use std::str::FromStr;

use toml_edit::{DocumentMut, Item, Value, value};

use crate::location::{LocationKey, RepoPath, RepoPathError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdInCode {
    Yes,
    No,
    Never,
    /// A value a newer V-NOC wrote. Kept as it is.
    Other(String),
}

impl IdInCode {
    fn as_str(&self) -> &str {
        match self {
            IdInCode::Yes => "yes",
            IdInCode::No => "no",
            IdInCode::Never => "never",
            IdInCode::Other(s) => s,
        }
    }
    fn read(s: &str) -> IdInCode {
        match s {
            "yes" => IdInCode::Yes,
            "no" => IdInCode::No,
            "never" => IdInCode::Never,
            other => IdInCode::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Active,
    Archived,
    /// A value a newer V-NOC wrote. Kept as it is.
    Other(String),
}

impl Status {
    fn as_str(&self) -> &str {
        match self {
            Status::Active => "active",
            Status::Archived => "archived",
            Status::Other(s) => s,
        }
    }
    fn read(s: &str) -> Status {
        match s {
            "active" => Status::Active,
            "archived" => Status::Archived,
            other => Status::Other(other.to_string()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeTomlError {
    #[error("node.toml is not valid TOML: {0}")]
    Syntax(#[from] toml_edit::TomlError),
    #[error("node.toml has no `{0}` (a string)")]
    Missing(&'static str),
    #[error("node.toml `path` is not a repo path: {0}")]
    Path(#[from] RepoPathError),
    #[error("node.toml `occurrence` is a whole number from 1")]
    Occurrence,
}

/// A `node.toml` held as a document, so that writing it back keeps what this version does not understand.
#[derive(Debug, Clone)]
pub struct NodeToml {
    doc: DocumentMut,
}

impl NodeToml {
    /// A new file for a node the user has just attached something to. Writes exactly the MVP's fields.
    pub fn new(location: &LocationKey) -> NodeToml {
        let mut doc = DocumentMut::new();
        doc["path"] = value(location.path().as_str());
        doc["qualname"] = value(location.qualname());
        if location.occurrence() > 1 {
            doc["occurrence"] = value(i64::from(location.occurrence()));
        }
        doc["id_in_code"] = value(IdInCode::Yes.as_str());
        doc["status"] = value(Status::Active.as_str());
        NodeToml { doc }
    }

    /// Reads a file. Fails only when the anchor (`path`, `qualname`) is missing or wrong; everything else is optional.
    pub fn parse(text: &str) -> Result<NodeToml, NodeTomlError> {
        let node = NodeToml { doc: DocumentMut::from_str(text)? };
        node.location()?; // validate the anchor now, so later reads cannot fail
        Ok(node)
    }

    /// The anchor as a location. `occurrence` is 1 when the file does not say.
    pub fn location(&self) -> Result<LocationKey, NodeTomlError> {
        let path = self.str_field("path").ok_or(NodeTomlError::Missing("path"))?;
        let qualname = self.str_field("qualname").ok_or(NodeTomlError::Missing("qualname"))?;
        let occurrence = match self.doc.get("occurrence") {
            None => 1,
            Some(item) => item.as_integer().and_then(|n| u32::try_from(n).ok()).filter(|n| *n >= 1).ok_or(NodeTomlError::Occurrence)?,
        };
        let path = RepoPath::new(path)?;
        LocationKey::new(path, qualname, occurrence).map_err(|_| NodeTomlError::Occurrence)
    }

    /// `None` when the file does not say (an old or hand-written file).
    pub fn id_in_code(&self) -> Option<IdInCode> {
        self.str_field("id_in_code").map(IdInCode::read)
    }

    /// Records whether the ID is in the code. Keeps everything else.
    pub fn set_id_in_code(&mut self, in_code: IdInCode) {
        self.doc["id_in_code"] = value(in_code.as_str());
    }

    /// A file without `status` is active.
    pub fn status(&self) -> Status {
        self.str_field("status").map_or(Status::Active, Status::read)
    }

    /// The date the node was archived, as written (`2026-11-06`), if any.
    pub fn archived_at(&self) -> Option<String> {
        match self.doc.get("archived_at")? {
            Item::Value(Value::Datetime(d)) => Some(d.value().to_string()),
            Item::Value(Value::String(s)) => Some(s.value().to_string()),
            _ => None,
        }
    }

    /// Moves the anchor (a found move, or "Update anchors"). Keeps everything else, including unknown fields.
    pub fn set_location(&mut self, location: &LocationKey) {
        self.doc["path"] = value(location.path().as_str());
        self.doc["qualname"] = value(location.qualname());
        if location.occurrence() > 1 {
            self.doc["occurrence"] = value(i64::from(location.occurrence()));
        } else {
            self.doc.remove("occurrence");
        }
    }

    /// Archives the node. `date` is `YYYY-MM-DD`. (The MVP never expires an archived node; the date is for what comes later.)
    pub fn archive(&mut self, date: toml_edit::Datetime) {
        self.doc["status"] = value(Status::Archived.as_str());
        self.doc["archived_at"] = value(date);
    }

    /// Un-archives. Removes `archived_at`.
    pub fn unarchive(&mut self) {
        self.doc["status"] = value(Status::Active.as_str());
        self.doc.remove("archived_at");
    }

    fn str_field(&self, key: &str) -> Option<&str> {
        self.doc.get(key)?.as_str()
    }
}

/// The text to write to disk. An untouched, parsed file comes back byte for byte.
impl std::fmt::Display for NodeToml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.doc.to_string())
    }
}
