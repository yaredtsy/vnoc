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

mod calls;
mod docs;
mod index;
mod tours;

use calls::{call_tree, has_calls};
use docs::{attach, docs, locate, relink, restore_plan, trash, trash_info, trash_list, write_id};
use index::{languages, project_info, search, search_cards, structure};
use tours::{
    tour, tour_ask, tour_calls, tour_code, tour_counts, tour_delete, tour_delete_finished,
    tour_edit, tour_export, tour_go, tour_halt, tour_ids, tour_import, tour_open, tour_pin,
    tour_restore, tour_settings, tour_share, tours,
};

use std::fmt::Display;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use async_lsp::lsp_types::{
    InitializeParams, InitializeResult, InitializedParams, ServerCapabilities, ServerInfo,
    notification, request,
};
use async_lsp::router::Router;
use async_lsp::{ClientSocket, ErrorCode, ResponseError};

use crate::cli::project::discover;
use crate::driver_host::DriverHost;
use crate::index::Index;
use crate::project::Project;
use crate::protocol;
use crate::protocol::Locate;
use crate::protocol::{
    Attach, CallTree, Docs, FilesChanged, FilesChangedParams, HasCalls, Languages, Ping,
    PingResult, ProjectInfo, Rebuild, Relink, RestorePlan, Search, SearchCards, Structure, Trash,
    TrashInfo, TrashList, WriteId,
};
use crate::tours::record::TourId;
use crate::tours::serve::{self, Touch, Tours};
use crate::tours::store::{self};
use crate::userdata::{LockError, UserData, WriteLock};
use crate::watchdog::timed;
use vnoc_search::{Corpus, DocIndex};

/// Where the first build is.
#[derive(Clone)]
enum Status {
    Building,
    Ready,
    /// the build could not run (no folder, no driver, a broken driver); the message is what `vnoc/structure` answers with
    Failed(String),
}

/// How long one update (the walk and all its driver calls together) may run. Each driver call has its own limit (`driver_host`), but an update makes one for each file: the
/// sum is bounded here. Past it the update is stopped where it is and the lock is free.
const UPDATE_LIMIT: Duration = Duration::from_secs(60);

/// How long a request waits for the first build or for an update that is running. A handler answers in about a second or says to ask again: a request that waits longer
/// holds one of the core's slots (as many as there are cores) and, with enough of them, `vnoc/ping` stops answering.
const WAIT_LIMIT: Duration = Duration::from_secs(8);

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
    /// The last update after a good first build failed or ran out of time, so some of the files it was told about were not read: the next update reads everything again.
    needs_everything: AtomicBool,
    /// [`UPDATE_LIMIT`]; a field so a test can make it short.
    update_limit: Duration,
    /// [`WAIT_LIMIT`]; a field for the same reason.
    wait_limit: Duration,
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

/// A duration from the environment, for tests of the editor against a slow core. Read in debug builds only: a release build ignores it.
pub(crate) fn test_millis(name: &str) -> Option<Duration> {
    if cfg!(debug_assertions) {
        let millis = std::env::var(name).ok()?.parse().ok()?;
        Some(Duration::from_millis(millis))
    } else {
        None
    }
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
            needs_everything: AtomicBool::new(false),
            update_limit: UPDATE_LIMIT,
            wait_limit: test_millis("VNOC_WAIT_LIMIT_MS").unwrap_or(WAIT_LIMIT),
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
        .request::<protocol::TourDeleteFinished, _>(tour_delete_finished)
        .request::<protocol::TourRestore, _>(tour_restore)
        .request::<protocol::TourSettings, _>(tour_settings)
        .request::<protocol::TourExport, _>(tour_export)
        .request::<protocol::TourImport, _>(tour_import)
        .request::<protocol::TourAsk, _>(tour_ask)
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

/// The answer of a request that waited as long as it may for the index: the request was fine, the index was not free. The words are for people; `data.askAgain` is
/// what the editor and the pages test (no other refusal carries it).
fn ask_again() -> ResponseError {
    ask_again_with("the index is being read and is not ready yet: ask again")
}

/// A refusal that says to ask again: `words` and the flag. Both of the core's ask-again answers are made here.
fn ask_again_with(words: &str) -> ResponseError {
    let mut e = ResponseError::new(ErrorCode::REQUEST_FAILED, words);
    e.data = Some(serde_json::json!({ "askAgain": true }));
    e
}

/// Runs `wait` for at most the state's wait limit; past it, "ask again". Nothing a request waits for may take a slot for longer.
async fn within_wait<T>(
    state: &State,
    wait: impl std::future::Future<Output = Result<T, ResponseError>>,
) -> Result<T, ResponseError> {
    tokio::time::timeout(state.wait_limit, wait)
        .await
        .unwrap_or_else(|_| Err(ask_again()))
}

/// Waits for the first build, then for an update that is running (a check made against the index it is about to replace would record a stale answer): both together
/// are one wait, at most the wait limit.
async fn wait_for_index(state: &State) -> Result<(), ResponseError> {
    within_wait(state, async {
        ready_project(state).await?;
        drop(state.one_at_a_time.lock().await);
        Ok(())
    })
    .await
}

/// Waits for the first build (at most the wait limit) and gives the project: what every call-tree request starts with.
async fn ready_project(state: &State) -> Result<&Project, ResponseError> {
    within_wait(state, async {
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
    })
    .await
}

fn bad_params(e: impl Display) -> ResponseError {
    ResponseError::new(ErrorCode::INVALID_PARAMS, e)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn quick(wait: Duration) -> State {
        State {
            wait_limit: wait,
            ..State::new()
        }
    }

    fn says_ask_again(e: &ResponseError) -> bool {
        e.code == ErrorCode::REQUEST_FAILED
            && e.data == Some(serde_json::json!({ "askAgain": true }))
    }

    #[test]
    fn no_other_refusal_carries_the_flag() {
        assert!(internal("x").data.is_none());
        assert!(bad_params("x").data.is_none());
    }

    #[tokio::test]
    async fn a_request_that_waits_for_the_first_build_says_ask_again_when_the_wait_is_up() {
        let state = quick(Duration::from_millis(150)); // still `Building`
        let started = std::time::Instant::now();
        let e = ready_project(&state).await.map(|_| ()).unwrap_err();
        assert!(says_ask_again(&e), "{e:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn a_request_that_waits_for_an_update_says_ask_again_and_goes_through_when_the_update_ends()
     {
        let state = quick(Duration::from_millis(150));
        state.status.send_replace(Status::Ready);
        assert!(
            state
                .project
                .set(Project::open(&std::env::temp_dir()).unwrap())
                .is_ok()
        );
        let running = state.one_at_a_time.lock().await;
        let started = std::time::Instant::now();
        let e = wait_for_index(&state).await.unwrap_err();
        assert!(says_ask_again(&e), "{e:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
        drop(running);
        wait_for_index(&state).await.expect("no update is running");
    }
}
