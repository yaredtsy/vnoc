//! Starts a language driver and talks to it (the wire in plan_vs/07-drivers/01-driver-protocol.md).
//!
//! One driver, found by an explicit path, with many requests in flight at once (06-drivers.md §4): one task reads every frame and
//! hands each answer to the request with that `id`. A request that times out, or whose caller goes away, is cancelled with
//! `$/cancelRequest`. A driver that fails or hangs is dropped (its process dies with it) and the next request starts a new one.
//! Several drivers and lookup in a drivers folder come later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use vnoc_driver_protocol::{
    CallTreeParams, CallTreeResult, InitializeParams, InitializeResult, OutlineParams,
    OutlineResult, PROTOCOL_VERSION, PlanFileIdParams, PlanFileIdResult, PlanIdsParams,
    PlanIdsResult, method,
};

/// How long the driver may take to answer `outline`, `planIds`, `planFileId` and the handshake before the request is given up (review C4).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A call tree is a real analysis: the driver itself stops one after 20 seconds, and this gives it time to answer.
const CALL_TREE_TIMEOUT: Duration = Duration::from_secs(30);

fn timeout_of(method: &str) -> Duration {
    match method {
        method::CALL_TREE | method::EXPAND => CALL_TREE_TIMEOUT,
        _ => REQUEST_TIMEOUT,
    }
}

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
    /// One answer, whole and well framed, whose body could not be read (not JSON, too deeply nested, not the shape asked for). It fails the request it
    /// belongs to and nothing else: the driver is not at fault for the whole connection, so it is kept.
    #[error("the driver's answer could not be read: {0}")]
    BadAnswer(String),
    #[error("the driver stopped: {0}")]
    Closed(String),
    #[error("the driver did not answer `{0}` in time")]
    Timeout(&'static str),
    #[error("the driver answered with an error ({code}): {message}")]
    Remote { code: i64, message: String },
}

type Reply = Result<Value, HostError>;

/// The requests waiting for an answer, by id. `None` once the driver is gone: nothing can wait any more.
type Waiting = Arc<Mutex<Option<HashMap<u64, oneshot::Sender<Reply>>>>>;

/// The driver's stdin. The lock is held while one frame is written, never while waiting for an answer.
type Writer = Arc<tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;

/// The driver's two pipes, shared by every request in flight.
struct Wire {
    writer: Writer,
    waiting: Waiting,
    next_id: AtomicU64,
}

pub struct DriverClient {
    wire: Arc<Wire>,
    reader: JoinHandle<()>,
    pub info: InitializeResult,
    _child: Option<Child>, // kill_on_drop: the driver never outlives the core
}

impl DriverClient {
    /// Starts the driver at `path`, does the handshake, and checks it can produce an outline.
    pub async fn start(path: &Path) -> Result<DriverClient, HostError> {
        if !path.exists() {
            return Err(HostError::NotFound(path.to_path_buf()));
        }
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(HostError::Spawn)?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        DriverClient::connect(stdin, stdout, Some(child)).await
    }

    /// The handshake over any pair of pipes: the driver's process is `child`, or none for a stand-in in a test.
    async fn connect(
        stdin: impl AsyncWrite + Send + Unpin + 'static,
        stdout: impl AsyncRead + Send + Unpin + 'static,
        child: Option<Child>,
    ) -> Result<DriverClient, HostError> {
        let waiting: Waiting = Arc::new(Mutex::new(Some(HashMap::new())));
        let wire = Arc::new(Wire {
            writer: Arc::new(tokio::sync::Mutex::new(Box::new(stdin))),
            waiting: waiting.clone(),
            next_id: AtomicU64::new(0),
        });
        let reader = tokio::spawn(read_replies(BufReader::new(stdout), waiting));
        match DriverClient::handshake(&wire).await {
            Ok(info) => Ok(DriverClient {
                wire,
                reader,
                info,
                _child: child,
            }),
            Err(e) => {
                reader.abort();
                Err(e)
            }
        }
    }

