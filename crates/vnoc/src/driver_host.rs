//! Starts a language driver and talks to it (the wire in plan_vs/07-drivers/01-driver-protocol.md).
//!
//! One driver, one request at a time, found by an explicit path. A driver that fails or hangs is dropped (its process dies
//! with it) and the next request starts a new one. Concurrent requests, several drivers and lookup in a drivers folder come later.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use serde::de::DeserializeOwned;
use vnoc_driver_protocol::{InitializeParams, InitializeResult, OutlineParams, OutlineResult, PROTOCOL_VERSION, PlanIdsParams, PlanIdsResult, method};

/// How long the driver may take to answer one request before it is treated as broken (review C4).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("no driver is configured: set `vnoc.driver.path`, or VNOC_DRIVER")]
    NotConfigured,
    #[error("the driver is not at {0}: build it with `cargo build` in drivers/python")]
    NotFound(PathBuf),
    #[error("could not start the driver: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("the driver refused the handshake: {0}")]
    Handshake(String),
    #[error("the driver speaks protocol {got}, this core speaks {PROTOCOL_VERSION}")]
    WrongProtocol { got: u32 },
    #[error("the driver does not implement `{0}`")]
    MissingCapability(&'static str),
    #[error("talking to the driver failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("the driver sent something that is not the protocol: {0}")]
    Protocol(String),
    #[error("the driver did not answer `{0}` in time")]
    Timeout(&'static str),
    #[error("the driver answered with an error ({code}): {message}")]
    Remote { code: i64, message: String },
}

struct Io {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

pub struct DriverClient {
    io: tokio::sync::Mutex<Io>,
    pub info: InitializeResult,
    _child: Child, // kill_on_drop: the driver never outlives the core
}

impl DriverClient {
    /// Starts the driver at `path`, does the handshake, and checks it can produce an outline.
    pub async fn start(path: &Path) -> Result<DriverClient, HostError> {
        if !path.exists() {
            return Err(HostError::NotFound(path.to_path_buf()));
        }
        let mut child = Command::new(path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true).spawn().map_err(HostError::Spawn)?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        let mut io = Io { stdin, stdout, next_id: 0 };

        let reply = request(&mut io, "initialize", &InitializeParams { protocol: PROTOCOL_VERSION }).await.map_err(|e| match e {
            HostError::Remote { message, .. } => HostError::Handshake(message),
            other => other,
        })?;
        let info: InitializeResult = serde_json::from_value(reply).map_err(|e| HostError::Protocol(e.to_string()))?;
        if info.protocol != PROTOCOL_VERSION {
            return Err(HostError::WrongProtocol { got: info.protocol });
        }
        if !info.capabilities.get("outline").copied().unwrap_or(false) {
            return Err(HostError::MissingCapability("outline"));
        }
        send(&mut io.stdin, &json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} })).await?;
        Ok(DriverClient { io: tokio::sync::Mutex::new(io), info, _child: child })
    }

    /// The defs of a file the driver reads itself.
    pub async fn outline_file(&self, root: &Path, file: &str) -> Result<OutlineResult, HostError> {
        let params = OutlineParams { root: Some(root.to_string_lossy().into_owned()), file: Some(file.to_string()), ..Default::default() };
        self.call(method::OUTLINE, &params).await
    }

    pub async fn plan_ids(&self, params: &PlanIdsParams) -> Result<PlanIdsResult, HostError> {
        if !self.info.capabilities.get("ids.plan").copied().unwrap_or(false) {
            return Err(HostError::MissingCapability("ids.plan"));
        }
        self.call(method::PLAN_IDS, params).await
    }

    /// One request, answered within `REQUEST_TIMEOUT`.
    async fn call<R: DeserializeOwned>(&self, method: &'static str, params: &impl Serialize) -> Result<R, HostError> {
        let mut io = self.io.lock().await;
        let reply = tokio::time::timeout(REQUEST_TIMEOUT, request(&mut io, method, params)).await.map_err(|_| HostError::Timeout(method))??;
        serde_json::from_value(reply).map_err(|e| HostError::Protocol(e.to_string()))
    }
}

/// Which driver to run, and the running connection to it.
#[derive(Default)]
pub struct DriverHost {
    path: Mutex<Option<PathBuf>>,
    client: tokio::sync::Mutex<Option<Arc<DriverClient>>>,
}

impl DriverHost {
    pub fn set_path(&self, path: Option<PathBuf>) {
        *self.path.lock().unwrap() = path;
    }

    /// The outline of one file (repo path relative to `root`), starting the driver on first use.
    pub async fn outline(&self, root: &Path, file: &str) -> Result<OutlineResult, HostError> {
        self.with_client(|client| async move { client.outline_file(root, file).await }).await
    }

    /// Where the `ID:` lines go in `params.text` (the editor's buffer).
    pub async fn plan_ids(&self, params: &PlanIdsParams) -> Result<PlanIdsResult, HostError> {
        self.with_client(|client| async move { client.plan_ids(params).await }).await
    }

    /// Runs `request` on the driver. An error from the driver about this request is returned as is; anything else means the
    /// connection is broken, so the driver is dropped (its process dies with it) and the next request starts a new one.
    async fn with_client<T, Fut: std::future::Future<Output = Result<T, HostError>>>(&self, request: impl FnOnce(Arc<DriverClient>) -> Fut) -> Result<T, HostError> {
        let client = self.client().await?;
        let result = request(client).await;
        if matches!(&result, Err(e) if !matches!(e, HostError::Remote { .. } | HostError::MissingCapability(_))) {
            *self.client.lock().await = None;
        }
        result
    }

    async fn client(&self) -> Result<Arc<DriverClient>, HostError> {
        let mut slot = self.client.lock().await;
        if let Some(c) = slot.as_ref() {
            return Ok(c.clone());
        }
        let path = self.path.lock().unwrap().clone().ok_or(HostError::NotConfigured)?;
        let client = Arc::new(DriverClient::start(&path).await?);
        tracing::info!("driver `{}` {} started from {}", client.info.name, client.info.version, path.display());
        *slot = Some(client.clone());
        Ok(client)
    }
}

async fn send(stdin: &mut ChildStdin, message: &Value) -> Result<(), HostError> {
    let body = serde_json::to_vec(message).map_err(|e| HostError::Protocol(e.to_string()))?;
    stdin.write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes()).await?;
    stdin.write_all(&body).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_frame(stdout: &mut BufReader<ChildStdout>) -> Result<Value, HostError> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).await? == 0 {
            return Err(HostError::Protocol("the driver closed its output".into()));
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        match line.strip_prefix("Content-Length:") {
            Some(n) => len = n.trim().parse().ok(),
            None => return Err(HostError::Protocol(format!("stray output on the driver's stdout: {line:?}"))),
        }
    }
    let len = len.ok_or_else(|| HostError::Protocol("a frame without Content-Length".into()))?;
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| HostError::Protocol(e.to_string()))
}

/// Lockstep: send one request, read until its response. (Notifications from the driver are not part of protocol 1.)
async fn request(io: &mut Io, method: &str, params: &impl Serialize) -> Result<Value, HostError> {
    io.next_id += 1;
    let id = io.next_id;
    send(&mut io.stdin, &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).await?;
    loop {
        let msg = read_frame(&mut io.stdout).await?;
        if msg.get("id") != Some(&json!(id)) {
            continue;
        }
        if let Some(e) = msg.get("error") {
            return Err(HostError::Remote { code: e["code"].as_i64().unwrap_or(0), message: e["message"].as_str().unwrap_or("").to_string() });
        }
        return Ok(msg["result"].clone());
    }
}
