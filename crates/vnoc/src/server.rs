//! The editor-facing server: which messages it answers, and what it does with the ones it does not know.
//!
//! The four conditions of VS-D67 live here:
//! 1. **Catch-all handlers** for unknown notifications and requests: log and continue, never exit. (async-lsp's default
//!    breaks the whole main loop on an unknown notification that does not start with `$/`.)
//! 2. **Lifecycle** (`initialize`, `initialized`, `shutdown`; `exit` is the lifecycle layer's) is handled explicitly.
//! 3. A test sends unknown messages and a full lifecycle and checks the core is still alive (`tests/lifecycle.rs`).
//! 4. **Async is for I/O only**: every handler body runs under `watchdog::timed`, and blocking work (walking the disk)
//!    goes through `spawn_blocking`.
//!
//! The index is built in the background once the editor says `initialized`; `vnoc/structure` waits for that first build, and
//! every later change is announced with `vnoc/indexChanged`.

use std::fmt::Display;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use async_lsp::lsp_types::{InitializeParams, InitializeResult, InitializedParams, ServerCapabilities, ServerInfo, notification, request};
use async_lsp::router::Router;
use async_lsp::{ClientSocket, ErrorCode, ResponseError};

use crate::calls::{self, Merged};
use crate::driver_host::{DriverHost, HostError};
use crate::index::{Index, Touched, refresh};
use crate::project::Project;
use crate::identity::{self, Env};
use crate::protocol::{Attach, AttachParams, AttachResult, CallSite, CallTree, CallTreeNode, CallTreeParams, CallTreeResult, DocInfo, Docs, DocsParams, FilesChanged, FilesChangedParams, IndexChanged, IndexChangedParams, Ping, PingResult, ProjectInfo, ProjectInfoResult, Rebuild, Relink, RelinkParams, RelinkResult, RestorePlan, RestorePlanParams, Trash, TrashItem, TrashList, TrashParams, MovePlan, Stats, Structure, StructureResult, TrashInfo, TrashInfoResult, WriteId, WriteIdParams, WriteIdResult};
use crate::userdata::{TrashError, UserData};
use vnoc_domain::{LocationKey, NodeId};
use vnoc_driver_protocol::CallEntry;
use crate::watchdog::timed;

/// Where the first build is.
#[derive(Clone)]
enum Status {
    Building,
    Ready,
    /// the build could not run (no folder, no driver, a broken driver); the message is what `vnoc/structure` answers with
    Failed(String),
}

/// What an update has to read again.
enum Work {
    Everything,
    Files(Vec<PathBuf>),
}

/// Shared between the handlers and the background tasks they start.
struct State {
    project: OnceLock<Project>,
    index: RwLock<Index>,
    status: tokio::sync::watch::Sender<Status>,
    /// Updates run one at a time: they read the index at the start and write it at the end.
    one_at_a_time: tokio::sync::Mutex<()>,
    /// One attach at a time: two at once could both pick the same ID.
    attaching: tokio::sync::Mutex<()>,
    /// An "everything" update is waiting or about to start. A second request for one (a double click on Refresh) is dropped.
    everything_pending: AtomicBool,
}

#[derive(Clone)]
pub struct Core {
    client: ClientSocket,
    host: Arc<DriverHost>,
    state: Arc<State>,
}

impl Core {
    fn new(client: ClientSocket) -> Core {
        let state = State {
            project: OnceLock::new(),
            index: RwLock::default(),
            status: tokio::sync::watch::channel(Status::Building).0,
            one_at_a_time: tokio::sync::Mutex::new(()),
            attaching: tokio::sync::Mutex::new(()),
            everything_pending: AtomicBool::new(false),
        };
        Core { client, host: Arc::default(), state: Arc::new(state) }
    }