    async fn handshake(wire: &Arc<Wire>) -> Result<InitializeResult, HostError> {
        let reply = wire
            .request(
                "initialize",
                &InitializeParams {
                    protocol: PROTOCOL_VERSION,
                },
                REQUEST_TIMEOUT,
            )
            .await
            .map_err(|e| match e {
                HostError::Remote { message, .. } => HostError::Handshake(message),
                other => other,
            })?;
        let info: InitializeResult =
            serde_json::from_value(reply).map_err(|e| HostError::Protocol(e.to_string()))?;
        if info.protocol != PROTOCOL_VERSION {
            return Err(HostError::WrongProtocol { got: info.protocol });
        }
        if !info.capabilities.get("outline").copied().unwrap_or(false) {
            return Err(HostError::MissingCapability("outline"));
        }
        wire.send(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }))
            .await?;
        Ok(info)
    }

    /// The defs of a file the driver reads itself.
    pub async fn outline_file(&self, root: &Path, file: &str) -> Result<OutlineResult, HostError> {
        let params = OutlineParams {
            root: Some(root.to_string_lossy().into_owned()),
            file: Some(file.to_string()),
            ..Default::default()
        };
        self.call(method::OUTLINE, &params).await
    }

    /// The defs of this text, which need not be on disk (the editor's buffer).
    pub async fn outline_text(&self, text: &str) -> Result<OutlineResult, HostError> {
        let params = OutlineParams {
            text: Some(text.to_string()),
            ..Default::default()
        };
        self.call(method::OUTLINE, &params).await
    }

    pub async fn plan_ids(&self, params: &PlanIdsParams) -> Result<PlanIdsResult, HostError> {
        if !self
            .info
            .capabilities
            .get("ids.plan")
            .copied()
            .unwrap_or(false)
        {
            return Err(HostError::MissingCapability("ids.plan"));
        }
        self.call(method::PLAN_IDS, params).await
    }

    pub async fn plan_file_id(
        &self,
        params: &PlanFileIdParams,
    ) -> Result<PlanFileIdResult, HostError> {
        if !self
            .info
            .capabilities
            .get("ids.file")
            .copied()
            .unwrap_or(false)
        {
            return Err(HostError::MissingCapability("ids.file"));
        }
        self.call(method::PLAN_FILE_ID, params).await
    }

    pub async fn call_tree(&self, params: &CallTreeParams) -> Result<CallTreeResult, HostError> {
        if !self
            .info
            .capabilities
            .get("callTree")
            .copied()
            .unwrap_or(false)
        {
            return Err(HostError::MissingCapability("callTree"));
        }
        self.call(method::CALL_TREE, params).await
    }

    /// One request, answered within the time its method is given (`timeout_of`).
    async fn call<R: DeserializeOwned>(
        &self,
        method: &'static str,
        params: &impl Serialize,
    ) -> Result<R, HostError> {
        let reply = self
            .wire
            .request(method, params, timeout_of(method))
            .await?;
        // an answer of the wrong shape is one bad answer, not a broken driver
        serde_json::from_value(reply).map_err(|e| HostError::BadAnswer(e.to_string()))
    }
}

impl Drop for DriverClient {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl Wire {
    /// Sends a request and waits for its answer, `limit` in all: writing it counts too. When this future is dropped, or the time runs out,
    /// and nobody has answered, the driver is told to stop working on it (`$/cancelRequest`).
    ///
    /// A request that cannot even be written in time means the driver has stopped reading its input: that is a broken connection
    /// (`Closed`, so the host drops the driver), not a slow answer (`Timeout`, which keeps it).
    async fn request(
        self: &Arc<Self>,
        method: &'static str,
        params: &impl Serialize,
        limit: Duration,
    ) -> Reply {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        match self.waiting.lock().unwrap().as_mut() {
            Some(waiting) => waiting.insert(id, tx),
            None => return Err(HostError::Closed("it is not running".into())),
        };
        let _pending = Pending {
            wire: self.clone(),
            id,
        };
        let deadline = tokio::time::Instant::now() + limit;
        match tokio::time::timeout_at(
            deadline,
            self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })),
        )
        .await
        {
            Ok(written) => written?,
            Err(_) => return Err(HostError::Closed("it is not reading its input".into())),
        }
        match tokio::time::timeout_at(deadline, rx).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(_)) => Err(HostError::Closed("it stopped answering".into())),
            Err(_) => Err(HostError::Timeout(method)),
        }
    }

    /// Writes one whole frame. It runs as its own task: a caller that goes away halfway through must not leave half a frame in the pipe.
    async fn send(&self, message: &Value) -> Result<(), HostError> {
        let body = serde_json::to_vec(message).map_err(|e| HostError::Protocol(e.to_string()))?;
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend(body);
        let writer = self.writer.clone();
        tokio::spawn(async move {
            let mut stdin = writer.lock().await;
            stdin.write_all(&frame).await?;
            stdin.flush().await
        })
        .await
        .map_err(|e| HostError::Closed(e.to_string()))??;
        Ok(())
    }
}

