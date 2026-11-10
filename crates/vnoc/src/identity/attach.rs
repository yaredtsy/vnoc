//! Attach a doc to a function, class, file or folder: ID → marker → `node.toml` → doc file (`04-storage/03-writing-safely.md` §3).
//!
//! The order is chosen so that stopping after any step leaves something harmless: a marker without a node folder is an unused ID,
//! and a node folder without a doc is a node with no docs. The marker is written first, and a marker that cannot be written
//! does not stop the attach: the doc is then attached by name (`id_in_code = "no"`).
//!
//! The marker is a docstring line for a function or class (the driver plans it), a header comment for a file (the driver plans it),
//! and a `.vnoc-id` file for a folder (the core writes it: no language is involved).

use std::path::PathBuf;
use std::sync::RwLock;

use async_lsp::lsp_types::{ApplyWorkspaceEditParams, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, TextDocumentEdit, WorkspaceEdit};
use async_lsp::{ClientSocket, LanguageClient};
use vnoc_domain::{IdInCode, LocationKey, LocationKeyError, NoFreeId, NodeId, NodeKind, NodeToml, RepoPath, first_free_id};
use vnoc_driver_protocol::{DefKind, PlanFileIdParams, PlanId, PlanIdsParams, TextEdit};

use crate::driver_host::DriverHost;
use crate::index::Index;
use crate::project::Project;
use super::folder_marker;
use crate::protocol::{AttachParams, AttachResult, Document};
use crate::userdata::{UserData, UserDataError};

