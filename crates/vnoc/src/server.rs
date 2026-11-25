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
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use async_lsp::lsp_types::{
    InitializeParams, InitializeResult, InitializedParams, ServerCapabilities, ServerInfo,
    notification, request,
};
use async_lsp::router::Router;
use async_lsp::{ClientSocket, ErrorCode, ResponseError};

use crate::calls::{self, CallsError};
use crate::cli::project::discover;
use crate::driver_host::{DriverHost, LanguageState};
use crate::identity::{self, EditSink, Env};
use crate::index::{self, Index, Touched, refresh};
use crate::project::Project;
use crate::protocol;
use crate::protocol::{
    Attach, AttachParams, AttachResult, CallTree, CallTreeParams, CallTreeResult, DocInfo, Docs,
    DocsParams, FilesChanged, FilesChangedParams, HasCalls, HasCallsParams, HasCallsResult,
    IndexChanged, IndexChangedParams, LanguageInfo, Languages, MovePlan, Ping, PingResult,
    ProjectInfo, ProjectInfoResult, Rebuild, Relink, RelinkParams, RelinkResult, RestorePlan,
    RestorePlanParams, Search, SearchCards, SearchCardsParams, SearchCardsResult, SearchParams,
    SearchResult, Structure, StructureResult, Trash, TrashInfo, TrashInfoResult, TrashItem,
    TrashList, TrashParams, WriteId, WriteIdParams, WriteIdResult,
};
use crate::protocol::{Locate, LocateParams, Located};
use crate::search::{Prepared, answer, prepare};
use crate::tours::act::{self, ActError};
use crate::tours::check::{Answer, Mode};
use crate::tours::find;
use crate::tours::on::{self, Query};
use crate::tours::place::Move;
use crate::tours::record::{StopId, TourId};
use crate::tours::route;
use crate::tours::serve::{self, Cannot, GoError, Touch, Tours};
use crate::tours::settings::{Choice, Key};
use crate::tours::store::{self, Halt, StoreError};
use crate::tours::view;
use crate::userdata::{LockError, TrashError, UserData, WriteLock};
use crate::watchdog::timed;
use vnoc_domain::NodeId;
use vnoc_search::{Corpus, DocIndex, Limits, matches, run};

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

/// What a corpus is made for: the index revision and the doc-text generation.
type CorpusKey = (u64, u64);

/// Shared between the handlers and the background tasks they start.
struct State {
    project: OnceLock<Project>,
    index: RwLock<Index>,
    status: tokio::sync::watch::Sender<Status>,
    /// Updates run one at a time: they read the index at the start and write it at the end.
    one_at_a_time: tokio::sync::Mutex<()>,
    /// One attach at a time: two at once could both pick the same ID.
    attaching: tokio::sync::Mutex<()>,
    /// The tours whose calls (`vnoc/tourCalls`) are being counted now, each by a task of its own.
    calls_running: Mutex<std::collections::BTreeSet<TourId>>,
    /// An "everything" update is waiting or about to start. A second request for one (a double click on Refresh) is dropped.
    everything_pending: AtomicBool,
    /// The corpus `vnoc/search` reads, kept with the index revision and the doc-text generation it was made for (`search::kept`). Taken after the index
    /// lock, never before it, and by nothing else.
    corpus: Mutex<Option<(CorpusKey, Arc<Corpus>)>>,
    /// The doc index, kept under the doc-text generation (03 §9). Taken inside `corpus`'s lock (while a corpus is built), never the other way round.
    doc_index: Mutex<Option<(u64, Arc<DocIndex>)>>,
    /// The tours the core has replayed, the answers it gave about them, and where it last put each place. Not the index's: a tour never moves the revision.
    tours: Tours,
}

#[derive(Clone)]
pub struct Core {
    client: ClientSocket,
    host: Arc<DriverHost>,
    state: Arc<State>,
}

impl State {
    fn new() -> State {
        State {
            project: OnceLock::new(),
            index: RwLock::default(),
            status: tokio::sync::watch::channel(Status::Building).0,
            one_at_a_time: tokio::sync::Mutex::new(()),
            attaching: tokio::sync::Mutex::new(()),
            calls_running: Mutex::new(std::collections::BTreeSet::new()),
            everything_pending: AtomicBool::new(false),
            corpus: Mutex::new(None),
            doc_index: Mutex::new(None),
            tours: Tours::default(),
        }
    }
}

impl Core {
    fn new(client: ClientSocket) -> Core {
        Core {
            client,
            host: Arc::default(),
            state: Arc::new(State::new()),
        }
    }

    /// Starts an update in the background.
    fn spawn_update(&self, work: Work) {
        if matches!(work, Work::Everything)
            && self.state.everything_pending.swap(true, Ordering::AcqRel)
        {
            tracing::debug!("a full read is already waiting: dropped");
            return;
        }
        let core = self.clone();
        tokio::spawn(async move { core.update(work).await });
    }

    async fn update(&self, work: Work) {
        let Some(project) = self.state.project.get() else {
            return;
        };
        let _only_one = self.state.one_at_a_time.lock().await;
        if matches!(work, Work::Everything) {
            self.state
                .everything_pending
                .store(false, Ordering::Release);
        }
        // until one build has worked, every update is a full one (this also retries a build that failed)
        let first = !matches!(*self.state.status.borrow(), Status::Ready);
        let result = match &work {
            Work::Files(paths) if !first => {
                refresh(
                    project,
                    &self.host,
                    &self.state.index,
                    Touched::Files(paths),
                )
                .await
            }
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
                self.state
                    .status
                    .send_replace(Status::Failed(e.to_string()));
            }
            Err(e) => tracing::warn!("update failed, the index is as it was: {e}"),
        }
    }

    fn announce(&self) {
        let revision = self.state.index.read().unwrap().revision();
        if let Err(e) = self
            .client
            .notify::<IndexChanged>(IndexChangedParams { revision })
        {
            tracing::warn!("could not tell the editor about revision {revision}: {e}");
        }
    }
}

