//! The driver as a process, spoken to over its wire. The harness is also the stdout guard: any byte on stdout that is not
//! inside a `Content-Length` frame fails the test (S2/C2).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use vnoc_domain::{LocationKey, RepoPath, first_free_id, occurrences};

#[allow(dead_code)] // this binary uses only part of the golden table
mod golden {
    include!("../../../crates/vnoc-domain/tests/golden_data.rs");
}

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

    fn outline(&mut self, text: &str) -> Value {
        self.request("vnoc/outline", json!({ "text": text })).expect("outline answers")
    }

    fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(self.child.wait().unwrap().success(), "the driver exits cleanly after shutdown + exit");
    }
}

/// Applies a plan's edits to ASCII `text` (positions are line/character; for ASCII a character is a byte).
fn apply_edits(text: &str, edits: &[Value]) -> String {
    let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let offset = |p: &Value| starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize;
    let mut out = text.to_string();
    for e in edits.iter().rev() {
        out.insert_str(offset(&e["range"]["start"]), e["newText"].as_str().unwrap());
    }
    out
}

fn defs(outline: &Value) -> &Vec<Value> {
    outline["defs"].as_array().unwrap()
}

#[test]
fn the_handshake_announces_what_the_driver_can_do() {
    let (d, init) = Driver::start();
    assert_eq!(init["protocol"], 1);
    assert_eq!(init["name"], "python");
    assert_eq!(init["capabilities"]["outline"], true);
    assert_eq!(init["capabilities"]["ids.plan"], true);
    assert_eq!(init["capabilities"]["callTree"], false, "not built yet, and it says so");
    d.finish();
}

#[test]
fn a_protocol_it_does_not_speak_is_refused_with_a_reason() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc-driver-python")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let (mut stdin, mut out) = (child.stdin.take().unwrap(), BufReader::new(child.stdout.take().unwrap()));
    let body = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":2}}"#;
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    stdin.write_all(body).unwrap();
    stdin.flush().unwrap();
    let mut header = String::new();
    out.read_line(&mut header).unwrap();
    let len: usize = header.strip_prefix("Content-Length:").unwrap().trim().parse().unwrap();
    out.read_line(&mut String::new()).unwrap();
    let mut buf = vec![0u8; len];
    out.read_exact(&mut buf).unwrap();
    let msg: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(msg["error"]["code"], -32600);
    assert!(msg["error"]["message"].as_str().unwrap().contains("protocol 1"), "{msg}");
    assert!(!child.wait().unwrap().success(), "and the driver stops");
}

#[test]
fn qualnames_and_ids_of_the_fixture_match_the_independent_oracle() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/vnoc-domain/tests/fixtures/nested.py")).unwrap();
    let (mut d, _) = Driver::start();
    let out = d.outline(&text);
    let found: Vec<&str> = defs(&out).iter().map(|x| x["qualname"].as_str().unwrap()).collect();

    // 1. the parser: Python's own __qualname__ (with <locals>), in source order
    let expected: Vec<&str> = golden::FIXTURE_DEFS.iter().map(|(q, ..)| *q).collect();
    assert_eq!(found, expected, "qualnames, including <locals>, nested classes and redefinitions");

    // 2. the domain: occurrence counting and ID derivation, against the oracle's IDs
    let occ = occurrences(found.iter().copied());
    for ((qualname, o), (expected_q, expected_o, expected_id)) in found.iter().zip(&occ).zip(golden::FIXTURE_DEFS) {
        assert_eq!((*qualname, *o), (*expected_q, *expected_o));
        let loc = LocationKey::new(RepoPath::new(golden::FIXTURE_PATH).unwrap(), *qualname, *o).unwrap();
        let id = first_free_id(&loc, |_| false).unwrap();
        assert_eq!(id.to_string(), *expected_id, "{qualname}#{o}");
    }
    d.finish();
}