#[derive(Debug, thiserror::Error)]
pub enum AttachError {
    #[error(transparent)]
    BadKey(#[from] LocationKeyError),
    #[error("`{0}` is not in the index (is the file saved, and is it Python?)")]
    NotInIndex(String),
    #[error("the document is not the file of `{0}`")]
    WrongDocument(String),
    #[error("`{0}` needs the open document: its ID goes into the file")]
    MissingDocument(String),
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

/// What the doc is attached to, with what the index knows of its ID.
enum Target {
    Def { in_code: Option<NodeId>, kind: DefKind },
    File { in_code: Option<NodeId> },
    Folder { in_code: Option<NodeId> },
}

impl Target {
    fn kind(&self) -> NodeKind {
        match self {
            Target::Def { kind: DefKind::Class, .. } => NodeKind::Class,
            Target::Def { kind: DefKind::Function, .. } => NodeKind::Function,
            Target::File { .. } => NodeKind::File,
            Target::Folder { .. } => NodeKind::Folder,
        }
    }

    fn in_code(&self) -> Option<NodeId> {
        match self {
            Target::Def { in_code, .. } | Target::File { in_code } | Target::Folder { in_code } => *in_code,
        }
    }
}

/// A key with `::` names a def; a bare repo path names a file or a folder (the tree's ids, `index::Node::id`).
fn find_target(index: &Index, key: &str) -> Result<(LocationKey, Target), AttachError> {
    let not_in_index = || AttachError::NotInIndex(key.to_string());
    if key.contains("::") {
        let location: LocationKey = key.parse()?;
        let def = index.def_at(&location).ok_or_else(not_in_index)?;
        let in_code = def.id.as_deref().and_then(|text| NodeId::parse(text).ok());
        return Ok((location, Target::Def { in_code, kind: def.kind }));
    }
    let path = RepoPath::new(key).map_err(|_| not_in_index())?;
    let whole = LocationKey::new(path.clone(), "", 1).expect("occurrences are 1-based");
    if index.is_file(&path) {
        Ok((whole, Target::File { in_code: index.file_id(&path) }))
    } else if index.is_folder(&path) {
        Ok((whole, Target::Folder { in_code: index.folder_id(&path) }))
    } else {
        Err(not_in_index())
    }
}

pub async fn attach(mut env: Env<'_>, params: AttachParams) -> Result<AttachResult, AttachError> {
    let (location, target) = find_target(&env.index.read().unwrap(), &params.key)?;
    // a function, class or file marker is an edit of the open document
    let document = match (&target, params.document.as_ref()) {
        (Target::Folder { .. }, _) => None,
        (_, Some(document)) => {
            if document.uri.to_file_path().ok() != Some(env.project.git_root().join(location.path().as_str())) {
                return Err(AttachError::WrongDocument(params.key));
            }
            Some(document)
        }
        (_, None) => return Err(AttachError::MissingDocument(params.key)),
    };

    let data = UserData::new(env.project.git_root());
    let settings = data.settings()?;
    // the question is asked once per project: afterwards `vnoc.toml` answers it
    let markers_on = if settings.initialized { settings.markers } else { params.markers.unwrap_or(true) };

    // 1. which ID: the one in the code, else the one this location already has, else a new one
    let in_code = target.in_code();
    let existing = if in_code.is_some() { None } else { data.find_node_at(&location) };
    let (id, is_new) = match (in_code, existing) {
        (Some(id), _) | (None, Some(id)) => (id, false),
        (None, None) => (first_free_id(&location, |id| data.is_taken(id))?, true),
    };

    // 2. the marker, for a node that has none and is allowed one
    let mut notice = None;
    let mut id_in_code = in_code.is_some();
    if is_new && markers_on {
        let written = match (&target, document) {
            (Target::Def { .. }, Some(document)) => write_def_marker(&mut env, &location, &id, document).await,
            (Target::File { .. }, Some(document)) => write_file_marker(&mut env, &id, document).await,
            _ => write_folder_marker(env.project.git_root(), location.path(), &id),
        };
        match written {
            Ok(()) => id_in_code = true,
            Err(reason) => notice = Some(format!("The ID could not be written ({reason}); this doc is attached by name.")),
        }
    }

    // 3. `.vnoc/`: first time set up, then the node, then the doc
    data.initialize(params.markers.unwrap_or(true))?;
    let mut toml = NodeToml::new(&location);
    toml.set_id_in_code(if id_in_code { IdInCode::Yes } else { IdInCode::No });
    toml.set_kind(&target.kind());
    data.create_node(&id, &toml)?;
    let hint = if location.qualname().is_empty() { location.path().to_string() } else { location.to_string() };
    let doc: PathBuf = data.create_doc(&id, &params.title, &hint)?;

    Ok(AttachResult { id: id.to_string(), doc: doc.to_string_lossy().into_owned(), id_in_code, notice })
}

/// Plans the `ID:` line with the driver and has the editor apply it. `Err` says why not, in words for the user.
async fn write_def_marker(env: &mut Env<'_>, location: &LocationKey, id: &NodeId, document: &Document) -> Result<(), String> {
    let ask = PlanIdsParams {
        text: document.text.clone(),
        version: i64::from(document.version),
        ids: vec![PlanId { qualname: location.qualname().to_string(), occurrence: location.occurrence(), id: id.to_string(), replace_existing: false }],
    };
    let plan = env.host.plan_ids(&ask).await.map_err(|e| e.to_string())?;
    if let Some(skipped) = plan.skipped.first() {
        return Err(skipped.reason.clone());
    }
    if plan.edits.is_empty() {
        return Err("the function is not in the open document".into());
    }
    apply_edits(env, document, "V-NOC: add an ID", &plan.edits).await
}

/// Plans the `# FileID:` comment with the driver and has the editor apply it.
async fn write_file_marker(env: &mut Env<'_>, id: &NodeId, document: &Document) -> Result<(), String> {
    let ask = PlanFileIdParams { text: document.text.clone(), version: i64::from(document.version), id: id.to_string() };
    let plan = env.host.plan_file_id(&ask).await.map_err(|e| e.to_string())?;
    if let Some(reason) = plan.skipped {
        return Err(reason);
    }
    apply_edits(env, document, "V-NOC: add a file ID", &plan.edits).await
}

/// Writes `.vnoc-id` in the folder. The core does it itself: nobody has it open, and no language is involved.
fn write_folder_marker(git_root: &std::path::Path, folder: &RepoPath, id: &NodeId) -> Result<(), String> {
    folder_marker::write(&git_root.join(folder.as_str()), id).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => format!("the folder already has a {} that V-NOC cannot read", folder_marker::FILE_NAME),
        _ => e.to_string(),
    })
}

async fn apply_edits(env: &mut Env<'_>, document: &Document, label: &str, edits: &[TextEdit]) -> Result<(), String> {
    let edit = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier { uri: document.uri.clone(), version: Some(document.version) },
        edits: edits.iter().map(|e| OneOf::Left(text_edit(e))).collect(),
    };
    let workspace_edit = WorkspaceEdit { document_changes: Some(DocumentChanges::Edits(vec![edit])), ..Default::default() };
    let answer = env.client.apply_edit(ApplyWorkspaceEditParams { label: Some(label.into()), edit: workspace_edit }).await.map_err(|e| e.to_string())?;
    if answer.applied { Ok(()) } else { Err(answer.failure_reason.unwrap_or_else(|| "the editor did not apply the edit; the document changed meanwhile?".into())) }
}

fn text_edit(e: &vnoc_driver_protocol::TextEdit) -> async_lsp::lsp_types::TextEdit {
    let position = |p: vnoc_driver_protocol::Position| async_lsp::lsp_types::Position { line: p.line, character: p.character };
    async_lsp::lsp_types::TextEdit { range: async_lsp::lsp_types::Range { start: position(e.range.start), end: position(e.range.end) }, new_text: e.new_text.clone() }
}