/// The project's write lock for the length of one attach, relink or write-ID: the mutex keeps two of them in this process apart, this keeps a
/// command (another process) out. Taken after the mutex, so a holder of the lock never waits for the mutex. `None`: no `.vnoc/` yet.
/// Busy is `REQUEST_FAILED`: the request was fine, the project was not free.
async fn write_lock(project: &Project) -> Result<Option<WriteLock>, ResponseError> {
    let data = UserData::new(project.git_root());
    tokio::task::spawn_blocking(move || data.write_lock()) // it sleeps between tries: not on a runtime thread
        .await
        .map_err(internal)?
        .map_err(|e| match e {
            LockError::Busy => ResponseError::new(ErrorCode::REQUEST_FAILED, e),
            LockError::Io(_) => internal(e),
        })
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
            timed("ping", async {
                Ok(PingResult {
                    pong: true,
                    pid: std::process::id(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                })
            })
        })
        .request::<Structure, _>(structure)
        .request::<Languages, _>(languages)
        .request::<Search, _>(search)
        .request::<SearchCards, _>(search_cards)
        .request::<ProjectInfo, _>(project_info)
        .request::<Attach, _>(attach)
        .request::<Relink, _>(relink)
        .request::<WriteId, _>(write_id)
        .request::<Locate, _>(locate)
        .request::<Trash, _>(trash)
        .request::<TrashList, _>(trash_list)
        .request::<TrashInfo, _>(trash_info)
        .request::<RestorePlan, _>(restore_plan)
        .request::<Docs, _>(docs)
        .request::<CallTree, _>(call_tree)
        .request::<HasCalls, _>(has_calls)
        .request::<protocol::Tours, _>(tours)
        .request::<protocol::Tour, _>(tour)
        .request::<protocol::TourGo, _>(tour_go)
        .request::<protocol::TourOpen, _>(tour_open)
        .request::<protocol::TourHalt, _>(tour_halt)
        .request::<protocol::TourCode, _>(tour_code)
        .request::<protocol::TourCounts, _>(tour_counts)
        .request::<protocol::TourCalls, _>(tour_calls)
        .request::<protocol::TourPin, _>(tour_pin)
        .request::<protocol::TourIds, _>(tour_ids)
        .request::<protocol::TourEdit, _>(tour_edit)
        .request::<protocol::TourShare, _>(tour_share)
        .request::<protocol::TourDelete, _>(tour_delete)
        .request::<protocol::TourRestore, _>(tour_restore)
        .request::<protocol::TourSettings, _>(tour_settings)
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
            async move {
                Err(ResponseError::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("unknown method `{method}`"),
                ))
            }
        });
    router
}

