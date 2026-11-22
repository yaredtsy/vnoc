//! The driver as a process, spoken to over its wire (`outline`, the handshake). The harness is also the stdout guard: any byte on stdout
//! that is not inside a `Content-Length` frame fails the test.

mod common;

use common::Driver;
use serde_json::{Value, json};

const KINDS: &str = include_str!("fixtures/kinds.bend");

fn defs(outline: &Value) -> &Vec<Value> {
    outline["defs"].as_array().unwrap()
}

fn outline(d: &mut Driver, text: &str) -> Value {
    d.request("vnoc/outline", json!({ "text": text }))
        .expect("outline answers")
}

/// `(qualname, kind)` of each def, in the order given.
fn names(outline: &Value) -> Vec<(&str, &str)> {
    defs(outline)
        .iter()
        .map(|d| (d["qualname"].as_str().unwrap(), d["kind"].as_str().unwrap()))
        .collect()
}

#[test]
fn the_handshake_announces_what_the_driver_can_do() {
    let d = Driver::start();
    assert_eq!(d.init["protocol"], 1);
    assert_eq!(d.init["name"], "bend");
    assert_eq!(d.init["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(d.init["files"]["extensions"], json!(["bend"]));
    let capabilities = &d.init["capabilities"];
    assert_eq!(capabilities["outline"], true);
    assert_eq!(capabilities["ids.plan"], true);
    assert_eq!(capabilities["ids.file"], true);
    assert_eq!(capabilities["callTree"], true);
    assert_eq!(
        capabilities["environmentInputs"], false,
        "Bend has no manifest"
    );
    d.finish();
}

#[test]
fn a_protocol_it_does_not_speak_is_refused_with_a_reason() {
    let mut d = Driver::spawn();
    let refused = d
        .request("initialize", json!({ "protocol": 2 }))
        .unwrap_err();
    assert_eq!(refused["code"], -32600);
    assert!(
        refused["message"].as_str().unwrap().contains("protocol 1"),
        "{refused}"
    );
    assert!(!d.exited_cleanly(), "and the driver stops");
}

#[test]
fn a_request_before_initialized_ends_the_driver() {
    let mut d = Driver::spawn();
    d.request("initialize", json!({ "protocol": 1 }))
        .expect("the handshake answers");
    // no `initialized`: the next message is not the one the handshake waits for
    d.send_request("vnoc/outline", json!({ "text": "" }));
    assert!(d.stdout_is_closed(), "no answer: the driver stopped");
    assert!(!d.exited_cleanly());
}

#[test]
fn unknown_notifications_are_ignored_and_unknown_requests_get_32601() {
    let mut d = Driver::start();
    d.notify("$/somethingNew", json!({ "x": 1 }));
    let e = d.request("vnoc/doesNotExist", json!({})).unwrap_err();
    assert_eq!(e["code"], -32601);
    // not built: `environmentInputs` is announced `false`, and asking anyway is an unknown method
    let e = d.request("vnoc/environmentInputs", json!({})).unwrap_err();
    assert_eq!(e["code"], -32601);
    assert_eq!(
        defs(&outline(&mut d, "def f() -> Nat: 0\n")).len(),
        1,
        "still alive and answering"
    );
    d.finish();
}

#[test]
fn bad_params_get_32602() {
    let mut d = Driver::start();
    assert_eq!(
        d.request("vnoc/outline", json!({})).unwrap_err()["code"],
        -32602
    );
    assert_eq!(
        d.request("vnoc/outline", json!({ "text": "x", "fingerprints": true }))
            .unwrap_err()["code"],
        -32602,
        "fingerprints are not built"
    );
    d.finish();
}

#[test]
fn every_declaration_is_a_def_and_a_type_is_a_class() {
    let mut d = Driver::start();
    let out = outline(&mut d, KINDS);
    assert_eq!(
        names(&out),
        [
            ("add", "function"),
            ("raw", "function"),
            ("twice", "function"),
            ("add_zero", "function"),
            ("add_zero", "function"),
            ("List", "class"),
            ("Nat.double", "function"),
        ],
        "a plain def, an @unsafe one, a template, a law and its proof, a type; the constructors Nil and Con are not defs"
    );
    let v = defs(&out);
    assert_eq!(v[0]["params"], json!(["a", "b"]));
    assert_eq!(
        v[2]["params"],
        json!(["~f", "x"]),
        "a template's marker stays in the name"
    );
    assert!(v.iter().all(|x| x["id"].is_null()), "no marker yet");
    assert!(out["file_id"].is_null());
    d.finish();
}

#[test]
fn a_law_and_its_proof_share_a_qualname_in_source_order() {
    let mut d = Driver::start();
    let out = outline(&mut d, KINDS);
    let v = defs(&out);
    assert_eq!(v[3]["qualname"], v[4]["qualname"]);
    let (law, proof) = (&v[3]["range"], &v[4]["range"]);
    assert!(
        law["end"]["line"].as_u64() < proof["start"]["line"].as_u64(),
        "the law comes first: it is occurrence 1, its proof occurrence 2"
    );
    d.finish();
}

#[test]
fn no_nesting_a_dotted_name_is_one_top_level_def() {
    let mut d = Driver::start();
    let out = outline(&mut d, "def Nat.double(n: Nat) -> Nat: n\n");
    let v = defs(&out);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0]["qualname"], "Nat.double");
    assert_eq!(v[0]["parent"], "");
    assert!(v[0]["parent_kind"].is_null());
    d.finish();
}

