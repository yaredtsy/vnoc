//! `vnoc/callTree` through the real binary, on `tests/fixtures/call_tree` (two files, one importing the other) and on small projects written
//! to a temp folder. The harness is `common`: any byte on stdout that is not inside a `Content-Length` frame fails the test.

mod common;

use std::path::{Path, PathBuf};

use common::{Driver, Project, callees, marks};
use serde_json::{Value, json};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/call_tree")
}

fn tree(file: &str, qualname: &str) -> Value {
    let mut d = Driver::start();
    let tree = d
        .tree(&fixture(), file, qualname)
        .unwrap_or_else(|e| panic!("{qualname}: {e}"));
    d.finish();
    tree
}

fn children(node: &Value) -> &Vec<Value> {
    node["children"].as_array().unwrap()
}

/// `(line, start character, end character)` of a call site that stays on one line.
fn site(node: &Value) -> (u64, u64, u64) {
    let s = &node["call_site"];
    assert_eq!(s["start"]["line"], s["end"]["line"], "one line: {node}");
    (
        s["start"]["line"].as_u64().unwrap(),
        s["start"]["character"].as_u64().unwrap(),
        s["end"]["character"].as_u64().unwrap(),
    )
}

/// What the text of `file` says at the call site of `node`.
fn written(root: &Path, file: &str, node: &Value) -> String {
    let text = std::fs::read_to_string(root.join(file)).unwrap();
    let (line, start, end) = site(node);
    text.lines()
        .nth(line as usize)
        .unwrap()
        .chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect()
}

#[test]
fn the_handshake_announces_the_call_tree() {
    let d = Driver::start();
    assert_eq!(d.init["capabilities"]["callTree"], true);
    assert_eq!(d.init["capabilities"]["environmentInputs"], false);
    d.finish();
}

#[test]
fn a_def_called_twice_is_two_nodes_in_source_order() {
    let twice = tree("main.bend", "twice");
    assert_eq!(
        (
            twice["target"]["qualname"].as_str(),
            twice["target"]["kind"].as_str(),
            twice["target"]["occurrence"].as_u64(),
            twice["target"]["file"].as_str()
        ),
        (Some("twice"), Some("function"), Some(1), Some("main.bend"))
    );
    assert!(twice["call_site"].is_null(), "the root is not called");
    assert!(twice.get("call_site_file").is_none());
    let [first, second] = &children(&twice)[..] else {
        panic!("two calls: {twice}")
    };
    assert_eq!(first["target"]["qualname"], "helper");
    assert_eq!(second["target"]["qualname"], "helper");
    assert_eq!(first["call_site_file"], "main.bend");
    assert_eq!(written(&fixture(), "main.bend", first), "helper");
    assert_eq!(written(&fixture(), "main.bend", second), "helper");
    let (a, b) = (site(first), site(second));
    assert_eq!(a.0, b.0, "both are on the line `helper(helper(x))`");
    assert!(a.1 < b.1, "the outer call first: {a:?} {b:?}");
}

#[test]
fn a_call_to_another_file_names_that_file_and_its_id_as_written() {
    let uses = tree("main.bend", "uses_lib");
    assert_eq!(callees(&uses), ["helper", "inc"]);
    let inc = &children(&uses)[1];
    assert_eq!(
        (
            inc["target"]["file"].as_str(),
            inc["target"]["id"].as_str(),
            inc["call_site_file"].as_str()
        ),
        (
            Some("lib.bend"),
            Some("0LIB0000000000000000000INC"),
            Some("main.bend")
        ),
        "the target is in lib.bend, the call is written in main.bend, and the ID has no prefix"
    );
    // the alias the caller wrote (`Lib.inc`) is not the name: the def is `inc` in `lib.bend`
    assert_eq!(written(&fixture(), "main.bend", inc), "Lib.inc");
    assert!(children(&uses)[0]["target"]["id"].is_null());
}

#[test]
fn a_dotted_name_is_the_entry_and_a_node_like_any_other() {
    let double = tree("main.bend", "Nat.double");
    assert_eq!(double["target"]["qualname"], "Nat.double");
    assert_eq!(callees(&double), ["helper"]);
}

#[test]
fn direct_recursion_is_shown_and_not_entered() {
    let forever = tree("main.bend", "forever");
    let [again] = &children(&forever)[..] else {
        panic!("one call: {forever}")
    };
    assert_eq!(again["target"]["qualname"], "forever");
    assert_eq!(again["marks"], json!(["recursive"]));
    assert!(children(again).is_empty());
}

