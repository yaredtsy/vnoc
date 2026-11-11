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

/// A copy of the fixtures in a fresh temp folder (a folder inside this repository would be inside its git repo, and ids would carry that path).
fn fixtures(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-lifecycle-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(repo_root().join("crates/vnoc-domain/tests/fixtures")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
    }
    dir
}

struct Core {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
    /// notifications that arrived while waiting for a response: (method, params)
    notifications: Vec<(String, Value)>,
    /// How this stand-in editor answers `workspace/applyEdit`: it applies the edits to the file on disk (and says so), or refuses.
    editor_applies_edits: bool,
    /// the `workspace/applyEdit` requests the core made
    edit_requests: Vec<Value>,
}

impl Core {
    /// Starts the core on `root`. `driver`: the path to put in `initializationOptions.driverPath` (None = leave it out).
    fn start(driver: Option<PathBuf>, root: &std::path::Path) -> Core {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vnoc"));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).env_remove("VNOC_DRIVER");
        let mut child = cmd.spawn().unwrap();
        let (stdin, out) = (child.stdin.take().unwrap(), BufReader::new(child.stdout.take().unwrap()));
        let mut core = Core { child, stdin, out, next: 0, notifications: vec![], editor_applies_edits: true, edit_requests: vec![] };
        let options = driver.map_or(json!({ "root": root }), |d| json!({ "driverPath": d, "root": root }));
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
        loop {
            let msg = self.read_frame();
            if let Some(method) = msg["method"].as_str() {
                if msg.get("id").is_some() {
                    self.answer_core_request(&msg);
                } else {
                    self.notifications.push((method.to_string(), msg["params"].clone()));
                }
                continue;
            }
            assert_eq!(msg["id"], json!(id), "{msg}");
            return match msg.get("error") {
                Some(e) => Err(e.clone()),
                None => Ok(msg["result"].clone()),
            };
        }
    }

    /// The core asked the editor for something. Only `workspace/applyEdit` is expected.
    fn answer_core_request(&mut self, request: &Value) {
        assert_eq!(request["method"], "workspace/applyEdit", "{request}");
        self.edit_requests.push(request["params"].clone());
        let applied = self.editor_applies_edits;
        if applied {
            apply_document_changes(&request["params"]["edit"]["documentChanges"]);
        }
        self.send(json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "applied": applied } }));
    }

    /// The params of the next `method` notification (reading frames until it comes).
    fn wait_for(&mut self, method: &str) -> Value {
        if let Some(at) = self.notifications.iter().position(|(m, _)| m == method) {
            return self.notifications.remove(at).1;
        }
        loop {
            let msg = self.read_frame();
            match msg["method"].as_str() {
                Some(m) if m == method => return msg["params"].clone(),
                Some(_) if msg.get("id").is_some() => self.answer_core_request(&msg),
                Some(m) => self.notifications.push((m.to_string(), msg["params"].clone())),
                None => panic!("a response nobody asked for: {msg}"),
            }
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

/// Applies a `documentChanges` list of text edits to the files on disk (ASCII text: a character is a byte).
fn apply_document_changes(changes: &Value) {
    for change in changes.as_array().unwrap() {
        let path = PathBuf::from(change["textDocument"]["uri"].as_str().unwrap().strip_prefix("file://").unwrap());
        let mut text = std::fs::read_to_string(&path).unwrap();
        let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let offset = |p: &Value| starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize;
        for edit in change["edits"].as_array().unwrap().iter().rev() {
            text.insert_str(offset(&edit["range"]["start"]), edit["newText"].as_str().unwrap());
        }
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn unknown_messages_do_not_kill_the_core_and_the_lifecycle_ends_cleanly() {
    let mut core = Core::start(Some(driver_path()), &fixtures("lifecycle"));
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
    let root = fixtures("structure");
    let mut core = Core::start(Some(driver_path()), &root);
    let r = core.request("vnoc/structure", Value::Null).expect("structure answers (is the driver built? `cd drivers/python && cargo build`)");

    let file = &r["root"]["children"][0];
    assert_eq!(r["root"]["kind"], "folder");
    assert_eq!(r["git_root"], json!(root), "no .git above the folder: the folder is the root");
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

    // the first build announces itself; asking again gives the same answer from the same revision
    assert_eq!(core.wait_for("vnoc/indexChanged")["revision"], r["revision"]);
    let again = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!((&again["root"], &again["revision"]), (&r["root"], &r["revision"]));
    core.finish();
}

#[test]
fn opening_a_subfolder_of_a_repo_keeps_ids_relative_to_the_git_root() {
    let repo = fixtures("subfolder-repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let sub = repo.join("services/api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::rename(repo.join("nested.py"), sub.join("nested.py")).unwrap();

    let mut core = Core::start(Some(driver_path()), &sub);
    let r = core.request("vnoc/structure", Value::Null).unwrap();
    let file = &r["root"]["children"][0];
    assert_eq!(r["git_root"], json!(repo));
    assert_eq!(file["path"], "nested.py", "the editor opens it below the opened folder");
    assert_eq!(file["id"], "services/api/nested.py", "the id starts at the git root");
    assert_eq!(file["children"][0]["id"], "services/api/nested.py::top");
    core.finish();
}

#[test]
fn a_changed_file_is_read_again_and_a_new_file_appears() {
    let root = fixtures("changes");
    let mut core = Core::start(Some(driver_path()), &root);
    let first = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(core.wait_for("vnoc/indexChanged")["revision"], first["revision"]);

    // an edit: the editor reports the path, the core reads that file again and says so
    let edited = root.join("nested.py");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\ndef added_later():\n    pass\n");
    std::fs::write(&edited, text).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [edited] }));
    let changed = core.wait_for("vnoc/indexChanged");
    assert_eq!(changed["revision"].as_u64().unwrap(), first["revision"].as_u64().unwrap() + 1);
    let after = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(after["stats"]["functions"], 13);

    // a new file the editor did not even list (it is found by the walk), and a deleted one
    std::fs::write(root.join("fresh.py"), "def hello():\n    pass\n").unwrap();
    std::fs::remove_file(&edited).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [] }));
    core.wait_for("vnoc/indexChanged");
    let after = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(after["root"]["children"][0]["name"], "fresh.py");
    assert_eq!((after["stats"]["files"].as_u64(), after["stats"]["functions"].as_u64()), (Some(1), Some(1)));

    // a rebuild of unchanged code is quiet; two at once are not run twice
    core.notify("vnoc/rebuild", Value::Null);
    core.notify("vnoc/rebuild", Value::Null);
    let still = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(still["revision"], after["revision"]);
    core.finish();
}