/// A request that has been sent and not yet answered. When it goes out of scope with its entry still in `waiting`, nobody answered:
/// the entry is removed (a late answer is ignored) and the driver is told to cancel.
struct Pending {
    wire: Arc<Wire>,
    id: u64,
}

impl Drop for Pending {
    fn drop(&mut self) {
        let unanswered = self
            .wire
            .waiting
            .lock()
            .unwrap()
            .as_mut()
            .is_some_and(|waiting| waiting.remove(&self.id).is_some());
        if !unanswered {
            return;
        }
        // there may be no runtime left (the core is shutting down): then the driver is going away too
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let (wire, id) = (self.wire.clone(), self.id);
            runtime.spawn(async move {
                let _ = wire.send(&json!({ "jsonrpc": "2.0", "method": "$/cancelRequest", "params": { "id": id } })).await;
            });
        }
    }
}

/// The one reader of the driver's output: each response goes to the request with its `id`. It ends when the driver closes its output or
/// sends something that is not a frame (no `Content-Length`, stray output): everyone still waiting then gets the reason, and so does every later request.
/// A frame that is whole but whose body cannot be read fails only its own request (`unreadable`) and the reader goes on.
async fn read_replies(mut stdout: impl AsyncBufRead + Unpin, waiting: Waiting) {
    let reason = loop {
        let body = match read_body(&mut stdout).await {
            Ok(body) => body,
            Err(HostError::Closed(reason)) => break reason,
            Err(other) => break other.to_string(),
        };
        let msg = match serde_json::from_slice::<Value>(&body) {
            Ok(msg) => msg,
            Err(e) => {
                unreadable(&waiting, &body, &e);
                continue;
            }
        };
        // a notification or a request from the driver: not part of protocol 1
        let Some(id) = msg
            .get("id")
            .and_then(Value::as_u64)
            .filter(|_| msg.get("method").is_none())
        else {
            continue;
        };
        let waiter = waiting
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|waiting| waiting.remove(&id));
        if let Some(tx) = waiter {
            let _ = tx.send(reply_of(&msg));
        } // else: an answer to a request that timed out or was cancelled
    };
    let gone = waiting.lock().unwrap().take();
    for (_, tx) in gone.into_iter().flatten() {
        let _ = tx.send(Err(HostError::Closed(reason.clone())));
    }
}

/// A well-framed body that is not readable JSON. The request it answers gets the error, so the user sees *that* request fail and the driver goes on.
/// Which request: the `id` read out of the raw text when there is one; else, when exactly one request is waiting, that one; else nobody can be told
/// apart, the error is logged, and each request still waiting is answered, or times out, on its own.
fn unreadable(waiting: &Waiting, body: &[u8], error: &serde_json::Error) {
    let id = salvage_id(body);
    tracing::warn!(
        "the driver sent an answer that could not be read ({error}); its request is {}",
        id.map_or("not known".to_string(), |id| id.to_string())
    );
    let mut guard = waiting.lock().unwrap();
    let Some(map) = guard.as_mut() else { return };
    let id = id.or_else(|| (map.len() == 1).then(|| *map.keys().next().expect("one entry")));
    if let Some(tx) = id.and_then(|id| map.remove(&id)) {
        let _ = tx.send(Err(HostError::BadAnswer(error.to_string())));
    }
}

/// The numeric `"id"` of a response whose body does not parse (too deep, cut, not JSON after it). The first `"id":` followed by digits: a quote inside a
/// JSON string is always escaped, so the unescaped text `"id":` is a key, and the driver writes the response's own `id` before its `result`.
fn salvage_id(body: &[u8]) -> Option<u64> {
    const KEY: &[u8] = b"\"id\"";
    let mut from = 0;
    while let Some(at) = body[from..].windows(KEY.len()).position(|w| w == KEY) {
        let mut i = from + at + KEY.len();
        while body.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
            i += 1;
        }
        if body.get(i) == Some(&b':') {
            i += 1;
            while body.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
                i += 1;
            }
            let digits = body[i..].iter().take_while(|b| b.is_ascii_digit()).count();
            if digits > 0 {
                return std::str::from_utf8(&body[i..i + digits]).ok()?.parse().ok();
            }
        }
        from += at + KEY.len();
    }
    None
}

