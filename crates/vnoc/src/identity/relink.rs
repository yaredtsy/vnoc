//! Relink: a Missing node's docs go to another place (`docs/11-missing-relink-remove.md` §3).
//!
//! The same order as attach: the ID is written at the new place first, then `node.toml` is moved, so stopping after any step leaves something
//! harmless (an ID with no node moved to it is a copy of an ID: the node is still Missing, and Relink can be run again).

use vnoc_domain::{IdInCode, NodeId, NodeKind};

use super::attach::{AttachError, Env, document_for, find_target, write_marker};
use crate::protocol::{RelinkParams, RelinkResult, Written};
use crate::userdata::{UserData, UserDataError};

#[derive(Debug, thiserror::Error)]
pub enum RelinkError {
    #[error("`{0}` is not a node ID")]
    BadId(String),
    #[error("{0}")]
    Refused(&'static str),
    #[error("that node is not Missing: its docs are shown where its code is")]
    NotMissing,
    #[error("that node has no readable node.toml")]
    NoRecord,
    #[error("this is a {to}, and the docs belong to a {kind}")]
    WrongKind { kind: String, to: String },
    #[error(transparent)]
    Attach(#[from] AttachError),
    #[error(transparent)]
    UserData(#[from] UserDataError),
}

const HAS_DOCS: &str = "that node already has its own docs";

pub async fn relink(mut env: Env<'_>, params: RelinkParams) -> Result<RelinkResult, RelinkError> {
    let id = NodeId::parse(&params.id).map_err(|_| RelinkError::BadId(params.id.clone()))?;
    let (to, target) = {
        let index = env.index.read().unwrap();
        let (to, target) = find_target(&index, &params.to)?;
        if !index.is_missing(&id) {
            return Err(RelinkError::NotMissing);
        }
        // the three refusals. A node found by name has no ID in the code, so the first check alone would miss it: the index is asked as well.
        if target.in_code().is_some_and(|there| there != id) || index.node_at(&to).is_some_and(|there| there != id) {
            return Err(RelinkError::Refused(HAS_DOCS));
        }
        (to, target)
    };
    let data = UserData::new(env.project.git_root());
    let mut toml = data.node_toml(&id).ok_or(RelinkError::NoRecord)?;
    let new_kind = target.kind();
    if let Some(kind @ (NodeKind::Function | NodeKind::Class | NodeKind::File | NodeKind::Folder)) = toml.kind() {
        // a method is a function, so a function and a class are different kinds: the lists never offer it, the core checks anyway
        if kind != new_kind {
            return Err(RelinkError::WrongKind { kind: kind.as_str().to_string(), to: new_kind.as_str().to_string() });
        }
    }
    let document = document_for(&env, &target, &to, &params.to, params.document.as_ref())?;
    // a node set to never have an ID in the code, or with a value a newer V-NOC wrote, gets none, and its field stays as it is
    let may_write = matches!(toml.id_in_code(), None | Some(IdInCode::Yes | IdInCode::No));

    // 1. the ID, when the project writes IDs, the node may have one, and the place does not carry it already
    let mut notice = None;
    let mut written = Written::Id;
    if target.in_code() != Some(id) {
        if may_write && data.settings()?.markers {
            if let Err(reason) = write_marker(&mut env, &target, &to, &id, document).await {
                notice = Some(format!("The ID could not be written ({reason}); the docs are attached by name."));
                written = Written::AnchorOnly;
            }
        } else {
            written = Written::AnchorOnly;
        }
    }

    // 2. node.toml: the place, the kind, and whether the ID is in the code
    toml.set_location(&to);
    toml.set_kind(&new_kind);
    if may_write {
        toml.set_id_in_code(if written == Written::Id { IdInCode::Yes } else { IdInCode::No });
    }
    data.write_node_toml(&id, &toml)?;
    Ok(RelinkResult { written, notice })
}
