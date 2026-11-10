//! Starts a language driver and talks to it (the wire in plan_vs/07-drivers/01-driver-protocol.md).
//!
//! Skeleton version: one driver, one request at a time, found by an explicit path. **Moves to `vnoc-driver-host` in M1**,
//! which adds lookup in the drivers folder, copy-on-first-run, restarts and concurrent requests.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use vnoc_driver_protocol::{InitializeParams, InitializeResult, OutlineParams, OutlineResult, PROTOCOL_VERSION, method};

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
        let mut io = self.io.lock().await;
        let reply = request(&mut io, method::OUTLINE, &params).await?;
        serde_json::from_value(reply).map_err(|e| HostError::Protocol(e.to_string()))
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
