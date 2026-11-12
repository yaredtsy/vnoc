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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use lsp_server::{Connection, ErrorCode, Message, Request, RequestId, Response};
use serde_json::Value;
use vnoc_driver_protocol::{CallTreeParams, InitializeParams, InitializeResult, OutlineParams, PROTOCOL_VERSION, PlanFileIdParams, PlanIdsParams, method};

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

    // The main loop only dispatches (VS-D68). Each request runs on its own thread with a cancel flag, so a cancel, a
    // shutdown or stdin EOF is handled while an analysis runs.
    let in_flight: Arc<Mutex<HashMap<RequestId, Arc<AtomicBool>>>> = Arc::default();
    for msg in &conn.receiver {
        match msg {
            Message::Request(req) => {
                if conn.handle_shutdown(&req)? {
                    break;
                }
                let cancel = Arc::new(AtomicBool::new(false));
                in_flight.lock().unwrap().insert(req.id.clone(), cancel.clone());
                let (sender, in_flight) = (conn.sender.clone(), in_flight.clone());
                std::thread::spawn(move || {
                    let id = req.id.clone();
                    let resp = respond(req, &cancel);
                    in_flight.lock().unwrap().remove(&id);
                    let _ = sender.send(Message::Response(resp));
                });
            }
            Message::Notification(n) if n.method == "$/cancelRequest" => {
                let id = n.params.get("id").and_then(|v| serde_json::from_value::<RequestId>(v.clone()).ok());
                if let Some(flag) = id.and_then(|id| in_flight.lock().unwrap().get(&id).cloned()) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            // An unknown notification is ignored: a driver never exits over a message it does not understand.
            Message::Notification(n) => tracing::debug!("ignored notification {}", n.method),
            Message::Response(_) => {}
        }
    }
    // shutdown, `exit`, or stdin EOF: tell running work to stop and leave
    for flag in in_flight.lock().unwrap().values() {
        flag.store(true, Ordering::Relaxed);
    }
    drop(conn);
    io.join()?;
    Ok(())
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
        method::CALL_TREE => call_tree_request(req.params.clone()),
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

fn call_tree_request(params: Value) -> Result<Value, Failure> {
    let params: CallTreeParams = serde_json::from_value(params).map_err(|e| invalid(e.to_string()))?;
    let result = call_tree::call_tree(params)?;
    serde_json::to_value(result).map_err(|e| Failure { code: ErrorCode::InternalError, message: e.to_string() })
}