    /// Starts an update in the background.
    fn spawn_update(&self, work: Work) {
        if matches!(work, Work::Everything) && self.state.everything_pending.swap(true, Ordering::AcqRel) {
            tracing::debug!("a full read is already waiting: dropped");
            return;
        }
        let core = self.clone();
        tokio::spawn(async move { core.update(work).await });
    }

    async fn update(&self, work: Work) {
        let Some(project) = self.state.project.get() else { return };
        let _only_one = self.state.one_at_a_time.lock().await;
        if matches!(work, Work::Everything) {
            self.state.everything_pending.store(false, Ordering::Release);
        }
        // until one build has worked, every update is a full one (this also retries a build that failed)
        let first = !matches!(*self.state.status.borrow(), Status::Ready);
        let result = match &work {
            Work::Files(paths) if !first => refresh(project, &self.host, &self.state.index, Touched::Files(paths)).await,
            _ => refresh(project, &self.host, &self.state.index, Touched::All).await,
        };
        match result {
            Ok(changed) => {
                self.state.status.send_replace(Status::Ready);
                if changed || first {
                    self.announce();
                }
            }
            // after a first success the old index stays; the driver was dropped, so the next update starts a new one
            Err(e) if first => {
                tracing::error!("the first build failed: {e}");
                self.state.status.send_replace(Status::Failed(e.to_string()));
            }
            Err(e) => tracing::warn!("update failed, the index is as it was: {e}"),
        }
    }

    fn announce(&self) {
        let revision = self.state.index.read().unwrap().revision();
        if let Err(e) = self.client.notify::<IndexChanged>(IndexChangedParams { revision }) {
            tracing::warn!("could not tell the editor about revision {revision}: {e}");
        }
    }
}

fn internal(e: impl Display) -> ResponseError {
    ResponseError::new(ErrorCode::INTERNAL_ERROR, e)
}

pub fn router(client: ClientSocket) -> Router<Core> {
    let mut router = Router::new(Core::new(client));
    router
        // 2. lifecycle
        .request::<request::Initialize, _>(initialize)
        .request::<request::Shutdown, _>(|_, _| async { Ok(()) })
        .notification::<notification::Initialized>(initialized)
        .request::<Ping, _>(|_, _ignored| {
            timed("ping", async { Ok(PingResult { pong: true, pid: std::process::id(), version: env!("CARGO_PKG_VERSION").to_string() }) })
        })
        .request::<Structure, _>(structure)
        .request::<ProjectInfo, _>(project_info)
        .request::<Attach, _>(attach)
        .request::<Relink, _>(relink)
        .request::<WriteId, _>(write_id)
        .request::<Trash, _>(trash)
        .request::<TrashList, _>(trash_list)
        .request::<TrashInfo, _>(trash_info)
        .request::<RestorePlan, _>(restore_plan)
        .request::<Docs, _>(docs)
        .request::<CallTree, _>(call_tree)
        .notification::<FilesChanged>(files_changed)
        .notification::<Rebuild>(|core, _| {
            core.spawn_update(Work::Everything);
            ControlFlow::Continue(())
        })
        // 1. catch-alls: log and continue, never exit
        .unhandled_notification(|_, n| {
            tracing::warn!("ignored notification `{}`", n.method);
            ControlFlow::Continue(())
        })
        .unhandled_request(|_, r| {
            let method = r.method.clone();
            async move { Err(ResponseError::new(ErrorCode::METHOD_NOT_FOUND, format!("unknown method `{method}`"))) }
        });
    router
}

