//! `vnoc/callTree` through the real binary, on `tests/fixtures/call_tree` (three modules in a package). The harness is the one of
//! `tests/outline.rs`, cut down: any byte on stdout that is not inside a `Content-Length` frame fails the test.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Driver {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Driver {
    /// Starts the driver, sends `initialize` and `initialized`. Returns the `initialize` result.
    fn start() -> (Driver, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc-driver-python")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let (stdin, out) = (child.stdin.take().unwrap(), BufReader::new(child.stdout.take().unwrap()));
        let mut d = Driver { child, stdin, out, next: 0 };
        let init = d.request("initialize", json!({ "protocol": 1 })).expect("the handshake works");
        d.notify("initialized", json!({}));
        (d, init)
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

    /// `Ok(result)` or `Err(error object)`. Panics if stdout holds anything but frames.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let msg = self.read_frame();
        assert_eq!(msg["id"], json!(id));
        match msg.get("error") {
            Some(e) => Err(e.clone()),
            None => Ok(msg["result"].clone()),
        }
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
        serde_json::from_slice(&buf).unwrap()
    }

    fn call_tree(&mut self, file: &str, qualname: &str, occurrence: u32, depth: Option<u32>) -> Result<Value, Value> {
        let mut params = json!({ "root": fixture(), "file": file, "entry": { "qualname": qualname, "occurrence": occurrence } });
        if let Some(depth) = depth {
            params["depth"] = json!(depth);
        }
        self.request("vnoc/callTree", params).map(|result| result["root"].clone())
    }

    fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success(), "the driver exits cleanly after shutdown + exit");
    }
}

fn fixture() -> String {
    format!("{}/tests/fixtures/call_tree", env!("CARGO_MANIFEST_DIR"))
}

/// The tree of an entry that must exist.
fn tree(file: &str, qualname: &str) -> Value {
    let (mut d, _) = Driver::start();
    let root = d.call_tree(file, qualname, 1, None).unwrap_or_else(|e| panic!("{qualname}: {e}"));
    d.finish();
    root
}

fn children(node: &Value) -> &Vec<Value> {
    node["children"].as_array().unwrap()
}

fn names(nodes: &[Value]) -> Vec<&str> {
    nodes.iter().map(|n| n["target"]["qualname"].as_str().unwrap()).collect()
}

/// `[line, start character, end character]` of a node's call site.
fn site(node: &Value) -> (u64, u64, u64) {
    let s = &node["call_site"];
    assert_eq!(s["start"]["line"], s["end"]["line"], "a call is on one line here");
    (s["start"]["line"].as_u64().unwrap(), s["start"]["character"].as_u64().unwrap(), s["end"]["character"].as_u64().unwrap())
}

#[test]
fn the_handshake_announces_the_call_tree() {
    let (d, init) = Driver::start();
    assert_eq!(init["capabilities"]["callTree"], true);
    d.finish();
}

#[test]
fn a_function_called_twice_is_two_nodes_in_source_order() {
    let a = tree("pkg/calc.py", "a");
    assert_eq!(a["call_site"], Value::Null, "the root is not called from anywhere");
    assert_eq!(a["target"]["file"], "pkg/calc.py");
    let kids = children(&a);
    assert_eq!(names(kids), ["b", "c", "b"], "the driver does not merge: that is the core's");
    assert_eq!(kids.iter().map(site).collect::<Vec<_>>(), [(9, 4, 7), (10, 4, 7), (11, 4, 7)]);
}

#[test]
fn a_call_to_another_file_is_a_range_in_the_callers_file() {
    let run = tree("pkg/app.py", "Service.run");
    let [helper, inner] = &children(&run)[..] else { panic!("helper, then inner: {run}") };
    // `helper` is defined in db.py, and called from app.py line 9 (0-based)
    assert_eq!(helper["target"]["file"], "pkg/db.py");
    assert_eq!(site(helper), (9, 8, 16));
    let app = std::fs::read_to_string(format!("{}/pkg/app.py", fixture())).unwrap();
    assert_eq!(&app.lines().nth(9).unwrap()[8..16], "helper()");

    // `a`, under `inner` (in app.py), is defined in calc.py and called from app.py line 12
    assert_eq!(inner["target"]["file"], "pkg/app.py");
    let a = &children(inner)[0];
    assert_eq!((a["target"]["file"].as_str(), site(a)), (Some("pkg/calc.py"), (12, 19, 22)));
    assert_eq!(&app.lines().nth(12).unwrap()[19..22], "a()");
}

#[test]
fn a_character_before_the_call_counts_in_utf16_units() {
    // line 21 is `    s = "é😀"; b()`: the call starts after 14 characters, but 15 UTF-16 units (the emoji is two)
    let accents = tree("pkg/calc.py", "accents");
    assert_eq!(site(&children(&accents)[0]), (21, 15, 18));
}

#[test]
fn a_method_and_a_nested_def_can_be_the_entry() {
    let run = tree("pkg/app.py", "Service.run");
    assert_eq!((run["target"]["qualname"].as_str(), run["target"]["kind"].as_str(), run["target"]["occurrence"].as_u64()), (Some("Service.run"), Some("function"), Some(1)));

    // the outline's own name for it, `<locals>` and all
    let inner = tree("pkg/app.py", "Service.run.<locals>.inner");
    assert_eq!(inner["target"]["qualname"], "Service.run.<locals>.inner");
    assert_eq!(names(children(&inner)), ["a"]);
}