/// (`use<>`: the returned future captures nothing from `core`; async-lsp needs it independent of the `&mut` borrow.)
///
/// `initialize` reads `initializationOptions`: the drivers, as `drivers` (a list of paths) or else `driverPath` (one; the extension resolves it,
/// and ignores workspace settings in Restricted Mode), else the `VNOC_DRIVERS` environment variable (a list), else `VNOC_DRIVER`, else every
/// driver beside the binary (`cli::project::discover`); and `root`, the opened folder.
fn initialize(
    core: &mut Core,
    params: InitializeParams,
) -> impl std::future::Future<Output = Result<InitializeResult, ResponseError>> + Send + use<> {
    let option = |key: &str| {
        params
            .initialization_options
            .as_ref()
            .and_then(|o| o.get(key))
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
    };
    let explicit: Vec<PathBuf> = match params
        .initialization_options
        .as_ref()
        .and_then(|o| o.get("drivers"))
        .and_then(|v| v.as_array())
    {
        Some(list) => list
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .collect(),
        None => option("driverPath").into_iter().collect(),
    };
    core.host.set_paths(discover(
        &explicit,
        std::env::var_os("VNOC_DRIVERS").as_deref(),
        std::env::var_os("VNOC_DRIVER").as_deref(),
        std::env::current_exe().ok().as_deref(),
    ));
    match option("root").map(|root| Project::open(&root)) {
        Some(Ok(project)) => {
            let _ = core.state.project.set(project);
        }
        Some(Err(e)) => {
            core.state
                .status
                .send_replace(Status::Failed(e.to_string()));
        }
        None => {
            core.state.status.send_replace(Status::Failed(
                "no folder is open: V-NOC shows the code of the folder you opened".into(),
            ));
        }
    }
    timed("initialize", async {
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo {
                name: "vnoc".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    })
}

/// The editor is ready for notifications: build the index in the background, and say so when it is there.
fn initialized(core: &mut Core, _: InitializedParams) -> ControlFlow<async_lsp::Result<()>> {
    core.spawn_update(Work::Everything);
    ControlFlow::Continue(())
}

/// `vnoc/filesChanged`: the split of 03 §8. A path of the tours store, or of a tour's state, is the tours' and starts no index update (D12); the tours lock is
/// nobody's; any other path is the index's as it always was, and also drops what the core answered about the tours that read it. A batch of only tour paths
/// starts no update and moves no revision; a mixed batch updates for the other paths only; an empty batch is "look again", as it was.
fn files_changed(
    core: &mut Core,
    params: FilesChangedParams,
) -> ControlFlow<async_lsp::Result<()>> {
    let paths: Vec<PathBuf> = params.paths.into_iter().map(PathBuf::from).collect();
    let Some(project) = core.state.project.get() else {
        core.spawn_update(Work::Files(paths));
        return ControlFlow::Continue(());
    };
    let vnoc_root = store::vnoc_root(project.git_root());
    let mut told = ToursTold::default();
    let mut others = Vec::new();
    for path in paths.iter() {
        match serve::classify(&vnoc_root, path) {
            Touch::Tour(Some(id)) => {
                told.replay.insert(id);
            }
            Touch::Tour(None) => told.rescan = true,
            Touch::State(id) => {
                told.places.insert(id);
            }
            Touch::Ignored => {}
            Touch::Other => others.push(path.clone()),
        }
    }
    told.read = others
        .iter()
        .filter_map(|p| project.repo_path_of(p))
        .map(|p| p.as_str().to_string())
        .collect();
    let update = paths.is_empty() || !others.is_empty();
    if update && told.is_empty() {
        core.spawn_update(Work::Files(others));
    } else if update {
        // the tours are told after the index has taken the files in: a check made before would read the old index (a function that was deleted is still there,
        // and the answer, recorded, would say `changed`)
        let core = core.clone();
        tokio::spawn(async move {
            core.update(Work::Files(others)).await;
            core.tours_told(told).await;
        });
    } else if !told.is_empty() {
        let core = core.clone();
        tokio::spawn(async move { core.tours_told(told).await });
    }
    ControlFlow::Continue(())
}

/// What a batch of file changes said to the tours.
#[derive(Default)]
struct ToursTold {
    /// tours whose file changed: replayed again
    replay: std::collections::BTreeSet<TourId>,
    /// a folder of the tours store changed in a way that names no tour: look at both again
    rescan: bool,
    /// tours whose state file changed
    places: std::collections::BTreeSet<TourId>,
    /// the project's files that changed (repo paths): the answers that read them are dropped
    read: Vec<String>,
}

impl ToursTold {
    fn is_empty(&self) -> bool {
        self.replay.is_empty() && !self.rescan && self.places.is_empty() && self.read.is_empty()
    }
}

impl Core {
    /// Does what a batch of file changes means to the tours, then tells the editor: `vnoc/toursChanged` for the tours that changed or whose answer no longer
    /// holds, `vnoc/tourPlaceChanged` for a place somebody else moved.
    async fn tours_told(&self, told: ToursTold) {
        let state = self.state.clone();
        let done = tokio::task::spawn_blocking(move || {
            let project = state.project.get()?;
            let vnoc_root = store::vnoc_root(project.git_root());
            let mut changed = std::collections::BTreeSet::new();
            for id in &told.replay {
                state.tours.replay(&vnoc_root, id);
                changed.insert(id.clone());
            }
            if told.rescan {
                changed.extend(state.tours.sync(&vnoc_root).1);
            }
            changed.extend(state.tours.drop_answers_reading(&told.read));
            let moved: Vec<_> = told
                .places
                .iter()
                .filter_map(|id| {
                    let place = state.tours.place_moved(&vnoc_root, id)?;
                    Some((id.clone(), state.tours.place_view(&vnoc_root, id, place)))
                })
                .collect();
            Some((changed, moved))
        })
        .await;
        let Ok(Some((changed, moved))) = done else {
            return;
        };
        if !changed.is_empty() {
            let ids = changed.iter().map(ToString::to_string).collect();
            if let Err(e) = self
                .client
                .notify::<protocol::ToursChanged>(protocol::ToursChangedParams { ids })
            {
                tracing::warn!("could not tell the editor that tours changed: {e}");
            }
        }
        for (id, place) in moved {
            let params = protocol::TourPlaceChangedParams {
                id: id.to_string(),
                place,
            };
            if let Err(e) = self.client.notify::<protocol::TourPlaceChanged>(params) {
                tracing::warn!("could not tell the editor that a place moved: {e}");
            }
        }
    }
}

/// `vnoc/structure`: the tree, from the index. Waits for the first build.
fn structure(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<StructureResult, ResponseError>> + Send + use<> {
    let (state, host) = (core.state.clone(), core.host.clone());
    timed("structure", async move {
        let status = state
            .status
            .subscribe()
            .wait_for(|s| !matches!(s, Status::Building))
            .await
            .map_err(internal)?
            .clone();
        if let Status::Failed(message) = status {
            return Err(internal(message));
        }
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let capabilities = host.capabilities().await;
        let index = state.index.read().unwrap();
        Ok(index::structure(&index, project, capabilities))
    })
}

/// `vnoc/languages`: the drivers and what each said. Waits for the first build (it starts every driver), and answers whether or not it worked: a
/// driver that failed is in the list with its reason, which is what the editor shows.
fn languages(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<Vec<LanguageInfo>, ResponseError>> + Send + use<> {
    let (state, host) = (core.state.clone(), core.host.clone());
    timed("languages", async move {
        state
            .status
            .subscribe()
            .wait_for(|s| !matches!(s, Status::Building))
            .await
            .map_err(internal)?;
        Ok(host
            .languages()
            .await
            .into_iter()
            .map(|l| {
                let (state, error) = match l.state {
                    LanguageState::Running => (protocol::LanguageState::Running, None),
                    LanguageState::Failed(why) => (protocol::LanguageState::Failed, Some(why)),
                };
                LanguageInfo {
                    name: l.name,
                    version: l.version,
                    path: l.path.to_string_lossy().into_owned(),
                    extensions: l.extensions,
                    capabilities: l.capabilities,
                    state,
                    error,
                    warnings: l.warnings,
                }
            })
            .collect())
    })
}

/// Waits for the first build, then the query read and its corpus (`search::prepare`): what `vnoc/search` and `vnoc/searchCards` both start from.
/// No `.await` while a lock is held: the waiting is done before `prepare` takes any.
async fn prepared(state: &State, text: &str) -> Result<Prepared, ResponseError> {
    let status = state
        .status
        .subscribe()
        .wait_for(|s| !matches!(s, Status::Building))
        .await
        .map_err(internal)?
        .clone();
    if let Status::Failed(message) = status {
        return Err(internal(message));
    }
    let project = state
        .project
        .get()
        .ok_or_else(|| internal("no folder is open"))?;
    Ok(prepare(
        project,
        &state.index,
        &state.corpus,
        &state.doc_index,
        text,
    ))
}

/// `vnoc/search`: the nodes a query finds. The corpus is built from the tree on the first search after a change and kept (`search::kept`); the search
/// itself runs on it with no lock held.
fn search(
    core: &mut Core,
    params: SearchParams,
) -> impl std::future::Future<Output = Result<SearchResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("search", async move {
        let found = prepared(&state, &params.query).await?;
        let defaults = Limits::default();
        let limits = Limits {
            nodes: params.nodes.unwrap_or(defaults.nodes),
            docs: params.docs.unwrap_or(defaults.docs),
        };
        let results = run(&found.corpus, &found.query, found.exact, limits);
        Ok(answer(found.revision, &found.corpus, &found.query, results))
    })
}

/// `vnoc/searchCards`: which of the asked node ids the query finds (`vnoc_search::matches`).
fn search_cards(
    core: &mut Core,
    params: SearchCardsParams,
) -> impl std::future::Future<Output = Result<SearchCardsResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("searchCards", async move {
        let found = prepared(&state, &params.query).await?;
        let asked: Vec<&str> = params.ids.iter().map(String::as_str).collect();
        let hit = matches(&found.corpus, &found.query, found.exact, &asked);
        Ok(SearchCardsResult {
            revision: found.revision,
            hits: params
                .ids
                .iter()
                .zip(hit)
                .filter(|&(_, hit)| hit)
                .map(|(id, _)| id.clone())
                .collect(),
        })
    })
}

/// `vnoc/projectInfo`: whether the first-attach question still has to be asked.
fn project_info(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<ProjectInfoResult, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("projectInfo", async move {
        let project = state
            .project
            .get()
            .ok_or_else(|| internal("no folder is open"))?;
        let settings = UserData::new(project.git_root())
            .settings()
            .map_err(internal)?;
        Ok(ProjectInfoResult {
            initialized: settings.initialized,
            markers: settings.markers,
        })
    })
}

/// `vnoc/attach`: see `identity::attach`.
fn attach(
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
fn relink(
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
fn write_id(
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
fn locate(
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
fn trash(
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
fn trash_list(
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
fn trash_info(
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
fn restore_plan(
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
fn docs(
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

/// `vnoc/callTree`: the driver's tree for the node `key`, merged, with each target given the key the index knows it by.
/// A request the editor cancels is dropped by the lifecycle layer, which drops the driver request under it, and that cancels the driver's work.
/// Every full tree is asked for with a depth cap (`calls::DEPTH_CAP`): a node at it is `cut`, and opening it asks for its own tree.
fn call_tree(
    core: &mut Core,
    params: CallTreeParams,
) -> impl std::future::Future<Output = Result<CallTreeResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("callTree", async move {
        let project = ready_project(&core.state).await?;
        calls::tree_for(
            project,
            &core.state.index,
            &core.host,
            &params.key,
            Some(calls::DEPTH_CAP),
        )
        .await
        .map_err(calls_error)
    })
}

/// `vnoc/hasCalls`: how many nodes opening `key` would show. The driver is asked for one level, and the answer is merged exactly as `vnoc/callTree`
/// merges its own; only the count leaves the core.
fn has_calls(
    core: &mut Core,
    params: HasCallsParams,
) -> impl std::future::Future<Output = Result<HasCallsResult, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("hasCalls", async move {
        let project = ready_project(&core.state).await?;
        let answer = calls::ask(project, &core.state.index, &core.host, &params.key, Some(1))
            .await
            .map_err(calls_error)?;
        let revision = core.state.index.read().unwrap().revision();
        Ok(HasCallsResult {
            revision,
            count: calls::count_calls(&answer.root),
        })
    })
}

/// Waits for the first build and gives the project: what every call-tree request starts with.
async fn ready_project(state: &State) -> Result<&Project, ResponseError> {
    let status = state
        .status
        .subscribe()
        .wait_for(|s| !matches!(s, Status::Building))
        .await
        .map_err(internal)?
        .clone();
    if let Status::Failed(message) = status {
        return Err(internal(message));
    }
    state
        .project
        .get()
        .ok_or_else(|| internal("no folder is open"))
}

/// A call-tree failure as the response code it has always had.
fn calls_error(e: CallsError) -> ResponseError {
    match e {
        CallsError::Invalid(message) => ResponseError::new(ErrorCode::INVALID_PARAMS, message),
        CallsError::MissingCapability(message) => {
            ResponseError::new(ErrorCode::REQUEST_FAILED, message)
        }
        CallsError::Internal(message) => internal(message),
    }
}

// ------------------------------------------------------------------------------------------------------------------------------------ tours

fn bad_params(e: impl Display) -> ResponseError {
    ResponseError::new(ErrorCode::INVALID_PARAMS, e)
}

/// A store failure as a response: a tour that is not there, is in both places or cannot be read is the request's; the lock is busy (`REQUEST_FAILED`: the request
/// was fine, the tour was not free); the disk is the core's.
fn tour_error(e: StoreError) -> ResponseError {
    match e {
        StoreError::NotFound(_)
        | StoreError::InBoth(_)
        | StoreError::TooBig(_)
        | StoreError::Refused(_)
        | StoreError::Unreadable { .. } => bad_params(e),
        StoreError::Lock(_) => ResponseError::new(ErrorCode::REQUEST_FAILED, e),
        StoreError::Io { .. } | StoreError::Init(_) | StoreError::NoFreeId => internal(e),
    }
}

fn parse_tour(id: &str) -> Result<TourId, ResponseError> {
    TourId::parse(id).map_err(bad_params)
}

fn open_project(state: &State) -> Result<&Project, ResponseError> {
    state
        .project
        .get()
        .ok_or_else(|| internal("no folder is open"))
}

/// Runs blocking work on the tours (the disk, the tours lock) off the runtime's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ResponseError> {
    tokio::task::spawn_blocking(work).await.map_err(internal)
}

/// The answer for each of these tours: the core's memory, the recorded check, a check (03 §3). The first two rungs need no index, so they are asked first with none:
/// a tour whose answer is remembered or recorded is answered while the first build or an update is still running. Only a real check waits: for the first build, then
/// for an update that is running (a check made against the index it is about to replace would record a stale answer), then it is made with the index.
async fn answers_for(
    state: &Arc<State>,
    rows: Vec<Arc<store::Row>>,
) -> Result<std::collections::BTreeMap<TourId, Answer>, ResponseError> {
    match ask_tours(state, &rows, false).await? {
        Ok(done) => return Ok(done),
        Err(Cannot::Store(e)) => return Err(tour_error(e)),
        Err(Cannot::NeedsIndex) => {}
    }
    ready_project(state).await?;
    drop(state.one_at_a_time.lock().await);
    match ask_tours(state, &rows, true).await? {
        Ok(done) => Ok(done),
        Err(Cannot::Store(e)) => Err(tour_error(e)),
        Err(Cannot::NeedsIndex) => Err(internal("the index is not ready")),
    }
}

/// One ask of the core's tours, with the index or without.
async fn ask_tours(
    state: &Arc<State>,
    rows: &[Arc<store::Row>],
    with_index: bool,
) -> Result<Result<std::collections::BTreeMap<TourId, Answer>, Cannot>, ResponseError> {
    let (held, rows) = (state.clone(), rows.to_vec());
    blocking(move || {
        let project = held.project.get().ok_or(Cannot::Store(StoreError::Refused(
            "no folder is open".into(),
        )))?;
        let ask = serve::Ask {
            vnoc_root: &store::vnoc_root(project.git_root()),
            project,
            index: with_index.then_some(&held.index),
        };
        held.tours.answers(&ask, &rows, Mode::default())
    })
    .await
}

/// The node `text` names, as the question `list --on` asks of the map. Waits for the first build. The root of the tree is every tour's: refused in the
/// command's words.
async fn node_query(state: &Arc<State>, text: &str) -> Result<Query, ResponseError> {
    let project = ready_project(state).await?;
    let index = state.index.read().unwrap_or_else(|e| e.into_inner());
    let tree = index.tree(&project.name(), project.prefix());
    let node = find::node_named(&index, project, &tree, text)
        .ok_or_else(|| bad_params(format!("no node is named `{text}`")))?;
    Query::of(node).ok_or_else(|| bad_params(on::ROOT_IS_EVERY_TOURS))
}

/// `vnoc/tours`: the rows of the list, newest first. The freshness of the rows named in `check` is asked for (and so, if need be, checked); the others say what the
/// core remembers.
fn tours(
    core: &mut Core,
    params: Option<protocol::ToursParams>,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tours", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let params = params.unwrap_or_default();
        let asked: Vec<TourId> = params
            .check
            .iter()
            .map(|id| parse_tour(id))
            .collect::<Result<_, _>>()?;
        let rows = {
            let (state, vnoc_root) = (state.clone(), vnoc_root.clone());
            blocking(move || state.tours.sync(&vnoc_root).0).await?
        };
        let to_check: Vec<_> = rows
            .iter()
            .filter(|r| asked.contains(&r.id))
            .cloned()
            .collect();
        let mut known = {
            let (state, ids) = (
                state.clone(),
                rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            );
            blocking(move || state.tours.remembered(&ids)).await?
        };
        if !to_check.is_empty() {
            for (id, answer) in answers_for(&state, to_check).await? {
                known.insert(id, answer.recorded);
            }
        }
        // the node, if one was asked for: the one thing here that needs the index; found by what the tree says of it
        let query = match params.on {
            Some(text) => Some(node_query(&state, &text).await?),
            None => None,
        };
        let shown = {
            let (state, rows) = (state.clone(), rows.clone());
            blocking(move || {
                let project = state.project.get()?;
                let root = store::vnoc_root(project.git_root());
                let shown = serve::list_rows(project, &rows, &known, true, &|id| {
                    store::read_place(&root, id)
                });
                Some(match &query {
                    Some(query) => serve::rows_on(shown, &rows, &state.tours.on(query)),
                    None => shown,
                })
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        Ok(serde_json::json!({ "tours": shown }))
    })
}

/// `vnoc/tour`: the tour as the stage plays it. Its freshness is asked for; the place is the one written, as the tour is now.
fn tour(
    core: &mut Core,
    params: protocol::TourParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tour", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        let Ok(found) = &row.tour else {
            let why = row.tour.as_ref().err().map(|e| e.why.clone());
            return Ok(serde_json::json!({ "unreadable": why }));
        };
        let answer = answers_for(&state, vec![row.clone()]).await?.remove(&id);
        let shown = {
            let (state, row, found) = (state.clone(), row.clone(), found.clone());
            blocking(move || {
                let project = state.project.get()?;
                let root = store::vnoc_root(project.git_root());
                let mut shown = serve::show_view(
                    project,
                    &found,
                    row.place,
                    &row.path,
                    store::read_place(&root, &row.id).as_ref(),
                );
                if let Some(answer) = &answer {
                    view::mark(&mut shown, answer);
                }
                Some(shown)
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        serde_json::to_value(shown).map_err(internal)
    })
}

/// `vnoc/tourGo`: the same function as `vnoc tour go`, under the tours lock; the place written is remembered, so the watcher's word of it is not an echo.
fn tour_go(
    core: &mut Core,
    params: protocol::TourGoParams,
) -> impl std::future::Future<Output = Result<crate::tours::view::PlaceView, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourGo", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let wanted = Move::parse(&params.r#move).map_err(bad_params)?;
        let gone = blocking(move || state.tours.go(&vnoc_root, &id, wanted))
            .await?
            .map_err(|e| match e {
                GoError::Store(e) => tour_error(e),
                GoError::NoSuch { .. } => bad_params(e),
            })?;
        Ok(view::place_view(&gone.tour, gone.place))
    })
}

/// `vnoc/tourOpen`: sets `place.open`, the same way.
fn tour_open(
    core: &mut Core,
    params: protocol::TourOpenParams,
) -> impl std::future::Future<Output = Result<crate::tours::view::PlaceView, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourOpen", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        blocking(move || {
            let place = state.tours.open(&vnoc_root, &id, params.open)?;
            Ok(state.tours.place_view(&vnoc_root, &id, place))
        })
        .await?
        .map_err(tour_error)
    })
}