/// (`use<>`: the returned future captures nothing from `core`; async-lsp needs it independent of the `&mut` borrow.)
///
/// `initialize` reads `initializationOptions`: `driverPath` (the extension resolves it, and ignores workspace settings in
/// Restricted Mode; the `VNOC_DRIVER` environment variable is a fallback for tests and terminals) and `root`, the opened folder.
fn initialize(core: &mut Core, params: InitializeParams) -> impl std::future::Future<Output = Result<InitializeResult, ResponseError>> + Send + use<> {
    let option = |key: &str| params.initialization_options.as_ref().and_then(|o| o.get(key)).and_then(|v| v.as_str()).map(PathBuf::from);
    core.host.set_path(option("driverPath").or_else(|| std::env::var_os("VNOC_DRIVER").map(PathBuf::from)));
    match option("root").map(|root| Project::open(&root)) {
        Some(Ok(project)) => {
            let _ = core.state.project.set(project);
        }
        Some(Err(e)) => {
            core.state.status.send_replace(Status::Failed(e.to_string()));
        }
        None => {
            core.state.status.send_replace(Status::Failed("no folder is open: V-NOC shows the code of the folder you opened".into()));
        }
    }
    timed("initialize", async {
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo { name: "vnoc".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }),
        })
    })
}

/// The editor is ready for notifications: build the index in the background, and say so when it is there.
fn initialized(core: &mut Core, _: InitializedParams) -> ControlFlow<async_lsp::Result<()>> {
    core.spawn_update(Work::Everything);
    ControlFlow::Continue(())
}

fn files_changed(core: &mut Core, params: FilesChangedParams) -> ControlFlow<async_lsp::Result<()>> {
    core.spawn_update(Work::Files(params.paths.into_iter().map(PathBuf::from).collect()));
    ControlFlow::Continue(())
}

/// `vnoc/structure`: the tree, from the index. Waits for the first build.
fn structure(core: &mut Core, _ignored: serde_json::Value) -> impl std::future::Future<Output = Result<StructureResult, ResponseError>> + Send + use<> {
    let (state, host) = (core.state.clone(), core.host.clone());
    timed("structure", async move {
        let status = state.status.subscribe().wait_for(|s| !matches!(s, Status::Building)).await.map_err(internal)?.clone();
        if let Status::Failed(message) = status {
            return Err(internal(message));
        }
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let capabilities = host.capabilities().await;
        let index = state.index.read().unwrap();
        let root = index.tree(&project.name(), project.prefix());
        let [folders, files, classes, functions] = root.counts();
        Ok(StructureResult {
            revision: index.revision(),
            git_root: project.git_root().to_string_lossy().into_owned(),
            capabilities,
            root,
            stats: Stats { folders: folders.saturating_sub(1), files, classes, functions }, // the opened folder is not a folder under itself
            truncated: index.truncated(),
            failed: index.failed(),
            missing: index.missing(),
            problems: index.problems(project.git_root()),
        })
    })
}

/// `vnoc/projectInfo`: whether the first-attach question still has to be asked.
fn project_info(core: &mut Core, _ignored: serde_json::Value) -> impl std::future::Future<Output = Result<ProjectInfoResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("projectInfo", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let settings = UserData::new(project.git_root()).settings().map_err(internal)?;
        Ok(ProjectInfoResult { initialized: settings.initialized, markers: settings.markers })
    })
}

/// `vnoc/attach`: see `identity::attach`.
fn attach(core: &mut Core, params: AttachParams) -> impl std::future::Future<Output = Result<AttachResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("attach", async move {
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core.state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let env = Env { project, index: &core.state.index, host: &core.host, client: core.client.clone() };
        let result = identity::attach(env, params).await.map_err(internal)?;
        core.spawn_update(Work::Files(vec![])); // the new node and doc are in `.vnoc/` now: the tree shows them
        Ok(result)
    })
}

/// `vnoc/relink`: see `identity::relink`. A refusal is an `INVALID_PARAMS` answer with the reason.
fn relink(core: &mut Core, params: RelinkParams) -> impl std::future::Future<Output = Result<RelinkResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("relink", async move {
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core.state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let env = Env { project, index: &core.state.index, host: &core.host, client: core.client.clone() };
        let result = identity::relink(env, params).await.map_err(|e| match e {
            identity::RelinkError::UserData(_) => internal(e),
            _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
        })?;
        core.spawn_update(Work::Files(vec![])); // node.toml moved: the tree shows the docs at the new place
        Ok(result)
    })
}