#[test]
fn a_chain_is_followed_to_its_end() {
    let top = tree("main.bend", "top");
    let mut names = vec![];
    let mut node = &top;
    loop {
        names.push(node["target"]["qualname"].as_str().unwrap());
        match children(node).first() {
            Some(next) => node = next,
            None => break,
        }
    }
    assert_eq!(names, ["top", "a", "b", "c", "helper"]);
}

#[test]
fn depth_shortens_the_tree_and_marks_the_cut() {
    let mut d = Driver::start();
    let top = d
        .tree_of(&fixture(), "main.bend", "top", 1, Some(1))
        .unwrap();
    let [a] = &children(&top)[..] else {
        panic!("one call: {top}")
    };
    assert_eq!(a["marks"], json!(["cut"]));
    assert!(children(a).is_empty(), "{top}");
    // asking for the def that was cut gives its own tree
    let again = d.tree_of(&fixture(), "main.bend", "a", 1, Some(1)).unwrap();
    assert_eq!(callees(&again), ["b"]);
    d.finish();
}

#[test]
fn an_unknown_entry_is_invalid_params() {
    let mut d = Driver::start();
    let e = d.tree(&fixture(), "main.bend", "nowhere").unwrap_err();
    assert_eq!(e["code"], -32602);
    assert_eq!(e["message"], "no def nowhere in main.bend");
    let e = d
        .tree_of(&fixture(), "main.bend", "helper", 2, None)
        .unwrap_err();
    assert_eq!(e["code"], -32602, "there is only one helper");
    d.finish();
}

#[test]
fn occurrence_zero_is_invalid_params_and_says_why() {
    let mut d = Driver::start();
    let e = d
        .tree_of(&fixture(), "main.bend", "helper", 0, None)
        .unwrap_err();
    assert_eq!(e["code"], -32602);
    assert_eq!(e["message"], "occurrence of helper starts at 1, not 0");
    d.finish();
}

#[test]
fn a_file_that_cannot_be_read_is_an_internal_error() {
    let mut d = Driver::start();
    let e = d.tree(&fixture(), "missing.bend", "f").unwrap_err();
    assert_eq!(e["code"], -32603);
    assert!(
        e["message"].as_str().unwrap().contains("missing.bend"),
        "{e}"
    );
    d.finish();
}

#[test]
fn a_character_before_the_call_counts_in_utf16_units() {
    // the emoji is one character and two UTF-16 units: the site starts after it, in the editor's columns
    let p = Project::new(&[(
        "app.bend",
        "def g(x: U32) -> U32: x\n\ndef f(x: U32) -> U32: g(\"\u{1F600}\", h(x))\n\ndef h(x: U32) -> U32: x\n",
    )]);
    let mut d = Driver::start();
    let f = d.tree(&p.root, "app.bend", "f").unwrap();
    assert_eq!(callees(&f), ["g", "h"]);
    // `def f(x: U32) -> U32: g("` is 25 characters, the emoji 2 units, `", ` 3: `h` starts at 25 + 2 + 3
    assert_eq!(site(&children(&f)[0]), (2, 22, 23), "g, before the emoji");
    assert_eq!(site(&children(&f)[1]), (2, 30, 31), "h, after it");
    d.finish();
}

#[test]
fn a_law_and_its_proof_are_two_entries_of_one_name() {
    let p = Project::new(&[(
        "app.bend",
        "def step(n: Nat) -> Nat: n\n\nlaw same:\n  for n: Nat\n  Eq(Nat, n, n)\n\ndef same(n):\n  step(n)\n",
    )]);
    let mut d = Driver::start();
    let law = d.tree_of(&p.root, "app.bend", "same", 1, None).unwrap();
    assert_eq!(law["target"]["qualname"], "same");
    assert_eq!(law["target"]["occurrence"], 1);
    let proof = d.tree_of(&p.root, "app.bend", "same", 2, None).unwrap();
    assert_eq!(proof["target"]["occurrence"], 2);
    assert_eq!(callees(&proof), ["step"]);
    d.finish();
}