/// `vnoc/tourHalt`: writes `halt` through `store::halt`, the function of `vnoc tour halt`, and answers the tour's state.
fn tour_halt(
    core: &mut Core,
    params: protocol::TourHaltParams,
) -> impl std::future::Future<Output = Result<protocol::TourHaltResult, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourHalt", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let (did, tour) = blocking(move || state.tours.halt(&vnoc_root, &id))
            .await?
            .map_err(tour_error)?;
        Ok(protocol::TourHaltResult {
            halt: match did {
                Halt::Written => "written",
                Halt::Already => "already",
                Halt::Finished => "finished",
            }
            .to_string(),
            state: view::state_word(tour.state).to_string(),
        })
    })
}

/// `vnoc/tourCode`: one stop of a pinned tour as it was written; `null` when there is none to give (03 §5).
fn tour_code(
    core: &mut Core,
    params: protocol::TourCodeParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourCode", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let stop = StopId::parse(&params.stop).map_err(bad_params)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        let tour = row.tour.as_ref().map_err(|e| {
            bad_params(StoreError::Unreadable {
                id: id.clone(),
                why: e.why.clone(),
                too_new: e.too_new,
            })
        })?;
        let tour = tour.clone();
        let code = blocking(move || {
            let project = state.project.get()?;
            Some(serve::stop_code(project, &tour, stop))
        })
        .await?
        .ok_or_else(|| internal("no folder is open"))?
        .map_err(bad_params)?;
        serde_json::to_value(code).map_err(internal)
    })
}