/// `vnoc/writeId`: see `identity::write_id`. A refusal is an `INVALID_PARAMS` answer with the reason.
fn write_id(core: &mut Core, params: WriteIdParams) -> impl std::future::Future<Output = Result<WriteIdResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("writeId", async move {
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let project = core.state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let env = Env { project, index: &core.state.index, host: &core.host, client: core.client.clone() };
        let result = identity::write_id(env, params).await.map_err(|e| match e {
            identity::WriteIdError::UserData(_) => internal(e),
            _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
        })?;
        if result.written {
            core.spawn_update(Work::Files(vec![])); // node.toml changed: the node is found by its ID from now on
        }
        Ok(result)
    })
}

fn move_plan(plan: crate::userdata::MovePlan) -> MovePlan {
    MovePlan { from: plan.from.to_string_lossy().into_owned(), to: plan.to.to_string_lossy().into_owned() }
}

fn trash_error(e: TrashError) -> ResponseError {
    match e {
        TrashError::UserData(_) => internal(e),
        _ => ResponseError::new(ErrorCode::INVALID_PARAMS, e),
    }
}

/// `vnoc/trash`: Remove, step 1. Only a Missing node: a node that still has its code keeps its docs (to drop one, delete its `.md` file).
fn trash(core: &mut Core, params: TrashParams) -> impl std::future::Future<Output = Result<MovePlan, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trash", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let id = NodeId::parse(&params.id).map_err(|e| ResponseError::new(ErrorCode::INVALID_PARAMS, e))?;
        if !state.index.read().unwrap().is_missing(&id) {
            return Err(ResponseError::new(ErrorCode::INVALID_PARAMS, "that node is not Missing: only a node whose code is gone can be removed"));
        }
        let data = UserData::new(project.git_root());
        let plan = tokio::task::spawn_blocking(move || data.trash_plan(&id)).await.map_err(internal)?.map_err(trash_error)?;
        Ok(move_plan(plan))
    })
}

/// `vnoc/trashList`: the removed nodes, newest first.
fn trash_list(core: &mut Core, _ignored: serde_json::Value) -> impl std::future::Future<Output = Result<Vec<TrashItem>, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trashList", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let entries = tokio::task::spawn_blocking(move || data.trash_list()).await.map_err(internal)?;
        Ok(entries.into_iter().map(|e| TrashItem { entry: e.entry, id: e.id.to_string(), removed_at: e.removed_at, docs: e.docs, name: e.name }).collect())
    })
}

/// `vnoc/trashInfo`: where the trash is, how many entries and how many bytes: what Empty Trash asks about and deletes.
fn trash_info(core: &mut Core, _ignored: serde_json::Value) -> impl std::future::Future<Output = Result<TrashInfoResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("trashInfo", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let (folder, entries, bytes) = tokio::task::spawn_blocking(move || data.trash_info()).await.map_err(internal)?;
        Ok(TrashInfoResult { folder: folder.to_string_lossy().into_owned(), entries, bytes })
    })
}

/// `vnoc/restorePlan`: Restore, step 2.
fn restore_plan(core: &mut Core, params: RestorePlanParams) -> impl std::future::Future<Output = Result<MovePlan, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("restorePlan", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let data = UserData::new(project.git_root());
        let plan = tokio::task::spawn_blocking(move || data.restore_plan(&params.entry)).await.map_err(internal)?.map_err(trash_error)?;
        Ok(move_plan(plan))
    })
}

/// `vnoc/docs`: the titles of a node's docs, read from the files.
fn docs(core: &mut Core, params: DocsParams) -> impl std::future::Future<Output = Result<Vec<DocInfo>, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("docs", async move {
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;
        let id = NodeId::parse(&params.id).map_err(|e| ResponseError::new(ErrorCode::INVALID_PARAMS, e))?;
        let data = UserData::new(project.git_root());
        let docs = tokio::task::spawn_blocking(move || data.docs_of(&id)).await.map_err(internal)?;
        Ok(docs.into_iter().map(|d| DocInfo { file: d.file, title: d.title, path: d.path.to_string_lossy().into_owned() }).collect())
    })
}

