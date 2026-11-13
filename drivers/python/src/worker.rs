//! The one thread that owns the projects (docs/16-ui-redesign/10-build-order.md 8.2a; the analyzer's `11-driver-needs.md` §8).
//!
//! `vnoc/callTree` is sent here. The worker keeps one open `Project` per root, brings it up to date with the disk before each request, and answers
//! one request at a time: the engine is not shared between threads, and a request on a kept project is ten times faster than on a new one
//! (the memo of what it has already worked out). Outline, `planIds` and `planFileId` stay on their own threads (`main.rs`): they need no project.
//!
//! A request that is cancelled while it waits in the queue is answered `-32800` without being run. One that panics is answered with an error, its
//! project is dropped (it may be half way through a change), and the worker goes on with the next.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

use lsp_server::{ErrorCode, RequestId, Response};
use python_analysis::Cancel;
use serde_json::{Value, json};
use vnoc_driver_protocol::CallTreeParams;

use crate::call_tree::{Projects, call_tree};
use crate::{Failure, invalid};

/// The worker's stack. The analyzer's `ENGINE.md` ("The stack"): a request that reaches `max_stack` (256 levels) needs just over 2 MiB, and an
/// overflow aborts the whole process. 2 MiB rounded up to the next power of two is 4 MiB, doubled for a margin: 8 MiB.
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
    Test { id: RequestId, method: String, params: Value, reply: Reply },
}

pub fn spawn() -> Sender<Task> {
    let (tx, rx) = channel();
    std::thread::Builder::new().name("vnoc-call-tree".into()).stack_size(STACK).spawn(move || run(rx)).expect("the worker thread starts");
    tx
}

fn run(queue: Receiver<Task>) {
    let mut projects = Projects::default();
    for task in queue {
        match task {
            Task::CallTree(job) => serve(&mut projects, job),
            Task::Test { id, method, params, reply } => reply(test_hook(&mut projects, id, &method, params)),
        }
    }
}

fn cancelled(id: RequestId) -> Response {
    Response::new_err(id, ErrorCode::RequestCanceled as i32, "cancelled".into())
}

fn serve(projects: &mut Projects, job: Job) {
    let Job { id, params, flag, cancel, reply } = job;
    // cancelled while it waited: never run (the solver would have cut it at once, but the project would still have been refreshed)
    if flag.load(Ordering::Relaxed) {
        return reply(cancelled(id));
    }
    let root = params.get("root").and_then(Value::as_str).map(str::to_string);
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<Value, Failure> {
        let params: CallTreeParams = serde_json::from_value(params).map_err(|e| invalid(e.to_string()))?;
        let result = call_tree(projects, params, &cancel)?;
        serde_json::to_value(result).map_err(|e| Failure { code: ErrorCode::InternalError, message: e.to_string() })
    }));
    let result = outcome.unwrap_or_else(|payload| {
        if let Some(root) = &root {
            projects.forget(root);
        }
        Err(Failure { code: ErrorCode::InternalError, message: format!("the analysis crashed: {}", panic_message(&payload)) })
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
    payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "no message".into())
}

/// The test hooks that need the worker: `vnoc/test/stats`, `vnoc/test/hold { ms }` (keeps the worker busy, so a request can be queued behind it) and
/// `vnoc/test/panic { root? }` (a request that panics, inside the same wrapper a call tree runs in).
fn test_hook(projects: &mut Projects, id: RequestId, method: &str, params: Value) -> Response {
    match method {
        "vnoc/test/stats" => {
            let c = projects.counts;
            Response::new_ok(id, json!({ "solves": c.solves, "opens": c.opens, "reopens": c.reopens, "projects": projects.len() }))
        }
        "vnoc/test/hold" => {
            std::thread::sleep(Duration::from_millis(params.get("ms").and_then(Value::as_u64).unwrap_or(0)));
            Response::new_ok(id, Value::Null)
        }
        "vnoc/test/panic" => {
            let root = params.get("root").and_then(Value::as_str).map(str::to_string);
            let crashed = catch_unwind(AssertUnwindSafe(|| -> () { panic!("a test hook panicked on purpose") }));
            if let Some(root) = root {
                projects.forget(&root);
            }
            match crashed {
                Err(payload) => Response::new_err(id, ErrorCode::InternalError as i32, format!("the analysis crashed: {}", panic_message(&payload))),
                Ok(()) => Response::new_ok(id, Value::Null),
            }
        }
        other => Response::new_err(id, ErrorCode::MethodNotFound as i32, format!("unknown method {other}")),
    }
}