#[test]
fn a_missing_driver_is_a_clear_error_and_the_core_stays_up() {
    let mut core = Core::start(None, &fixtures("nodriver"));
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("no driver is configured"), "{e}");
    core.finish();

    let mut core = Core::start(Some(PathBuf::from("/definitely/not/a/driver")), &fixtures("baddriver"));
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("is not at /definitely/not/a/driver"), "{e}");
    assert_eq!(core.request("vnoc/ping", Value::Null).unwrap()["pong"], true, "still alive");
    core.finish();
}

#[test]
fn a_folder_that_does_not_exist_is_an_error_not_a_crash() {
    let mut core = Core::start(Some(driver_path()), std::path::Path::new("/definitely/not/a/folder"));
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("not a folder"), "{e}");
    assert_eq!(core.request("vnoc/ping", Value::Null).unwrap()["pong"], true);
    core.finish();
}

const ORDERS: &str = "class OrderService:\n    def parse_order(self, raw):\n        return raw\n\n    def other(self):\n        pass\n";

/// A project with one file, `orders.py`, opened in a core that is ready.
fn orders_project(label: &str, vnoc_toml: Option<&str>) -> (Core, PathBuf) {
    let root = std::env::temp_dir().join(format!("vnoc-attach-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".vnoc")).unwrap();
    std::fs::write(root.join("orders.py"), ORDERS).unwrap();
    match vnoc_toml {
        Some(text) => std::fs::write(root.join(".vnoc/vnoc.toml"), text).unwrap(),
        None => std::fs::remove_dir_all(root.join(".vnoc")).unwrap(),
    }
    let mut core = Core::start(Some(driver_path()), &root);
    core.request("vnoc/structure", Value::Null).unwrap();
    (core, root)
}

fn attach_params(root: &std::path::Path, key: &str, title: &str, markers: Option<bool>) -> Value {
    let file = root.join("orders.py");
    json!({ "key": key, "title": title, "markers": markers, "document": { "uri": format!("file://{}", file.display()), "version": 3, "text": std::fs::read_to_string(&file).unwrap() } })
}

#[test]
fn attaching_a_doc_writes_the_id_the_node_and_the_doc() {
    let (mut core, root) = orders_project("full", None);
    assert_eq!(core.request("vnoc/projectInfo", Value::Null).unwrap(), json!({ "initialized": false, "markers": true }));

    let key = "orders.py::OrderService.parse_order";
    let r = core.request("vnoc/attach", attach_params(&root, key, "Why we parse twice", Some(true))).expect("attach answers");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!((r["id_in_code"].as_bool(), r["notice"].is_null()), (Some(true), true));

    // the editor was asked once, for a versioned edit of that document
    assert_eq!(core.edit_requests.len(), 1);
    assert_eq!(core.edit_requests[0]["edit"]["documentChanges"][0]["textDocument"]["version"], 3);
    // the ID line is in the code, in the docstring that was added
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    assert!(code.contains(&format!("def parse_order(self, raw):\n        \"\"\"ID: {id}\"\"\"\n")), "{code}");

    // .vnoc/ was set up and the node and doc are where the plan says
    let (shard, node) = (&id[24..], root.join(".vnoc/nodes").join(&id[24..]).join(&id));
    assert!(root.join(".vnoc/vnoc.toml").is_file() && root.join(".vnoc/.gitignore").is_file(), "{shard}");
    let toml = std::fs::read_to_string(node.join("node.toml")).unwrap();
    assert!(toml.contains("path = \"orders.py\"") && toml.contains("qualname = \"OrderService.parse_order\"") && toml.contains("id_in_code = \"yes\"") && toml.contains("kind = \"function\""), "{toml}");
    assert_eq!(r["doc"], json!(node.join("why-we-parse-twice.md")));
    assert!(std::fs::read_to_string(node.join("why-we-parse-twice.md")).unwrap().starts_with(&format!("---\nnode: {id}\nhint: {key}\n---\n# Why we parse twice")));
    assert_eq!(core.request("vnoc/projectInfo", Value::Null).unwrap(), json!({ "initialized": true, "markers": true }));

    // a second doc on the same function: same ID, no new edit, no new question
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py")] }));
    core.wait_for("vnoc/indexChanged");
    let second = core.request("vnoc/attach", attach_params(&root, key, "Edge cases", None)).unwrap();
    assert_eq!(second["id"], json!(id));
    assert_eq!(core.edit_requests.len(), 1);
    assert!(second["doc"].as_str().unwrap().ends_with("edge-cases.md"));
    core.finish();
}

#[test]
fn a_marker_that_cannot_be_written_does_not_stop_the_attach() {
    let (mut core, root) = orders_project("refused", None);
    core.editor_applies_edits = false; // the document changed meanwhile, or the user undid it
    let r = core.request("vnoc/attach", attach_params(&root, "orders.py::OrderService.other", "Notes", Some(true))).unwrap();
    assert_eq!(r["id_in_code"], false);
    assert!(r["notice"].as_str().unwrap().contains("attached by name"), "{r}");
    assert_eq!(std::fs::read_to_string(root.join("orders.py")).unwrap(), ORDERS, "the code is untouched");
    let node = root.join(".vnoc/nodes").join(&r["id"].as_str().unwrap()[24..]).join(r["id"].as_str().unwrap());
    assert!(std::fs::read_to_string(node.join("node.toml")).unwrap().contains("id_in_code = \"no\""));
    assert!(node.join("notes.md").is_file());
    core.finish();
}

#[test]
fn a_project_set_to_no_ids_never_touches_the_code() {
    let (mut core, root) = orders_project("none", Some("format = 1\n[ids]\ndefault = \"none\"\n"));
    let r = core.request("vnoc/attach", attach_params(&root, "orders.py::OrderService.other", "Notes", None)).unwrap();
    assert_eq!((r["id_in_code"].as_bool(), r["notice"].is_null()), (Some(false), true), "off is a choice, not a failure");
    assert!(core.edit_requests.is_empty());
    assert_eq!(std::fs::read_to_string(root.join("orders.py")).unwrap(), ORDERS);
    core.finish();
}

#[test]
fn attach_refuses_what_is_not_in_the_index_or_not_that_document() {
    let (mut core, root) = orders_project("refuse", None);
    let e = core.request("vnoc/attach", attach_params(&root, "orders.py::Nope", "x", None)).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("not in the index"), "{e}");
    let mut wrong = attach_params(&root, "orders.py::OrderService.other", "x", None);
    wrong["document"]["uri"] = json!("file:///somewhere/else.py");
    assert!(core.request("vnoc/attach", wrong).unwrap_err()["message"].as_str().unwrap().contains("not the file"));
    assert!(!root.join(".vnoc").exists(), "a refused attach writes nothing");
    core.finish();
}

/// The first node of a tree of `orders.py`: the file's children, by name.
fn def_named<'a>(structure: &'a Value, name: &str) -> &'a Value {
    structure["root"]["children"][0]["children"].as_array().and_then(|nodes| nodes.iter().flat_map(|n| std::iter::once(n).chain(n["children"].as_array().into_iter().flatten())).find(|n| n["name"] == name)).unwrap_or_else(|| panic!("no node `{name}` in {structure}"))
}

