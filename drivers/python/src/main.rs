//! `vnoc-driver-python`: speaks the V-NOC driver protocol (plan_vs/07-drivers/01-driver-protocol.md) on stdin / stdout.
//!
//! stdout IS the protocol. Nothing else may write there, not even a panic message: the two lints below, a stderr-only
//! logger, and the test harness that fails a run on any byte outside a frame keep it that way.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod call_tree;
mod file_id;
mod outline;
mod plan_ids;
mod positions;
mod worker;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use lsp_server::{Connection, ErrorCode, Message, Request, RequestId, Response};
use python_analysis::Cancel;
use serde_json::Value;
use vnoc_driver_protocol::{InitializeParams, InitializeResult, OutlineParams, PROTOCOL_VERSION, PlanFileIdParams, PlanIdsParams, method};

fn main() -> anyhow::Result<()> {
    // Filtered by default: the type checker underneath traces every query, which is tens of MB per run.
    let filter = tracing_subscriber::EnvFilter::try_from_env("VNOC_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,vnoc_driver_python=info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_ansi(false).init();

    let (conn, io) = Connection::stdio();
    if let Err(refused) = handshake(&conn) {
        // The reason is still on its way out: the writer thread holds stdout's lock until it has written it, so taking the lock
        // waits for the whole reply. (`io.join()` would wait for stdin to close as well, and the core is still holding it open.)
        drop(conn);
        drop(std::io::stdout().lock());
        return Err(refused);
    }

    // The main loop only dispatches (VS-D68). Outline and the ids run each on a thread of their own; `vnoc/callTree` goes to the worker, which
    // owns the projects and answers one at a time (worker.rs). Every request has a cancel flag (and for a call tree the analyzer's `Cancel`, with its
    // deadline), so a cancel, a shutdown or stdin EOF is handled while an analysis runs or waits.
    let wire_format = analyzer_core::wire::WIRE_FORMAT_VERSION;
    if call_tree::WIRE_FORMATS_READ.contains(&wire_format) {
        tracing::info!("analyzer wire format {wire_format}");
    } else {
        tracing::warn!("analyzer wire format {wire_format} is not one this driver was checked against {:?}", call_tree::WIRE_FORMATS_READ);
    }
    let worker = worker::spawn();
    let in_flight: Arc<Mutex<HashMap<RequestId, Arc<InFlight>>>> = Arc::default();
    for msg in &conn.receiver {
        match msg {
            Message::Request(req) => {
                if conn.handle_shutdown(&req)? {
                    break;
                }
                let received = Instant::now();
                let request = Arc::new(InFlight { flag: Arc::new(AtomicBool::new(false)), cancel: Cancel::new().with_deadline(received + call_deadline()) });
                in_flight.lock().unwrap().insert(req.id.clone(), request.clone());
                let (sender, in_flight) = (conn.sender.clone(), in_flight.clone());
                // how an answer gets out, and forgets the request
                let reply = {
                    let (sender, in_flight, id) = (sender.clone(), in_flight.clone(), req.id.clone());
                    Box::new(move |resp: Response| {
                        in_flight.lock().unwrap().remove(&id);
                        let _ = sender.send(Message::Response(resp));
                    }) as worker::Reply
                };
                if req.method == method::CALL_TREE {
                    let job = worker::Job { id: req.id, params: req.params, flag: request.flag.clone(), cancel: request.cancel.clone(), reply };
                    let _ = worker.send(worker::Task::CallTree(job));
                } else if test_hooks() && req.method.starts_with("vnoc/test/") {
                    match req.method.as_str() {
                        "vnoc/test/deadline" => reply(set_test_deadline(req.id, &req.params)),
                        _ => {
                            let _ = worker.send(worker::Task::Test { id: req.id, method: req.method, params: req.params, reply });
                        }
                    }
                } else {
                    std::thread::spawn(move || {
                        let resp = respond(req, &request.flag);
                        reply(resp);
                    });
                }
            }
            Message::Notification(n) if n.method == "$/cancelRequest" => {
                let id = n.params.get("id").and_then(|v| serde_json::from_value::<RequestId>(v.clone()).ok());
                if let Some(request) = id.and_then(|id| in_flight.lock().unwrap().get(&id).cloned()) {
                    request.flag.store(true, Ordering::Relaxed);
                    request.cancel.cancel();
                }
            }
            // An unknown notification is ignored: a driver never exits over a message it does not understand.
            Message::Notification(n) => tracing::debug!("ignored notification {}", n.method),
            Message::Response(_) => {}
        }
    }
    // shutdown, `exit`, or stdin EOF: tell running work to stop and leave
    for request in in_flight.lock().unwrap().values() {
        request.flag.store(true, Ordering::Relaxed);
        request.cancel.cancel();
    }
    drop(conn);
    io.join()?;
    Ok(())
}

/// What a request in flight can be told: the flag every request has, and the analyzer's `Cancel` that a call tree's solver reads.
struct InFlight {
    flag: Arc<AtomicBool>,
    cancel: Cancel,
}

/// A solve that runs this long (counted from when the request arrived, so the queue counts) ends by itself: the tree then carries `incomplete`.
const CALL_DEADLINE: Duration = Duration::from_secs(20);
/// `vnoc/test/deadline { ms }` (test hooks) replaces it; `u64::MAX` is "not replaced".
static TEST_DEADLINE_MS: AtomicU64 = AtomicU64::new(u64::MAX);

fn call_deadline() -> Duration {
    match TEST_DEADLINE_MS.load(Ordering::Relaxed) {
        u64::MAX => CALL_DEADLINE,
        ms => Duration::from_millis(ms),
    }
}

/// The hooks the window and driver tests use are off unless the environment says `VNOC_TEST_HOOKS`: a driver in a user's window answers none of them.
fn test_hooks() -> bool {
    std::env::var_os("VNOC_TEST_HOOKS").is_some()
}

fn set_test_deadline(id: RequestId, params: &Value) -> Response {
    TEST_DEADLINE_MS.store(params.get("ms").and_then(Value::as_u64).unwrap_or(u64::MAX), Ordering::Relaxed);
    Response::new_ok(id, Value::Null)
}

/// `initialize { protocol: 1 }`: refuse a protocol we do not speak, with the reason.
fn handshake(conn: &Connection) -> anyhow::Result<()> {
    let (id, params) = conn.initialize_start()?;
    let asked = serde_json::from_value::<InitializeParams>(params).ok().map(|p| p.protocol);
    if asked != Some(PROTOCOL_VERSION) {
        let reason = format!("this driver speaks protocol {PROTOCOL_VERSION}, the core asked for {asked:?}");
        conn.sender.send(Message::Response(Response::new_err(id, ErrorCode::InvalidRequest as i32, reason.clone())))?;
        return Err(anyhow!(reason));
    }
    let reply = InitializeResult {
        protocol: PROTOCOL_VERSION,
        name: "python".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        // present even when small: stock LSP clients refuse a reply without it. Optional methods are announced here.
        capabilities: [("outline", true), ("ids.plan", true), ("ids.file", true), ("callTree", true), ("environmentInputs", false)].into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    };
    conn.initialize_finish(id, serde_json::to_value(reply)?)?;
    Ok(())
}

fn respond(req: Request, cancel: &AtomicBool) -> Response {
    let cancelled = |id| Response::new_err(id, ErrorCode::RequestCanceled as i32, "cancelled".into());
    if cancel.load(Ordering::Relaxed) {
        return cancelled(req.id);
    }
    let result = match req.method.as_str() {
        method::OUTLINE => outline_request(&req.params),
        method::PLAN_IDS => plan_ids_request(req.params.clone()),
        method::PLAN_FILE_ID => plan_file_id_request(req.params.clone()),
        other => return Response::new_err(req.id, ErrorCode::MethodNotFound as i32, format!("unknown method {other}")),
    };
    // A running analysis cannot be interrupted (the analyzer has no cancel flag, VS-D69): its answer is dropped.
    if cancel.load(Ordering::Relaxed) {
        return cancelled(req.id);
    }
    match result {
        Ok(v) => Response::new_ok(req.id, v),
        Err(e) => Response::new_err(req.id, e.code as i32, e.message),
    }
}

struct Failure {
    code: ErrorCode,
    message: String,
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure { code: ErrorCode::InvalidParams, message: message.into() }
}

fn outline_request(params: &Value) -> Result<Value, Failure> {
    if params.get("fingerprints").and_then(Value::as_bool) == Some(true) {
        return Err(invalid("`fingerprints` is not supported by this driver version"));
    }
    let p: OutlineParams = serde_json::from_value(params.clone()).map_err(|e| invalid(e.to_string()))?;
    let text = match (p.text, p.root, p.file) {
        (Some(text), _, _) => text,
        (None, Some(root), Some(file)) => {
            std::fs::read_to_string(std::path::Path::new(&root).join(&file)).map_err(|e| Failure { code: ErrorCode::InternalError, message: format!("cannot read {file}: {e}") })?
        }
        _ => return Err(invalid("give `text`, or `root` and `file`")),
    };
    serde_json::to_value(outline::outline(&text)).map_err(|e| Failure { code: ErrorCode::InternalError, message: e.to_string() })
}

fn plan_ids_request(params: Value) -> Result<Value, Failure> {
    let params: PlanIdsParams = serde_json::from_value(params).map_err(|e| invalid(e.to_string()))?;
    serde_json::to_value(plan_ids::plan(params)?).map_err(|e| Failure { code: ErrorCode::InternalError, message: e.to_string() })
}

fn plan_file_id_request(params: Value) -> Result<Value, Failure> {
    let params: PlanFileIdParams = serde_json::from_value(params).map_err(|e| invalid(e.to_string()))?;
    serde_json::to_value(file_id::plan(params)).map_err(|e| Failure { code: ErrorCode::InternalError, message: e.to_string() })
}
