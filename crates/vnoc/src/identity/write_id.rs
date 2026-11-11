//! Write ID: a node found by name gets its ID back in the code (`docs/11-missing-relink-remove.md` §4).
//!
//! The ID is written the way Relink writes it at the new place (attach's `write_marker`), then `node.toml` says the ID is in the code. A marker that cannot
//! be written changes nothing: the node stays found by name, and the notice says why.

use vnoc_domain::IdInCode;

use super::attach::{AttachError, Env, document_for, find_target, write_marker};
use crate::protocol::{WriteIdParams, WriteIdResult};
use crate::userdata::{UserData, UserDataError};

#[derive(Debug, thiserror::Error)]
pub enum WriteIdError {
    #[error("that node has no docs, so there is no ID to write")]
    NoDocs,
    #[error("the ID of that node is already in the code")]
    AlreadyInCode,
    #[error("this project does not write IDs (`[ids] default` is \"none\")")]
    IdsOff,
    #[error("this node was set to never have an ID in the code")]
    Never,
    #[error("that node has no readable node.toml")]
    NoRecord,
    #[error(transparent)]
    Attach(#[from] AttachError),
    #[error(transparent)]
    UserData(#[from] UserDataError),
}

pub async fn write_id(mut env: Env<'_>, params: WriteIdParams) -> Result<WriteIdResult, WriteIdError> {
    let (location, target) = find_target(&env.index.read().unwrap(), &params.key)?;
    if target.in_code().is_some() {
        return Err(WriteIdError::AlreadyInCode);
    }
    let data = UserData::new(env.project.git_root());
    let id = data.find_node_at(&location).ok_or(WriteIdError::NoDocs)?;
    let mut toml = data.node_toml(&id).ok_or(WriteIdError::NoRecord)?;
    if toml.id_in_code() == Some(IdInCode::Never) {
        return Err(WriteIdError::Never);
    }
    if !data.settings()?.markers {
        return Err(WriteIdError::IdsOff);
    }
    let document = document_for(&env, &target, &location, &params.key, params.document.as_ref())?;

    if let Err(reason) = write_marker(&mut env, &target, &location, &id, document).await {
        return Ok(WriteIdResult { written: false, notice: Some(format!("The ID could not be written ({reason}); the docs stay attached by name.")) });
    }
    toml.set_id_in_code(IdInCode::Yes);
    data.write_node_toml(&id, &toml)?;
    Ok(WriteIdResult { written: true, notice: None })
}
