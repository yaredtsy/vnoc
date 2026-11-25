use async_lsp::{ErrorCode, ResponseError};

use super::{Core, Work, internal, write_lock};
use crate::identity::{self, EditSink, Env};
use crate::protocol::{
    AttachParams, AttachResult, DocInfo, DocsParams, MovePlan, RelinkParams, RelinkResult,
    RestorePlanParams, TrashInfoResult, TrashItem, TrashParams, WriteIdParams, WriteIdResult,
};
use crate::protocol::{LocateParams, Located};
use crate::userdata::{TrashError, UserData};
use crate::watchdog::timed;
use vnoc_domain::NodeId;

/// `vnoc/attach`: see `identity::attach`.
pub(super) fn attach(
    core: &mut Core,
    params: AttachParams,
) -> impl std::future::Future<Output = Result<AttachResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("attach", async move {
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core
            .state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let _one_writer = write_lock(project).await?;
        let env = Env {
            project,
            index: &core.state.index,
            host: &core.host,
            edits: EditSink::Editor(core.client.clone()),
        };
        let result = identity::attach(env, params, identity::AttachOptions::editor())
            .await
            .map_err(internal)?
            .result;
        core.spawn_update(Work::Files(vec![])); // the new node and doc are in `.vnoc/` now: the tree shows them
        Ok(result)
    })
}

/// `vnoc/relink`: see `identity::relink`. A refusal is an `INVALID_PARAMS` answer with the reason.
pub(super) fn relink(
    core: &mut Core,
    params: RelinkParams,
) -> impl std::future::Future<Output = Result<RelinkResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("relink", async move {
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core
            .state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let _one_writer = write_lock(project).await?;
        let env = Env {
            project,
            index: &core.state.index,
            host: &core.host,
            edits: EditSink::Editor(core.client.clone()),
        };
        let result = identity::relink(env, params)
            .await
            .map_err(|e| match e {
                identity::RelinkError::UserData(_) => internal(e),
                _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
            })?
            .result;
        core.spawn_update(Work::Files(vec![])); // node.toml moved: the tree shows the docs at the new place
        Ok(result)
    })
}

/// `vnoc/writeId`: see `identity::write_id`. A refusal is an `INVALID_PARAMS` answer with the reason.
pub(super) fn write_id(
    core: &mut Core,
    params: WriteIdParams,
) -> impl std::future::Future<Output = Result<WriteIdResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("writeId", async move {
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core
            .state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let _one_writer = write_lock(project).await?;
        let env = Env {
            project,
            index: &core.state.index,
            host: &core.host,
            edits: EditSink::Editor(core.client.clone()),
        };
        let result = identity::write_id(env, params)
            .await
            .map_err(|e| match e {
                identity::WriteIdError::UserData(_) => internal(e),
                _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
            })?
            .result;
        if result.written {
            core.spawn_update(Work::Files(vec![])); // node.toml changed: the node is found by its ID from now on
        }
        Ok(result)
    })
}

/// `vnoc/locate`: see `identity::locate`. Reads the index and asks the driver about the text it is given; changes nothing.
pub(super) fn locate(
    core: &mut Core,
    params: LocateParams,
) -> impl std::future::Future<Output = Result<Option<Located>, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("locate", async move {
        let project = core
            .state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        identity::locate(project, &core.state.index, &core.host, params)
            .await
            .map_err(|e| match e {
                identity::LocateError::Driver(_) => internal(e),
                _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
            })
    })
}

fn move_plan(plan: crate::userdata::MovePlan) -> MovePlan {
    MovePlan {
        from: plan.from.to_string_lossy().into_owned(),
        to: plan.to.to_string_lossy().into_owned(),
    }
}

fn trash_error(e: TrashError) -> ResponseError {
    match e {
        TrashError::UserData(_) => internal(e),
        _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
    }
}

/// `vnoc/trash`: Remove, step 1. Only a Missing node: a node that still has its code keeps its docs (to drop one, delete its `.md` file).
pub(super) fn trash(
    core: &mut Core,
    params: TrashParams,
) -> impl std::future::Future<Output = Result<MovePlan, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trash", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let id = NodeId::parse(&params.id)
            .map_err(|e| ResponseError::new(ErrorCode::INVALID_PARAMS, e))?;
        if !state.index.read().unwrap().is_missing(&id) {
            return Err(ResponseError::new(
                ErrorCode::INVALID_PARAMS,
                "that node is not Missing: only a node whose code is gone can be removed",
            ));
        }
        let data = UserData::new(project.git_root());
        let plan = tokio::task::spawn_blocking(move || data.trash_plan(&id))
            .await
            .map_err(internal)?
            .map_err(trash_error)?;
        Ok(move_plan(plan))
    })
}

/// `vnoc/trashList`: the removed nodes, newest first.
pub(super) fn trash_list(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<Vec<TrashItem>, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trashList", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let entries = tokio::task::spawn_blocking(move || data.trash_list())
            .await
            .map_err(internal)?;
        Ok(entries
            .into_iter()
            .map(|e| TrashItem {
                entry: e.entry,
                id: e.id.to_string(),
                removed_at: e.removed_at,
                docs: e.docs,
                name: e.name,
            })
            .collect())
    })
}

/// `vnoc/trashInfo`: where the trash is, how many entries and how many bytes: what Empty Trash asks about and deletes.
pub(super) fn trash_info(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<TrashInfoResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trashInfo", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let (folder, entries, bytes) = tokio::task::spawn_blocking(move || data.trash_info())
            .await
            .map_err(internal)?;
        Ok(TrashInfoResult {
            folder: folder.to_string_lossy().into_owned(),
            entries,
            bytes,
        })
    })
}

/// `vnoc/restorePlan`: Restore, step 2.
pub(super) fn restore_plan(
    core: &mut Core,
    params: RestorePlanParams,
) -> impl std::future::Future<Output = Result<MovePlan, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("restorePlan", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let plan = tokio::task::spawn_blocking(move || data.restore_plan(&params.entry))
            .await
            .map_err(internal)?
            .map_err(trash_error)?;
        Ok(move_plan(plan))
    })
}

/// `vnoc/docs`: the titles of a node's docs, read from the files.
pub(super) fn docs(
    core: &mut Core,
    params: DocsParams,
) -> impl std::future::Future<Output = Result<Vec<DocInfo>, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("docs", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let id = NodeId::parse(&params.id)
            .map_err(|e| ResponseError::new(ErrorCode::INVALID_PARAMS, e))?;
        let data = UserData::new(project.git_root());
        let docs = tokio::task::spawn_blocking(move || data.docs_of(&id))
            .await
            .map_err(internal)?;
        Ok(docs
            .into_iter()
            .map(|d| DocInfo {
                file: d.file,
                title: d.title,
                path: d.path.to_string_lossy().into_owned(),
                tags: d.tags,
                author: d.author,
            })
            .collect())
    })
}
