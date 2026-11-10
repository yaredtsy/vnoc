//! The editor-facing server: which messages it answers, and what it does with the ones it does not know.
//!
//! The four conditions of VS-D67 live here:
//! 1. **Catch-all handlers** for unknown notifications and requests: log and continue, never exit. (async-lsp's default
//!    breaks the whole main loop on an unknown notification that does not start with `$/`.)
//! 2. **Lifecycle** (`initialize`, `initialized`, `shutdown`; `exit` is the lifecycle layer's) is handled explicitly.
//! 3. A test sends unknown messages and a full lifecycle and checks the core is still alive (`tests/lifecycle.rs`).
//! 4. **Async is for I/O only**: every handler body runs under `watchdog::timed`, and blocking work (walking the disk,
//!    building the tree) goes through `spawn_blocking`.

use std::fmt::Display;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_lsp::lsp_types::{InitializeParams, InitializeResult, ServerCapabilities, ServerInfo, notification, request};
use async_lsp::router::Router;
use async_lsp::{ErrorCode, ResponseError};

use crate::driver_host::{DriverClient, HostError};
use crate::protocol::{FailedFile, Ping, PingResult, Stats, Structure, StructureParams, StructureResult};
use crate::structure::{self, FileDefs, Node};
use crate::watchdog::timed;
use crate::scan;

/// More files than this and the rest are not read (the answer says so). A skeleton guard, not a design.
const MAX_FILES: usize = 20_000;

/// Which driver, and the running connection to it.
#[derive(Default)]
pub struct Host {
    driver_path: Mutex<Option<PathBuf>>,
    client: tokio::sync::Mutex<Option<Arc<DriverClient>>>,
}

impl Host {
    /// The running driver, started on first use. A driver that failed is dropped (see `forget`), so the next call starts a new one.
    async fn client(&self) -> Result<Arc<DriverClient>, HostError> {
        let mut slot = self.client.lock().await;
        if let Some(c) = slot.as_ref() {
            return Ok(c.clone());
        }
        let path = self.driver_path.lock().unwrap().clone().ok_or(HostError::NotConfigured)?;
        let client = Arc::new(DriverClient::start(&path).await?);
        tracing::info!("driver `{}` {} started from {}", client.info.name, client.info.version, path.display());
        *slot = Some(client.clone());
        Ok(client)
    }

    async fn forget(&self) {
        *self.client.lock().await = None;
    }
}

/// The core's state. The structure is **held in memory** here (SQLite arrives with the store trait in M1).
#[derive(Default)]
pub struct Core {
    host: Arc<Host>,
    structure: Arc<Mutex<Option<Node>>>,
}

fn internal(e: impl Display) -> ResponseError {
    ResponseError::new(ErrorCode::INTERNAL_ERROR, e)
}

pub fn router() -> Router<Core> {
    let mut router = Router::new(Core::default());
    router
        // 2. lifecycle
        .request::<request::Initialize, _>(initialize)
        .request::<request::Shutdown, _>(|_, _| async { Ok(()) })
        .notification::<notification::Initialized>(|_, _| ControlFlow::Continue(()))
        // the messages of this skeleton
        .request::<Ping, _>(|_, _ignored| {
            timed("ping", async { Ok(PingResult { pong: true, pid: std::process::id(), version: env!("CARGO_PKG_VERSION").to_string() }) })
        })
        .request::<Structure, _>(structure)
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
/// `initialize`: the driver's path comes in `initializationOptions.driverPath` (the extension resolves it, and ignores
/// workspace settings in Restricted Mode), with the `VNOC_DRIVER` environment variable as a fallback for tests and terminals.
fn initialize(core: &mut Core, params: InitializeParams) -> impl std::future::Future<Output = Result<InitializeResult, ResponseError>> + Send + use<> {
    let from_options = params.initialization_options.as_ref().and_then(|o| o.get("driverPath")).and_then(|p| p.as_str()).map(PathBuf::from);
    *core.host.driver_path.lock().unwrap() = from_options.or_else(|| std::env::var_os("VNOC_DRIVER").map(PathBuf::from));
    timed("initialize", async {
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo { name: "vnoc".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }),
        })
    })
}

/// `vnoc/structure`: start the driver if needed, list the Python files, ask the driver for each one's outline, build the tree.
fn structure(core: &mut Core, params: StructureParams) -> impl std::future::Future<Output = Result<StructureResult, ResponseError>> + Send + use<> {
    let (host, held) = (core.host.clone(), core.structure.clone());
    timed("structure", async move {
        let started = Instant::now();
        let root = PathBuf::from(&params.root);

        let client = host.client().await.map_err(internal)?;
        let scan_root = root.clone();
        let scan = tokio::task::spawn_blocking(move || scan::python_files(&scan_root, MAX_FILES)).await.map_err(internal)?.map_err(internal)?;

        let mut files = Vec::with_capacity(scan.files.len());
        let mut failed = Vec::new();
        for path in &scan.files {
            match client.outline_file(&root, path).await {
                Ok(outline) => files.push(FileDefs { path: path.clone(), defs: outline.defs }),
                // the driver answered with an error for THIS file: note it, carry on with the others
                Err(HostError::Remote { message, .. }) => failed.push(FailedFile { path: path.clone(), error: message }),
                // anything else means the connection is broken: forget the driver so the next request starts a fresh one
                Err(e) => {
                    host.forget().await;
                    return Err(internal(e));
                }
            }
        }

        let root_name = root.file_name().map_or_else(|| params.root.clone(), |n| n.to_string_lossy().into_owned());
        // building the tree is CPU work that grows with the repo: off the async workers
        let tree = tokio::task::spawn_blocking(move || structure::build(&root_name, files)).await.map_err(internal)?;
        let [folders, files, classes, functions] = tree.counts();
        let folders = folders.saturating_sub(1); // the opened folder is not a folder under itself
        *held.lock().unwrap() = Some(tree.clone());

        Ok(StructureResult {
            root: tree,
            stats: Stats { folders, files, classes, functions, ms: started.elapsed().as_millis() },
            truncated: scan.truncated,
            failed,
        })
    })
}