#[test]
fn defs_come_with_their_parents_kinds_and_params() {
    let (mut d, _) = Driver::start();
    let out = d.outline("class A:\n    def m(self, x, *rest, **kw):\n        def inner(): pass\n");
    let v = defs(&out);
    assert_eq!(v.len(), 3);
    assert_eq!((v[0]["qualname"].as_str(), v[0]["kind"].as_str(), v[0]["parent"].as_str()), (Some("A"), Some("class"), Some("")));
    assert_eq!(v[1]["qualname"], "A.m");
    assert_eq!((v[1]["parent"].as_str(), v[1]["parent_kind"].as_str()), (Some("A"), Some("class")));
    assert_eq!(v[1]["params"], json!(["self", "x", "*rest", "**kw"]));
    assert_eq!(v[2]["qualname"], "A.m.<locals>.inner");
    assert_eq!((v[2]["parent"].as_str(), v[2]["parent_kind"].as_str()), (Some("A.m"), Some("function")));
    d.finish();
}

#[test]
fn an_id_in_a_docstring_is_read_and_a_def_without_one_has_none() {
    let (mut d, _) = Driver::start();
    let out = d.outline("def a():\n    \"\"\"Summary.\n\n    ID: 3RGJ87CMS6TK775XRJV8BW977Y\n    \"\"\"\n    return 1\n\n\ndef b():\n    return 2\n");
    let v = defs(&out);
    assert_eq!(v[0]["id"], "3RGJ87CMS6TK775XRJV8BW977Y");
    assert!(v[1]["id"].is_null());
    d.finish();
}

#[test]
fn a_syntax_error_flags_the_broken_def_and_not_its_neighbours() {
    let (mut d, _) = Driver::start();
    let out = d.outline("def good(x):\n    return x\n\n\ndef broken(x):\n    if x\n        return 1\n\n\ndef also_good(x):\n    return 2\n");
    let flags: Vec<(String, bool)> = defs(&out).iter().map(|x| (x["qualname"].as_str().unwrap().to_string(), x["syntax_error"].as_bool().unwrap())).collect();
    assert_eq!(flags, vec![("good".into(), false), ("broken".into(), true), ("also_good".into(), false)]);
    assert!(!out["syntax_errors"].as_array().unwrap().is_empty());
    d.finish();
}

#[test]
fn positions_are_zero_based_lines_and_utf16_columns() {
    let (mut d, _) = Driver::start();
    // the last line ends after an emoji, which is two UTF-16 code units
    let out = d.outline("x = 1\ndef f(): return \"\u{1F600}\"\n");
    let r = &defs(&out)[0]["range"];
    assert_eq!(r["start"], json!({ "line": 1, "character": 0 }));
    // `def f(): return "` is 17 chars, the emoji is 2 UTF-16 units, the closing quote is 1
    assert_eq!(r["end"], json!({ "line": 1, "character": 20 }));
    d.finish();
}

