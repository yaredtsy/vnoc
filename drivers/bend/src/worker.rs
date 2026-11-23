//! The one thread that owns the sessions (docs/16-ui-redesign/10-build-order.md 8.2a).
//!
//! `vnoc/callTree` is sent here. The worker keeps one open `Session` per (root, entry file), brings it up to date with the disk before each request, and answers
//! one request at a time. Outline, `planIds` and `planFileId` stay on their own threads (`main.rs`): they need no session.
//!
//! A request that is cancelled while it waits in the queue is answered `-32800` without being run. One that panics is answered with an error, its
//! sessions are dropped (they may be half way through a change), and the worker goes on with the next.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use bend_analysis::Cancel;
use lsp_server::{ErrorCode, RequestId, Response};
use serde_json::{Value, json};
use vnoc_driver_protocol::CallTreeParams;

use crate::call_tree::{Sessions, call_tree};
use crate::{Failure, invalid};

/// The worker's stack: as the Python driver's, whose analyzer measured that a request at its deepest needs just over 2 MiB, and an overflow aborts the
/// whole process. Bend's own depth limit is 1,000 steps (`Limits::default`) and its walk is not measured against this number: the first by-hand check
/// on a deep project (B6) is where to look.
const STACK: usize = 8 << 20;

/// How an answer leaves the worker: the main loop gives each job the way to its own connection.
pub type Reply = Box<dyn FnOnce(Response) + Send>;

pub struct Job {
    pub id: RequestId,
    pub params: Value,
    /// set by `$/cancelRequest`, and when the driver shuts down
    pub flag: Arc<AtomicBool>,
    /// the same request as the solver sees it: `cancel()` is called with the flag; its deadline counts from when the request arrived
    pub cancel: Cancel,
    pub reply: Reply,
}

pub enum Task {
    CallTree(Job),
    /// a request of the test hooks (`VNOC_TEST_HOOKS=1`): it goes through the same queue, so it is ordered with the call trees
    Test {
        id: RequestId,
        method: String,
        params: Value,
        reply: Reply,
    },
}

pub fn spawn() -> Sender<Task> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("vnoc-call-tree".into())
        .stack_size(STACK)
        .spawn(move || run(rx))
        .expect("the worker thread starts");
    tx
}

fn run(queue: Receiver<Task>) {
    let mut sessions = Sessions::default();
    for task in queue {
        match task {
            Task::CallTree(job) => serve(&mut sessions, job, |sessions, params, cancel| {
                let params: CallTreeParams =
                    serde_json::from_value(params).map_err(|e| invalid(e.to_string()))?;
                to_value(call_tree(sessions, params, cancel)?)
            }),
            Task::Test {
                id,
                method,
                params,
                reply,
            } => reply(test_hook(&mut sessions, id, &method, params)),
        }
    }
}

fn cancelled(id: RequestId) -> Response {
    Response::new_err(id, ErrorCode::RequestCanceled as i32, "cancelled".into())
}

fn to_value(result: impl serde::Serialize) -> Result<Value, Failure> {
    serde_json::to_value(result).map_err(|e| Failure {
        code: ErrorCode::InternalError,
        message: e.to_string(),
    })
}

/// Runs one request on the worker: not at all if it was cancelled while it waited, inside a catch for a panic, and its answer dropped if it was
/// cancelled while it ran.
fn serve(
    sessions: &mut Sessions,
    job: Job,
    run: impl FnOnce(&mut Sessions, Value, &Cancel) -> Result<Value, Failure>,
) {
    let Job {
        id,
        params,
        flag,
        cancel,
        reply,
    } = job;
    // cancelled while it waited: never run (the solver would have cut it at once, but the session would still have been refreshed)
    if flag.load(Ordering::Relaxed) {
        return reply(cancelled(id));
    }
    let root = params
        .get("root")
        .and_then(Value::as_str)
        .map(str::to_string);
    let outcome = catch_unwind(AssertUnwindSafe(|| run(sessions, params, &cancel)));
    let result = outcome.unwrap_or_else(|payload| {
        if let Some(root) = &root {
            sessions.forget(root);
        }
        Err(Failure {
            code: ErrorCode::InternalError,
            message: format!("the analysis crashed: {}", panic_message(&payload)),
        })
    });
    // a running solve is stopped by the cancel; whatever it made is dropped, as it always was
    if flag.load(Ordering::Relaxed) {
        return reply(cancelled(id));
    }
    reply(match result {
        Ok(v) => Response::new_ok(id, v),
        Err(e) => Response::new_err(id, e.code as i32, e.message),
    });
}

fn panic_message(payload: &Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "no message".into())
}

/// The test hooks that need the worker: `vnoc/test/stats`, `vnoc/test/hold { ms }` (keeps the worker busy, so a request can be queued behind it) and
/// `vnoc/test/panic { root? }` (a request that panics, inside the same wrapper a call tree runs in).
fn test_hook(sessions: &mut Sessions, id: RequestId, method: &str, params: Value) -> Response {
    match method {
        "vnoc/test/stats" => {
            let c = sessions.counts;
            Response::new_ok(
                id,
                json!({ "solves": c.solves, "opens": c.opens, "reopens": c.reopens, "sessions": sessions.len() }),
            )
        }
        "vnoc/test/hold" => {
            std::thread::sleep(Duration::from_millis(
                params.get("ms").and_then(Value::as_u64).unwrap_or(0),
            ));
            Response::new_ok(id, Value::Null)
        }
        "vnoc/test/panic" => {
            let root = params
                .get("root")
                .and_then(Value::as_str)
                .map(str::to_string);
            let crashed = catch_unwind(AssertUnwindSafe(|| -> () {
                panic!("a test hook panicked on purpose")
            }));
            if let Some(root) = root {
                sessions.forget(&root);
            }
            match crashed {
                Err(payload) => Response::new_err(
                    id,
                    ErrorCode::InternalError as i32,
                    format!("the analysis crashed: {}", panic_message(&payload)),
                ),
                Ok(()) => Response::new_ok(id, Value::Null),
            }
        }
        other => Response::new_err(
            id,
            ErrorCode::MethodNotFound as i32,
            format!("unknown method {other}"),
        ),
    }
}
