//! The driver keeps the project open (docs/16-ui-redesign/10-build-order.md 8.2a; the analyzer's `11-driver-needs.md` §8): one worker thread owns one
//! `Project` per root and refreshes it before each request. These tests run the real binary with the test hooks on (`VNOC_TEST_HOOKS=1`), which can
//! count what the worker did and hold it busy.

mod common;

use common::{Driver, Project, callees, marks};
use serde_json::json;

const LIB: &str = "def helper():\n    return 1\n";
const APP: &str = "from lib import helper\n\n\ndef run():\n    helper()\n";

fn app_and_lib() -> Project {
    Project::new(&[("app.py", APP), ("lib.py", LIB)])
}

#[test]
fn a_kept_project_sees_an_edit_with_no_restart() {
    let p = app_and_lib();
    let mut d = Driver::start();
    assert_eq!(callees(&d.tree(&p.root, "app.py", "run").unwrap()), ["helper"]);
    // change both files on disk: a new function in lib.py, called from run
    p.write("lib.py", "def helper():\n    return 1\n\n\ndef other():\n    return 2\n");
    p.write("app.py", "from lib import helper, other\n\n\ndef run():\n    helper()\n    other()\n");
    assert_eq!(callees(&d.tree(&p.root, "app.py", "run").unwrap()), ["helper", "other"], "the new call is there");
    let stats = d.stats();
    assert_eq!((stats["opens"].as_u64(), stats["reopens"].as_u64(), stats["solves"].as_u64()), (Some(1), Some(0), Some(2)), "one project, never opened again: {stats}");
    d.finish();
}

#[test]
fn a_deleted_file_and_a_new_file_are_seen_the_same_way() {
    let p = app_and_lib();
    let mut d = Driver::start();
    let before = d.tree(&p.root, "app.py", "run").unwrap();
    assert_eq!(before["children"][0]["target"]["file"], "lib.py");

    p.remove("lib.py");
    let after = d.tree(&p.root, "app.py", "run").unwrap();
    assert!(
        after["children"].as_array().unwrap().iter().all(|c| c["target"]["file"] != "lib.py"),
        "lib.py is gone: nothing in the tree is in it any more: {after}"
    );

    p.write("fresh.py", "def fresh():\n    return 3\n");
    p.write("app.py", "from fresh import fresh\n\n\ndef run():\n    fresh()\n");
    let tree = d.tree(&p.root, "app.py", "run").unwrap();
    assert_eq!(callees(&tree), ["fresh"], "a file made after the project was opened is found");
    assert_eq!(tree["children"][0]["target"]["file"], "fresh.py");
    assert_eq!(callees(&d.tree(&p.root, "fresh.py", "fresh").unwrap()), Vec::<String>::new(), "and it is an entry too");
    assert_eq!(d.stats()["opens"], 1, "all of it on one open project");
    d.finish();
}

#[test]
fn a_changed_environment_input_opens_the_project_again() {
    let p = app_and_lib();
    let mut d = Driver::start();
    d.tree(&p.root, "app.py", "run").unwrap();
    assert_eq!(d.stats()["reopens"], 0);
    p.write("pyproject.toml", "[project]\nname = \"demo\"\nrequires-python = \">=3.9\"\n");
    assert_eq!(callees(&d.tree(&p.root, "app.py", "run").unwrap()), ["helper"], "still answers");
    let stats = d.stats();
    assert_eq!((stats["reopens"].as_u64(), stats["opens"].as_u64()), (Some(1), Some(1)), "reopened by the refresh, not a second project: {stats}");
    d.finish();
}

#[test]
fn two_roots_are_two_projects_and_their_answers_are_never_mixed() {
    let a = Project::new(&[("app.py", "def run():\n    only_in_a()\n\n\ndef only_in_a():\n    pass\n")]);
    let b = Project::new(&[("app.py", "def run():\n    only_in_b()\n\n\ndef only_in_b():\n    pass\n")]);
    let mut d = Driver::start();
    for _ in 0..2 {
        assert_eq!(callees(&d.tree(&a.root, "app.py", "run").unwrap()), ["only_in_a"]);
        assert_eq!(callees(&d.tree(&b.root, "app.py", "run").unwrap()), ["only_in_b"]);
    }
    let stats = d.stats();
    assert_eq!((stats["projects"].as_u64(), stats["opens"].as_u64(), stats["reopens"].as_u64()), (Some(2), Some(2), Some(0)), "{stats}");
    d.finish();
}