fn reply_of(msg: &Value) -> Reply {
    match msg.get("error") {
        Some(e) => Err(HostError::Remote {
            code: e["code"].as_i64().unwrap_or(0),
            message: e["message"].as_str().unwrap_or("").to_string(),
        }),
        None => Ok(msg["result"].clone()),
    }
}

/// Which driver to run, and the running connection to it.
#[derive(Default)]
pub struct DriverHost {
    path: Mutex<Option<PathBuf>>,
    client: tokio::sync::Mutex<Option<Arc<DriverClient>>>,
}

impl DriverHost {
    /// What the running driver says it can do; empty when none is running.
    pub async fn capabilities(&self) -> std::collections::BTreeMap<String, bool> {
        self.client
            .lock()
            .await
            .as_ref()
            .map(|client| client.info.capabilities.clone())
            .unwrap_or_default()
    }

    pub fn set_path(&self, path: Option<PathBuf>) {
        *self.path.lock().unwrap() = path;
    }

    /// The outline of one file (repo path relative to `root`), starting the driver on first use.
    pub async fn outline(&self, root: &Path, file: &str) -> Result<OutlineResult, HostError> {
        self.with_client(|client| async move { client.outline_file(root, file).await })
            .await
    }

    /// The outline of this text, starting the driver on first use. Nothing is cached: the text is the editor's buffer, and it changes with every keystroke.
    pub async fn outline_text(&self, text: &str) -> Result<OutlineResult, HostError> {
        let text = text.to_string();
        self.with_client(|client| async move { client.outline_text(&text).await })
            .await
    }

    /// Where the `ID:` lines go in `params.text` (the editor's buffer).
    pub async fn plan_ids(&self, params: &PlanIdsParams) -> Result<PlanIdsResult, HostError> {
        self.with_client(|client| async move { client.plan_ids(params).await })
            .await
    }

    /// Where the `# FileID:` comment goes in `params.text` (the editor's buffer).
    pub async fn plan_file_id(
        &self,
        params: &PlanFileIdParams,
    ) -> Result<PlanFileIdResult, HostError> {
        self.with_client(|client| async move { client.plan_file_id(params).await })
            .await
    }

    /// The calls made by one function, or by a class's `__init__`: unmerged, as the driver gives them.
    pub async fn call_tree(&self, params: &CallTreeParams) -> Result<CallTreeResult, HostError> {
        self.with_client(|client| async move { client.call_tree(params).await })
            .await
    }

    /// Runs `request` on the driver. An error from the driver about this request is returned as is, and so is a timeout (the driver has been
    /// told to cancel, and other requests may be running on it). A broken connection drops the driver (its process dies with it) and the next
    /// request starts a new one.
    async fn with_client<T, Fut: std::future::Future<Output = Result<T, HostError>>>(
        &self,
        request: impl FnOnce(Arc<DriverClient>) -> Fut,
    ) -> Result<T, HostError> {
        let client = self.client().await?;
        let result = request(client.clone()).await;
        if matches!(
            &result,
            Err(HostError::Io(_) | HostError::Protocol(_) | HostError::Closed(_))
        ) {
            self.forget(&client).await;
        }
        result
    }

    /// Drops `broken`, unless another request has already replaced it with a new driver.
    async fn forget(&self, broken: &Arc<DriverClient>) {
        let mut slot = self.client.lock().await;
        if slot
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, broken))
        {
            *slot = None;
        }
    }

    async fn client(&self) -> Result<Arc<DriverClient>, HostError> {
        let mut slot = self.client.lock().await;
        if let Some(c) = slot.as_ref() {
            return Ok(c.clone());
        }
        let path = self
            .path
            .lock()
            .unwrap()
            .clone()
            .ok_or(HostError::NotConfigured)?;
        let client = Arc::new(DriverClient::start(&path).await?);
        tracing::info!(
            "driver `{}` {} started from {}",
            client.info.name,
            client.info.version,
            path.display()
        );
        *slot = Some(client.clone());
        Ok(client)
    }
}

/// One frame's body, read whole. The framing errors here (stray output, no `Content-Length`, a closed pipe) are fatal: after them nothing says where the
/// next frame starts.
async fn read_body(stdout: &mut (impl AsyncBufRead + Unpin)) -> Result<Vec<u8>, HostError> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).await? == 0 {
            return Err(HostError::Closed("it closed its output".into()));
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        match line.strip_prefix("Content-Length:") {
            Some(n) => len = n.trim().parse().ok(),
            None => {
                return Err(HostError::Protocol(format!(
                    "stray output on the driver's stdout: {line:?}"
                )));
            }
        }
    }
    let len = len.ok_or_else(|| HostError::Protocol("a frame without Content-Length".into()))?;
    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await?;
    Ok(buf)
}