#[test]
fn a_documented_node_is_found_again_after_a_rename_and_a_move() {
    let (mut core, root) = orders_project("refind", None);
    let key = "orders.py::OrderService.parse_order";
    core.request("vnoc/attach", attach_params(&root, key, "Why we parse twice", Some(true))).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py")] }));

    // the tree shows the id, the count and a marker link
    let shown = loop {
        core.wait_for("vnoc/indexChanged");
        let s = core.request("vnoc/structure", Value::Null).unwrap();
        if def_named(&s, "parse_order")["link"] == "marker" {
            break s;
        }
    };
    let node = def_named(&shown, "parse_order");
    assert_eq!(node["docs"], 1);
    let id = node["node_id"].as_str().unwrap().to_string();
    assert_eq!(def_named(&shown, "other")["docs"], 0);

    // the doc list comes from the files
    let docs = core.request("vnoc/docs", json!({ "id": id })).unwrap();
    assert_eq!((docs[0]["file"].as_str(), docs[0]["title"].as_str()), (Some("why-we-parse-twice.md"), Some("Why we parse twice")));

    // rename the function AND move it to another file; the ID line travels with it
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    let moved = code.replace("parse_order", "parse_payload");
    std::fs::write(root.join("orders.py"), ORDERS.replace("    def parse_order(self, raw):\n        return raw\n\n", "")).unwrap();
    let body_start = moved.find("    def parse_payload").unwrap();
    let body_end = moved.find("    def other").unwrap();
    std::fs::write(root.join("payments.py"), format!("class Payments:\n{}", &moved[body_start..body_end])).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py"), root.join("payments.py")] }));
    let after = loop {
        core.wait_for("vnoc/indexChanged");
        let s = core.request("vnoc/structure", Value::Null).unwrap();
        if s["stats"]["files"] == 2 {
            break s;
        }
    };
    let found = after["root"]["children"].as_array().unwrap().iter().flat_map(|f| f["children"].as_array().unwrap().iter().flat_map(|c| std::iter::once(c).chain(c["children"].as_array().unwrap().iter()))).find(|n| n["node_id"] == json!(id)).expect("the node is still found");
    assert_eq!((found["name"].as_str(), found["path"].as_str(), found["link"].as_str(), found["docs"].as_u64()), (Some("parse_payload"), Some("payments.py"), Some("marker"), Some(1)));
    core.finish();
}