/// `vnoc/tourCounts`: how many tours pass through each node, by the tree's key. Waits for the first build; the map of nodes is the one `vnoc/tours { on }`
/// asks, so nothing is replayed that was not.
fn tour_counts(
    core: &mut Core,
    _ignored: serde_json::Value,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourCounts", async move {
        let project = ready_project(&state).await?;
        let vnoc_root = store::vnoc_root(project.git_root());
        // the tours are looked at (`stat`s) before they are counted: one that was written by another process and not announced is counted too
        {
            let (state, vnoc_root) = (state.clone(), vnoc_root.clone());
            blocking(move || state.tours.sync(&vnoc_root)).await?;
        }
        let counts = {
            let state = state.clone();
            blocking(move || {
                let project = state.project.get()?;
                let tree = {
                    let index = state.index.read().unwrap_or_else(|e| e.into_inner());
                    index.tree(&project.name(), project.prefix())
                };
                Some(state.tours.counts(&tree))
            })
            .await?
            .ok_or_else(|| internal("no folder is open"))?
        };
        Ok(serde_json::json!({ "counts": counts }))
    })
}

/// How long a `vnoc/tourCalls` request waits for the count: the core answers a request at a time per slot (`ConcurrencyLayer`: as many as there are cores), and a count asks the
/// driver once for each stop. A count that takes longer goes on in the background and the request is answered with an error that says to ask again; the answer is kept when it is done.
const CALLS_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);
/// How long the driver is given for one stop's calls.
const CALLS_ASK: std::time::Duration = std::time::Duration::from_secs(4);

