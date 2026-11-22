//! The harness of the driver's integration tests: the real binary on stdio. Any byte on stdout that is not inside a
//! `Content-Length` frame fails the test.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

pub struct Driver {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
    /// the `initialize` result
    pub init: Value,
}

impl Driver {
    /// Starts the driver, sends `initialize` and `initialized`.
    pub fn start() -> Driver {
        let mut d = Driver::spawn();
        d.init = d
            .request("initialize", json!({ "protocol": 1 }))
            .expect("the handshake works");
        d.notify("initialized", json!({}));
        d
    }

    /// Starts the driver and says nothing to it.
    pub fn spawn() -> Driver {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc-driver-bend"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (stdin, out) = (
            child.stdin.take().unwrap(),
            BufReader::new(child.stdout.take().unwrap()),
        );
        Driver {
            child,
            stdin,
            out,
            next: 0,
            init: Value::Null,
        }
    }

    /// True when the driver has closed its stdout without another frame (it stopped).
    pub fn stdout_is_closed(&mut self) -> bool {
        self.out.fill_buf().unwrap().is_empty()
    }

    /// Waits for the driver to end: did it exit with success?
    pub fn exited_cleanly(mut self) -> bool {
        drop(self.stdin);
        self.child.wait().unwrap().success()
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
        (
            id,
            match msg.get("error") {
                Some(e) => Err(e.clone()),
                None => Ok(msg["result"].clone()),
            },
        )
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
        let len: usize = line
            .strip_prefix("Content-Length:")
            .unwrap_or_else(|| panic!("STDOUT GUARD: stray bytes on stdout: {line:?}"))
            .trim()
            .parse()
            .unwrap();
        let mut blank = String::new();
        self.out.read_line(&mut blank).unwrap();
        assert_eq!(
            blank, "\r\n",
            "STDOUT GUARD: no blank line after the header"
        );
        let mut buf = vec![0u8; len];
        self.out.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    pub fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(
            self.child.wait().unwrap().success(),
            "the driver exits cleanly after shutdown + exit"
        );
    }
}
