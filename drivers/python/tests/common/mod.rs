//! The harness of the driver's integration tests: the real binary on stdio, with the test hooks on. Any byte on stdout that is not inside a
//! `Content-Length` frame fails the test.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Deserialize;
use serde_json::{Value, json};

pub struct Driver {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Driver {
    pub fn start() -> Driver {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc-driver-python"))
            .env("VNOC_TEST_HOOKS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (stdin, out) = (child.stdin.take().unwrap(), BufReader::new(child.stdout.take().unwrap()));
        let mut d = Driver { child, stdin, out, next: 0 };
        d.request("initialize", json!({ "protocol": 1 })).expect("the handshake works");
        d.notify("initialized", json!({}));
        d
    }

    fn send(&mut self, v: Value) {
        let body = serde_json::to_vec(&v).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Sends a request without waiting for its answer; returns its id. Read the answers with `read_response`, in the order they come.
    pub fn send_request(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    /// The next response, whichever request it answers: `(id, Ok(result) | Err(error object))`.
    pub fn read_response(&mut self) -> (u64, Result<Value, Value>) {
        let msg = self.read_frame();
        let id = msg["id"].as_u64().expect("a response has an id");
        (id, match msg.get("error") {
            Some(e) => Err(e.clone()),
            None => Ok(msg["result"].clone()),
        })
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        let id = self.send_request(method, params);
        let (answered, result) = self.read_response();
        assert_eq!(answered, id);
        result
    }

    fn read_frame(&mut self) -> Value {
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let len: usize = line.strip_prefix("Content-Length:").unwrap_or_else(|| panic!("STDOUT GUARD: stray bytes on stdout: {line:?}")).trim().parse().unwrap();
        let mut blank = String::new();
        self.out.read_line(&mut blank).unwrap();
        assert_eq!(blank, "\r\n", "STDOUT GUARD: no blank line after the header");
        let mut buf = vec![0u8; len];
        self.out.read_exact(&mut buf).unwrap();
        // a call tree can be deeper than serde_json's default limit of 128 levels (the analyzer goes to 256)
        let mut de = serde_json::Deserializer::from_slice(&buf);
        de.disable_recursion_limit();
        Value::deserialize(&mut de).unwrap()
    }

    /// The tree of an entry of `root`: the root node of the answer.
    pub fn tree(&mut self, root: &Path, file: &str, qualname: &str) -> Result<Value, Value> {
        self.request("vnoc/callTree", json!({ "root": root.to_str().unwrap(), "file": file, "entry": { "qualname": qualname, "occurrence": 1 } })).map(|r| r["root"].clone())
    }

    /// `{ solves, opens, reopens, projects }` as the worker counts them.
    pub fn stats(&mut self) -> Value {
        self.request("vnoc/test/stats", json!({})).unwrap()
    }

    pub fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success(), "the driver exits cleanly after shutdown + exit");
    }
}

/// A folder in the temp directory that goes away with the value.
pub struct Project {
    pub root: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl Project {
    pub fn new(files: &[(&str, &str)]) -> Project {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("vnoc-driver-test-{}-{}-{nanos}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&root).unwrap();
        // the engine canonicalizes the root; so that file names and the root agree on macOS (/var → /private/var), do it here too
        let root = std::fs::canonicalize(root).unwrap();
        let p = Project { root };
        for (name, text) in files {
            p.write(name, text);
        }
        p
    }

    pub fn write(&self, name: &str, text: &str) {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn remove(&self, name: &str) {
        std::fs::remove_file(self.root.join(name)).unwrap();
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The names of what a node calls, in order.
pub fn callees(node: &Value) -> Vec<String> {
    node["children"].as_array().unwrap().iter().map(|c| c["target"]["qualname"].as_str().unwrap().to_string()).collect()
}

/// Every mark in a tree, in the order met.
pub fn marks(node: &Value) -> Vec<String> {
    let mut out: Vec<String> = node["marks"].as_array().map(|m| m.iter().map(|x| x.as_str().unwrap().to_string()).collect()).unwrap_or_default();
    for child in node["children"].as_array().into_iter().flatten() {
        out.extend(marks(child));
    }
    out
}