#[test]
fn direct_recursion_is_shown_and_not_entered() {
    let fact = tree("pkg/calc.py", "fact");
    let [again] = &children(&fact)[..] else { panic!("one call: {fact}") };
    assert_eq!(again["target"]["qualname"], "fact");
    assert_eq!(again["marks"], json!(["recursive"]));
    assert!(children(again).is_empty());
    assert_eq!(fact["marks"], json!([]));
}

#[test]
fn a_target_has_its_id_as_written_and_no_prefix() {
    let run = tree("pkg/app.py", "Service.run");
    let helper = &children(&run)[0];
    assert_eq!(helper["target"]["id"], "0HELPER000000000000000000");
    assert_eq!(run["target"]["id"], Value::Null, "no `ID:` in its docstring");

    let helper = tree("pkg/db.py", "helper");
    assert_eq!(helper["target"]["id"], "0HELPER000000000000000000");
}

#[test]
fn an_unknown_entry_is_invalid_params() {
    let (mut d, _) = Driver::start();
    let err = d.call_tree("pkg/app.py", "Service.nope", 1, None).unwrap_err();
    assert_eq!(err["code"], -32602);
    assert_eq!(err["message"], "no function Service.nope in pkg/app.py");
    // the right name, but there is no second one
    assert_eq!(d.call_tree("pkg/db.py", "helper", 2, None).unwrap_err()["code"], -32602);
    assert_eq!(d.call_tree("pkg/db.py", "helper", 0, None).unwrap_err()["code"], -32602);
    d.finish();
}

#[test]
fn occurrence_zero_is_invalid_params_and_says_why() {
    // not "no function": the def may well exist, the request is what is wrong
    let (mut d, _) = Driver::start();
    let err = d.call_tree("pkg/db.py", "helper", 0, None).unwrap_err();
    assert_eq!(err["code"], -32602);
    assert_eq!(err["message"], "occurrence of helper starts at 1, not 0");
    d.finish();
}

#[test]
fn a_file_that_cannot_be_read_is_an_internal_error() {
    let (mut d, _) = Driver::start();
    let err = d.call_tree("pkg/missing.py", "f", 1, None).unwrap_err();
    assert_eq!(err["code"], -32603);
    assert!(err["message"].as_str().unwrap().starts_with("cannot read pkg/missing.py"), "{err}");
    d.finish();
}

#[test]
fn code_outside_the_project_is_not_in_the_tree() {
    // `Db.save` calls `os.getcwd()`: a library call is not a node
    let save = tree("pkg/db.py", "Db.save");
    assert!(children(&save).is_empty());
}

#[test]
fn a_class_is_shown_with_what_its_init_calls() {
    let service = tree("pkg/app.py", "Service");
    assert_eq!((service["target"]["kind"].as_str(), service["target"]["qualname"].as_str(), service["target"]["file"].as_str()), (Some("class"), Some("Service"), Some("pkg/app.py")));
    assert_eq!(service["call_site"], Value::Null);
    // `Service.__init__` calls `Db()`; there is no node for `__init__` itself
    let [db] = &children(&service)[..] else { panic!("one call: {service}") };
    assert_eq!((db["target"]["qualname"].as_str(), db["target"]["kind"].as_str(), db["target"]["id"].as_str()), (Some("Db"), Some("class"), Some("0DB00000000000000000000000")));
    assert_eq!(site(db), (6, 8, 12));
}

#[test]
fn a_class_without_init_is_a_root_with_no_children() {
    let plain = tree("pkg/app.py", "Plain");
    assert_eq!(plain["target"]["kind"], "class");
    assert!(children(&plain).is_empty());
}

#[test]
fn a_class_with_two_inits_shows_the_second() {
    // the first calls `helper()`, the second `a()`: the second is the one Python runs
    let twice = tree("pkg/app.py", "Twice");
    assert_eq!(names(children(&twice)), ["a"]);
}

#[test]
fn a_class_that_inherits_its_init_has_the_call_site_in_the_base_file() {
    // `Child` has no `__init__`: it runs `Base.__init__`, in another file, which calls `a()`
    let child = tree("pkg/child.py", "Child");
    assert_eq!((child["target"]["kind"].as_str(), child["target"]["file"].as_str()), (Some("class"), Some("pkg/child.py")));
    let [a] = &children(&child)[..] else { panic!("one call: {child}") };
    assert_eq!((a["target"]["qualname"].as_str(), a["target"]["file"].as_str()), (Some("a"), Some("pkg/calc.py")));
    // the range is in base.py, not in child.py (which has only five lines): cut the call out of base.py with it
    let base = std::fs::read_to_string(format!("{}/pkg/base.py", fixture())).unwrap();
    let (line, start, end) = site(a);
    assert_eq!(&base.lines().nth(line as usize).unwrap()[start as usize..end as usize], "a()");
}

#[test]
fn a_node_names_the_file_its_call_is_written_in() {
    // an inherited constructor: `Child` is in child.py, the call `a()` is written in base.py
    let child = tree("pkg/child.py", "Child");
    assert!(child.get("call_site_file").is_none(), "the root is not called from anywhere: {child}");
    assert_eq!(children(&child)[0]["call_site_file"], "pkg/base.py");
    // an ordinary function: its calls are in its own file
    let a = tree("pkg/calc.py", "a");
    assert!(a.get("call_site_file").is_none());
    assert!(children(&a).iter().all(|c| c["call_site_file"] == "pkg/calc.py"), "{a}");
}

#[test]
fn depth_shortens_the_tree_and_marks_the_cut() {
    let (mut d, _) = Driver::start();
    let run = d.call_tree("pkg/app.py", "Service.run", 1, Some(1)).unwrap();
    let inner = &children(&run)[1];
    assert_eq!(inner["marks"], json!(["cut"]));
    assert!(children(inner).is_empty());
    d.finish();
}