#[test]
fn a_def_that_has_a_marker_gives_its_id_and_a_neighbour_does_not() {
    let mut d = Driver::start();
    let out = outline(
        &mut d,
        "# Adds.\n# ID: add-1\n@unsafe def a() -> Nat: 0\n\ndef b() -> Nat: 1\n",
    );
    let v = defs(&out);
    assert_eq!(v[0]["id"], "add-1");
    assert!(v[1]["id"].is_null());
    d.finish();
}

#[test]
fn a_file_of_imports_only_has_no_defs() {
    let mut d = Driver::start();
    let out = outline(&mut d, include_str!("fixtures/imports_only.bend"));
    assert!(defs(&out).is_empty());
    assert!(out["syntax_errors"].as_array().unwrap().is_empty());
    d.finish();
}

#[test]
fn a_syntax_error_flags_the_broken_def_and_not_its_neighbours() {
    let mut d = Driver::start();
    let out = outline(&mut d, include_str!("fixtures/broken.bend"));
    let flags: Vec<(&str, bool)> = defs(&out)
        .iter()
        .map(|x| {
            (
                x["qualname"].as_str().unwrap(),
                x["syntax_error"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(flags, [("first", false), ("broken", true), ("last", false)]);
    assert!(!out["syntax_errors"].as_array().unwrap().is_empty());
    d.finish();
}

#[test]
fn positions_are_zero_based_lines_and_utf16_columns() {
    let mut d = Driver::start();
    // an emoji is two UTF-16 code units: one in a doc comment above the def, one in a string on its line
    let text =
        "# \u{1F600} smile\ndef s() -> String: \"h\u{e9}llo \u{1F30D}\"\ndef next() -> Nat: 0\n";
    let out = outline(&mut d, text);
    let v = defs(&out);
    assert_eq!(v.len(), 2);
    // the range starts at the doc comment above the declaration
    assert_eq!(v[0]["range"]["start"], json!({ "line": 0, "character": 0 }));
    // `def s() -> String: "` is 20 characters, then h é l l o and a space (6), the globe (2 units), the closing quote (1)
    assert_eq!(v[0]["range"]["end"], json!({ "line": 1, "character": 29 }));
    assert_eq!(v[1]["range"]["start"], json!({ "line": 2, "character": 0 }));
    d.finish();
}

#[test]
fn a_file_can_be_read_by_the_driver_from_root_and_file() {
    let mut d = Driver::start();
    let out = d
        .request(
            "vnoc/outline",
            json!({ "root": concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"), "file": "kinds.bend" }),
        )
        .unwrap();
    assert_eq!(defs(&out).len(), 7);
    d.finish();
}