/// The first node in the tree (any depth) that `pred` accepts.
fn find_node<'a>(node: &'a Value, pred: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
    if pred(node) {
        return Some(node);
    }
    node["children"].as_array()?.iter().find_map(|child| find_node(child, pred))
}

/// Waits for index changes until the tree satisfies `ready`, and returns that tree.
fn structure_when(core: &mut Core, ready: impl Fn(&Value) -> bool) -> Value {
    loop {
        let s = core.request("vnoc/structure", Value::Null).unwrap();
        if ready(&s) {
            return s;
        }
        core.wait_for("vnoc/indexChanged");
    }
}

#[test]
fn a_file_gets_a_header_comment_and_is_found_again_after_a_rename() {
    let (mut core, root) = orders_project("file", None);
    let r = core.request("vnoc/attach", attach_params(&root, "orders.py", "About orders", Some(true))).expect("attach answers");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!((r["id_in_code"].as_bool(), r["notice"].is_null()), (Some(true), true));
    assert_eq!(core.edit_requests.len(), 1, "one edit of the open document");
    assert_eq!(std::fs::read_to_string(root.join("orders.py")).unwrap(), format!("# FileID: {id}\n{ORDERS}"));

    let node = root.join(".vnoc/nodes").join(&id[24..]).join(&id);
    let toml = std::fs::read_to_string(node.join("node.toml")).unwrap();
    assert!(toml.contains("path = \"orders.py\"") && toml.contains("qualname = \"\"") && toml.contains("id_in_code = \"yes\"") && toml.contains("kind = \"file\""), "{toml}");
    assert!(std::fs::read_to_string(node.join("about-orders.md")).unwrap().contains("hint: orders.py\n"));

    // saved: the tree shows the file with its doc, found by its marker
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py")] }));
    let shown = structure_when(&mut core, |s| s["root"]["children"][0]["link"] == "marker");
    assert_eq!((shown["root"]["children"][0]["node_id"].as_str(), shown["root"]["children"][0]["docs"].as_u64()), (Some(id.as_str()), Some(1)));

    // renamed in the terminal: the comment travels with the file
    std::fs::rename(root.join("orders.py"), root.join("billing.py")).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py"), root.join("billing.py")] }));
    let after = structure_when(&mut core, |s| s["root"]["children"][0]["name"] == "billing.py");
    let file = &after["root"]["children"][0];
    assert_eq!((file["node_id"].as_str(), file["link"].as_str(), file["docs"].as_u64()), (Some(id.as_str()), Some("marker"), Some(1)));
    core.finish();
}