#[test]
fn a_type_is_a_class_when_a_def_builds_it() {
    let p = Project::new(&[(
        "app.bend",
        "type Pair is Data:\n  Pair{a: U32, b: U32}\n\ndef make(x: U32) -> Pair: Pair{x, x}\n",
    )]);
    let mut d = Driver::start();
    let make = d.tree(&p.root, "app.bend", "make").unwrap();
    // whatever the analyzer draws for the construction, a node that stands for the type says `class`
    for node in std::iter::once(&make).chain(children(&make)) {
        if node["target"]["qualname"] == "Pair" {
            assert_eq!(node["target"]["kind"], "class", "{make}");
        }
    }
    d.finish();
}

#[test]
fn the_session_is_kept_and_a_saved_change_is_seen_without_opening_again() {
    let p = Project::new(&[(
        "app.bend",
        "def run() -> U32: a()\n\ndef a() -> U32: 1\n\ndef b() -> U32: 2\n",
    )]);
    let mut d = Driver::start();
    assert_eq!(callees(&d.tree(&p.root, "app.bend", "run").unwrap()), ["a"]);
    assert_eq!(callees(&d.tree(&p.root, "app.bend", "run").unwrap()), ["a"]);
    let stats = d.stats();
    assert_eq!(
        (
            stats["solves"].as_u64(),
            stats["opens"].as_u64(),
            stats["reopens"].as_u64()
        ),
        (Some(2), Some(1), Some(0)),
        "the second request opened nothing: {stats}"
    );

    p.write(
        "app.bend",
        "def run() -> U32: b()\n\ndef a() -> U32: 1\n\ndef b() -> U32: 2\n",
    );
    assert_eq!(callees(&d.tree(&p.root, "app.bend", "run").unwrap()), ["b"]);
    let stats = d.stats();
    assert_eq!(
        (stats["opens"].as_u64(), stats["reopens"].as_u64()),
        (Some(1), Some(1)),
        "the change is a rebuild, not a second session: {stats}"
    );
    d.finish();
}

#[test]
fn an_imported_file_saved_later_is_seen() {
    let p = Project::new(&[
        (
            "main.bend",
            "import ./lib.bend as Lib\n\ndef run() -> U32: Lib.f()\n",
        ),
        ("lib.bend", "def f() -> U32: 1\n"),
    ]);
    let mut d = Driver::start();
    let before = d.tree(&p.root, "main.bend", "run").unwrap();
    assert_eq!(children(&before)[0]["target"]["file"], "lib.bend");
    p.write(
        "lib.bend",
        "def other() -> U32: 2\n\ndef f() -> U32: other()\n",
    );
    let after = d.tree(&p.root, "main.bend", "run").unwrap();
    let f = &children(&after)[0];
    assert_eq!(callees(f), ["other"], "{after}");
    d.finish();
}

#[test]
fn a_file_that_is_gone_is_an_error_and_comes_back_when_it_does() {
    let p = Project::new(&[("app.bend", "def run() -> U32: 1\n")]);
    let mut d = Driver::start();
    d.tree(&p.root, "app.bend", "run").unwrap();
    p.remove("app.bend");
    let e = d.tree(&p.root, "app.bend", "run").unwrap_err();
    assert_eq!(e["code"], -32603);
    assert!(e["message"].as_str().unwrap().contains("app.bend"), "{e}");
    p.write(
        "app.bend",
        "def run() -> U32: helper()\n\ndef helper() -> U32: 1\n",
    );
    assert_eq!(
        callees(&d.tree(&p.root, "app.bend", "run").unwrap()),
        ["helper"]
    );
    d.finish();
}

#[test]
fn each_entry_file_has_a_session_of_its_own() {
    let p = Project::new(&[
        ("one.bend", "def run() -> U32: 1\n"),
        ("two.bend", "def run() -> U32: 2\n"),
    ]);
    let mut d = Driver::start();
    d.tree(&p.root, "one.bend", "run").unwrap();
    d.tree(&p.root, "two.bend", "run").unwrap();
    d.tree(&p.root, "one.bend", "run").unwrap();
    let stats = d.stats();
    assert_eq!(
        (stats["sessions"].as_u64(), stats["opens"].as_u64()),
        (Some(2), Some(2)),
        "{stats}"
    );
    d.finish();
}

