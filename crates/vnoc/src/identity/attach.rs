//! Attach a doc to a function or class: ID → marker in the code → `node.toml` → doc file (`04-storage/03-writing-safely.md` §3).
//!
//! The order is chosen so that stopping after any step leaves something harmless: a marker without a node folder is an unused ID,
//! and a node folder without a doc is a node with no docs. The marker is written first, and a marker that cannot be written
//! does not stop the attach: the doc is then attached by name (`id_in_code = "no"`).

use std::path::PathBuf;
use std::sync::RwLock;

use async_lsp::lsp_types::{ApplyWorkspaceEditParams, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit, WorkspaceEdit};
use async_lsp::{ClientSocket, LanguageClient};
use vnoc_domain::{IdInCode, LocationKey, LocationKeyError, NoFreeId, NodeId, NodeToml, first_free_id};
use vnoc_driver_protocol::{PlanId, PlanIdsParams};

use crate::driver_host::DriverHost;
use crate::index::Index;
use crate::project::Project;
use crate::protocol::{AttachParams, AttachResult};
use crate::userdata::{UserData, UserDataError};

#[derive(Debug, thiserror::Error)]
pub enum AttachError {
    #[error(transparent)]
    BadKey(#[from] LocationKeyError),
    #[error("`{0}` is not in the index (is the file saved, and is it Python?)")]
    NotInIndex(String),
    #[error("the document is not the file of `{0}`")]
    WrongDocument(String),
    #[error(transparent)]
    NoFreeId(#[from] NoFreeId),
    #[error(transparent)]
    UserData(#[from] UserDataError),
}

/// What an attach works with.
pub struct Env<'a> {
    pub project: &'a Project,
    pub index: &'a RwLock<Index>,
    pub host: &'a DriverHost,
    /// to ask the editor to apply an edit
    pub client: ClientSocket,
}

pub async fn attach(mut env: Env<'_>, params: AttachParams) -> Result<AttachResult, AttachError> {
    let location: LocationKey = params.key.parse()?;
    let def = env.index.read().unwrap().def_at(&location).ok_or_else(|| AttachError::NotInIndex(params.key.clone()))?;
    if params.document.uri.to_file_path().ok() != Some(env.project.git_root().join(location.path().as_str())) {
        return Err(AttachError::WrongDocument(params.key));
    }

    let data = UserData::new(env.project.git_root());
    let settings = data.settings()?;
    // the question is asked once per project: afterwards `vnoc.toml` answers it
    let markers_on = if settings.initialized { settings.markers } else { params.markers.unwrap_or(true) };

    // 1. which ID: the one in the code, else the one this location already has, else a new one
    let in_code = def.id.as_deref().and_then(|text| NodeId::parse(text).ok());
    let existing = if in_code.is_some() { None } else { data.find_node_at(&location) };
    let (id, is_new) = match (in_code, existing) {
        (Some(id), _) | (None, Some(id)) => (id, false),
        (None, None) => (first_free_id(&location, |id| data.is_taken(id))?, true),
    };

    // 2. the marker, for a node that has none and is allowed one
    let mut notice = None;
    let mut id_in_code = in_code.is_some();
    if is_new && markers_on {
        match write_marker(&mut env, &location, &id, &params).await {
            Ok(()) => id_in_code = true,
            Err(reason) => notice = Some(format!("The ID could not be written into the code ({reason}); this doc is attached by name.")),
        }
    }

    // 3. `.vnoc/`: first time set up, then the node, then the doc
    data.initialize(params.markers.unwrap_or(true))?;
    let mut toml = NodeToml::new(&location);
    toml.set_id_in_code(if id_in_code { IdInCode::Yes } else { IdInCode::No });
    data.create_node(&id, &toml)?;
    let doc: PathBuf = data.create_doc(&id, &params.title, &location.to_string())?;

    Ok(AttachResult { id: id.to_string(), doc: doc.to_string_lossy().into_owned(), id_in_code, notice })
}

/// Plans the `ID:` line with the driver and has the editor apply it. `Err` says why not, in words for the user.
async fn write_marker(env: &mut Env<'_>, location: &LocationKey, id: &NodeId, params: &AttachParams) -> Result<(), String> {
    let ask = PlanIdsParams {
        text: params.document.text.clone(),
        version: i64::from(params.document.version),
        ids: vec![PlanId { qualname: location.qualname().to_string(), occurrence: location.occurrence(), id: id.to_string(), replace_existing: false }],
    };
    let plan = env.host.plan_ids(&ask).await.map_err(|e| e.to_string())?;
    if let Some(skipped) = plan.skipped.first() {
        return Err(skipped.reason.clone());
    }
    if plan.edits.is_empty() {
        return Err("the function is not in the open document".into());
    }

    let edit = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier { uri: params.document.uri.clone(), version: Some(params.document.version) },
        edits: plan.edits.iter().map(|e| OneOf::Left(text_edit(e))).collect(),
    };
    let workspace_edit = WorkspaceEdit { document_changes: Some(DocumentChanges::Edits(vec![edit])), ..Default::default() };
    let answer = env.client.apply_edit(ApplyWorkspaceEditParams { label: Some("V-NOC: add an ID".into()), edit: workspace_edit }).await.map_err(|e| e.to_string())?;
    if answer.applied { Ok(()) } else { Err(answer.failure_reason.unwrap_or_else(|| "the editor did not apply the edit; the document changed meanwhile?".into())) }
}

fn text_edit(e: &vnoc_driver_protocol::TextEdit) -> async_lsp::lsp_types::TextEdit {
    let position = |p: vnoc_driver_protocol::Position| async_lsp::lsp_types::Position { line: p.line, character: p.character };
    async_lsp::lsp_types::TextEdit { range: async_lsp::lsp_types::Range { start: position(e.range.start), end: position(e.range.end) }, new_text: e.new_text.clone() }
}