#[test]
fn a_folder_gets_a_marker_file_and_is_found_again_after_a_move() {
    let (mut core, root) = orders_project("folder", None);
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(root.join("pkg/m.py"), "def f():\n    pass\n").unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("pkg/m.py")] }));
    structure_when(&mut core, |s| s["stats"]["files"] == 2);

    let r = core.request("vnoc/attach", json!({ "key": "pkg", "title": "The pkg folder", "markers": true })).expect("a folder needs no document");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!((r["id_in_code"].as_bool(), r["notice"].is_null()), (Some(true), true));
    assert!(core.edit_requests.is_empty(), "the core writes the marker itself");
    let marker = std::fs::read_to_string(root.join("pkg/.vnoc-id")).unwrap();
    assert!(marker.contains(&format!("id = \"{id}\"")), "{marker}");
    let node = root.join(".vnoc/nodes").join(&id[24..]).join(&id);
    assert!(std::fs::read_to_string(node.join("node.toml")).unwrap().contains("path = \"pkg\"") && std::fs::read_to_string(node.join("node.toml")).unwrap().contains("kind = \"folder\""));

    let shown = structure_when(&mut core, |s| find_node(&s["root"], &|n| n["id"] == "pkg").is_some_and(|n| n["link"] == "marker"));
    assert_eq!(find_node(&shown["root"], &|n| n["id"] == "pkg").unwrap()["docs"], 1);

    // moved and renamed in the terminal; the marker file went along inside it
    std::fs::create_dir_all(root.join("core")).unwrap();
    std::fs::rename(root.join("pkg"), root.join("core/pkg2")).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("core/pkg2/.vnoc-id")] }));
    let after = structure_when(&mut core, |s| find_node(&s["root"], &|n| n["id"] == "core/pkg2").is_some());
    let folder = find_node(&after["root"], &|n| n["id"] == "core/pkg2").unwrap();
    assert_eq!((folder["node_id"].as_str(), folder["link"].as_str(), folder["docs"].as_u64()), (Some(id.as_str()), Some("marker"), Some(1)));

    // a second doc on the same folder reuses its ID and does not touch the marker
    let second = core.request("vnoc/attach", json!({ "key": "core/pkg2", "title": "More" })).unwrap();
    assert_eq!(second["id"], json!(id));
    core.finish();
}

#[test]
fn a_folder_marker_that_cannot_be_read_is_never_overwritten() {
    let (mut core, root) = orders_project("badmarker", None);
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(root.join("pkg/m.py"), "x = 1\n").unwrap();
    std::fs::write(root.join("pkg/.vnoc-id"), "id = [").unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("pkg/m.py")] }));
    structure_when(&mut core, |s| s["stats"]["files"] == 2);

    let r = core.request("vnoc/attach", json!({ "key": "pkg", "title": "Notes", "markers": true })).unwrap();
    assert_eq!(r["id_in_code"], false);
    assert!(r["notice"].as_str().unwrap().contains("cannot read"), "{r}");
    assert_eq!(std::fs::read_to_string(root.join("pkg/.vnoc-id")).unwrap(), "id = [", "the unreadable file is left as it was");
    core.finish();
}