/// `vnoc/tourCalls`: for each written function or class stop, how many of the functions it calls, one level down, are not a stop of this tour (design 04 §3: the
/// card's `+N`). The callees are the ones `vnoc tour route` finds ([`route::from_calls`], one level, the same `known`), each counted once, by tree key. Kept in the
/// core until the tour's file or a stop's file changes; waits for the first build. The count is made by a task of its own (one at a time for a tour), so a driver that is slow
/// holds no request: this one answers within [`CALLS_WAIT`], or says to ask again.
fn tour_calls(
    core: &mut Core,
    params: protocol::TourParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourCalls", async move {
        let state = core.state.clone();
        let project = ready_project(&state).await?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let id = parse_tour(&params.id)?;
        let row = {
            let (state, vnoc_root, id) = (state.clone(), vnoc_root.clone(), id.clone());
            blocking(move || state.tours.row(&vnoc_root, &id)).await?
        }
        .ok_or_else(|| bad_params(StoreError::NotFound(id.clone())))?;
        if let Err(e) = &row.tour {
            return Err(bad_params(StoreError::Unreadable {
                id: id.clone(),
                why: e.why.clone(),
                too_new: e.too_new,
            }));
        }
        if let Some(off) = state.tours.calls_kept(&id) {
            return Ok(serde_json::json!({ "off": off }));
        }
        if state.calls_running.lock().unwrap().insert(id.clone()) {
            let (core, row, id) = (core.clone(), row.clone(), id.clone());
            tokio::spawn(async move {
                if let Err(e) = count_calls(&core, &row).await {
                    tracing::debug!("the calls of tour {id} were not counted: {}", e.message);
                }
                core.state.calls_running.lock().unwrap().remove(&id);
            });
        }
        let deadline = tokio::time::Instant::now() + CALLS_WAIT;
        loop {
            if let Some(off) = state.tours.calls_kept(&id) {
                return Ok(serde_json::json!({ "off": off }));
            }
            let running = state.calls_running.lock().unwrap().contains(&id);
            if !running || tokio::time::Instant::now() >= deadline {
                return Err(ResponseError::new(
                    ErrorCode::REQUEST_FAILED,
                    "the calls of this tour are being counted: ask again",
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}

/// The count of `vnoc/tourCalls` for one tour, kept when it is whole and the tour and the index are as they were when it started.
async fn count_calls(core: &Core, row: &Arc<store::Row>) -> Result<(), ResponseError> {
    let state = core.state.clone();
    let project = ready_project(&state).await?;
    let found = row
        .tour
        .as_ref()
        .map_err(|e| bad_params(e.why.clone()))?
        .clone();
    // an update that is running goes first: it needs the driver too, and the calls are of the index it is about to give
    drop(state.one_at_a_time.lock().await);
    // the stops as the view has them: where each is now (a check moved its key), which are written, which are gone
    let answer = answers_for(&state, vec![row.clone()])
        .await?
        .remove(&row.id);
    let shown = {
        let (state, row) = (state.clone(), row.clone());
        blocking(move || {
            let project = state.project.get()?;
            let root = store::vnoc_root(project.git_root());
            let mut shown = serve::show_view(
                project,
                &found,
                row.place,
                &row.path,
                store::read_place(&root, &row.id).as_ref(),
            );
            if let Some(answer) = &answer {
                view::mark(&mut shown, answer);
            }
            Some(shown)
        })
        .await?
        .ok_or_else(|| internal("no folder is open"))?
    };
    let revision = state.index.read().unwrap().revision();
    let tree = {
        let index = state.index.read().unwrap_or_else(|e| e.into_inner());
        index.tree(&project.name(), project.prefix())
    };
    let stops_of_the_tour: std::collections::BTreeSet<&str> = shown
        .stops
        .iter()
        .filter_map(|s| s.key.as_deref())
        .collect();
    let known = |target: &vnoc_driver_protocol::CallTarget| {
        let location = vnoc_domain::LocationKey::new(
            project.repo_path(target.file.as_deref()?).ok()?,
            target.qualname.clone(),
            target.occurrence,
        )
        .ok()?;
        tree.find(&location.to_string()).and_then(route::known_of)
    };
    let mut off = std::collections::BTreeMap::new();
    let mut whole = true;
    for stop in &shown.stops {
        let (Some(key), Some("function" | "class")) = (stop.key.as_deref(), stop.kind.as_deref())
        else {
            continue;
        };
        if stop.planned || stop.check == Some("gone") {
            continue;
        }
        let Some(root) = tree.find(key).and_then(route::known_of) else {
            continue;
        };
        let asked = tokio::time::timeout(
            CALLS_ASK,
            calls::ask(project, &state.index, &core.host, key, Some(1)),
        )
        .await;
        let answered = match asked {
            Ok(Ok(answered)) => answered,
            // a driver with no call trees has no calls to count; a node the driver cannot start a tree at counts none
            Ok(Err(CallsError::MissingCapability(_))) => break,
            Ok(Err(CallsError::Invalid(_))) => continue,
            // the driver is slow or broke: this stop is left out, and the count is not kept (the next ask makes it again)
            Ok(Err(_)) | Err(_) => {
                whole = false;
                continue;
            }
        };
        let item = route::from_calls(&calls::merge(&answered.root), root, 1, &known);
        let outside: std::collections::BTreeSet<&str> = item
            .children
            .iter()
            .map(|c| c.known.id.as_str())
            .filter(|callee| !stops_of_the_tour.contains(callee))
            .collect();
        if !outside.is_empty() {
            off.insert(stop.id.clone(), outside.len());
        }
    }
    let files: std::collections::BTreeSet<String> = shown
        .stops
        .iter()
        .flat_map(|s| std::iter::once(s.path.clone()).chain(s.now.as_ref().map(|n| n.path.clone())))
        .collect();
    // not kept when the index moved while the calls were asked: they were made against one that is gone
    if whole && state.index.read().unwrap().revision() == revision {
        state.tours.keep_calls(row, off, files);
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------------------------------------------ the user's actions

/// An action's failure as a response: the request's mistakes are `INVALID_PARAMS` with the command's sentence, the store's are its own, the rest is the core's.
fn act_error(e: ActError) -> ResponseError {
    use crate::identity::AttachError as A;
    match e {
        ActError::Store(e) => tour_error(e),
        ActError::Usage(said) | ActError::NoStop { said, .. } => bad_params(said),
        ActError::Attach(
            e @ (A::TitleTaken { .. }
            | A::BadTitle
            | A::BodyTooLarge
            | A::BodyHasFrontmatter
            | A::BadTag(_)
            | A::BadAuthor
            | A::BadKey(_)
            | A::NotInIndex(_)),
        ) => bad_params(e),
        ActError::Attach(e) => internal(e),
        ActError::Environment(said) => internal(said),
    }
}

/// `vnoc/tourPin`: [`act::pin_begin`] and [`act::pin_finish`], the functions of `vnoc tour pin`. With `commit` the project's write lock is taken first (and the
/// attach mutex before it), then the tours lock inside the pin: the order of every writer.
fn tour_pin(
    core: &mut Core,
    params: protocol::TourPinParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourPin", async move {
        let state = core.state.clone();
        let project = ready_project(&state).await?;
        let id = parse_tour(&params.id)?;
        let _one_at_a_time;
        let _one_writer;
        if params.commit.is_some() {
            _one_at_a_time = Some(state.attaching.lock().await);
            _one_writer = Some(write_lock(project).await?);
        } else {
            _one_at_a_time = None;
            _one_writer = None;
        }
        let vnoc_root = store::vnoc_root(project.git_root());
        let message = params.commit.as_ref().and_then(|c| c.message.clone());
        let commit = params.commit.is_some();
        let held = state.clone();
        let done = blocking(move || {
            let state = held;
            let project = state
                .project
                .get()
                .ok_or(ActError::Environment("no folder is open".into()))?;
            let ask = act::Ask::Pin {
                commit,
                message: message.as_deref(),
            };
            act::not_in_both(&vnoc_root, &id)?;
            let tour = act::read_tour(&vnoc_root, &id)?;
            if let Some(commit) = act::pinned_already(project, &tour) {
                return Ok(serde_json::json!({
                    "tour": id.to_string(),
                    "already": true,
                    "state": "pinned",
                    "commit": commit.as_str(),
                }));
            }
            let out = match act::pin_begin(project, &vnoc_root, &id, &ask)? {
                act::Begun::Done(pinned) => pinned.out,
                act::Begun::Check { measured, head } => {
                    act::pin_finish(
                        project,
                        &state.index,
                        &vnoc_root,
                        &id,
                        &ask,
                        &measured,
                        head,
                    )?
                    .out
                }
            };
            let mut shown =
                serde_json::to_value(&out).map_err(|e| ActError::Environment(e.to_string()))?;
            shown["tour"] = serde_json::json!(id.to_string());
            Ok(shown)
        })
        .await?
        .map_err(act_error)?;
        Ok(done)
    })
}

/// `vnoc/tourIds`: [`act::write_ids`], the function of `vnoc tour ids`, with the server's drivers. The edits go to the files on disk, as the command's do: a
/// tour's stops are not open documents the editor could give (`vnoc/writeId` is for the node under the cursor); the watcher tells the core of the files, as of
/// an `add`. The project's write lock is first; each marker's edit takes the tours lock to rebase the tours.
fn tour_ids(
    core: &mut Core,
    params: protocol::TourIdsParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let core = core.clone();
    timed("tourIds", async move {
        let project = ready_project(&core.state).await?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        // the same lock as attach: both write an ID and a node.toml
        let _one_at_a_time = core.state.attaching.lock().await;
        let _one_writer = write_lock(project).await?;
        let mut env = Env {
            project,
            index: &core.state.index,
            host: &core.host,
            edits: EditSink::Disk,
        };
        let written = act::write_ids(&mut env, &vnoc_root, &id, &[])
            .await
            .map_err(act_error)?;
        serde_json::to_value(written).map_err(internal)
    })
}

/// `vnoc/tourEdit`: [`act::edit`].
fn tour_edit(
    core: &mut Core,
    params: protocol::TourEditParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourEdit", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let (title, question) = (params.title.clone(), params.question.clone());
        {
            let id = id.clone();
            blocking(move || {
                act::edit(&vnoc_root, &id, title.as_deref(), question.as_deref(), None)
            })
            .await?
            .map_err(act_error)?;
        }
        Ok(serde_json::json!({
            "tour": id.to_string(),
            "title": params.title.as_deref().map(str::trim),
            "question": params.question.as_deref().map(str::trim),
        }))
    })
}

/// `vnoc/tourShare`: [`store::share`], the function of `vnoc tour share`.
fn tour_share(
    core: &mut Core,
    params: protocol::TourShareParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourShare", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let root = project.git_root().to_path_buf();
        let vnoc_root = store::vnoc_root(&root);
        let to = if params.share {
            store::Where::Project
        } else {
            store::Where::Private
        };
        let moved = {
            let id = id.clone();
            blocking(move || store::share(&vnoc_root, &id, to))
                .await?
                .map_err(tour_error)?
        };
        let path = moved
            .strip_prefix(&root)
            .unwrap_or(&moved)
            .to_string_lossy()
            .replace('\\', "/");
        Ok(serde_json::json!({
            "tour": id.to_string(),
            "folder": view::place_word(to),
            "path": path,
        }))
    })
}

