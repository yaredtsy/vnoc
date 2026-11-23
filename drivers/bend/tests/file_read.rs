//! A file read from disk (`root` + `file`) is read as the core reads it (`crates/vnoc/src/source.rs`): a leading byte-order mark is dropped and
//! bytes that are not UTF-8 become U+FFFD. The harness is `common`: any byte on stdout outside a frame fails the test.

mod common;

use common::{Driver, Project};
use serde_json::{Value, json};

/// A def on the first line, so that a mark in front of it would move its columns.
const TEXT: &str = "def main() -> U32: g(1)\n\ndef g(x: U32) -> U32: x\n";

fn from_disk(d: &mut Driver, p: &Project, file: &str) -> Value {
    d.request(
        "vnoc/outline",
        json!({ "root": p.root.to_str().unwrap(), "file": file }),
    )
    .unwrap_or_else(|e| panic!("{file}: {e}"))
}

#[test]
fn an_outline_from_disk_ignores_a_byte_order_mark() {
    let p = Project::new(&[
        ("plain.bend", TEXT),
        ("marked.bend", &format!("\u{feff}{TEXT}")),
    ]);
    let mut d = Driver::start();
    let by_text = d.request("vnoc/outline", json!({ "text": TEXT })).unwrap();
    assert_eq!(by_text["defs"][0]["range"]["start"]["character"], 0);
    assert_eq!(from_disk(&mut d, &p, "plain.bend"), by_text);
    assert_eq!(from_disk(&mut d, &p, "marked.bend"), by_text);
    d.finish();
}

#[test]
fn a_file_with_a_bad_byte_is_outlined() {
    let p = Project::new(&[]);
    std::fs::write(p.root.join("bad.bend"), b"def main() -> U32: 0 # caf\xe9\n").unwrap();
    let mut d = Driver::start();
    let out = from_disk(&mut d, &p, "bad.bend");
    assert_eq!(out["defs"][0]["qualname"], "main");
    d.finish();
}

#[test]
fn a_call_tree_ignores_a_byte_order_mark() {
    let p = Project::new(&[
        ("plain.bend", TEXT),
        ("marked.bend", &format!("\u{feff}{TEXT}")),
    ]);
    let mut d = Driver::start();
    let plain = d.tree(&p.root, "plain.bend", "main").unwrap();
    let marked = d.tree(&p.root, "marked.bend", "main").unwrap();
    // the call to `g` is on the first line, after the mark: its columns are those of the text without it
    assert_eq!(
        plain["children"][0]["call_site"],
        json!({ "start": { "line": 0, "character": 19 }, "end": { "line": 0, "character": 20 } })
    );
    assert_eq!(
        marked["children"][0]["call_site"],
        plain["children"][0]["call_site"]
    );
    d.finish();
}