#[test]
fn with_ids_off_a_file_is_attached_by_path_and_goes_missing_when_renamed() {
    let (mut core, root) = orders_project("filenone", Some("format = 1\n[ids]\ndefault = \"none\"\n"));
    let r = core.request("vnoc/attach", attach_params(&root, "orders.py", "Notes", None)).unwrap();
    assert_eq!((r["id_in_code"].as_bool(), r["notice"].is_null()), (Some(false), true));
    assert!(core.edit_requests.is_empty());
    assert_eq!(std::fs::read_to_string(root.join("orders.py")).unwrap(), ORDERS);

    core.notify("vnoc/filesChanged", json!({ "paths": [] }));
    let shown = structure_when(&mut core, |s| s["root"]["children"][0]["link"] == "name");
    assert_eq!(shown["root"]["children"][0]["docs"], 1, "found by its path");

    std::fs::rename(root.join("orders.py"), root.join("billing.py")).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("orders.py"), root.join("billing.py")] }));
    let after = structure_when(&mut core, |s| s["root"]["children"][0]["name"] == "billing.py");
    assert_eq!(after["root"]["children"][0]["node_id"], Value::Null, "no marker, so nothing follows the rename: it is Missing");
    core.finish();
}

#[test]
fn attach_to_a_file_needs_its_document() {
    let (mut core, _root) = orders_project("nodocument", None);
    let e = core.request("vnoc/attach", json!({ "key": "orders.py", "title": "x" })).unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("needs the open document"), "{e}");
    core.finish();
}

/// Writes a record by hand: a node folder with a `node.toml` that anchors the node at `path::qualname` (no docs needed to be listed).
fn write_record(root: &std::path::Path, id: &str, path: &str, qualname: &str) {
    let dir = root.join(".vnoc/nodes").join(&id[24..]).join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("node.toml"), format!("path = \"{path}\"\nqualname = \"{qualname}\"\nid_in_code = \"no\"\nkind = \"function\"\n")).unwrap();
    std::fs::write(dir.join("notes.md"), "# Notes\n").unwrap();
}

#[test]
fn structure_lists_missing_nodes_and_problems_and_leaves_unscanned_code_alone() {
    let root = std::env::temp_dir().join(format!("vnoc-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ignored")).unwrap();
    std::fs::write(root.join("orders.py"), ORDERS).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::write(root.join("ignored/x.py"), "def f():\n    pass\n").unwrap();
    // not valid UTF-8: the driver cannot outline it
    std::fs::write(root.join("bad.py"), [0xff, 0xfe, 0xfd]).unwrap();
    let (gone, ignored, failed) = ("0000000000000000000000000A", "0000000000000000000000000B", "0000000000000000000000000C");
    write_record(&root, gone, "gone.py", "parse_order");
    write_record(&root, ignored, "ignored/x.py", "f");
    write_record(&root, failed, "bad.py", "g");
    std::fs::create_dir_all(root.join(".vnoc/nodes/zz/not-an-id")).unwrap();

    let mut core = Core::start(Some(driver_path()), &root);
    let s = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(s["failed"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect::<Vec<_>>(), ["bad.py"], "the driver failed on it");
    assert_eq!(s["missing"], json!([{ "id": gone, "kind": "function", "path": "gone.py", "qualname": "parse_order", "docs": 1 }]), "the others were not looked for");
    let problems = s["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["text"], ".vnoc/nodes/zz/not-an-id: the folder name is not a node ID");
    assert_eq!(problems[0]["file"], json!(root.join(".vnoc/nodes/zz/not-an-id")));
    assert_eq!(problems[0]["line"], Value::Null);

    // the ignored file goes: now the name was looked for, and the node is Missing
    std::fs::remove_file(root.join("ignored/x.py")).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [root.join("ignored/x.py")] }));
    let after = structure_when(&mut core, |s| s["missing"].as_array().unwrap().len() == 2);
    assert_eq!(after["missing"].as_array().unwrap().iter().map(|m| m["path"].as_str().unwrap()).collect::<Vec<_>>(), ["gone.py", "ignored/x.py"]);
    core.finish();
}