/// `vnoc/tourDelete`: [`store::delete`], the function of `vnoc tour delete`; the token is the trash entry.
fn tour_delete(
    core: &mut Core,
    params: protocol::TourDeleteParams,
) -> impl std::future::Future<Output = Result<protocol::TourDeleteResult, ResponseError>> + Send + use<>
{
    let state = core.state.clone();
    timed("tourDelete", async move {
        let project = open_project(&state)?;
        let id = parse_tour(&params.id)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let gone = blocking(move || store::delete(&vnoc_root, &id))
            .await?
            .map_err(tour_error)?;
        Ok(protocol::TourDeleteResult { token: gone.entry })
    })
}

/// `vnoc/tourRestore`: [`store::restore`]: the file and the state come back where they were.
fn tour_restore(
    core: &mut Core,
    params: protocol::TourRestoreParams,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourRestore", async move {
        let project = open_project(&state)?;
        let vnoc_root = store::vnoc_root(project.git_root());
        let back = blocking(move || store::restore(&vnoc_root, &params.token))
            .await?
            .map_err(tour_error)?;
        Ok(serde_json::json!({
            "tour": back.id.to_string(),
            "folder": view::place_word(back.was),
        }))
    })
}

/// `vnoc/tourSettings`: [`act::settings`], the function of `vnoc tour settings`. A key that is given is changed first, under the project's write lock (it is
/// the project's file).
fn tour_settings(
    core: &mut Core,
    params: Option<protocol::TourSettingsParams>,
) -> impl std::future::Future<Output = Result<serde_json::Value, ResponseError>> + Send + use<> {
    let state = core.state.clone();
    timed("tourSettings", async move {
        let project = open_project(&state)?;
        let params = params.unwrap_or_default();
        let choice = |text: &str| {
            Choice::parse(text)
                .ok_or_else(|| bad_params(format!("`{text}` is not one of always, ask, never")))
        };
        let mut set = Vec::new();
        for (key, text) in [(Key::Ids, &params.ids), (Key::Commit, &params.commit)] {
            if let Some(text) = text {
                set.push((key, choice(text)?));
            }
        }
        let _one_writer = if set.is_empty() {
            None
        } else {
            Some(write_lock(project).await?)
        };
        let shown = {
            let state = state.clone();
            blocking(move || {
                let project = state
                    .project
                    .get()
                    .ok_or(ActError::Environment("no folder is open".into()))?;
                act::settings(project, &set)
            })
            .await?
            .map_err(act_error)?
        };
        serde_json::to_value(shown).map_err(internal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::check::Source;
    use crate::tours::record::Detail;
    use std::time::Duration;

    /// A state with a folder open and no index: the first build has not happened (`Status::Building`), and a tour with no stop in it.
    fn state_with_a_tour(label: &str) -> (Arc<State>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vnoc-server-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(dir.join(".vnoc")).unwrap();
        store::start(
            &dir,
            store::StartParams {
                title: "T".into(),
                question: None,
                author: None,
                detail: Detail::Normal,
            },
        )
        .unwrap();
        (opened(&dir), dir)
    }

    fn opened(dir: &std::path::Path) -> Arc<State> {
        let state = Arc::new(State::new());
        assert!(state.project.set(Project::open(dir).unwrap()).is_ok());
        state
    }

    fn rows_of(state: &State, dir: &std::path::Path) -> Vec<Arc<store::Row>> {
        let rows = state.tours.sync(&dir.join(".vnoc")).0;
        assert_eq!(rows.len(), 1);
        rows
    }

    #[tokio::test]
    async fn a_tour_whose_record_is_valid_is_answered_while_an_update_holds_the_lock_and_the_first_build_is_not_done()
     {
        let (state, dir) = state_with_a_tour("no-wait");
        // the first ask records the tour (it has no written stop: nothing to check)
        let rows = rows_of(&state, &dir);
        let first = answers_for(&state, rows).await.unwrap();
        assert_eq!(first.values().next().unwrap().from, Source::Check);

        // an update is running (it holds `one_at_a_time`) and there is no index: the memory answers
        let running = state.one_at_a_time.lock().await;
        let rows = rows_of(&state, &dir);
        let asked = tokio::time::timeout(Duration::from_secs(5), answers_for(&state, rows))
            .await
            .expect("the answer did not wait for the update");
        assert_eq!(asked.unwrap().values().next().unwrap().from, Source::Memory);

        // a core that has just started has no memory, but the record on disk is valid: the record answers
        let fresh = opened(&dir);
        let rows = rows_of(&fresh, &dir);
        let asked = tokio::time::timeout(Duration::from_secs(5), answers_for(&fresh, rows))
            .await
            .expect("the answer did not wait for the update");
        assert_eq!(asked.unwrap().values().next().unwrap().from, Source::Record);
        drop(running);
    }
}
