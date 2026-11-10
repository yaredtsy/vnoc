//! The core as a process, spoken to over LSP framing, with the real Python driver behind it.
//! Needs the driver built first: `cd drivers/python && cargo build`.
//!
//! VS-D67 condition 3: unknown notifications and unknown requests plus a full lifecycle, and the core is still alive.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn driver_path() -> PathBuf {
    std::env::var_os("VNOC_DRIVER").map(PathBuf::from).unwrap_or_else(|| repo_root().join("drivers/python/target/debug/vnoc-driver-python"))
}

fn fixtures() -> PathBuf {
    repo_root().join("crates/vnoc-domain/tests/fixtures")
}

struct Core {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Core {
    /// Starts the core. `driver`: the path to put in `initializationOptions.driverPath` (None = leave it out).
    fn start(driver: Option<PathBuf>) -> Core {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vnoc"));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).env_remove("VNOC_DRIVER");
        let mut child = cmd.spawn().unwrap();
        let (stdin, out) = (child.stdin.take().unwrap(), BufReader::new(child.stdout.take().unwrap()));
        let mut core = Core { child, stdin, out, next: 0 };
        let options = driver.map_or(json!({}), |d| json!({ "driverPath": d }));
        let init = core.request("initialize", json!({ "processId": null, "rootUri": null, "capabilities": {}, "initializationOptions": options })).expect("initialize answers");
        assert_eq!(init["serverInfo"]["name"], "vnoc");
        core.notify("initialized", json!({}));
        core
    }

    fn send(&mut self, v: Value) {
        let body = serde_json::to_vec(&v).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let msg = self.read_frame();
        assert_eq!(msg["id"], json!(id), "{msg}");
        match msg.get("error") {
            Some(e) => Err(e.clone()),
            None => Ok(msg["result"].clone()),
        }
    }

    /// Panics if stdout holds anything but frames: the core's stdout IS the protocol.
    fn read_frame(&mut self) -> Value {
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let len: usize = line.strip_prefix("Content-Length:").unwrap_or_else(|| panic!("STDOUT GUARD: stray bytes on stdout: {line:?}")).trim().parse().unwrap();
        self.out.read_line(&mut String::new()).unwrap();
        let mut buf = vec![0u8; len];
        self.out.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success(), "the core exits cleanly after shutdown + exit");
    }
}

#[test]
fn unknown_messages_do_not_kill_the_core_and_the_lifecycle_ends_cleanly() {
    let mut core = Core::start(Some(driver_path()));
    // unknown notifications, one under `$/` and one not (async-lsp's default would stop the loop on the second)
    core.notify("$/somethingNew", json!({}));
    core.notify("workspace/didChangeConfiguration", json!({ "settings": {} }));
    core.notify("vnoc/neverHeardOfIt", json!({ "x": 1 }));
    // an unknown request is an error, not silence and not a crash
    assert_eq!(core.request("vnoc/nope", json!({})).unwrap_err()["code"], -32601);
    // and the core still answers
    let pong = core.request("vnoc/ping", Value::Null).unwrap();
    assert_eq!(pong["pong"], true);
    assert_eq!(pong["pid"], json!(core.child.id()));
    // a request that takes nothing must not fail on HOW the client says so: null (above), absent, or [] (what vscode-languageclient sends)
    assert_eq!(core.request("vnoc/ping", json!([])).unwrap()["pong"], true);
    assert_eq!(core.request("vnoc/ping", json!({})).unwrap()["pong"], true);
    core.finish();
}

#[test]
fn the_structure_of_a_folder_comes_from_the_driver() {
    let mut core = Core::start(Some(driver_path()));
    let r = core.request("vnoc/structure", json!({ "root": fixtures() })).expect("structure answers (is the driver built? `cd drivers/python && cargo build`)");

    let file = &r["root"]["children"][0];
    assert_eq!(r["root"]["kind"], "folder");
    assert_eq!((file["kind"].as_str(), file["name"].as_str()), (Some("file"), Some("nested.py")));
    // 15 definitions in the fixture; the tree nests them: top-level defs and classes under the file
    let top_level: Vec<&str> = file["children"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(top_level, vec!["top", "Outer", "top", "cond", "cond", "decorated", "coro"]);
    assert_eq!(r["stats"]["files"], 1);
    assert_eq!(r["stats"]["classes"], 3); // Local, Outer, Outer.Nested
    assert_eq!(r["stats"]["functions"], 12);

    // keys (the real LocationKey, from vnoc-domain) are unique even for redefinitions
    let ids: Vec<&str> = file["children"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["nested.py::top", "nested.py::Outer", "nested.py::top#2", "nested.py::cond", "nested.py::cond#2", "nested.py::decorated", "nested.py::coro"]);
    // nesting through <locals>: top > inner > deepest
    let inner = &file["children"][0]["children"][0];
    assert_eq!(inner["qualname"], "top.<locals>.inner");
    assert_eq!(inner["children"][0]["qualname"], "top.<locals>.inner.<locals>.deepest");
    assert_eq!(r["truncated"], false);
    assert_eq!(r["failed"], json!([]));

    // asking again reuses the driver and gives the same answer
    let again = core.request("vnoc/structure", json!({ "root": fixtures() })).unwrap();
    assert_eq!(again["root"], r["root"]);
    core.finish();
}

#[test]
fn a_missing_driver_is_a_clear_error_and_the_core_stays_up() {
    let mut core = Core::start(None);
    let e = core.request("vnoc/structure", json!({ "root": fixtures() })).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("no driver is configured"), "{e}");
    core.finish();

    let mut core = Core::start(Some(PathBuf::from("/definitely/not/a/driver")));
    let e = core.request("vnoc/structure", json!({ "root": fixtures() })).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("is not at /definitely/not/a/driver"), "{e}");
    assert_eq!(core.request("vnoc/ping", Value::Null).unwrap()["pong"], true, "still alive");
    core.finish();
}

#[test]
fn a_folder_that_does_not_exist_is_an_error_not_a_crash() {
    let mut core = Core::start(Some(driver_path()));
    let e = core.request("vnoc/structure", json!({ "root": "/definitely/not/a/folder" })).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("not a folder"), "{e}");
    assert_eq!(core.request("vnoc/ping", Value::Null).unwrap()["pong"], true);
    core.finish();
}