#[test]
fn a_request_cancelled_while_it_is_queued_is_answered_cancelled_and_the_solver_never_ran() {
    let mut d = Driver::start();
    let hold = d.send_request("vnoc/test/hold", json!({ "ms": 700 })); // the worker is busy
    let queued = d.send_request(
        "vnoc/callTree",
        json!({ "root": fixture().to_str().unwrap(), "file": "main.bend", "entry": { "qualname": "twice", "occurrence": 1 } }),
    );
    d.notify("$/cancelRequest", json!({ "id": queued }));
    let (first, held) = d.read_response();
    assert_eq!(first, hold, "the worker finishes what it was doing first");
    held.unwrap();
    let (second, answer) = d.read_response();
    assert_eq!(second, queued);
    assert_eq!(answer.unwrap_err()["code"], -32800, "cancelled");
    let stats = d.stats();
    assert_eq!(
        (stats["solves"].as_u64(), stats["opens"].as_u64()),
        (Some(0), Some(0)),
        "never run: the session was not even opened: {stats}"
    );
    d.finish();
}

#[test]
fn a_request_past_its_deadline_is_answered_and_the_next_one_is_served() {
    let mut d = Driver::start();
    d.request("vnoc/test/deadline", json!({ "ms": 0 })).unwrap(); // every request is past its deadline when the worker gets it
    let late = d
        .tree(&fixture(), "main.bend", "top")
        .expect("a late request is answered, not an error");
    assert!(
        marks(&late).contains(&"incomplete".to_string())
            || marks(&late).contains(&"cut".to_string()),
        "the tree says what it left out: {late}"
    );
    d.request("vnoc/test/deadline", json!({})).unwrap(); // the usual 20 s again
    let whole = d.tree(&fixture(), "main.bend", "top").unwrap();
    assert!(marks(&whole).is_empty(), "this one is whole: {whole}");
    d.finish();
}

#[test]
fn a_panic_inside_one_request_does_not_kill_the_worker() {
    let mut d = Driver::start();
    d.tree(&fixture(), "main.bend", "twice").unwrap();
    let crashed = d
        .request(
            "vnoc/test/panic",
            json!({ "root": fixture().to_str().unwrap() }),
        )
        .unwrap_err();
    assert_eq!(crashed["code"], -32603, "an internal error");
    assert!(
        crashed["message"].as_str().unwrap().contains("crashed"),
        "{crashed}"
    );
    assert_eq!(
        callees(&d.tree(&fixture(), "main.bend", "twice").unwrap()),
        ["helper", "helper"]
    );
    let stats = d.stats();
    assert_eq!(
        stats["opens"], 2,
        "the session that was in use is dropped and opened again: {stats}"
    );
    d.finish();
}

#[test]
fn a_deep_chain_runs_on_the_worker_without_overflowing_its_stack() {
    // 1,500 levels: past the analyzer's own depth limit (1,000), so the tree stops there, and the answer must still come. The answer is a JSON value
    // a thousand levels deep: the test reads and drops it on a thread with a stack to match (the driver's own worker has its 8 MiB).
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            let mut text = String::from("def main() -> U32: f0()\n\n");
            for i in 0..1500 {
                text.push_str(&format!("def f{i}() -> U32: f{}()\n\n", i + 1));
            }
            text.push_str("def f1500() -> U32: 0\n");
            let p = Project::new(&[("chain.bend", &text)]);
            let mut d = Driver::start();
            let tree = d
                .tree(&p.root, "chain.bend", "main")
                .expect("answered, the driver did not abort");
            assert_eq!(callees(&tree), ["f0"]);
            let (mut depth, mut node) = (0, &tree);
            while let Some(next) = node["children"].as_array().and_then(|c| c.first()) {
                (depth, node) = (depth + 1, next);
            }
            assert_eq!(
                (depth, node["target"]["qualname"].as_str()),
                (1000, Some("f999")),
                "walked to the analyzer's own limit and no further"
            );
            assert_eq!(
                marks(&tree),
                ["incomplete"],
                "its own limit is `incomplete`, not `cut` (that is the request's depth)"
            );
            d.finish();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_def_with_a_syntax_error_still_answers() {
    let p = Project::new(&[(
        "app.bend",
        "def good() -> U32: helper()\n\ndef helper() -> U32: 1\n\ndef broken(a: U32, , b) -> U32:\n  body\n",
    )]);
    let mut d = Driver::start();
    let good = d.tree(&p.root, "app.bend", "good").unwrap();
    assert_eq!(callees(&good), ["helper"]);
    d.finish();
}