#[test]
fn a_request_cancelled_while_it_is_queued_is_answered_cancelled_and_the_solver_never_ran() {
    let p = app_and_lib();
    let mut d = Driver::start();
    let hold = d.send_request("vnoc/test/hold", json!({ "ms": 700 })); // the worker is busy
    let queued = d.send_request("vnoc/callTree", json!({ "root": p.root.to_str().unwrap(), "file": "app.py", "entry": { "qualname": "run", "occurrence": 1 } }));
    d.notify("$/cancelRequest", json!({ "id": queued }));
    let (first, held) = d.read_response();
    assert_eq!(first, hold, "the worker finishes what it was doing first");
    held.unwrap();
    let (second, answer) = d.read_response();
    assert_eq!(second, queued);
    assert_eq!(answer.unwrap_err()["code"], -32800, "cancelled");
    let stats = d.stats();
    assert_eq!((stats["solves"].as_u64(), stats["opens"].as_u64()), (Some(0), Some(0)), "never run: the project was not even opened: {stats}");
    d.finish();
}

#[test]
fn a_request_past_its_deadline_is_answered_marked_incomplete_and_the_next_one_is_served() {
    let p = Project::new(&[("app.py", "def run():\n    a()\n    b()\n\n\ndef a():\n    b()\n\n\ndef b():\n    pass\n")]);
    let mut d = Driver::start();
    d.request("vnoc/test/deadline", json!({ "ms": 0 })).unwrap(); // every request is past its deadline when the worker gets it
    let late = d.tree(&p.root, "app.py", "run").expect("a late request is answered, not an error");
    assert!(marks(&late).contains(&"incomplete".to_string()), "the tree says it is incomplete: {late}");
    d.request("vnoc/test/deadline", json!({})).unwrap(); // the usual 20 s again
    let whole = d.tree(&p.root, "app.py", "run").unwrap();
    assert_eq!(callees(&whole), ["a", "b"]);
    assert!(!marks(&whole).contains(&"incomplete".to_string()), "and this one is whole: {whole}");
    assert_eq!(d.stats()["solves"], 2);
    d.finish();
}

#[test]
fn a_panic_inside_one_request_does_not_kill_the_worker() {
    let p = app_and_lib();
    let mut d = Driver::start();
    d.tree(&p.root, "app.py", "run").unwrap();
    let crashed = d.request("vnoc/test/panic", json!({ "root": p.root.to_str().unwrap() })).unwrap_err();
    assert_eq!(crashed["code"], -32603, "an internal error");
    assert!(crashed["message"].as_str().unwrap().contains("crashed"), "{crashed}");
    assert_eq!(callees(&d.tree(&p.root, "app.py", "run").unwrap()), ["helper"], "the next request is answered");
    let stats = d.stats();
    assert_eq!(stats["opens"], 2, "the project that was in use is dropped and opened again: {stats}");
    d.finish();
}

#[test]
fn a_deep_chain_runs_on_the_worker_without_overflowing_its_stack() {
    // 300 levels: past `max_stack` (256), which needs just over 2 MiB; a thread with Rust's default 2 MiB would abort the whole driver
    let mut text = String::from("def main():\n    f0()\n\n\n");
    for i in 0..300 {
        text.push_str(&format!("def f{i}():\n    f{}()\n\n\n", i + 1));
    }
    text.push_str("def f300():\n    pass\n");
    let p = Project::new(&[("chain.py", &text)]);
    let mut d = Driver::start();
    let tree = d.tree(&p.root, "chain.py", "main").expect("answered, the driver did not abort");
    assert_eq!(callees(&tree), ["f0"]);
    d.finish();
}
