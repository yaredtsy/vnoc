//! A file read from disk (`root` + `file`) is read as the core reads it (`crates/vnoc/src/source.rs`): a leading byte-order mark is dropped and
//! bytes that are not UTF-8 become U+FFFD. The harness is `common`: any byte on stdout outside a frame fails the test.

mod common;

use common::{Driver, Project};
use serde_json::{Value, json};

/// A def on the first line, so that a mark in front of it would move its columns.
const TEXT: &str = "def main(): g()\n\n\ndef g():\n    return 1\n";

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
        ("plain.py", TEXT),
        ("marked.py", &format!("\u{feff}{TEXT}")),
    ]);
    let mut d = Driver::start();
    let by_text = d.request("vnoc/outline", json!({ "text": TEXT })).unwrap();
    assert_eq!(by_text["defs"][0]["range"]["start"]["character"], 0);
    assert_eq!(from_disk(&mut d, &p, "plain.py"), by_text);
    assert_eq!(from_disk(&mut d, &p, "marked.py"), by_text);
    d.finish();
}

#[test]
fn a_file_with_a_bad_byte_is_outlined() {
    let p = Project::new(&[]);
    std::fs::write(p.root.join("bad.py"), b"def main(): return 0  # caf\xe9\n").unwrap();
    let mut d = Driver::start();
    let out = from_disk(&mut d, &p, "bad.py");
    assert_eq!(out["defs"][0]["qualname"], "main");
    d.finish();
}

/// Measured, not wished for: the analyzer reads the file itself and counts a leading byte-order mark as a column, so a call site on the first line of
/// a marked file is one column late (13, where the text without the mark says 12). The driver's own read of the file did the same before it dropped the
/// mark. The analyzer is not changed here; when it is, this is the line to turn into `plain`.
#[test]
fn a_call_site_on_line_zero_of_a_marked_file_is_one_column_late() {
    let p = Project::new(&[
        ("plain.py", TEXT),
        ("marked.py", &format!("\u{feff}{TEXT}")),
    ]);
    let mut d = Driver::start();
    let plain = d.tree(&p.root, "plain.py", "main").unwrap();
    let marked = d.tree(&p.root, "marked.py", "main").unwrap();
    let site = |tree: &Value| tree["children"][0]["call_site"]["start"]["character"].clone();
    assert_eq!(site(&plain), 12);
    assert_eq!(site(&marked), 13);
    d.finish();
}