#[test]
fn a_file_can_be_read_by_the_driver_from_root_and_file() {
    let (mut d, _) = Driver::start();
    let out = d.request("vnoc/outline", json!({ "root": concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/vnoc-domain/tests/fixtures"), "file": "nested.py" })).unwrap();
    assert_eq!(defs(&out).len(), golden::FIXTURE_DEFS.len());
    d.finish();
}

#[test]
fn unknown_notifications_are_ignored_and_unknown_requests_get_32601() {
    let (mut d, _) = Driver::start();
    d.notify("$/somethingNew", json!({ "x": 1 }));
    let e = d.request("vnoc/doesNotExist", json!({})).unwrap_err();
    assert_eq!(e["code"], -32601);
    assert!(d.outline("def f(): pass\n")["defs"].as_array().unwrap().len() == 1, "still alive and answering");
    d.finish();
}

#[test]
fn bad_params_get_32602() {
    let (mut d, _) = Driver::start();
    assert_eq!(d.request("vnoc/outline", json!({})).unwrap_err()["code"], -32602);
    assert_eq!(d.request("vnoc/outline", json!({ "text": "x", "fingerprints": true })).unwrap_err()["code"], -32602, "fingerprints are not built yet");
    d.finish();
}

#[test]
fn plan_ids_puts_the_id_line_in_the_docstring_and_says_what_it_did() {
    let (mut d, init) = Driver::start();
    assert_eq!(init["capabilities"]["ids.plan"], true);
    let text = "def with_doc():\n    \"\"\"Summary.\n\n    More.\n    \"\"\"\n\ndef no_doc():\n    return 1\n\ndef broken(:\n    pass\n\ndef no_doc():\n    return 2\n";
    let ask = |qualname: &str, occurrence: u32, id: &str| json!({ "qualname": qualname, "occurrence": occurrence, "id": id, "replace_existing": false });
    let plan = d
        .request("vnoc/planIds", json!({ "text": text, "version": 7, "ids": [ask("with_doc", 1, "AAAA"), ask("no_doc", 2, "BBBB"), ask("nowhere", 1, "CCCC"), ask("broken", 1, "DDDD")] }))
        .expect("planIds answers");
    assert_eq!(plan["version"], 7, "the version is echoed");

    let new_text = apply_edits(text, plan["edits"].as_array().unwrap());
    assert!(new_text.contains("    More.\n\n    ID: AAAA\n    \"\"\""), "extends the docstring:\n{new_text}");
    assert!(new_text.contains("def no_doc():\n    \"\"\"ID: BBBB\"\"\"\n    return 2"), "the second no_doc, and only that one:\n{new_text}");
    assert_eq!(new_text.matches("ID: ").count(), 2);

    let written: Vec<(&str, &str)> = plan["written"].as_array().unwrap().iter().map(|w| (w["id"].as_str().unwrap(), w["qualname"].as_str().unwrap())).collect();
    assert_eq!(written, vec![("AAAA", "with_doc"), ("BBBB", "no_doc")]);
    assert_eq!(plan["unmatched"][0]["qualname"], "nowhere");
    assert_eq!(plan["skipped"][0]["qualname"], "broken", "a def with a syntax error is skipped on its own");
    assert_eq!(plan["text_hash"], blake3::hash(new_text.as_bytes()).to_hex().as_str());

    // a def that already carries an ID is left alone
    let again = d.request("vnoc/planIds", json!({ "text": new_text, "version": 8, "ids": [ask("with_doc", 1, "ZZZZ")] })).unwrap();
    assert_eq!(again["edits"], json!([]));
    assert_eq!(again["skipped"][0]["reason"], "already has an ID");
    d.finish();
}

#[test]
fn plan_file_id_writes_the_header_comment_and_outline_reads_it_back() {
    let (mut d, init) = Driver::start();
    assert_eq!(init["capabilities"]["ids.file"], true);
    let text = "#!/usr/bin/env python3\n\"\"\"Orders.\"\"\"\n\ndef f():\n    pass\n";
    assert_eq!(d.request("vnoc/outline", json!({ "text": text })).unwrap()["file_id"], serde_json::Value::Null);

    let plan = d.request("vnoc/planFileId", json!({ "text": text, "version": 4, "id": "AAAA" })).expect("planFileId answers");
    assert_eq!(plan["version"], 4, "the version is echoed");
    let new_text = apply_edits(text, plan["edits"].as_array().unwrap());
    assert_eq!(new_text, "#!/usr/bin/env python3\n# FileID: AAAA\n\"\"\"Orders.\"\"\"\n\ndef f():\n    pass\n");
    assert_eq!(plan["written"]["text"], "# FileID: AAAA\n");
    assert_eq!(plan["text_hash"], blake3::hash(new_text.as_bytes()).to_hex().as_str());
    assert_eq!(d.request("vnoc/outline", json!({ "text": new_text })).unwrap()["file_id"], "AAAA");

    let again = d.request("vnoc/planFileId", json!({ "text": new_text, "version": 5, "id": "BBBB" })).unwrap();
    assert_eq!(again["edits"], json!([]));
    d.finish();
}