/// A frame as JSON: framing errors as `read_body`'s, and a body that does not parse is `Protocol` (only the tests read frames this way; the reader
/// of the driver's answers treats an unreadable body as one failed request).
#[cfg(test)]
async fn read_frame(stdout: &mut (impl AsyncBufRead + Unpin)) -> Result<Value, HostError> {
    serde_json::from_slice(&read_body(stdout).await?)
        .map_err(|e| HostError::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, ReadHalf, WriteHalf, split};

    /// The driver's side of the pipes: a stand-in that reads the core's frames and answers as the test says.
    struct Fake {
        from_core: BufReader<ReadHalf<DuplexStream>>,
        to_core: WriteHalf<DuplexStream>,
    }

    impl Fake {
        async fn read(&mut self) -> Value {
            read_frame(&mut self.from_core)
                .await
                .expect("the core sent a frame")
        }

        async fn write(&mut self, message: Value) {
            let body = serde_json::to_vec(&message).unwrap();
            self.to_core
                .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
                .await
                .unwrap();
            self.to_core.write_all(&body).await.unwrap();
            self.to_core.flush().await.unwrap();
        }

        /// A frame whose body is these bytes, whatever they are.
        async fn write_raw(&mut self, body: &[u8]) {
            self.to_core
                .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
                .await
                .unwrap();
            self.to_core.write_all(body).await.unwrap();
            self.to_core.flush().await.unwrap();
        }

        async fn answer(&mut self, id: &Value, result: Value) {
            self.write(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                .await;
        }

        /// `initialize`, then `initialized`.
        async fn handshake(&mut self) {
            let init = self.read().await;
            assert_eq!(init["method"], "initialize");
            let capabilities =
                json!({ "outline": true, "ids.plan": true, "ids.file": true, "callTree": true });
            self.answer(&init["id"], json!({ "protocol": PROTOCOL_VERSION, "name": "fake", "version": "0", "capabilities": capabilities })).await;
            assert_eq!(self.read().await["method"], "initialized");
        }
    }

    async fn connected() -> (Arc<DriverClient>, Fake) {
        let (core_out, fake_in) = tokio::io::duplex(64 * 1024);
        let (fake_out, core_in) = tokio::io::duplex(64 * 1024);
        let (from_core, _) = split(fake_in);
        let (_, to_core) = split(fake_out);
        let mut fake = Fake {
            from_core: BufReader::new(from_core),
            to_core,
        };
        let (client, ()) = tokio::join!(
            async {
                DriverClient::connect(core_out, core_in, None)
                    .await
                    .expect("the handshake works")
            },
            fake.handshake()
        );
        (Arc::new(client), fake)
    }

    fn asked(n: u32) -> Value {
        json!({ "n": n })
    }

    #[tokio::test]
    async fn two_requests_answered_in_reverse_order_each_get_their_own_answer() {
        let (client, mut fake) = connected().await;
        let ask = |n: u32| {
            let client = client.clone();
            tokio::spawn(async move { client.call::<Value>(method::OUTLINE, &asked(n)).await })
        };
        let (one, two) = (ask(1), ask(2));

        let (first, second) = (fake.read().await, fake.read().await);
        assert_ne!(first["id"], second["id"]);
        // the later request is answered first; each answer carries the `n` of the request it belongs to
        for request in [&second, &first] {
            fake.answer(&request["id"], json!({ "echo": request["params"]["n"] }))
                .await;
        }

        assert_eq!(one.await.unwrap().unwrap(), json!({ "echo": 1 }));
        assert_eq!(two.await.unwrap().unwrap(), json!({ "echo": 2 }));
    }

    #[tokio::test]
    async fn a_request_that_times_out_is_cancelled_and_its_late_answer_is_ignored() {
        let (client, mut fake) = connected().await;
        let slow = client
            .wire
            .request(method::OUTLINE, &asked(1), Duration::from_millis(50))
            .await;
        assert!(
            matches!(slow, Err(HostError::Timeout(m)) if m == method::OUTLINE),
            "{slow:?}"
        );

        let request = fake.read().await;
        let cancel = fake.read().await;
        assert_eq!(
            (
                cancel["method"].as_str(),
                &cancel["params"]["id"],
                cancel.get("id")
            ),
            (Some("$/cancelRequest"), &request["id"], None),
            "a notification naming the request"
        );
        assert!(
            client
                .wire
                .waiting
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_empty(),
            "nothing is waiting any more"
        );

        // the answer comes anyway, then the answer to the next request: only the second is used
        fake.answer(&request["id"], json!("late")).await;
        let next = tokio::spawn({
            let client = client.clone();
            async move { client.call::<Value>(method::OUTLINE, &asked(2)).await }
        });
        let second = fake.read().await;
        fake.answer(&second["id"], json!("on time")).await;
        assert_eq!(next.await.unwrap().unwrap(), json!("on time"));
    }

    #[tokio::test]
    async fn a_request_whose_caller_goes_away_is_cancelled() {
        let (client, mut fake) = connected().await;
        let task = tokio::spawn({
            let client = client.clone();
            async move { client.call::<Value>(method::CALL_TREE, &asked(1)).await }
        });
        let request = fake.read().await;
        task.abort();
        let _ = task.await;

        let cancel = fake.read().await;
        assert_eq!(
            (cancel["method"].as_str(), &cancel["params"]["id"]),
            (Some("$/cancelRequest"), &request["id"])
        );
    }

    #[tokio::test]
    async fn a_driver_that_dies_fails_every_waiting_request_and_the_next_one() {
        let (client, mut fake) = connected().await;
        let ask = |n: u32| {
            let client = client.clone();
            tokio::spawn(async move { client.call::<Value>(method::OUTLINE, &asked(n)).await })
        };
        let (one, two) = (ask(1), ask(2));
        fake.read().await;
        fake.read().await;

        drop(fake); // the driver's process ends: its output closes
        for waiting in [one, two] {
            assert!(matches!(waiting.await.unwrap(), Err(HostError::Closed(_))));
        }
        let after = client.call::<Value>(method::OUTLINE, &asked(3)).await;
        assert!(matches!(after, Err(HostError::Closed(_))), "{after:?}");
    }

    #[tokio::test]
    async fn only_the_driver_that_broke_is_forgotten() {
        let (old, _fake_old) = connected().await;
        let (new, _fake_new) = connected().await;
        let host = DriverHost::default();
        *host.client.lock().await = Some(new.clone());

        host.forget(&old).await; // a late failure of the old driver must not drop the new one
        assert!(host.client.lock().await.is_some());
        host.forget(&new).await;
        assert!(host.client.lock().await.is_none());
    }

    #[tokio::test]
    async fn a_timeout_keeps_the_driver_and_a_broken_connection_drops_it() {
        let (client, mut fake) = connected().await;
        let host = DriverHost::default();
        *host.client.lock().await = Some(client);

        let slow = host
            .with_client(|client| async move {
                client
                    .wire
                    .request(method::OUTLINE, &asked(1), Duration::from_millis(50))
                    .await
            })
            .await;
        assert!(matches!(slow, Err(HostError::Timeout(_))), "{slow:?}");
        assert!(
            host.client.lock().await.is_some(),
            "other requests may be running on it: it stays"
        );

        // the same driver answers the next request
        let next = host.with_client(|client| async move {
            client.call::<Value>(method::OUTLINE, &asked(2)).await
        });
        let answer = async {
            // the first request, its cancel and the second request arrive in no fixed order: answer the second
            loop {
                let frame = fake.read().await;
                if frame["method"] == method::OUTLINE && frame["params"]["n"] == 2 {
                    fake.answer(&frame["id"], json!("still here")).await;
                    break fake;
                }
            }
        };
        let (next, fake) = tokio::join!(next, answer);
        assert_eq!(next.unwrap(), json!("still here"));

        drop(fake);
        let gone = host
            .with_client(
                |client| async move { client.call::<Value>(method::OUTLINE, &asked(3)).await },
            )
            .await;
        // the write may fail before the reader has seen the driver's output close: either way the connection is broken
        assert!(
            matches!(gone, Err(HostError::Closed(_) | HostError::Io(_))),
            "{gone:?}"
        );
        assert!(
            host.client.lock().await.is_none(),
            "a driver that is gone is dropped, and the next request starts a new one"
        );
    }

    #[tokio::test]
    async fn a_driver_that_stops_reading_fails_the_request_in_time_and_is_dropped() {
        let (client, _fake) = connected().await; // the fake never reads again, so a request bigger than the pipe cannot be written
        let host = DriverHost::default();
        *host.client.lock().await = Some(client);

        let big = json!({ "pad": "x".repeat(200_000) });
        let stuck = host.with_client(|client| async move {
            client
                .wire
                .request(method::OUTLINE, &big, Duration::from_millis(50))
                .await
        });
        let stuck = tokio::time::timeout(Duration::from_secs(5), stuck)
            .await
            .expect("the limit covers the write: it does not hang");
        assert!(matches!(stuck, Err(HostError::Closed(_))), "{stuck:?}");
        assert!(
            host.client.lock().await.is_none(),
            "nothing else would end a driver that is stuck"
        );
    }

    #[test]
    fn a_call_tree_is_given_more_time_than_an_outline() {
        let limits: Vec<_> = [
            method::OUTLINE,
            method::PLAN_IDS,
            method::PLAN_FILE_ID,
            method::CALL_TREE,
        ]
        .map(timeout_of)
        .into();
        assert_eq!(
            limits,
            [
                REQUEST_TIMEOUT,
                REQUEST_TIMEOUT,
                REQUEST_TIMEOUT,
                CALL_TREE_TIMEOUT
            ]
        );
        assert_eq!(
            (REQUEST_TIMEOUT.as_secs(), CALL_TREE_TIMEOUT.as_secs()),
            (10, 30)
        );
    }

    // ── one unreadable answer fails its request, not the driver ─────────────────────────────────────────────────────────

    /// A host that holds this connected client, as `with_client` needs.
    async fn hosted() -> (DriverHost, Arc<DriverClient>, Fake) {
        let (client, fake) = connected().await;
        let host = DriverHost::default();
        *host.client.lock().await = Some(client.clone());
        (host, client, fake)
    }

    /// The next request is answered, on the same driver (the pointer is the one that was there, and nothing was started again).
    async fn next_request_is_answered(
        host: &DriverHost,
        client: &Arc<DriverClient>,
        fake: &mut Fake,
    ) {
        let next =
            host.with_client(|c| async move { c.call::<Value>(method::OUTLINE, &asked(2)).await });
        let answer = async {
            loop {
                let frame = fake.read().await;
                if frame["method"] == method::OUTLINE && frame["params"]["n"] == 2 {
                    fake.answer(&frame["id"], json!("fine")).await;
                    break;
                }
            }
        };
        let (next, ()) = tokio::join!(next, answer);
        assert_eq!(next.unwrap(), json!("fine"));
        assert!(
            Arc::ptr_eq(
                host.client
                    .lock()
                    .await
                    .as_ref()
                    .expect("the driver is still held"),
                client
            ),
            "the same driver: it was never restarted"
        );
    }

    #[tokio::test]
    async fn a_well_framed_body_that_is_not_json_fails_its_request_with_a_readable_message_and_the_next_one_is_answered()
     {
        let (host, client, mut fake) = hosted().await;
        let first =
            host.with_client(|c| async move { c.call::<Value>(method::OUTLINE, &asked(1)).await });
        let garbage = async {
            let request = fake.read().await;
            // the id is in the text, so the right request is the one that fails
            fake.write_raw(
                format!(
                    r#"{{"jsonrpc":"2.0","id":{},"result":{{ this is not json"#,
                    request["id"]
                )
                .as_bytes(),
            )
            .await;
        };
        let (first, ()) = tokio::join!(first, garbage);
        let error = first.unwrap_err();
        assert!(matches!(error, HostError::BadAnswer(_)), "{error:?}");
        assert!(
            error
                .to_string()
                .starts_with("the driver's answer could not be read: "),
            "{error}"
        );
        next_request_is_answered(&host, &client, &mut fake).await;
    }

    #[tokio::test]
    async fn a_body_with_no_id_fails_the_one_request_that_is_waiting_and_the_driver_goes_on() {
        let (host, client, mut fake) = hosted().await;
        let first =
            host.with_client(|c| async move { c.call::<Value>(method::OUTLINE, &asked(1)).await });
        let garbage = async {
            fake.read().await;
            fake.write_raw(b"not json at all").await;
        };
        let (first, ()) = tokio::join!(first, garbage);
        assert!(matches!(first, Err(HostError::BadAnswer(_))), "{first:?}");
        next_request_is_answered(&host, &client, &mut fake).await;
    }

    #[tokio::test]
    async fn with_two_requests_waiting_a_body_with_no_id_cannot_say_which_so_both_keep_waiting_for_their_own_answers()
     {
        let (client, mut fake) = connected().await;
        let ask = |n: u32| {
            let client = client.clone();
            tokio::spawn(async move { client.call::<Value>(method::OUTLINE, &asked(n)).await })
        };
        let (one, two) = (ask(1), ask(2));
        let (a, b) = (fake.read().await, fake.read().await);
        fake.write_raw(b"{ nonsense").await;
        fake.answer(&a["id"], json!("a")).await;
        fake.answer(&b["id"], json!("b")).await;
        let mut answers = vec![one.await.unwrap().unwrap(), two.await.unwrap().unwrap()];
        answers.sort_by_key(|v| v.as_str().unwrap().to_string());
        assert_eq!(
            answers,
            [json!("a"), json!("b")],
            "neither was failed for something it did not cause"
        );
    }

    #[tokio::test]
    async fn an_answer_nested_deeper_than_the_parser_allows_fails_its_request_and_not_the_driver() {
        // the real case: a call tree of a long chain (valid JSON, 200 levels): serde_json's limit is 128
        let (host, client, mut fake) = hosted().await;
        let first = host
            .with_client(|c| async move { c.call::<Value>(method::CALL_TREE, &asked(1)).await });
        let deep = async {
            let request = fake.read().await;
            let body = format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{}{}}}"#,
                request["id"],
                "[".repeat(200),
                "]".repeat(200)
            );
            fake.write_raw(body.as_bytes()).await;
        };
        let (first, ()) = tokio::join!(first, deep);
        let error = first.unwrap_err();
        assert!(
            matches!(&error, HostError::BadAnswer(m) if m.contains("recursion limit")),
            "{error:?}"
        );
        next_request_is_answered(&host, &client, &mut fake).await;
    }

    #[tokio::test]
    async fn an_answer_of_the_wrong_shape_is_one_bad_answer_too() {
        let (host, client, mut fake) = hosted().await;
        let first = host
            .with_client(|c| async move { c.call::<Vec<u32>>(method::OUTLINE, &asked(1)).await });
        let wrong = async {
            let request = fake.read().await;
            fake.answer(&request["id"], json!("a string, not a list"))
                .await;
        };
        let (first, ()) = tokio::join!(first, wrong);
        assert!(matches!(first, Err(HostError::BadAnswer(_))), "{first:?}");
        next_request_is_answered(&host, &client, &mut fake).await;
    }

    #[tokio::test]
    async fn a_frame_without_content_length_still_ends_the_driver() {
        let (host, _client, mut fake) = hosted().await;
        let first =
            host.with_client(|c| async move { c.call::<Value>(method::OUTLINE, &asked(1)).await });
        let broken = async {
            fake.read().await;
            fake.to_core.write_all(b"\r\n").await.unwrap(); // a header block with nothing in it: no Content-Length
            fake.to_core.flush().await.unwrap();
        };
        let (first, ()) = tokio::join!(first, broken);
        assert!(matches!(first, Err(HostError::Closed(_))), "{first:?}");
        assert!(
            host.client.lock().await.is_none(),
            "after a framing error nothing says where the next frame starts: the driver is dropped"
        );
    }

    #[tokio::test]
    async fn stray_output_on_the_drivers_stdout_still_ends_it() {
        let (host, _client, mut fake) = hosted().await;
        let first =
            host.with_client(|c| async move { c.call::<Value>(method::OUTLINE, &asked(1)).await });
        let stray = async {
            fake.read().await;
            fake.to_core
                .write_all(b"hello from a print statement\r\n")
                .await
                .unwrap();
            fake.to_core.flush().await.unwrap();
        };
        let (first, ()) = tokio::join!(first, stray);
        assert!(matches!(first, Err(HostError::Closed(_))), "{first:?}");
        assert!(host.client.lock().await.is_none());
    }

    #[test]
    fn the_id_of_a_response_is_read_out_of_text_that_does_not_parse() {
        let table: &[(&str, Option<u64>)] = &[
            (r#"{"jsonrpc":"2.0","id":7,"result":{ oops"#, Some(7)),
            (r#"{"id" : 42 , "result": [[[["#, Some(42)),
            (r#"{"result":"x","id":9"#, Some(9)),
            (
                r#"{"jsonrpc":"2.0","result":"has \"id\":5 inside a string"#,
                None,
            ),
            (r#"{"id":"a-string-id"}"#, None),
            (r#"{"id":null,"error":{"code":1}"#, None),
            ("not json", None),
            ("", None),
        ];
        for (body, expected) in table {
            assert_eq!(salvage_id(body.as_bytes()), *expected, "{body}");
        }
    }
}