/// `vnoc/callTree`: the driver's tree for the node `key`, merged, with each target given the key the index knows it by.
/// A request the editor cancels is dropped by the lifecycle layer, which drops the driver request under it, and that cancels the driver's work.
fn call_tree(core: &mut Core, params: CallTreeParams) -> impl std::future::Future<Output = Result<CallTreeResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("callTree", async move {
        let state = &core.state;
        let status = state.status.subscribe().wait_for(|s| !matches!(s, Status::Building)).await.map_err(internal)?.clone();
        if let Status::Failed(message) = status {
            return Err(internal(message));
        }
        let project = state.project.get().ok_or_else(|| internal("no folder is open"))?;

        let invalid = |message: String| ResponseError::new(ErrorCode::INVALID_PARAMS, message);
        if !params.key.contains("::") {
            return Err(invalid(format!("`{}` is not a function or a class: a call tree starts at one", params.key)));
        }
        let location: LocationKey = params.key.parse().map_err(|e| invalid(format!("{e}")))?;
        if state.index.read().unwrap().def_at(&location).is_none() {
            return Err(invalid(format!("`{}` is not in the index (is the file saved, and is it Python?)", params.key)));
        }

        // The driver is given the opened folder: it is the project the user sees, and what its imports resolve against. Its paths are then
        // relative to that folder, like the editor's; a repo path (what the index's keys are made of) is the folder's prefix and the path.
        let inside = location.path().as_str().strip_prefix(project.prefix()).map(|p| p.trim_start_matches('/'));
        let file = inside.filter(|p| !p.is_empty()).ok_or_else(|| internal(format!("`{}` is not under the opened folder", params.key)))?;
        let asked = vnoc_driver_protocol::CallTreeParams {
            root: project.open_dir().to_string_lossy().into_owned(),
            file: file.to_string(),
            entry: CallEntry { qualname: location.qualname().to_string(), occurrence: location.occurrence() },
            depth: None,
        };
        let answer = core.host.call_tree(&asked).await.map_err(|e| match e {
            HostError::MissingCapability(_) => ResponseError::new(ErrorCode::REQUEST_FAILED, format!("{e}: call trees need a driver that has `callTree`")),
            HostError::Remote { code: -32602, message } => invalid(message),
            other => internal(other),
        })?;

        let merged = calls::merge(&answer.root);
        // one read of the index for the whole mapping: the revision says which index the keys are from
        let index = state.index.read().unwrap();
        let known = |target: &vnoc_driver_protocol::CallTarget| {
            let location = LocationKey::new(project.repo_path(target.file.as_deref()?).ok()?, target.qualname.clone(), target.occurrence).ok()?;
            index.def_at(&location).map(|_| location.to_string())
        };
        let root = tree_node(&merged, &known);
        Ok(CallTreeResult { revision: index.revision(), root })
    })
}

/// A merged node as the editor gets it. `known`: the key of a target the index has.
fn tree_node(node: &Merged, known: &dyn Fn(&vnoc_driver_protocol::CallTarget) -> Option<String>) -> CallTreeNode {
    let target = &node.target;
    CallTreeNode {
        key: known(target),
        name: target.qualname.rsplit('.').next().unwrap_or(&target.qualname).to_string(),
        qualname: target.qualname.clone(),
        path: target.file.clone(), // already relative to the opened folder, `null` outside it
        kind: target.kind,
        call_count: node.call_count,
        call_sites: node.call_sites.iter().map(|site| CallSite { path: site.file.clone(), range: site.range }).collect(),
        marks: node.marks.clone(),
        children: node.children.iter().map(|child| tree_node(child, known)).collect(),
    }
}
