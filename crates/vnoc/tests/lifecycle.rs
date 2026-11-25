//! The core as a process, spoken to over LSP framing, with the real Python driver behind it.
//! Needs the driver built first: `cd drivers/python && cargo build`.
//!
//! VS-D67 condition 3: unknown notifications and unknown requests plus a full lifecycle, and the core is still alive.

#[path = "common/testdir.rs"]
mod testdir;

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use async_lsp::lsp_types::Url;
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

fn driver_path() -> PathBuf {
    std::env::var_os("VNOC_DRIVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            repo_root()
                .join("drivers/python/target/debug")
                .join(format!(
                    "vnoc-driver-python{}",
                    std::env::consts::EXE_SUFFIX
                ))
        })
}

/// A copy of the fixtures in a fresh temp folder (a folder inside this repository would be inside its git repo, and ids would carry that path).
fn fixtures(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-lifecycle-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    testdir::remove_at_end(&dir);
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
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_remove("VNOC_DRIVER");
        let mut child = cmd.spawn().unwrap();
        let (stdin, out) = (
            child.stdin.take().unwrap(),
            BufReader::new(child.stdout.take().unwrap()),
        );
        let mut core = Core {
            child,
            stdin,
            out,
            next: 0,
            notifications: vec![],
            editor_applies_edits: true,
            edit_requests: vec![],
        };
        let options = driver.map_or(
            json!({ "root": root }),
            |d| json!({ "driverPath": d, "root": root }),
        );
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
                    self.notifications
                        .push((method.to_string(), msg["params"].clone()));
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
        self.send(
            json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "applied": applied } }),
        );
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
                Some(m) => self
                    .notifications
                    .push((m.to_string(), msg["params"].clone())),
                None => panic!("a response nobody asked for: {msg}"),
            }
        }
    }

    /// Panics if stdout holds anything but frames: the core's stdout IS the protocol.
    fn read_frame(&mut self) -> Value {
        let mut line = String::new();
        self.out.read_line(&mut line).unwrap();
        let len: usize = line
            .strip_prefix("Content-Length:")
            .unwrap_or_else(|| panic!("STDOUT GUARD: stray bytes on stdout: {line:?}"))
            .trim()
            .parse()
            .unwrap();
        self.out.read_line(&mut String::new()).unwrap();
        let mut buf = vec![0u8; len];
        self.out.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        assert!(
            self.child.wait().unwrap().success(),
            "the core exits cleanly after shutdown + exit"
        );
    }
}

/// A file's URI as an editor sends it (`file:///C:/…` on Windows: a path glued after `file://` is not one there).
fn file_uri(file: &std::path::Path) -> String {
    Url::from_file_path(file).unwrap().to_string()
}

/// Applies a `documentChanges` list of text edits to the files on disk (ASCII text: a character is a byte).
fn apply_document_changes(changes: &Value) {
    for change in changes.as_array().unwrap() {
        let path = Url::parse(change["textDocument"]["uri"].as_str().unwrap())
            .unwrap()
            .to_file_path()
            .unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        let starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let offset = |p: &Value| {
            starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
        };
        for edit in change["edits"].as_array().unwrap().iter().rev() {
            text.insert_str(
                offset(&edit["range"]["start"]),
                edit["newText"].as_str().unwrap(),
            );
        }
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn unknown_messages_do_not_kill_the_core_and_the_lifecycle_ends_cleanly() {
    let mut core = Core::start(Some(driver_path()), &fixtures("lifecycle"));
    // unknown notifications, one under `$/` and one not (async-lsp's default would stop the loop on the second)
    core.notify("$/somethingNew", json!({}));
    core.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": {} }),
    );
    core.notify("vnoc/neverHeardOfIt", json!({ "x": 1 }));
    // an unknown request is an error, not silence and not a crash
    assert_eq!(
        core.request("vnoc/nope", json!({})).unwrap_err()["code"],
        -32601
    );
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
    let r = core
        .request("vnoc/structure", Value::Null)
        .expect("structure answers (is the driver built? `cd drivers/python && cargo build`)");

    let file = &r["root"]["children"][0];
    assert_eq!(r["root"]["kind"], "folder");
    assert_eq!(
        r["git_root"],
        json!(root),
        "no .git above the folder: the folder is the root"
    );
    assert_eq!(
        (file["kind"].as_str(), file["name"].as_str()),
        (Some("file"), Some("nested.py"))
    );
    // 15 definitions in the fixture; the tree nests them: top-level defs and classes under the file
    let top_level: Vec<&str> = file["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        top_level,
        vec!["top", "Outer", "top", "cond", "cond", "decorated", "coro"]
    );
    assert_eq!(r["stats"]["files"], 1);
    assert_eq!(r["stats"]["classes"], 3); // Local, Outer, Outer.Nested
    assert_eq!(r["stats"]["functions"], 12);

    // keys (the real LocationKey, from vnoc-domain) are unique even for redefinitions
    let ids: Vec<&str> = file["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            "nested.py::top",
            "nested.py::Outer",
            "nested.py::top#2",
            "nested.py::cond",
            "nested.py::cond#2",
            "nested.py::decorated",
            "nested.py::coro"
        ]
    );
    // nesting through <locals>: top > inner > deepest
    let inner = &file["children"][0]["children"][0];
    assert_eq!(inner["qualname"], "top.<locals>.inner");
    assert_eq!(
        inner["children"][0]["qualname"],
        "top.<locals>.inner.<locals>.deepest"
    );
    assert_eq!(r["truncated"], false);
    assert_eq!(r["failed"], json!([]));

    // the first build announces itself; asking again gives the same answer from the same revision
    assert_eq!(
        core.wait_for("vnoc/indexChanged")["revision"],
        r["revision"]
    );
    let again = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(
        (&again["root"], &again["revision"]),
        (&r["root"], &r["revision"])
    );
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
    assert_eq!(
        file["path"], "nested.py",
        "the editor opens it below the opened folder"
    );
    assert_eq!(
        file["id"], "services/api/nested.py",
        "the id starts at the git root"
    );
    assert_eq!(file["children"][0]["id"], "services/api/nested.py::top");
    core.finish();
}

#[test]
fn a_changed_file_is_read_again_and_a_new_file_appears() {
    let root = fixtures("changes");
    let mut core = Core::start(Some(driver_path()), &root);
    let first = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(
        core.wait_for("vnoc/indexChanged")["revision"],
        first["revision"]
    );

    // an edit: the editor reports the path, the core reads that file again and says so
    let edited = root.join("nested.py");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\ndef added_later():\n    pass\n");
    std::fs::write(&edited, text).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [edited] }));
    let changed = core.wait_for("vnoc/indexChanged");
    assert_eq!(
        changed["revision"].as_u64().unwrap(),
        first["revision"].as_u64().unwrap() + 1
    );
    let after = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(after["stats"]["functions"], 13);

    // a new file the editor did not even list (it is found by the walk), and a deleted one
    std::fs::write(root.join("fresh.py"), "def hello():\n    pass\n").unwrap();
    std::fs::remove_file(&edited).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [] }));
    core.wait_for("vnoc/indexChanged");
    let after = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(after["root"]["children"][0]["name"], "fresh.py");
    assert_eq!(
        (
            after["stats"]["files"].as_u64(),
            after["stats"]["functions"].as_u64()
        ),
        (Some(1), Some(1))
    );

    // a rebuild of unchanged code is quiet; two at once are not run twice
    core.notify("vnoc/rebuild", Value::Null);
    core.notify("vnoc/rebuild", Value::Null);
    let still = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(still["revision"], after["revision"]);
    core.finish();
}

/// The ids of a `vnoc/search` answer's node rows.
fn found_ids(answer: &Value) -> Vec<&str> {
    answer["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect()
}

#[test]
fn search_answers_from_the_tree_before_and_after_a_change() {
    let root = fixtures("search");
    let mut core = Core::start(Some(driver_path()), &root);

    // asked at once, without waiting for the first build: it answers when the build is done
    let found = core
        .request("vnoc/search", json!({ "query": "method" }))
        .unwrap();
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(found["revision"], structure["revision"]);
    let rows = found["nodes"].as_array().unwrap();
    // `Local.method` (a class inside `top`), `Outer.method` twice, `Outer.Nested.method`
    assert_eq!(rows.len(), 4, "{found}");
    for row in rows {
        assert_eq!(row["in_class"], true, "{row}");
        assert_eq!(row["tier"], 0, "{row}");
        assert_eq!(row["match"], json!([[0, 6]]), "{row}");
        assert_eq!(row["kind"], "function", "{row}");
        assert_eq!(row["path"], "nested.py", "{row}");
        assert!(row["line"].is_u64(), "{row}");
    }
    assert_eq!(found["nodes_total"], rows.len());
    assert_eq!(found["docs"], json!([]));
    assert_eq!(found["docs_total"], 0);
    assert_eq!(found["notes"], json!([]));

    // tier 3: the qualified name holds it, the name does not
    let found = core
        .request("vnoc/search", json!({ "query": "outer.meth" }))
        .unwrap();
    assert_eq!(
        found_ids(&found),
        ["nested.py::Outer.method", "nested.py::Outer.method#2"]
    );
    for row in found["nodes"].as_array().unwrap() {
        assert_eq!(row["tier"], 3, "{row}");
        assert_eq!(row["match"], json!([]), "{row}");
    }

    // the limit cuts the rows, not the count
    let found = core
        .request("vnoc/search", json!({ "query": "method", "nodes": 1 }))
        .unwrap();
    assert_eq!(found["nodes"].as_array().unwrap().len(), 1);
    assert!(found["nodes_total"].as_u64().unwrap() > 1, "{found}");

    // nothing typed finds nothing
    let found = core
        .request("vnoc/search", json!({ "query": "  " }))
        .unwrap();
    assert_eq!(
        (found["nodes"].clone(), found["nodes_total"].clone()),
        (json!([]), json!(0))
    );

    // a changed file: the next search is on the new index
    core.wait_for("vnoc/indexChanged");
    let edited = root.join("nested.py");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\n\ndef added_later(): pass\n");
    std::fs::write(&edited, text).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [edited] }));
    core.wait_for("vnoc/indexChanged");
    let found = core
        .request("vnoc/search", json!({ "query": "added_later" }))
        .unwrap();
    assert_eq!(found_ids(&found), ["nested.py::added_later"]);
    assert_eq!(found["nodes"][0]["tier"], 0);
    assert_eq!(
        found["revision"].as_u64().unwrap(),
        structure["revision"].as_u64().unwrap() + 1
    );
    core.finish();
}

/// The ids `vnoc/searchCards` says the query finds, with its revision.
fn search_cards(core: &mut Core, query: &str, ids: &[&str]) -> (Value, Vec<String>) {
    let answer = core
        .request("vnoc/searchCards", json!({ "query": query, "ids": ids }))
        .unwrap();
    let hits = answer["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap().to_string())
        .collect();
    (answer["revision"].clone(), hits)
}

#[test]
fn search_cards_says_which_of_the_asked_ids_the_query_finds() {
    let root = fixtures("search-cards");
    let mut core = Core::start(Some(driver_path()), &root);
    // asked at once, before the first build is done: it answers when it is
    let ids = [
        "nested.py::Outer.method#2",
        "nested.py::Outer",
        "nested.py::Outer.method",
        "nested.py::not.a.node",
    ];
    let (revision, hits) = search_cards(&mut core, "outer.meth", &ids);
    assert_eq!(
        hits,
        ["nested.py::Outer.method#2", "nested.py::Outer.method"],
        "the two methods, in the order asked; not the class, not an id that is made up"
    );
    let found = core
        .request("vnoc/search", json!({ "query": "outer.meth" }))
        .unwrap();
    assert_eq!(revision, found["revision"]);

    // a pasted reference keeps its card bright, as it is `exact` in `vnoc/search`
    let (_, hits) = search_cards(&mut core, "nested.py:25", &ids);
    assert!(
        hits.is_empty(),
        "{hits:?}: that line is in `Outer.Nested.method`"
    );
    let (_, hits) = search_cards(
        &mut core,
        "nested.py:25",
        &["nested.py::Outer.Nested.method"],
    );
    assert_eq!(hits, ["nested.py::Outer.Nested.method"]);

    // nothing typed, and a value that means nothing: nothing is found
    assert!(search_cards(&mut core, "", &ids).1.is_empty());
    assert!(search_cards(&mut core, "kind:fnn outer", &ids).1.is_empty());
    // filters only
    let (_, hits) = search_cards(&mut core, "kind:class", &ids);
    assert_eq!(hits, ["nested.py::Outer"]);
    core.finish();
}

#[test]
fn search_filters_narrow_nodes_and_a_value_that_means_nothing_is_a_note() {
    let root = fixtures("search-filters");
    let mut core = Core::start(Some(driver_path()), &root);

    // a value that means nothing keeps no node and says why; it is not an error
    let found = core
        .request("vnoc/search", json!({ "query": "kind:fnn" }))
        .unwrap();
    assert_eq!(found["nodes"], json!([]), "{found}");
    assert_eq!(found["nodes_total"], 0);
    assert_eq!(
        found["notes"],
        json!(["`kind:fnn` is not a kind (folder, file, class, function, method)"])
    );

    // only filters: every node that passes, in reading order, at tier 0
    let found = core
        .request("vnoc/search", json!({ "query": "kind:method" }))
        .unwrap();
    let rows = found["nodes"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "{found}");
    for row in rows {
        assert_eq!(row["in_class"], true, "{row}");
        assert_eq!(row["tier"], 0, "{row}");
        assert_eq!(row["match"], json!([]), "{row}");
    }
    assert_eq!(found["nodes_total"], 4);
    assert_eq!(found["notes"], json!([]));
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    let mut methods = Vec::new();
    fn walk<'a>(node: &'a Value, parent: &str, out: &mut Vec<&'a str>) {
        for child in node["children"].as_array().unwrap() {
            let kind = child["kind"].as_str().unwrap();
            if kind == "function" && parent == "class" {
                out.push(child["id"].as_str().unwrap());
            }
            walk(child, kind, out);
        }
    }
    walk(&structure["root"], "", &mut methods);
    assert_eq!(found_ids(&found), methods, "the tree's reading order");

    // a filter and a word: the word's tiers, on the nodes that pass
    let found = core
        .request(
            "vnoc/search",
            json!({ "query": "kind:class in:nested.py nested" }),
        )
        .unwrap();
    assert!(!found_ids(&found).is_empty(), "{found}");
    for row in found["nodes"].as_array().unwrap() {
        assert_eq!(row["kind"], "class", "{row}");
        assert_eq!(row["path"], "nested.py", "{row}");
    }
    let found = core
        .request("vnoc/search", json!({ "query": "kind:class in:other/" }))
        .unwrap();
    assert_eq!(found["nodes"], json!([]), "{found}");
    assert_eq!(
        found["notes"],
        json!([]),
        "a filter that keeps nothing is not a note"
    );
    core.finish();
}

/// The first row of a `vnoc/search` answer and how many rows have its id, as `(id, exact, copies)`.
fn first_row(answer: &Value) -> Option<(String, bool, usize)> {
    let rows = answer["nodes"].as_array().unwrap();
    let first = rows.first()?;
    let id = first["id"].as_str().unwrap();
    let copies = rows.iter().filter(|r| r["id"] == id).count();
    Some((id.to_string(), first["exact"].as_bool().unwrap(), copies))
}

/// The ids of the rows that are marked `exact`.
fn exact_ids(answer: &Value) -> Vec<&str> {
    answer["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["exact"] == true)
        .map(|r| r["id"].as_str().unwrap())
        .collect()
}

#[test]
fn a_pasted_reference_finds_its_node_first() {
    let root = fixtures("search-reference");
    let (outer, gone) = ("0000000000000000000000000D", "0000000000000000000000000E");
    let mut core = Core::start(Some(driver_path()), &root);
    let ask = |core: &mut Core, query: &str| {
        core.request("vnoc/search", json!({ "query": query }))
            .unwrap()
    };

    // a file and a line: the innermost def that holds it, else the file (lines are 1-based, as an editor shows them)
    let at = |core: &mut Core, query: &str| {
        let found = core
            .request("vnoc/search", json!({ "query": query }))
            .unwrap();
        (first_row(&found), exact_ids(&found).len())
    };
    let rows: &[(&str, &str)] = &[
        (
            "nested.py:8",
            "nested.py::top.<locals>.inner.<locals>.deepest",
        ),
        ("nested.py:25", "nested.py::Outer.Nested.method"),
        ("nested.py:28", "nested.py::Outer.method#2"),
        ("nested.py:25:9", "nested.py::Outer.Nested.method"),
        ("nested.py:3", "nested.py"),
        ("nested.py:999", "nested.py"),
        ("nested.py:0", "nested.py"),
        ("nested.py::Outer.method#2", "nested.py::Outer.method#2"),
        ("nested.py::Outer.method", "nested.py::Outer.method"),
        ("nested.py::", "nested.py"),
    ];
    for (query, id) in rows {
        let (first, exact_rows) = at(&mut core, query);
        assert_eq!(first, Some((id.to_string(), true, 1)), "{query}");
        assert_eq!(exact_rows, 1, "{query}: only the first row is exact");
    }

    // an absolute path is the same as the path relative to the opened folder
    let absolute = format!("{}:25", root.join("nested.py").display());
    assert_eq!(
        first_row(&ask(&mut core, &absolute)),
        Some(("nested.py::Outer.Nested.method".to_string(), true, 1)),
        "{absolute}"
    );
    let absolute = format!("{}::Outer", root.join("nested.py").display());
    assert_eq!(
        first_row(&ask(&mut core, &absolute)),
        Some(("nested.py::Outer".to_string(), true, 1)),
        "{absolute}"
    );

    // a reference to nothing marks no row, and the words still find what they find
    for query in [
        "nested.py::Nope",
        "other.py:3",
        "../nested.py:3",
        "/not/under/the/folder.py:3",
        "nested.py::Outer.method#9",
    ] {
        let found = ask(&mut core, query);
        assert_eq!(exact_ids(&found), Vec::<&str>::new(), "{query}");
    }
    // as words nothing holds it either: no rows, and no crash
    assert_eq!(ask(&mut core, "nested.py::Nope")["nodes"], json!([]));

    // a bare path is not tried as a reference: the path tiers find it, none is exact
    let found = ask(&mut core, "nested.py");
    assert!(!found_ids(&found).is_empty());
    assert_eq!(exact_ids(&found), Vec::<&str>::new());

    // an ID: a record anchored at `nested.py::Outer`
    write_record(&root, outer, "nested.py", "Outer");
    write_record(&root, gone, "nested.py", "Gone");
    tell_core_about_vnoc(&mut core, &root);
    structure_when(&mut core, |s| {
        fn has(node: &Value, id: &str) -> bool {
            node["node_id"] == id
                || node["children"]
                    .as_array()
                    .is_some_and(|c| c.iter().any(|n| has(n, id)))
        }
        has(&s["root"], outer)
            && s["missing"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"] == gone)
    });
    for text in [outer.to_string(), outer.to_lowercase()] {
        let found = ask(&mut core, &text);
        assert_eq!(
            first_row(&found),
            Some(("nested.py::Outer".to_string(), true, 1)),
            "{text}"
        );
    }
    // a Missing ID marks no row, and does not crash; the core still answers
    for text in [gone.to_string(), gone.to_lowercase()] {
        let found = ask(&mut core, &text);
        assert_eq!(exact_ids(&found), Vec::<&str>::new(), "{text}");
    }
    // an ID nobody knows
    let found = ask(&mut core, "0000000000000000000000000Z");
    assert_eq!(exact_ids(&found), Vec::<&str>::new());

    // a reference with a filter: the node is first though the filter would drop it (04 §4, 01 §3)
    let found = ask(&mut core, "nested.py:25 kind:fn");
    assert_eq!(
        exact_ids(&found),
        Vec::<&str>::new(),
        "two tokens are not a reference"
    );
    core.finish();
}

/// `pkg/nested.py` is a file, `pkg/` a folder: a path with a `/` is a reference, so the file or the folder is first, and exact.
#[test]
fn a_pasted_path_puts_its_file_or_folder_first() {
    let root = fixtures("search-bare-path");
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::rename(root.join("nested.py"), root.join("pkg/nested.py")).unwrap();
    let mut core = Core::start(Some(driver_path()), &root);
    let mut ask = |query: &str| {
        let found = core
            .request("vnoc/search", json!({ "query": query }))
            .unwrap();
        (first_row(&found), exact_ids(&found).len())
    };
    for (query, id) in [
        ("pkg/nested.py", "pkg/nested.py"),
        ("./pkg/nested.py", "pkg/nested.py"),
        ("pkg/", "pkg"),
        ("pkg/nested.py#L25", "pkg/nested.py::Outer.Nested.method"),
        ("pkg/nested.py:L25", "pkg/nested.py::Outer.Nested.method"),
        (
            "pkg/nested.py#L25-L28",
            "pkg/nested.py::Outer.Nested.method",
        ),
        (
            "File \"pkg/nested.py\", line 25, in method",
            "pkg/nested.py::Outer.Nested.method",
        ),
    ] {
        let (first, exact_rows) = ask(query);
        assert_eq!(first, Some((id.to_string(), true, 1)), "{query}");
        assert_eq!(exact_rows, 1, "{query}");
    }
    // a path that names nothing marks no row
    for query in ["pkg/none.py", "nope/", "pkg/nested.py#Lx"] {
        assert_eq!(ask(query).1, 0, "{query}");
    }
    core.finish();
}

/// A folder below the git root is open: the pasted path is tried relative to it, as a repo path, and with its name dropped; a file name alone
/// needs exactly one file of that name.
#[test]
fn a_pasted_path_is_tried_against_each_base_then_a_file_name() {
    let repo = fixtures("search-bases");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let svc = repo.join("svc");
    std::fs::create_dir_all(svc.join("pkg")).unwrap();
    std::fs::create_dir_all(svc.join("other")).unwrap();
    std::fs::rename(repo.join("nested.py"), svc.join("pkg/nested.py")).unwrap();
    std::fs::copy(svc.join("pkg/nested.py"), svc.join("other/nested.py")).unwrap();
    std::fs::write(svc.join("pkg/only_here.py"), "def solo():\n    pass\n").unwrap();
    let mut core = Core::start(Some(driver_path()), &svc);
    let mut ask = |query: &str| {
        let found = core
            .request("vnoc/search", json!({ "query": query }))
            .unwrap();
        (first_row(&found), exact_ids(&found).len())
    };
    let method = "svc/pkg/nested.py::Outer.Nested.method";
    let rows = [
        // relative to the opened folder
        ("pkg/nested.py:25", method),
        // as a repo path, the tree's own ids
        ("svc/pkg/nested.py:25", method),
        ("svc/pkg/nested.py#L25", method),
        ("svc/pkg/nested.py::Outer.Nested.method", method),
        ("svc/pkg/nested.py::Outer", "svc/pkg/nested.py::Outer"),
        ("svc/pkg/nested.py", "svc/pkg/nested.py"),
        ("svc/pkg/", "svc/pkg"),
        // a file name alone, when only one file has it (a bare name is no reference in the box: the path tiers find it)
        ("only_here.py:2", "svc/pkg/only_here.py::solo"),
        ("only_here.py::solo", "svc/pkg/only_here.py::solo"),
    ];
    for (query, id) in rows {
        let (first, exact_rows) = ask(query);
        assert_eq!(first, Some((id.to_string(), true, 1)), "{query}");
        assert_eq!(exact_rows, 1, "{query}");
    }
    // two files have the name: nothing is exact, the words still find
    for query in ["nested.py:25", "nested.py::Outer"] {
        assert_eq!(ask(query).1, 0, "{query}");
    }
    core.finish();
}

/// A reference pasted with the opened folder's own name in front (no git folder, so the root is the opened folder and only that try can find it).
#[test]
fn a_pasted_path_may_start_with_the_opened_folders_name() {
    let root = fixtures("search-folder-name");
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::rename(root.join("nested.py"), root.join("pkg/nested.py")).unwrap();
    let name = root.file_name().unwrap().to_str().unwrap().to_string();
    let mut core = Core::start(Some(driver_path()), &root);
    for (query, id) in [
        (
            format!("{name}/pkg/nested.py:25"),
            "pkg/nested.py::Outer.Nested.method",
        ),
        (
            format!("{name}/pkg/nested.py::Outer"),
            "pkg/nested.py::Outer",
        ),
        (format!("{name}/pkg/nested.py"), "pkg/nested.py"),
        (format!("{name}/pkg/"), "pkg"),
    ] {
        let found = core
            .request("vnoc/search", json!({ "query": query }))
            .unwrap();
        assert_eq!(
            first_row(&found),
            Some((id.to_string(), true, 1)),
            "{query}"
        );
    }
    core.finish();
}

/// Lines of `nested.py` that are not a statement of a body, and what the driver's ranges make of them. The rule is the plan's (the innermost def
/// whose range holds the line, else the file), not bent: a def's range starts at its decorator and ends at its last line, so a blank line
/// inside a def belongs to it, and one between two defs, or an `else:`, to the file.
#[test]
fn a_line_that_is_not_in_a_body_gives_what_the_ranges_hold() {
    let root = fixtures("search-reference-lines");
    let mut core = Core::start(Some(driver_path()), &root);
    let rows: &[(u32, &str, &str)] = &[
        (1, "nested.py", "the docstring, before any def"),
        (4, "nested.py", "a blank line before the first def"),
        (5, "nested.py::top", "a `def` line is its def's"),
        (
            9,
            "nested.py::top.<locals>.inner",
            "a blank line inside `inner`, after `deepest` ended",
        ),
        (
            11,
            "nested.py::top",
            "a blank line inside `top`, after `inner` ended",
        ),
        (
            17,
            "nested.py",
            "a blank line after `top` ended, before `Outer`",
        ),
        (
            27,
            "nested.py::Outer.method#2",
            "the line with a comment after the def",
        ),
        (29, "nested.py", "a blank line after `Outer` ended"),
        (31, "nested.py::top#2", "the second `top`"),
        (35, "nested.py", "an `if` line holds no def"),
        (36, "nested.py::cond", "the first twin"),
        (38, "nested.py", "an `else:` line holds no def"),
        (39, "nested.py::cond#2", "the second twin"),
        (42, "nested.py", "a blank line before a decorated def"),
        (
            43,
            "nested.py::decorated",
            "the decorator's line: the range starts there",
        ),
        (
            44,
            "nested.py::decorated",
            "the `def` line under the decorator",
        ),
        (46, "nested.py", "a blank line after `decorated`"),
        (48, "nested.py::coro", "an `async def`"),
        (49, "nested.py::coro", "the last line of the file"),
    ];
    for (line, id, why) in rows {
        let found = core
            .request(
                "vnoc/search",
                json!({ "query": format!("nested.py:{line}") }),
            )
            .unwrap();
        assert_eq!(
            first_row(&found),
            Some((id.to_string(), true, 1)),
            "nested.py:{line}: {why}"
        );
    }
    core.finish();
}

#[test]
fn a_missing_driver_is_a_clear_error_and_the_core_stays_up() {
    let mut core = Core::start(None, &fixtures("nodriver"));
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("no driver is configured"),
        "{e}"
    );
    core.finish();

    let mut core = Core::start(
        Some(PathBuf::from("/definitely/not/a/driver")),
        &fixtures("baddriver"),
    );
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("is not at /definitely/not/a/driver"),
        "{e}"
    );
    assert_eq!(
        core.request("vnoc/ping", Value::Null).unwrap()["pong"],
        true,
        "still alive"
    );
    core.finish();
}

#[test]
fn a_folder_that_does_not_exist_is_an_error_not_a_crash() {
    let mut core = Core::start(
        Some(driver_path()),
        std::path::Path::new("/definitely/not/a/folder"),
    );
    let e = core.request("vnoc/structure", Value::Null).unwrap_err();
    assert!(
        e["message"].as_str().unwrap().contains("not a folder"),
        "{e}"
    );
    assert_eq!(
        core.request("vnoc/ping", Value::Null).unwrap()["pong"],
        true
    );
    core.finish();
}

const ORDERS: &str = "class OrderService:\n    def parse_order(self, raw):\n        return raw\n\n    def other(self):\n        pass\n";

/// A project with one file, `orders.py`, opened in a core that is ready.
fn orders_project(label: &str, vnoc_toml: Option<&str>) -> (Core, PathBuf) {
    let root = std::env::temp_dir().join(format!("vnoc-attach-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
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
    json!({ "key": key, "title": title, "markers": markers, "document": { "uri": file_uri(&file), "version": 3, "text": std::fs::read_to_string(&file).unwrap() } })
}

#[test]
fn attaching_a_doc_writes_the_id_the_node_and_the_doc() {
    let (mut core, root) = orders_project("full", None);
    assert_eq!(
        core.request("vnoc/projectInfo", Value::Null).unwrap(),
        json!({ "initialized": false, "markers": true })
    );

    let key = "orders.py::OrderService.parse_order";
    let r = core
        .request(
            "vnoc/attach",
            attach_params(&root, key, "Why we parse twice", Some(true)),
        )
        .expect("attach answers");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!(
        (r["id_in_code"].as_bool(), r["notice"].is_null()),
        (Some(true), true)
    );

    // the editor was asked once, for a versioned edit of that document
    assert_eq!(core.edit_requests.len(), 1);
    assert_eq!(
        core.edit_requests[0]["edit"]["documentChanges"][0]["textDocument"]["version"],
        3
    );
    // the ID line is in the code, in the docstring that was added
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    assert!(
        code.contains(&format!(
            "def parse_order(self, raw):\n        \"\"\"ID: {id}\"\"\"\n"
        )),
        "{code}"
    );

    // .vnoc/ was set up and the node and doc are where the plan says
    let (shard, node) = (
        &id[24..],
        root.join(".vnoc").join("nodes").join(&id[24..]).join(&id),
    );
    assert!(
        root.join(".vnoc/vnoc.toml").is_file() && root.join(".vnoc/.gitignore").is_file(),
        "{shard}"
    );
    let toml = std::fs::read_to_string(node.join("node.toml")).unwrap();
    assert!(
        toml.contains("path = \"orders.py\"")
            && toml.contains("qualname = \"OrderService.parse_order\"")
            && toml.contains("id_in_code = \"yes\"")
            && toml.contains("kind = \"function\""),
        "{toml}"
    );
    assert_eq!(
        std::path::Path::new(r["doc"].as_str().unwrap()),
        node.join("why-we-parse-twice.md")
    );
    assert!(
        std::fs::read_to_string(node.join("why-we-parse-twice.md"))
            .unwrap()
            .starts_with(&format!(
                "---\nnode: {id}\nhint: {key}\n---\n# Why we parse twice"
            ))
    );
    assert_eq!(
        core.request("vnoc/projectInfo", Value::Null).unwrap(),
        json!({ "initialized": true, "markers": true })
    );

    // a second doc on the same function: same ID, no new edit, no new question
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    core.wait_for("vnoc/indexChanged");
    let second = core
        .request("vnoc/attach", attach_params(&root, key, "Edge cases", None))
        .unwrap();
    assert_eq!(second["id"], json!(id));
    assert_eq!(core.edit_requests.len(), 1);
    assert!(second["doc"].as_str().unwrap().ends_with("edge-cases.md"));
    core.finish();
}

/// `vnoc tour …` in the project (the command line, as another process: an agent writing a tour while the core is open), with `stdin` for its body.
fn tour_command(root: &std::path::Path, args: &[&str], stdin: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc"))
        .current_dir(root)
        .env_remove("VNOC_DRIVER")
        .env_remove("VNOC_DRIVERS")
        .arg("--driver")
        .arg(driver_path())
        .arg("tour")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let out = child.wait_with_output().unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), said)
}

#[test]
fn an_attach_through_the_editor_tells_the_tours_of_the_file_where_their_lines_went() {
    // The editor's sink: the core asks the editor to apply the edit (here a stand-in that writes the file), and after it the tours with a stop in
    // that file get a `rebase` record, so the ID line the core just wrote does not make them stale. (`tests/cli.rs` covers the disk's sink.)
    let (mut core, root) = orders_project("rebase", Some("format = 1\n"));
    let (ok, said) = tour_command(
        &root,
        &[
            "start",
            "--title",
            "orders",
            "--question",
            "what do orders do",
            "--json",
        ],
        "",
    );
    assert!(ok, "{said}");
    let started: Value = serde_json::from_str(&said).unwrap();
    let tour = started["tour"].as_str().unwrap().to_string();
    for (node, text) in [
        (
            "orders.py::OrderService.parse_order",
            "parse_order hands back the raw request unchanged, for now.\n",
        ),
        (
            "orders.py::OrderService.other",
            "other does nothing yet, and is here so there is a second stop after the first.\n",
        ),
    ] {
        let (ok, said) = tour_command(&root, &["add", &tour, node, "--body-file", "-"], text);
        assert!(ok, "{node}: {said}");
    }
    assert!(tour_command(&root, &["check"], "").1.contains("all fresh"));

    let key = "orders.py::OrderService.parse_order";
    core.request("vnoc/attach", attach_params(&root, key, "Why", Some(true)))
        .expect("attach answers");
    assert_eq!(core.edit_requests.len(), 1, "the editor applied the edit");
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    assert_eq!(code.lines().count(), ORDERS.lines().count() + 1, "{code}");

    let file =
        std::fs::read_to_string(root.join(format!(".vnoc/local/tours/{tour}.jsonl"))).unwrap();
    let rebase: Value = serde_json::from_str(
        file.lines()
            .find(|l| l.contains(r#""type":"rebase""#))
            .expect("a rebase record"),
    )
    .unwrap();
    // `parse_order` 2-3 is 2-4 now; `other` moved down by one: 5-6 is 6-7
    assert_eq!(rebase["steps"]["s1"]["lines"], json!([2, 4]));
    assert_eq!(rebase["steps"]["s2"]["lines"], json!([6, 7]));
    let (ok, said) = tour_command(&root, &["check", "--again"], "");
    assert!(ok && said.contains("all fresh"), "{said}");
    core.finish();
}

#[test]
fn a_marker_that_cannot_be_written_does_not_stop_the_attach() {
    let (mut core, root) = orders_project("refused", None);
    core.editor_applies_edits = false; // the document changed meanwhile, or the user undid it
    let r = core
        .request(
            "vnoc/attach",
            attach_params(&root, "orders.py::OrderService.other", "Notes", Some(true)),
        )
        .unwrap();
    assert_eq!(r["id_in_code"], false);
    assert!(
        r["notice"].as_str().unwrap().contains("attached by name"),
        "{r}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS,
        "the code is untouched"
    );
    let node = root
        .join(".vnoc/nodes")
        .join(&r["id"].as_str().unwrap()[24..])
        .join(r["id"].as_str().unwrap());
    assert!(
        std::fs::read_to_string(node.join("node.toml"))
            .unwrap()
            .contains("id_in_code = \"no\"")
    );
    assert!(node.join("notes.md").is_file());
    core.finish();
}

#[test]
fn a_project_set_to_no_ids_never_touches_the_code() {
    let (mut core, root) = orders_project("none", Some("format = 1\n[ids]\ndefault = \"none\"\n"));
    let r = core
        .request(
            "vnoc/attach",
            attach_params(&root, "orders.py::OrderService.other", "Notes", None),
        )
        .unwrap();
    assert_eq!(
        (r["id_in_code"].as_bool(), r["notice"].is_null()),
        (Some(false), true),
        "off is a choice, not a failure"
    );
    assert!(core.edit_requests.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS
    );
    core.finish();
}

#[test]
fn attach_refuses_what_is_not_in_the_index_or_not_that_document() {
    let (mut core, root) = orders_project("refuse", None);
    let e = core
        .request(
            "vnoc/attach",
            attach_params(&root, "orders.py::Nope", "x", None),
        )
        .unwrap_err();
    assert!(
        e["message"].as_str().unwrap().contains("not in the index"),
        "{e}"
    );
    let mut wrong = attach_params(&root, "orders.py::OrderService.other", "x", None);
    wrong["document"]["uri"] = json!("file:///somewhere/else.py");
    assert!(
        core.request("vnoc/attach", wrong).unwrap_err()["message"]
            .as_str()
            .unwrap()
            .contains("not the file")
    );
    assert!(
        !root.join(".vnoc").exists(),
        "a refused attach writes nothing"
    );
    core.finish();
}

/// The first node of a tree of `orders.py`: the file's children, by name.
fn def_named<'a>(structure: &'a Value, name: &str) -> &'a Value {
    structure["root"]["children"][0]["children"]
        .as_array()
        .and_then(|nodes| {
            nodes
                .iter()
                .flat_map(|n| {
                    std::iter::once(n).chain(n["children"].as_array().into_iter().flatten())
                })
                .find(|n| n["name"] == name)
        })
        .unwrap_or_else(|| panic!("no node `{name}` in {structure}"))
}

#[test]
fn a_documented_node_is_found_again_after_a_rename_and_a_move() {
    let (mut core, root) = orders_project("refind", None);
    let key = "orders.py::OrderService.parse_order";
    core.request(
        "vnoc/attach",
        attach_params(&root, key, "Why we parse twice", Some(true)),
    )
    .unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );

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
    assert_eq!(
        (docs[0]["file"].as_str(), docs[0]["title"].as_str()),
        (Some("why-we-parse-twice.md"), Some("Why we parse twice"))
    );

    // rename the function AND move it to another file; the ID line travels with it
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    let moved = code.replace("parse_order", "parse_payload");
    std::fs::write(
        root.join("orders.py"),
        ORDERS.replace(
            "    def parse_order(self, raw):\n        return raw\n\n",
            "",
        ),
    )
    .unwrap();
    let body_start = moved.find("    def parse_payload").unwrap();
    let body_end = moved.find("    def other").unwrap();
    std::fs::write(
        root.join("payments.py"),
        format!("class Payments:\n{}", &moved[body_start..body_end]),
    )
    .unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py"), root.join("payments.py")] }),
    );
    let after = loop {
        core.wait_for("vnoc/indexChanged");
        let s = core.request("vnoc/structure", Value::Null).unwrap();
        if s["stats"]["files"] == 2 {
            break s;
        }
    };
    let found =
        after["root"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|f| {
                f["children"].as_array().unwrap().iter().flat_map(|c| {
                    std::iter::once(c).chain(c["children"].as_array().unwrap().iter())
                })
            })
            .find(|n| n["node_id"] == json!(id))
            .expect("the node is still found");
    assert_eq!(
        (
            found["name"].as_str(),
            found["path"].as_str(),
            found["link"].as_str(),
            found["docs"].as_u64()
        ),
        (
            Some("parse_payload"),
            Some("payments.py"),
            Some("marker"),
            Some(1)
        )
    );
    core.finish();
}

/// The first node in the tree (any depth) that `pred` accepts.
fn find_node<'a>(node: &'a Value, pred: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
    if pred(node) {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find_node(child, pred))
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
    let r = core
        .request(
            "vnoc/attach",
            attach_params(&root, "orders.py", "About orders", Some(true)),
        )
        .expect("attach answers");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!(
        (r["id_in_code"].as_bool(), r["notice"].is_null()),
        (Some(true), true)
    );
    assert_eq!(core.edit_requests.len(), 1, "one edit of the open document");
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        format!("# FileID: {id}\n{ORDERS}")
    );

    let node = root.join(".vnoc/nodes").join(&id[24..]).join(&id);
    let toml = std::fs::read_to_string(node.join("node.toml")).unwrap();
    assert!(
        toml.contains("path = \"orders.py\"")
            && toml.contains("qualname = \"\"")
            && toml.contains("id_in_code = \"yes\"")
            && toml.contains("kind = \"file\""),
        "{toml}"
    );
    assert!(
        std::fs::read_to_string(node.join("about-orders.md"))
            .unwrap()
            .contains("hint: orders.py\n")
    );

    // saved: the tree shows the file with its doc, found by its marker
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    let shown = structure_when(&mut core, |s| s["root"]["children"][0]["link"] == "marker");
    assert_eq!(
        (
            shown["root"]["children"][0]["node_id"].as_str(),
            shown["root"]["children"][0]["docs"].as_u64()
        ),
        (Some(id.as_str()), Some(1))
    );

    // renamed in the terminal: the comment travels with the file
    std::fs::rename(root.join("orders.py"), root.join("billing.py")).unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py"), root.join("billing.py")] }),
    );
    let after = structure_when(&mut core, |s| {
        s["root"]["children"][0]["name"] == "billing.py"
    });
    let file = &after["root"]["children"][0];
    assert_eq!(
        (
            file["node_id"].as_str(),
            file["link"].as_str(),
            file["docs"].as_u64()
        ),
        (Some(id.as_str()), Some("marker"), Some(1))
    );
    core.finish();
}

#[test]
fn a_folder_gets_a_marker_file_and_is_found_again_after_a_move() {
    let (mut core, root) = orders_project("folder", None);
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(root.join("pkg/m.py"), "def f():\n    pass\n").unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("pkg/m.py")] }),
    );
    structure_when(&mut core, |s| s["stats"]["files"] == 2);

    let r = core
        .request(
            "vnoc/attach",
            json!({ "key": "pkg", "title": "The pkg folder", "markers": true }),
        )
        .expect("a folder needs no document");
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!(
        (r["id_in_code"].as_bool(), r["notice"].is_null()),
        (Some(true), true)
    );
    assert!(
        core.edit_requests.is_empty(),
        "the core writes the marker itself"
    );
    let marker = std::fs::read_to_string(root.join("pkg/.vnoc-id")).unwrap();
    assert!(marker.contains(&format!("id = \"{id}\"")), "{marker}");
    let node = root.join(".vnoc/nodes").join(&id[24..]).join(&id);
    assert!(
        std::fs::read_to_string(node.join("node.toml"))
            .unwrap()
            .contains("path = \"pkg\"")
            && std::fs::read_to_string(node.join("node.toml"))
                .unwrap()
                .contains("kind = \"folder\"")
    );

    let shown = structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["id"] == "pkg").is_some_and(|n| n["link"] == "marker")
    });
    assert_eq!(
        find_node(&shown["root"], &|n| n["id"] == "pkg").unwrap()["docs"],
        1
    );

    // moved and renamed in the terminal; the marker file went along inside it
    std::fs::create_dir_all(root.join("core")).unwrap();
    std::fs::rename(root.join("pkg"), root.join("core/pkg2")).unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("core/pkg2/.vnoc-id")] }),
    );
    let after = structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["id"] == "core/pkg2").is_some()
    });
    let folder = find_node(&after["root"], &|n| n["id"] == "core/pkg2").unwrap();
    assert_eq!(
        (
            folder["node_id"].as_str(),
            folder["link"].as_str(),
            folder["docs"].as_u64()
        ),
        (Some(id.as_str()), Some("marker"), Some(1))
    );

    // a second doc on the same folder reuses its ID and does not touch the marker
    let second = core
        .request(
            "vnoc/attach",
            json!({ "key": "core/pkg2", "title": "More" }),
        )
        .unwrap();
    assert_eq!(second["id"], json!(id));
    core.finish();
}

#[test]
fn a_folder_marker_that_cannot_be_read_is_never_overwritten() {
    let (mut core, root) = orders_project("badmarker", None);
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::write(root.join("pkg/m.py"), "x = 1\n").unwrap();
    std::fs::write(root.join("pkg/.vnoc-id"), "id = [").unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("pkg/m.py")] }),
    );
    structure_when(&mut core, |s| s["stats"]["files"] == 2);

    let r = core
        .request(
            "vnoc/attach",
            json!({ "key": "pkg", "title": "Notes", "markers": true }),
        )
        .unwrap();
    assert_eq!(r["id_in_code"], false);
    assert!(r["notice"].as_str().unwrap().contains("cannot read"), "{r}");
    assert_eq!(
        std::fs::read_to_string(root.join("pkg/.vnoc-id")).unwrap(),
        "id = [",
        "the unreadable file is left as it was"
    );
    core.finish();
}

#[test]
fn with_ids_off_a_file_is_attached_by_path_and_goes_missing_when_renamed() {
    let (mut core, root) =
        orders_project("filenone", Some("format = 1\n[ids]\ndefault = \"none\"\n"));
    let r = core
        .request(
            "vnoc/attach",
            attach_params(&root, "orders.py", "Notes", None),
        )
        .unwrap();
    assert_eq!(
        (r["id_in_code"].as_bool(), r["notice"].is_null()),
        (Some(false), true)
    );
    assert!(core.edit_requests.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS
    );

    core.notify("vnoc/filesChanged", json!({ "paths": [] }));
    let shown = structure_when(&mut core, |s| s["root"]["children"][0]["link"] == "name");
    assert_eq!(shown["root"]["children"][0]["docs"], 1, "found by its path");

    std::fs::rename(root.join("orders.py"), root.join("billing.py")).unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py"), root.join("billing.py")] }),
    );
    let after = structure_when(&mut core, |s| {
        s["root"]["children"][0]["name"] == "billing.py"
    });
    assert_eq!(
        after["root"]["children"][0]["node_id"],
        Value::Null,
        "no marker, so nothing follows the rename: it is Missing"
    );
    core.finish();
}

#[test]
fn attach_to_a_file_needs_its_document() {
    let (mut core, _root) = orders_project("nodocument", None);
    let e = core
        .request("vnoc/attach", json!({ "key": "orders.py", "title": "x" }))
        .unwrap_err();
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("needs the open document"),
        "{e}"
    );
    core.finish();
}

/// Takes away a file's read permission, so that the driver cannot read it. (A file that is not UTF-8 is read now: the drivers read bad bytes as U+FFFD, as the
/// core does, docs/23 B4c.) Unix only: the callers do it under `cfg!(unix)`.
#[cfg(unix)]
fn lock(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
}

#[cfg(not(unix))]
fn lock(_: &Path) {}

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
    testdir::remove_at_end(&root);
    std::fs::create_dir_all(root.join("ignored")).unwrap();
    std::fs::write(root.join("orders.py"), ORDERS).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::write(root.join("ignored/x.py"), "def f():\n    pass\n").unwrap();
    // a file the driver cannot read (no read permission; unix only: a file that is not UTF-8 is read now, bad bytes as U+FFFD)
    std::fs::write(root.join("bad.py"), "def g():\n    pass\n").unwrap();
    if cfg!(unix) {
        lock(&root.join("bad.py"));
    }
    let (gone, ignored, failed) = (
        "0000000000000000000000000A",
        "0000000000000000000000000B",
        "0000000000000000000000000C",
    );
    write_record(&root, gone, "gone.py", "parse_order");
    write_record(&root, ignored, "ignored/x.py", "f");
    write_record(&root, failed, "bad.py", "g");
    std::fs::create_dir_all(root.join(".vnoc/nodes/zz/not-an-id")).unwrap();

    let mut core = Core::start(Some(driver_path()), &root);
    let s = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(
        s["failed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        if cfg!(unix) { vec!["bad.py"] } else { vec![] },
        "the driver failed on it"
    );
    assert_eq!(
        s["missing"],
        json!([{ "id": gone, "kind": "function", "path": "gone.py", "qualname": "parse_order", "docs": 1 }]),
        "the others were not looked for"
    );
    let problems = s["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(
        problems[0]["text"],
        ".vnoc/nodes/zz/not-an-id: the folder name is not a node ID"
    );
    assert_eq!(
        problems[0]["file"],
        json!(
            root.join(".vnoc")
                .join("nodes")
                .join("zz")
                .join("not-an-id")
        )
    );
    assert_eq!(problems[0]["line"], Value::Null);

    // the ignored file goes: now the name was looked for, and the node is Missing
    std::fs::remove_file(root.join("ignored/x.py")).unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("ignored/x.py")] }),
    );
    let after = structure_when(&mut core, |s| s["missing"].as_array().unwrap().len() == 2);
    assert_eq!(
        after["missing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["gone.py", "ignored/x.py"]
    );
    core.finish();
}

const R: &str = "0000000000000000000000000R";
const S: &str = "0000000000000000000000000S";

fn relink_params(root: &std::path::Path, id: &str, to: &str) -> Value {
    let file = root.join("orders.py");
    json!({ "id": id, "to": to, "document": { "uri": file_uri(&file), "version": 4, "text": std::fs::read_to_string(&file).unwrap() } })
}

/// The structure once it lists exactly these Missing IDs.
fn missing_is(core: &mut Core, ids: &[&str]) -> Value {
    structure_when(core, |s| {
        s["missing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect::<Vec<_>>()
            == ids
    })
}

fn tell_core_about_vnoc(core: &mut Core, root: &std::path::Path) {
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join(".vnoc/nodes")] }),
    );
}

fn message(error: Result<Value, Value>) -> String {
    error.expect_err("the core refuses")["message"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn relink_writes_the_id_at_the_new_place_then_moves_the_node_and_keeps_what_it_does_not_know() {
    let (mut core, root) = orders_project("relink", Some("format = 1\n"));
    write_record(&root, R, "gone.py", "old");
    let toml = root
        .join(".vnoc/nodes")
        .join(&R[24..])
        .join(R)
        .join("node.toml");
    std::fs::write(
        &toml,
        format!("{}future = 7\n", std::fs::read_to_string(&toml).unwrap()),
    )
    .unwrap();
    tell_core_about_vnoc(&mut core, &root);
    missing_is(&mut core, &[R]);

    let r = core
        .request(
            "vnoc/relink",
            relink_params(&root, R, "orders.py::OrderService.other"),
        )
        .expect("relink answers");
    assert_eq!(
        (r["written"].as_str(), r["notice"].is_null()),
        (Some("id"), true)
    );
    assert_eq!(
        core.edit_requests.len(),
        1,
        "the editor was asked once, as for attach"
    );
    assert!(
        std::fs::read_to_string(root.join("orders.py"))
            .unwrap()
            .contains(&format!("def other(self):\n        \"\"\"ID: {R}\"\"\""))
    );
    let text = std::fs::read_to_string(&toml).unwrap();
    assert!(
        text.contains("path = \"orders.py\"")
            && text.contains("qualname = \"OrderService.other\"")
            && text.contains("kind = \"function\"")
            && text.contains("id_in_code = \"yes\"")
            && text.contains("future = 7"),
        "{text}"
    );

    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    // found by name as soon as node.toml moved; by its ID once the saved file is read again
    let s = structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["name"] == "other").is_some_and(|n| n["link"] == "marker")
    });
    assert!(s["missing"].as_array().unwrap().is_empty());
    let other = find_node(&s["root"], &|n| n["name"] == "other").unwrap();
    assert_eq!(
        (
            other["node_id"].as_str(),
            other["link"].as_str(),
            other["docs"].as_u64()
        ),
        (Some(R), Some("marker"), Some(1))
    );
    core.finish();
}

#[test]
fn relink_refuses_a_place_with_another_nodes_id_a_node_found_by_name_the_wrong_kind_and_a_node_that_is_not_missing()
 {
    let (mut core, root) = orders_project("refuse", None);
    // X: a documented function with its ID in the code. S: a node found by name at `other`. R: Missing.
    core.request(
        "vnoc/attach",
        attach_params(
            &root,
            "orders.py::OrderService.parse_order",
            "Why",
            Some(true),
        ),
    )
    .unwrap();
    write_record(&root, S, "orders.py", "OrderService.other");
    write_record(&root, R, "gone.py", "old");
    tell_core_about_vnoc(&mut core, &root);
    missing_is(&mut core, &[R]);
    let code = std::fs::read_to_string(root.join("orders.py")).unwrap();
    let edits = core.edit_requests.len();

    let ask = |core: &mut Core, id: &str, to: &str| {
        message(core.request("vnoc/relink", relink_params(&root, id, to)))
    };
    assert_eq!(
        ask(&mut core, R, "orders.py::OrderService.parse_order"),
        "that node already has its own docs",
        "the place carries another ID"
    );
    assert_eq!(
        ask(&mut core, R, "orders.py::OrderService.other"),
        "that node already has its own docs",
        "found by name: no ID in the code to see"
    );
    assert_eq!(
        ask(&mut core, R, "orders.py"),
        "this is a file, and the docs belong to a function"
    );
    assert!(ask(&mut core, S, "orders.py::OrderService").contains("not Missing"));
    assert!(ask(&mut core, R, "nowhere.py::f").contains("not in the index"));

    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        code,
        "a refusal writes nothing into the code"
    );
    assert_eq!(core.edit_requests.len(), edits);
    let record = std::fs::read_to_string(
        root.join(".vnoc/nodes")
            .join(&R[24..])
            .join(R)
            .join("node.toml"),
    )
    .unwrap();
    assert!(
        record.contains("path = \"gone.py\""),
        "and nothing in .vnoc/: {record}"
    );
    core.finish();
}

#[test]
fn relink_with_ids_off_moves_only_the_stored_place() {
    let (mut core, root) =
        orders_project("relinkoff", Some("format = 1\n[ids]\ndefault = \"none\"\n"));
    write_record(&root, R, "gone.py", "old");
    tell_core_about_vnoc(&mut core, &root);
    missing_is(&mut core, &[R]);
    let r = core
        .request(
            "vnoc/relink",
            relink_params(&root, R, "orders.py::OrderService.other"),
        )
        .unwrap();
    assert_eq!(
        (r["written"].as_str(), r["notice"].is_null()),
        (Some("anchorOnly"), true),
        "off is a choice, not a failure"
    );
    assert!(core.edit_requests.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS
    );
    assert!(
        std::fs::read_to_string(
            root.join(".vnoc/nodes")
                .join(&R[24..])
                .join(R)
                .join("node.toml")
        )
        .unwrap()
        .contains("id_in_code = \"no\"")
    );
    core.finish();
}

const T: &str = "0000000000000000000000000T";

fn write_id_params(root: &std::path::Path, key: &str) -> Value {
    let file = root.join("orders.py");
    json!({ "key": key, "document": { "uri": file_uri(&file), "version": 5, "text": std::fs::read_to_string(&file).unwrap() } })
}

/// The structure once `name` is drawn with this `link`.
fn link_of_is(core: &mut Core, name: &str, link: &str) -> Value {
    structure_when(core, |s| {
        find_node(&s["root"], &|n| n["name"] == name).is_some_and(|n| n["link"] == link)
    })
}

fn locate_params(root: &std::path::Path, key: &str, text: &str) -> Value {
    let file = root.join("orders.py");
    json!({ "key": key, "document": { "uri": file_uri(&file), "version": 9, "text": text } })
}

/// The first line of the range `vnoc/locate` answered, or `None` for `null`.
fn located_line(answer: &Value) -> Option<u64> {
    answer["range"]["start"]["line"].as_u64()
}

/// `vnoc/locate` (docs/17-edit-in-canvas.md §6, §11 b): where a node is in the text the editor holds now.
#[test]
fn locate_finds_a_function_where_it_moved_by_name_and_says_null_when_it_is_gone() {
    let (mut core, root) = orders_project("locate-name", None);
    let key = "orders.py::OrderService.parse_order";

    // the same text: the node is where the index has it, on the second line
    let same = core
        .request("vnoc/locate", locate_params(&root, key, ORDERS))
        .unwrap();
    assert_eq!(located_line(&same), Some(1), "{same}");

    // ten lines were added above: it is ten lines down, and the range is as long as it was
    let moved = format!("{}{}", "# pad\n".repeat(10), ORDERS);
    let down = core
        .request("vnoc/locate", locate_params(&root, key, &moved))
        .unwrap();
    assert_eq!(located_line(&down), Some(11), "{down}");
    assert_eq!(
        down["range"]["end"]["line"].as_u64().unwrap() - 11,
        same["range"]["end"]["line"].as_u64().unwrap() - 1
    );

    // a sibling is found by its own name, not by the order of the defs
    let other = core
        .request(
            "vnoc/locate",
            locate_params(&root, "orders.py::OrderService.other", &moved),
        )
        .unwrap();
    assert_eq!(located_line(&other), Some(14), "{other}");

    // deleted: null
    let deleted = "class OrderService:\n    def other(self):\n        pass\n";
    assert_eq!(
        core.request("vnoc/locate", locate_params(&root, key, deleted))
            .unwrap(),
        Value::Null
    );
    // a key the index never had is looked for by name too
    assert_eq!(
        core.request(
            "vnoc/locate",
            locate_params(&root, "orders.py::OrderService.nope", ORDERS)
        )
        .unwrap(),
        Value::Null
    );
    core.finish();
}

#[test]
fn locate_follows_an_id_through_a_rename_and_a_move_and_looks_by_name_when_the_id_is_gone() {
    let (mut core, root) = orders_project("locate-id", None);
    let key = "orders.py::OrderService.parse_order";
    core.request(
        "vnoc/attach",
        attach_params(&root, key, "Parsing", Some(true)),
    )
    .expect("attach answers");
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    link_of_is(&mut core, "parse_order", "marker"); // the index has the ID written in the code
    let with_id = std::fs::read_to_string(root.join("orders.py")).unwrap();
    assert!(with_id.contains("ID: "), "{with_id}");

    // renamed and moved down ten lines, the ID still in its docstring: found by the ID, under its new name
    let renamed = format!(
        "{}{}",
        "# pad\n".repeat(10),
        with_id.replace("parse_order", "parse_renamed")
    );
    let found = core
        .request("vnoc/locate", locate_params(&root, key, &renamed))
        .unwrap();
    assert_eq!(located_line(&found), Some(11), "{found}");

    // renamed and its ID line deleted: nothing is left to find it by
    let without_id = |text: &str| -> String {
        text.lines()
            .filter(|l| !l.contains("ID: "))
            .map(|l| format!("{l}\n"))
            .collect()
    };
    assert_eq!(
        core.request(
            "vnoc/locate",
            locate_params(&root, key, &without_id(&renamed))
        )
        .unwrap(),
        Value::Null
    );
    // the same, not renamed: found by name
    let kept_name = without_id(&format!("{}{}", "# pad\n".repeat(10), with_id));
    let by_name = core
        .request("vnoc/locate", locate_params(&root, key, &kept_name))
        .unwrap();
    assert_eq!(located_line(&by_name), Some(11), "{by_name}");
    core.finish();
}

#[test]
fn locate_refuses_a_key_that_is_not_a_definition_and_a_document_that_is_not_its_file() {
    let (mut core, root) = orders_project("locate-refuses", None);
    let message = |e: Value| e["message"].as_str().unwrap_or_default().to_string();
    let file = core.request("vnoc/locate", locate_params(&root, "orders.py", ORDERS));
    assert!(message(file.unwrap_err()).contains("not a function or a class"));
    let other = json!({ "key": "orders.py::OrderService.other", "document": { "uri": file_uri(&root.join("elsewhere.py")), "version": 1, "text": ORDERS } });
    assert!(message(core.request("vnoc/locate", other).unwrap_err()).contains("not the file"));
    core.finish();
}

#[test]
fn write_id_puts_the_id_back_in_the_code_of_a_node_found_by_name() {
    let (mut core, root) = orders_project("writeid", Some("format = 1\n"));
    write_record(&root, S, "orders.py", "OrderService.other");
    let toml = root
        .join(".vnoc/nodes")
        .join(&S[24..])
        .join(S)
        .join("node.toml");
    std::fs::write(
        &toml,
        format!("{}future = 7\n", std::fs::read_to_string(&toml).unwrap()),
    )
    .unwrap();
    tell_core_about_vnoc(&mut core, &root);
    link_of_is(&mut core, "other", "name");

    let r = core
        .request(
            "vnoc/writeId",
            write_id_params(&root, "orders.py::OrderService.other"),
        )
        .expect("writeId answers");
    assert_eq!(
        (r["written"].as_bool(), r["notice"].is_null()),
        (Some(true), true),
        "{r}"
    );
    assert_eq!(core.edit_requests.len(), 1, "the editor was asked once");
    assert!(
        std::fs::read_to_string(root.join("orders.py"))
            .unwrap()
            .contains(&format!("def other(self):\n        \"\"\"ID: {S}\"\"\""))
    );
    let text = std::fs::read_to_string(&toml).unwrap();
    assert!(
        text.contains("path = \"orders.py\"")
            && text.contains("qualname = \"OrderService.other\"")
            && text.contains("id_in_code = \"yes\"")
            && text.contains("future = 7"),
        "{text}"
    );

    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    link_of_is(&mut core, "other", "marker");
    core.finish();
}

#[test]
fn write_id_refuses_what_has_no_docs_has_its_id_or_is_not_allowed_one_and_changes_nothing() {
    let (mut core, root) = orders_project("writeidrefuse", None);
    core.request(
        "vnoc/attach",
        attach_params(
            &root,
            "orders.py::OrderService.parse_order",
            "Why",
            Some(true),
        ),
    )
    .unwrap(); // its ID is in the code
    write_record(&root, S, "orders.py", "OrderService.other");
    write_record(&root, T, "orders.py", "OrderService");
    let record = |id: &str| {
        root.join(".vnoc/nodes")
            .join(&id[24..])
            .join(id)
            .join("node.toml")
    };
    std::fs::write(
        record(S),
        std::fs::read_to_string(record(S))
            .unwrap()
            .replace("\"no\"", "\"never\""),
    )
    .unwrap();
    tell_core_about_vnoc(&mut core, &root);
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    link_of_is(&mut core, "parse_order", "marker");
    link_of_is(&mut core, "other", "name");
    let (code, edits, s_toml, t_toml) = (
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        core.edit_requests.len(),
        std::fs::read_to_string(record(S)).unwrap(),
        std::fs::read_to_string(record(T)).unwrap(),
    );

    let ask = |core: &mut Core, key: &str| {
        message(core.request("vnoc/writeId", write_id_params(&root, key)))
    };
    assert!(ask(&mut core, "orders.py::Nope").contains("not in the index"));
    assert_eq!(
        ask(&mut core, "orders.py"),
        "that node has no docs, so there is no ID to write"
    );
    assert_eq!(
        ask(&mut core, "orders.py::OrderService.parse_order"),
        "the ID of that node is already in the code"
    );
    assert_eq!(
        ask(&mut core, "orders.py::OrderService.other"),
        "this node was set to never have an ID in the code"
    );
    std::fs::write(
        root.join(".vnoc/vnoc.toml"),
        "format = 1\n[ids]\ndefault = \"none\"\n",
    )
    .unwrap(); // the settings are read on every call
    assert!(ask(&mut core, "orders.py::OrderService").contains("does not write IDs"));

    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        code
    );
    assert_eq!(
        (
            std::fs::read_to_string(record(S)).unwrap(),
            std::fs::read_to_string(record(T)).unwrap()
        ),
        (s_toml, t_toml)
    );
    assert_eq!(core.edit_requests.len(), edits);
    core.finish();
}

#[test]
fn write_id_that_cannot_write_says_so_and_changes_nothing() {
    let (mut core, root) = orders_project("writeidfail", Some("format = 1\n"));
    write_record(&root, S, "orders.py", "OrderService.other");
    tell_core_about_vnoc(&mut core, &root);
    link_of_is(&mut core, "other", "name");
    core.editor_applies_edits = false;
    let r = core
        .request(
            "vnoc/writeId",
            write_id_params(&root, "orders.py::OrderService.other"),
        )
        .unwrap();
    assert_eq!(r["written"], false);
    assert!(
        r["notice"]
            .as_str()
            .unwrap()
            .contains("could not be written"),
        "{r}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS,
        "the code is untouched"
    );
    assert!(
        std::fs::read_to_string(
            root.join(".vnoc/nodes")
                .join(&S[24..])
                .join(S)
                .join("node.toml")
        )
        .unwrap()
        .contains("id_in_code = \"no\"")
    );
    core.finish();
}

#[test]
fn write_id_does_not_take_a_node_whose_id_is_elsewhere_in_the_code_however_old_its_anchor() {
    let (mut core, root) = orders_project("writeidelsewhere", Some("format = 1\n"));
    // S still has its stored anchor at `other`, but its ID is in the code of `parse_order`
    write_record(&root, S, "orders.py", "OrderService.other");
    let moved = ORDERS.replace(
        "def parse_order(self, raw):\n",
        &format!("def parse_order(self, raw):\n        \"\"\"ID: {S}\"\"\"\n"),
    );
    std::fs::write(root.join("orders.py"), &moved).unwrap();
    tell_core_about_vnoc(&mut core, &root);
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py")] }),
    );
    structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["name"] == "parse_order").is_some_and(|n| n["node_id"] == S)
    });

    let refused = message(core.request(
        "vnoc/writeId",
        write_id_params(&root, "orders.py::OrderService.other"),
    ));
    assert!(refused.contains("has no docs"), "{refused}");
    assert!(core.edit_requests.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        moved
    );
    core.finish();
}

#[test]
fn relink_keeps_never_and_a_value_it_does_not_know_and_writes_no_id() {
    let (mut core, root) = orders_project("relinknever", Some("format = 1\n"));
    write_record(&root, R, "gone.py", "old");
    write_record(&root, T, "gone.py", "older");
    let toml = |id: &str| {
        root.join(".vnoc/nodes")
            .join(&id[24..])
            .join(id)
            .join("node.toml")
    };
    for (id, value) in [(R, "never"), (T, "sometimes")] {
        std::fs::write(
            toml(id),
            std::fs::read_to_string(toml(id))
                .unwrap()
                .replace("\"no\"", &format!("\"{value}\"")),
        )
        .unwrap();
    }
    tell_core_about_vnoc(&mut core, &root);
    missing_is(&mut core, &[R, T]);

    for (id, to, value) in [
        (R, "orders.py::OrderService.other", "never"),
        (T, "orders.py::OrderService.parse_order", "sometimes"),
    ] {
        let r = core
            .request("vnoc/relink", relink_params(&root, id, to))
            .expect("relink answers");
        assert_eq!(
            (r["written"].as_str(), r["notice"].is_null()),
            (Some("anchorOnly"), true),
            "{r}"
        );
        let text = std::fs::read_to_string(toml(id)).unwrap();
        assert!(
            text.contains(&format!("id_in_code = \"{value}\"")),
            "the field is as it was: {text}"
        );
        assert!(
            text.contains("path = \"orders.py\""),
            "the place moved: {text}"
        );
    }
    assert!(core.edit_requests.is_empty(), "the editor was not asked");
    assert_eq!(
        std::fs::read_to_string(root.join("orders.py")).unwrap(),
        ORDERS,
        "no ID line in the code"
    );
    core.finish();
}

#[test]
fn trash_refuses_a_node_that_is_not_missing_and_remove_then_restore_gives_back_the_same_bytes() {
    let (mut core, root) = orders_project("trash", Some("format = 1\n"));
    write_record(&root, S, "orders.py", "OrderService.other"); // found by name: not Missing
    write_record(&root, R, "gone.py", "old");
    tell_core_about_vnoc(&mut core, &root);
    missing_is(&mut core, &[R]);
    let folder = |id: &str| root.join(".vnoc/nodes").join(&id[24..]).join(id);

    let refused = message(core.request("vnoc/trash", json!({ "id": S })));
    assert!(refused.contains("not Missing"), "{refused}");
    assert!(folder(S).is_dir(), "a refusal changes nothing");

    let before = std::fs::read(folder(R).join("notes.md")).unwrap();
    let plan = core
        .request("vnoc/trash", json!({ "id": R }))
        .expect("a Missing node can be removed");
    let (from, to) = (
        PathBuf::from(plan["from"].as_str().unwrap()),
        PathBuf::from(plan["to"].as_str().unwrap()),
    );
    assert_eq!(from, folder(R));
    assert!(from.is_dir(), "the core only plans: the editor moves");
    std::fs::rename(&from, &to).unwrap(); // what WorkspaceEdit.renameFile does
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [from.join("node.toml")] }),
    );
    missing_is(&mut core, &[]);

    let list = core.request("vnoc/trashList", Value::Null).unwrap();
    assert_eq!(
        (
            list.as_array().unwrap().len(),
            list[0]["id"].as_str(),
            list[0]["docs"].as_u64(),
            list[0]["name"].as_str()
        ),
        (1, Some(R), Some(1), Some("old"))
    );
    let back = core
        .request("vnoc/restorePlan", json!({ "entry": list[0]["entry"] }))
        .unwrap();
    assert_eq!(
        PathBuf::from(back["to"].as_str().unwrap()),
        folder(R),
        "worked out from the ID"
    );
    std::fs::rename(back["from"].as_str().unwrap(), back["to"].as_str().unwrap()).unwrap();
    assert_eq!(
        std::fs::read(folder(R).join("notes.md")).unwrap(),
        before,
        "the same bytes"
    );
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [folder(R).join("node.toml")] }),
    );
    missing_is(&mut core, &[R]);
    assert!(
        core.request("vnoc/trashList", Value::Null)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty(),
        "the emptied entry is gone"
    );
    assert!(
        message(core.request("vnoc/restorePlan", json!({ "entry": "../../nodes" })))
            .contains("not in the trash")
    );
    core.finish();
}

const CALLS_LIB: &str = "def leaf():\n    return 1\n\n\ndef twice():\n    leaf()\n    leaf()\n";
const CALLS_APP: &str = "import os\n\nfrom lib import leaf, twice\n\n\nclass Box:\n    def __init__(self):\n        leaf()\n\n\ndef main():\n    leaf()\n    twice()\n    leaf()\n    os.getcwd()\n";

/// A folder with `lib.py` and `app.py` (what each calls is above), opened in a core whose first build is done.
fn calls_project(folder: &std::path::Path, open: &std::path::Path) -> Core {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join("lib.py"), CALLS_LIB).unwrap();
    std::fs::write(folder.join("app.py"), CALLS_APP).unwrap();
    let mut core = Core::start(Some(driver_path()), open);
    core.request("vnoc/structure", Value::Null)
        .expect("structure answers");
    core
}

/// `(line, start, end)` of each call site of a call-tree node.
fn sites(node: &Value) -> Vec<(u64, u64, u64)> {
    node["call_sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["start"]["line"].as_u64().unwrap(),
                r["start"]["character"].as_u64().unwrap(),
                r["end"]["character"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_call_tree_comes_back_merged_and_mapped_to_the_structure() {
    let root = std::env::temp_dir().join(format!("vnoc-calltree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
    let mut core = calls_project(&root, &root);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(
        structure["capabilities"]["callTree"], true,
        "the editor shows its command because the structure says the driver can"
    );

    let result = core
        .request("vnoc/callTree", json!({ "key": "app.py::main" }))
        .expect("the call tree answers (is the driver built? `cd drivers/python && cargo build`)");
    assert_eq!(
        result["revision"], structure["revision"],
        "the keys were mapped against the index the editor shows"
    );

    let main = &result["root"];
    assert_eq!(
        (
            main["key"].as_str(),
            main["name"].as_str(),
            main["qualname"].as_str(),
            main["path"].as_str(),
            main["kind"].as_str()
        ),
        (
            Some("app.py::main"),
            Some("main"),
            Some("main"),
            Some("app.py"),
            Some("function")
        )
    );
    assert_eq!(
        (
            main["call_count"].as_u64(),
            sites(main),
            main["marks"].clone()
        ),
        (Some(1), vec![], json!([]))
    );

    // `main` calls leaf, twice, leaf, and `os.getcwd()`: two nodes (the library call is not in the tree), `leaf` merged from two calls
    let kids = main["children"].as_array().unwrap();
    assert_eq!(
        kids.iter()
            .map(|k| k["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["leaf", "twice"]
    );
    let (leaf, twice) = (&kids[0], &kids[1]);
    assert_eq!(
        (
            leaf["key"].as_str(),
            leaf["path"].as_str(),
            leaf["call_count"].as_u64()
        ),
        (Some("lib.py::leaf"), Some("lib.py"), Some(2))
    );
    assert_eq!(
        sites(leaf),
        [(11, 4, 10), (13, 4, 10)],
        "both calls, in app.py, in source order"
    );
    assert_eq!(
        (
            twice["key"].as_str(),
            twice["call_count"].as_u64(),
            sites(twice)
        ),
        (Some("lib.py::twice"), Some(1), vec![(12, 4, 11)])
    );

    // `twice` calls `leaf` twice from lib.py: one node, two sites, in the PARENT's file (lib.py)
    let inner = &twice["children"][0];
    assert_eq!(
        (
            inner["key"].as_str(),
            inner["call_count"].as_u64(),
            sites(inner)
        ),
        (Some("lib.py::leaf"), Some(2), vec![(5, 4, 10), (6, 4, 10)])
    );
    assert_eq!(inner["children"], json!([]));
    core.finish();
}

#[test]
fn a_class_is_the_root_with_what_its_init_calls_and_a_bad_key_is_refused() {
    let root = std::env::temp_dir().join(format!("vnoc-calltree-class-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
    let mut core = calls_project(&root, &root);

    let result = core
        .request("vnoc/callTree", json!({ "key": "app.py::Box" }))
        .unwrap();
    let boxed = &result["root"];
    assert_eq!(
        (
            boxed["key"].as_str(),
            boxed["kind"].as_str(),
            boxed["name"].as_str()
        ),
        (Some("app.py::Box"), Some("class"), Some("Box"))
    );
    let kids = boxed["children"].as_array().unwrap();
    assert_eq!(
        kids.iter()
            .map(|k| k["key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["lib.py::leaf"],
        "there is no node for __init__ itself"
    );
    assert_eq!(sites(&kids[0]), [(7, 8, 14)]);

    // not a function or a class, or not in the index: INVALID_PARAMS, and the core goes on
    for key in ["app.py", "app.py::nope", "nowhere.py::main", "::"] {
        let error = core
            .request("vnoc/callTree", json!({ "key": key }))
            .unwrap_err();
        assert_eq!(error["code"], -32602, "{key}: {error}");
    }
    assert_eq!(
        core.request("vnoc/callTree", json!({ "key": "app.py::main" }))
            .unwrap()["root"]["children"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    core.finish();
}

#[test]
fn a_call_tree_of_a_subfolder_has_keys_from_the_git_root_and_paths_from_the_folder() {
    let repo = std::env::temp_dir().join(format!("vnoc-calltree-sub-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    testdir::remove_at_end(&repo);
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let sub = repo.join("services/api");
    let mut core = calls_project(&sub, &sub);

    let result = core
        .request(
            "vnoc/callTree",
            json!({ "key": "services/api/app.py::main" }),
        )
        .unwrap();
    let main = &result["root"];
    assert_eq!(
        (main["key"].as_str(), main["path"].as_str()),
        (Some("services/api/app.py::main"), Some("app.py")),
        "the key starts at the git root, the path at the opened folder"
    );
    let leaf = &main["children"][0];
    assert_eq!(
        (leaf["key"].as_str(), leaf["path"].as_str()),
        (Some("services/api/lib.py::leaf"), Some("lib.py"))
    );
    core.finish();
}

/// `src` and everything under it, copied to `dst`.
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

#[test]
fn a_call_site_in_an_inherited_constructor_names_the_base_file() {
    // `class Child(Base)` is in pkg/child.py and has no `__init__`; `Base.__init__` in pkg/base.py calls `a()` from pkg/calc.py
    let folder = std::env::temp_dir().join(format!("vnoc-calltree-inherit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    testdir::remove_at_end(&folder);
    copy_tree(
        &repo_root().join("drivers/python/tests/fixtures/call_tree"),
        &folder,
    );
    let mut core = Core::start(Some(driver_path()), &folder);
    core.request("vnoc/structure", Value::Null)
        .expect("structure answers");

    let result = core
        .request("vnoc/callTree", json!({ "key": "pkg/child.py::Child" }))
        .expect("the call tree answers (is the driver built? `cd drivers/python && cargo build`)");
    let child = &result["root"];
    assert_eq!(
        (child["path"].as_str(), child["kind"].as_str()),
        (Some("pkg/child.py"), Some("class"))
    );
    let [a] = &child["children"].as_array().unwrap()[..] else {
        panic!("one call: {child}")
    };
    assert_eq!(
        (a["qualname"].as_str(), a["path"].as_str()),
        (Some("a"), Some("pkg/calc.py"))
    );
    let sites = a["call_sites"].as_array().unwrap();
    assert_eq!(sites.len(), 1, "{a}");
    assert_eq!(
        sites[0]["path"], "pkg/base.py",
        "the call is written in the base class's file, not in child.py"
    );
    core.finish();
}

/// A folder with these files, opened in a core whose first build is done.
fn files_project(name: &str, files: &[(&str, String)]) -> Core {
    let folder = std::env::temp_dir().join(format!("vnoc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    testdir::remove_at_end(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    for (file, text) in files {
        std::fs::write(folder.join(file), text).unwrap();
    }
    let mut core = Core::start(Some(driver_path()), &folder);
    core.request("vnoc/structure", Value::Null)
        .expect("structure answers");
    core
}

/// `main` calls `f0`, which calls `f1`, … down to `f{n-1}`, which calls nothing.
fn chain(n: usize) -> String {
    let mut text = String::from("def main():\n    f0()\n\n\n");
    for i in 0..n - 1 {
        text.push_str(&format!("def f{i}():\n    f{}()\n\n\n", i + 1));
    }
    text.push_str(&format!("def f{}():\n    return 1\n", n - 1));
    text
}

/// The nodes down the only child of each node, from `root`, root first.
fn spine(root: &Value) -> Vec<&Value> {
    let mut nodes = vec![root];
    while let Some(next) = nodes.last().unwrap()["children"]
        .as_array()
        .and_then(|c| c.first())
    {
        nodes.push(next);
    }
    nodes
}

#[test]
fn has_calls_counts_what_opening_would_show_and_nothing_else_leaves_the_core() {
    let root = std::env::temp_dir().join(format!("vnoc-hascalls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
    let mut core = calls_project(&root, &root);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();

    let count = |core: &mut Core, key: &str| {
        core.request("vnoc/hasCalls", json!({ "key": key }))
            .unwrap_or_else(|e| panic!("{key}: {e}"))
    };
    // main calls leaf twice, twice, leaf again, and os.getcwd(): two nodes, as the tree has them (the library call is not one)
    let main = count(&mut core, "app.py::main");
    assert_eq!(main["count"], 2);
    assert_eq!(
        main["revision"], structure["revision"],
        "the index the keys belong to"
    );
    assert_eq!(
        main.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["count", "revision"],
        "the depth-1 tree's marks and children never leave the core: {main}"
    );
    assert_eq!(
        count(&mut core, "lib.py::twice")["count"],
        1,
        "two calls to one callee are one node"
    );
    assert_eq!(
        count(&mut core, "lib.py::leaf")["count"],
        0,
        "calls nothing"
    );
    assert_eq!(
        count(&mut core, "app.py::Box")["count"],
        1,
        "a class: what its __init__ calls"
    );

    // the same count as opening would show
    let tree = core
        .request("vnoc/callTree", json!({ "key": "app.py::main" }))
        .unwrap();
    assert_eq!(
        tree["root"]["children"].as_array().unwrap().len() as u64,
        main["count"].as_u64().unwrap()
    );

    for key in ["app.py", "app.py::nope", "nowhere.py::main", "::"] {
        let error = core
            .request("vnoc/hasCalls", json!({ "key": key }))
            .unwrap_err();
        assert_eq!(error["code"], -32602, "{key}: {error}");
    }
    core.finish();
}

#[test]
fn a_function_that_calls_only_library_code_has_no_calls() {
    let mut core = files_project("hascalls-library", &[("app.py", "import os\nimport json\n\n\ndef only_library():\n    os.getcwd()\n    json.dumps({})\n    len([])\n".to_string())]);
    assert_eq!(
        core.request("vnoc/hasCalls", json!({ "key": "app.py::only_library" }))
            .unwrap()["count"],
        0
    );
    assert_eq!(
        core.request("vnoc/callTree", json!({ "key": "app.py::only_library" }))
            .unwrap()["root"]["children"],
        json!([]),
        "and the tree agrees"
    );
    core.finish();
}

#[test]
fn a_deep_chain_comes_back_cut_at_the_cap_and_parses_and_opening_the_cut_node_gives_the_rest() {
    // 70 functions: past the ~60 a call tree can nest before the core's JSON reader (limit 128, two levels a node) gives up
    let mut core = files_project("deep-chain", &[("chain.py", chain(70))]);

    let first = core
        .request("vnoc/callTree", json!({ "key": "chain.py::main" }))
        .expect("a deep tree parses: the core asks for a depth cap");
    let down = spine(&first["root"]);
    let last = down.last().unwrap();
    assert!(
        down.len() >= 30 && down.len() <= 34,
        "cut at about 32 levels, not 70: {} nodes deep",
        down.len()
    );
    assert_eq!(
        last["marks"],
        json!(["cut"]),
        "the node at the cap says the walk stopped there"
    );
    assert_eq!(last["children"], json!([]));
    let key = last["key"]
        .as_str()
        .expect("the cut node is a function of the project: it has a key")
        .to_string();
    assert!(
        down.iter().all(|n| n["name"] != "f69"),
        "the end of the chain is not in the first tree"
    );

    // opening the cut node asks for its own tree: it goes on from there, and is cut again (38 more levels are left)
    let second = core
        .request("vnoc/callTree", json!({ "key": key }))
        .expect("the cut node's own tree");
    assert_eq!(second["root"]["key"], json!(key));
    let rest = spine(&second["root"]);
    assert!(rest.len() > 1, "it has calls of its own");
    assert!(
        rest.iter().any(|n| n["name"] == "f69") || rest.last().unwrap()["marks"] == json!(["cut"]),
        "it reaches the end or is cut again"
    );

    // a probe of the chain's head is one level and says one call
    assert_eq!(
        core.request("vnoc/hasCalls", json!({ "key": "chain.py::f0" }))
            .unwrap()["count"],
        1
    );
    core.finish();
}

/// The doc rows of a `vnoc/search` answer for `query`.
fn doc_rows(core: &mut Core, query: &str) -> Vec<Value> {
    let found = core
        .request("vnoc/search", json!({ "query": query }))
        .unwrap();
    found["docs"].as_array().unwrap().clone()
}

/// Asks `vnoc/search` for `query` until it gives a doc row (at most 10 s); the rows when it does, and none when it never does.
fn wait_for_doc_rows(core: &mut Core, query: &str) -> Vec<Value> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let rows = doc_rows(core, query);
        if !rows.is_empty() || std::time::Instant::now() > until {
            return rows;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn search_finds_doc_text_and_an_edited_body_neither_moves_the_revision_nor_announces() {
    let root = fixtures("search-docs");
    std::fs::write(root.join("orders.py"), ORDERS).unwrap(); // `attach_params` reads this file; the document is replaced below
    // code the scan does not read, with a doc: its node is neither in the tree nor Missing
    std::fs::create_dir_all(root.join("ignored")).unwrap();
    std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    std::fs::write(root.join("ignored/x.py"), "def f():\n    pass\n").unwrap();
    let mut core = Core::start(Some(driver_path()), &root);
    core.request("vnoc/structure", Value::Null).unwrap();

    // a doc on `Outer`, with a distinctive word in its body
    let mut params = attach_params(&root, "nested.py::Outer", "Zoo notes", Some(false));
    let nested = root.join("nested.py");
    params["document"] = json!({ "uri": file_uri(&nested), "version": 1, "text": std::fs::read_to_string(&nested).unwrap() });
    let attached = core.request("vnoc/attach", params).expect("attach answers");
    let id = attached["id"].as_str().unwrap().to_string();
    let doc = PathBuf::from(attached["doc"].as_str().unwrap());
    let write_body = |word: &str| {
        std::fs::write(
            &doc,
            format!("---\nnode: {id}\nhint: nested.py::Outer\n---\n# Zoo notes\n\nThe animals of {word} are many.\n"),
        )
        .unwrap();
    };
    core.wait_for("vnoc/indexChanged"); // the node and its doc are in `.vnoc/` now
    write_body("zanzibar");
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));
    let rows = wait_for_doc_rows(&mut core, "zanzibar");
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row["node"], "nested.py::Outer", "{row}");
    assert_eq!(row["missing"], false, "{row}");
    // a word only a doc of `Outer` holds: `Outer` is a hit for the canvas, its methods are not
    let (_, hits) = search_cards(
        &mut core,
        "zanzibar",
        &["nested.py::Outer.method", "nested.py::Outer"],
    );
    assert_eq!(hits, ["nested.py::Outer"]);
    assert_eq!(
        (
            row["node_id"].as_str(),
            row["node_name"].as_str(),
            row["node_kind"].as_str()
        ),
        (Some(id.as_str()), Some("Outer"), Some("class")),
        "{row}"
    );
    assert_eq!(row["node_path"], "nested.py", "{row}");
    assert_eq!(row["title"], "Zoo notes", "{row}");
    assert_eq!(row["doc_path"], json!(doc), "{row}");
    let snippet = row["snippet"].as_str().unwrap();
    assert!(
        snippet.contains("zanzibar") && !snippet.contains("Zoo notes"),
        "{row}"
    );
    let [start, end] = {
        let m = row["snippet_match"].as_array().unwrap();
        assert_eq!(m.len(), 1, "{row}");
        [
            m[0][0].as_u64().unwrap() as usize,
            m[0][1].as_u64().unwrap() as usize,
        ]
    };
    assert_eq!(&snippet[start..end], "zanzibar", "{row}");
    let found = core
        .request("vnoc/search", json!({ "query": "zanzibar" }))
        .unwrap();
    assert_eq!(found["docs_total"], 1);
    let revision = found["revision"].clone();

    // the body changes on disk: found by its new word, not by the old, under the same revision, and nothing is announced
    core.request("vnoc/ping", Value::Null).unwrap();
    core.notifications.clear();
    write_body("kilimanjaro");
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));
    let rows = wait_for_doc_rows(&mut core, "kilimanjaro");
    assert_eq!(rows.len(), 1, "the new word finds the doc");
    assert!(
        doc_rows(&mut core, "zanzibar").is_empty(),
        "the old word does not"
    );
    let found = core
        .request("vnoc/search", json!({ "query": "kilimanjaro" }))
        .unwrap();
    assert_eq!(
        found["revision"], revision,
        "a body is not a change to the index"
    );
    // an update that announced would have done so right after it stored the text; give it the time, then flush the frames in flight
    std::thread::sleep(std::time::Duration::from_millis(500));
    core.request("vnoc/ping", Value::Null).unwrap();
    let announced: Vec<_> = core
        .notifications
        .iter()
        .filter(|(method, _)| method == "vnoc/indexChanged")
        .collect();
    assert!(announced.is_empty(), "a body edit announced {announced:?}");
    assert_eq!(
        core.request("vnoc/structure", Value::Null).unwrap()["revision"],
        revision
    );

    // a doc of a Missing node: its code is not there, and the doc is found all the same
    let gone = "0000000000000000000000000A";
    write_record(&root, gone, "nested.py", "Gone");
    let notes = root
        .join(".vnoc/nodes")
        .join(&gone[24..])
        .join(gone)
        .join("notes.md");
    std::fs::write(&notes, "# Notes\nthe plains of serengeti\n").unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [&notes] }));
    core.wait_for("vnoc/indexChanged");
    let rows = wait_for_doc_rows(&mut core, "serengeti");
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(
        (row["missing"].clone(), row["node"].clone()),
        (json!(true), Value::Null),
        "{row}"
    );
    assert_eq!(
        (
            row["node_id"].as_str(),
            row["node_name"].as_str(),
            row["node_kind"].as_str(),
            row["node_path"].as_str()
        ),
        (
            Some(gone),
            Some("Gone"),
            Some("function"),
            Some("nested.py")
        ),
        "{row}"
    );
    assert_eq!(
        std::path::Path::new(row["doc_path"].as_str().unwrap()),
        notes,
        "{row}"
    );

    // a doc of a node whose code is not read (ignored): no node, and not Missing
    let unread = "0000000000000000000000000B";
    write_record(&root, unread, "ignored/x.py", "f");
    let hush = root
        .join(".vnoc/nodes")
        .join(&unread[24..])
        .join(unread)
        .join("notes.md");
    std::fs::write(&hush, "# Hush\nthe savannah of tanganyika\n").unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [&hush] }));
    let rows = wait_for_doc_rows(&mut core, "tanganyika");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0]["missing"].clone(), rows[0]["node"].clone()),
        (json!(false), Value::Null),
        "{}",
        rows[0]
    );
    assert_eq!(rows[0]["node_id"], unread, "{}", rows[0]);
    assert_eq!(rows[0]["node_path"], "ignored/x.py", "{}", rows[0]);

    // `kind:` and `in:` narrow the docs by their node
    write_body("zanzibar");
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));
    assert_eq!(wait_for_doc_rows(&mut core, "zanzibar").len(), 1);
    assert_eq!(doc_rows(&mut core, "kind:class zanzibar").len(), 1);
    assert!(doc_rows(&mut core, "kind:fn zanzibar").is_empty());
    assert_eq!(doc_rows(&mut core, "in:nested.py zanzibar").len(), 1);
    assert!(doc_rows(&mut core, "in:other/ zanzibar").is_empty());
    core.finish();
}

#[test]
fn structure_carries_the_lost_marker_of_the_sample_and_only_it() {
    // `tests/sample/` (a copy of it, outside this repository): `rate_for`'s record says its ID is in the code, and the code has none
    let root = std::env::temp_dir().join(format!("vnoc-lifecycle-lost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
    copy_tree(&repo_root().join("crates/vnoc/tests/sample"), &root);
    let mut core = Core::start(Some(driver_path()), &root);
    let s = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(
        s["lost_markers"],
        json!([{
            "id": "4KF02248CHCTYAG49B0KKZ5SD7",
            "key": "billing/tax.py::rate_for",
            "kind": "function",
            "path": "billing/tax.py",
            "qualname": "rate_for"
        }]),
        "{s}"
    );
    // the Missing node is Missing, not lost, and the nodes found by their marker are not in it
    assert_eq!(s["missing"].as_array().unwrap().len(), 1);
    core.finish();
}

#[test]
fn a_doc_made_by_the_command_shows_in_a_running_core_and_the_editors_attach_still_makes_a_second_file_for_one_title()
 {
    // a started project (`vnoc init` would have written this), and a core running on it
    let (mut core, root) = orders_project("by-command", Some("format = 1\n"));
    let key = "orders.py::OrderService.parse_order";

    // the command attaches a doc, in another process, while the core runs
    let out = Command::new(env!("CARGO_BIN_EXE_vnoc"))
        .current_dir(&root)
        .env_remove("VNOC_DRIVER")
        .stdin(Stdio::null())
        .arg("--driver")
        .arg(driver_path())
        .args(["doc", "new", key, "--title", "Made by a command"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // the watcher says what changed (`.vnoc/**` and `*.py`): the file the command wrote the ID line into, and the doc it made
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let doc = said
        .lines()
        .find_map(|l| l.strip_prefix("created      "))
        .unwrap();
    let doc = root.join(doc.split("   ").next().unwrap());
    assert!(doc.is_file(), "{said}");
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders.py"), doc] }),
    );
    // the core's tree has the node, found by the ID line the command wrote, with one doc
    let shown = structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["id"] == key).is_some_and(|n| n["docs"] == 1)
    });
    let node = find_node(&shown["root"], &|n| n["id"] == key).unwrap();
    assert_eq!(node["link"], "marker");
    let id = node["node_id"].as_str().unwrap().to_string();
    let docs = core.request("vnoc/docs", json!({ "id": id })).unwrap();
    assert_eq!(docs[0]["file"], "made-by-a-command.md");
    assert_eq!(docs[0]["title"], "Made by a command");

    // the extension's attach (the editor sink, `again`) with the same title makes a second file, as it always did, with no new edit
    let second = core
        .request(
            "vnoc/attach",
            attach_params(&root, key, "Made by a command", None),
        )
        .expect("attach answers");
    assert_eq!(second["id"], json!(id));
    assert!(
        second["doc"]
            .as_str()
            .unwrap()
            .ends_with("made-by-a-command-2.md"),
        "{second}"
    );
    assert!(
        core.edit_requests.is_empty(),
        "the ID was in the code already"
    );
    core.finish();
}

/// A doc's file with this frontmatter between its `---` lines (what a person or an agent writes by hand) and the title `# <title>` after it.
fn write_doc_with(doc: &std::path::Path, id: &str, extra: &str, title: &str) {
    std::fs::write(
        doc,
        format!("---\nnode: {id}\nhint: orders.py::OrderService.parse_order\n{extra}---\n# {title}\n\nA note.\n"),
    )
    .unwrap();
}

#[test]
fn vnoc_docs_and_the_search_rows_carry_the_tags_and_the_author_of_a_doc() {
    let (mut core, root) = orders_project("tags-wire", Some("format = 1\n"));
    let key = "orders.py::OrderService.parse_order";
    let plain = core
        .request("vnoc/attach", attach_params(&root, key, "Plain", None))
        .expect("attach answers");
    let id = plain["id"].as_str().unwrap().to_string();
    core.wait_for("vnoc/indexChanged");
    let tagged = core
        .request("vnoc/attach", attach_params(&root, key, "Tagged", None))
        .expect("attach answers");
    let doc = PathBuf::from(tagged["doc"].as_str().unwrap());
    write_doc_with(
        &doc,
        &id,
        "author: claude-code\ntags: [plan, billing]\n",
        "Tagged",
    );
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));

    // `vnoc/docs`: each doc says its tags and author; a doc with none says so (an empty list, null)
    let docs = core.request("vnoc/docs", json!({ "id": id })).unwrap();
    let docs = docs.as_array().unwrap();
    let by_file = |file: &str| docs.iter().find(|d| d["file"] == file).unwrap();
    assert_eq!(by_file("tagged.md")["tags"], json!(["plan", "billing"]));
    assert_eq!(by_file("tagged.md")["author"], "claude-code");
    assert_eq!(by_file("plain.md")["tags"], json!([]));
    assert_eq!(by_file("plain.md")["author"], Value::Null);

    // `vnoc/search`: a query of a tag alone is a doc row, with the tags and the author; `kind:fn` alone is none
    let rows = wait_for_doc_rows(&mut core, "tag:plan");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["title"], "Tagged");
    assert_eq!(rows[0]["tags"], json!(["plan", "billing"]));
    assert_eq!(rows[0]["author"], "claude-code");
    assert_eq!(rows[0]["node"], key);
    assert!(
        rows[0]["snippet"].as_str().unwrap().starts_with("A note."),
        "the start of the body: {}",
        rows[0]
    );
    assert_eq!(rows[0]["title_match"], json!([]));
    assert!(doc_rows(&mut core, "kind:fn").is_empty());
    assert_eq!(doc_rows(&mut core, "author:Claude-Code").len(), 1);
    assert!(doc_rows(&mut core, "tag:plan author:somebody-else").is_empty());
    // the nodes of that query: the function that has such a doc
    let found = core
        .request("vnoc/search", json!({ "query": "tag:plan" }))
        .unwrap();
    assert_eq!(found["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(found["nodes"][0]["id"], key);
    assert!(found["notes"].as_array().unwrap().is_empty());
    core.finish();
}

#[test]
fn a_doc_whose_tags_were_edited_by_hand_is_found_by_the_new_tag_with_no_index_changed() {
    // IDs off: the attach writes no line into the code, so only the doc's own text changes below
    let (mut core, root) = orders_project(
        "tags-by-hand",
        Some("format = 1\n\n[ids]\ndefault = \"none\"\n"),
    );
    let key = "orders.py::OrderService.parse_order";
    let made = core
        .request("vnoc/attach", attach_params(&root, key, "Zoo notes", None))
        .expect("attach answers");
    let id = made["id"].as_str().unwrap().to_string();
    let doc = PathBuf::from(made["doc"].as_str().unwrap());
    // the attach's own update has landed when the tree shows the doc (an `indexChanged` already waiting may be the first build's)
    let shown = structure_when(&mut core, |s| {
        find_node(&s["root"], &|n| n["id"] == key).is_some_and(|n| n["docs"] == 1)
    });
    assert!(doc_rows(&mut core, "tag:plan").is_empty(), "no tags yet");
    let revision = shown["revision"].clone();

    // a person writes `tags: [plan]` by hand
    core.request("vnoc/ping", Value::Null).unwrap();
    core.notifications.clear();
    write_doc_with(&doc, &id, "tags: [plan]\n", "Zoo notes");
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));
    let rows = wait_for_doc_rows(&mut core, "tag:plan");
    assert_eq!(rows.len(), 1, "found by the new tag");
    assert_eq!(rows[0]["tags"], json!(["plan"]));

    // then another, in the YAML block form, in capitals: read lower case, the old one is gone
    write_doc_with(&doc, &id, "tags:\n  - Billing\n  - draft\n", "Zoo notes");
    core.notify("vnoc/filesChanged", json!({ "paths": [&doc] }));
    let rows = wait_for_doc_rows(&mut core, "tag:billing");
    assert_eq!(rows.len(), 1, "the block form is read");
    assert_eq!(rows[0]["tags"], json!(["billing", "draft"]));
    assert!(
        doc_rows(&mut core, "tag:plan").is_empty(),
        "the old tag does not"
    );
    assert!(doc_rows(&mut core, "-tag:draft").is_empty());

    // the tags are not a change to the index: the same revision, and nothing announced
    assert_eq!(
        core.request("vnoc/structure", Value::Null).unwrap()["revision"],
        revision
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    core.request("vnoc/ping", Value::Null).unwrap();
    let announced: Vec<_> = core
        .notifications
        .iter()
        .filter(|(method, _)| method == "vnoc/indexChanged")
        .collect();
    assert!(announced.is_empty(), "a tag edit announced {announced:?}");
    core.finish();
}

// ---------------------------------------------------------------------------------------------------------------------------------- tours (TR15a)
//
// The core answers the extension for tours (docs/26-tours/03-core.md §8): the requests, the notifications, and what a path of `vnoc/filesChanged` is to a tour. The
// agent and the reader are other processes here: `vnoc tour …` commands, as in life.

/// A copy of the sample project outside this repository, symlinks resolved (the paths the editor sends are under the folder it opened).
fn tour_project(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vnoc-tours-core-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    testdir::remove_at_end(&dir);
    copy_tree(&repo_root().join("crates/vnoc/tests/sample"), &dir);
    dunce::canonicalize(dir).unwrap()
}

/// `vnoc <args>` in the project, as another process, with `input` on its standard input.
fn vnoc_tour(root: &Path, args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vnoc"))
        .current_dir(root)
        .env_remove("VNOC_DRIVER")
        .env_remove("VNOC_DRIVERS")
        .arg("--driver")
        .arg(driver_path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    child.wait_with_output().unwrap()
}

fn vnoc_ok(root: &Path, args: &[&str], input: &str) -> String {
    let out = vnoc_tour(root, args, input);
    assert!(
        out.status.success(),
        "vnoc {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

const RETRY: &str = "billing/charge.py::retry_charge";

const RETRY_BODY: &str = "retry_charge calls the gateway and tries again when the call drops, up to a fixed number of attempts, then gives up with an error that names the order.

## 14-15 One attempt, as a function
one_try is the single charge call; keeping it in a function lets the loop below treat \"charge\" as one thing that either returns or raises.

### 15 The call itself
This is the only line that talks to the gateway, so it is the only place a dropped connection can come from.

## 17-22 Try, and try again
Only a dropped connection is retried: any other error leaves the loop at once. When every attempt drops, the function raises ChargeError.
";

/// A tour started in the project by another process; its id.
fn start_tour(root: &Path) -> String {
    let out = vnoc_ok(
        root,
        &["tour", "start", "--title", "retry_charge", "--json"],
        "",
    );
    serde_json::from_str::<Value>(&out).unwrap()["tour"]
        .as_str()
        .unwrap()
        .to_string()
}

fn add_retry(root: &Path, tour: &str) {
    vnoc_ok(
        root,
        &["tour", "add", tour, RETRY, "--body-file", "-"],
        RETRY_BODY,
    );
}

fn tour_file(root: &Path, id: &str) -> PathBuf {
    root.join(format!(".vnoc/local/tours/{id}.jsonl"))
}

fn state_file(root: &Path, id: &str) -> PathBuf {
    root.join(format!(".vnoc/local/tour-state/{id}.json"))
}

/// A core on the project with its first build done: the index revision it has.
fn ready_core(root: &Path) -> (Core, u64) {
    let mut core = Core::start(Some(driver_path()), root);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    let revision = structure["revision"].as_u64().unwrap();
    core.wait_for("vnoc/indexChanged"); // the first build's: what comes after is a change
    (core, revision)
}

impl Core {
    /// Waits long enough for anything the core was going to send about what it was just told, then drains what arrived: `notifications` is complete.
    fn settle(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.request("vnoc/ping", Value::Null).unwrap();
    }

    fn saw(&self, method: &str) -> usize {
        self.notifications
            .iter()
            .filter(|(m, _)| m == method)
            .count()
    }

    /// The tour as the stage has it.
    fn tour(&mut self, id: &str) -> Value {
        self.request("vnoc/tour", json!({ "id": id })).unwrap()
    }
}

#[test]
fn tours_lists_a_tour_started_in_another_process() {
    let root = tour_project("list");
    let (mut core, _) = ready_core(&root);
    let empty = core.request("vnoc/tours", Value::Null).unwrap();
    assert_eq!(empty["tours"], json!([]));
    let id = start_tour(&root);
    // a request with no `check` and one with the id: both answer, `null` params too
    for params in [json!({}), json!({ "check": [id] }), Value::Null] {
        let listed = core.request("vnoc/tours", params).unwrap();
        let rows = listed["tours"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{listed}");
        assert_eq!(rows[0]["id"], json!(id));
        assert_eq!(rows[0]["state"], "writing");
        assert_eq!(rows[0]["quiet"], false);
        assert_eq!(rows[0]["place"], Value::Null, "nobody has opened it: new");
        assert_eq!(rows[0]["root"], Value::Null, "no stop yet");
    }
    // the row asked for has its freshness; a tour with nothing written is fresh
    let listed = core
        .request("vnoc/tours", json!({ "check": [id] }))
        .unwrap();
    assert_eq!(listed["tours"][0]["check"], "fresh");
    assert_eq!(
        core.request("vnoc/tours", json!({ "check": ["no such tour"] }))
            .unwrap_err()["code"],
        -32602
    );
    core.finish();
}

#[test]
fn an_add_is_tours_changed_and_never_index_changed_and_a_batch_of_tour_paths_starts_no_update() {
    let root = tour_project("add");
    let (mut core, revision) = ready_core(&root);
    let id = start_tour(&root);
    // a file the core was never told about: an index update would find it (a walk reads what is new), and announce it
    std::fs::write(root.join("unlisted.py"), "def unlisted():\n    pass\n").unwrap();

    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [tour_file(&root, &id)] }),
    );
    assert_eq!(core.wait_for("vnoc/toursChanged"), json!({ "ids": [id] }));
    add_retry(&root, &id);
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [
            tour_file(&root, &id),
            root.join(".vnoc/local/tours.lock"),
            root.join(".vnoc/local/tours"),
        ] }),
    );
    core.settle();
    // everything one `add` caused, from the tour's file, the lock and the folder the watcher named: one word, and not the index's
    assert_eq!(
        core.notifications,
        [("vnoc/toursChanged".to_string(), json!({ "ids": [id] }))]
    );
    assert_eq!(core.saw("vnoc/indexChanged"), 0, "a tour is not the index");
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(structure["revision"].as_u64().unwrap(), revision);
    let names: Vec<&str> = structure["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    assert!(!names.contains(&"unlisted.py"), "no update ran: {names:?}");
    // the tour is there, with its stop
    let listed = core.request("vnoc/tours", json!({})).unwrap();
    assert_eq!(listed["tours"][0]["written"], 1);
    assert_eq!(listed["tours"][0]["root"]["name"], "retry_charge");

    // a mixed batch updates for the other path only; an empty batch is "look again", as it was
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [tour_file(&root, &id), root.join("unlisted.py")] }),
    );
    core.wait_for("vnoc/toursChanged");
    let changed = core.wait_for("vnoc/indexChanged");
    assert_eq!(changed["revision"].as_u64().unwrap(), revision + 1);
    core.finish();
}

#[test]
fn vnoc_tour_twice_reads_the_stops_files_once_and_a_change_to_one_of_them_is_tours_changed() {
    let root = tour_project("memory");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    add_retry(&root, &id);
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [tour_file(&root, &id)] }),
    );
    core.wait_for("vnoc/toursChanged");

    let first = core.tour(&id);
    assert_eq!(first["check"]["fresh"], true, "{first}");
    assert_ne!(first["check"]["from"], "memory");
    let recorded = std::fs::read(state_file(&root, &id)).unwrap();
    // the stop's file is gone from the disk, and nobody told the core: an answer from memory does not know, a read would
    let source = root.join("billing/charge.py");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::remove_file(&source).unwrap();
    let second = core.tour(&id);
    assert_eq!(second["check"]["from"], "memory", "{second}");
    assert_eq!(second["check"]["fresh"], true);
    assert_eq!(second["stops"], first["stops"]);
    assert_eq!(
        std::fs::read(state_file(&root, &id)).unwrap(),
        recorded,
        "nothing was recorded again: nothing was read"
    );

    // told, the core drops it and says so; the next ask reads again and finds the stop gone
    core.notify("vnoc/filesChanged", json!({ "paths": [source] }));
    assert_eq!(core.wait_for("vnoc/toursChanged"), json!({ "ids": [id] }));
    let third = core.tour(&id);
    assert_eq!(third["check"]["from"], "check");
    assert_eq!(third["check"]["gone"], 1);
    assert_eq!(third["stops"][0]["check"], "gone");

    // the file is back, changed in the lines the stop was about: the stop is changed
    std::fs::write(
        &source,
        text.replacen("def retry_charge", "def retry_charge_v2", 1),
    )
    .unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [source] }));
    core.wait_for("vnoc/toursChanged");
    let fourth = core.tour(&id);
    assert_eq!(fourth["check"]["from"], "check");
    assert_ne!(fourth["stops"][0]["check"], Value::Null, "{fourth}");
    core.finish();
}

#[test]
fn a_change_to_another_file_is_not_tours_changed_and_the_answer_is_kept() {
    let root = tour_project("other");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    add_retry(&root, &id);
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [tour_file(&root, &id)] }),
    );
    core.wait_for("vnoc/toursChanged");
    core.tour(&id);

    let other = root.join("orders/status.py");
    let mut text = std::fs::read_to_string(&other).unwrap();
    text.push_str("\n\ndef added_later():\n    pass\n");
    std::fs::write(&other, text).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [other] }));
    core.wait_for("vnoc/indexChanged"); // the index is the source's
    core.settle();
    assert_eq!(core.saw("vnoc/toursChanged"), 0);
    assert_eq!(core.tour(&id)["check"]["from"], "memory");
    core.finish();
}

#[test]
fn tour_go_writes_the_state_file_and_does_not_echo_but_another_process_does() {
    let root = tour_project("go");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    add_retry(&root, &id);

    // the core's own move: the place is written, the answer is the place
    let place = core
        .request("vnoc/tourGo", json!({ "id": id, "move": "next" }))
        .unwrap();
    assert_eq!(place["at"], "s1");
    assert_eq!(place["by"], "reader");
    // the answer has the current stop and the numbers of "stop 1 of N · 1 of M": made from the tour, not kept in the state file
    assert_eq!(place["stop"], "s1");
    assert_eq!(place["position"]["stop"], 1);
    assert_eq!(place["position"]["step"], 1);
    assert!(
        place["position"]["of"].as_u64().unwrap() >= 1
            && place["position"]["steps"].as_u64().unwrap() >= 1
    );
    let written: Value =
        serde_json::from_slice(&std::fs::read(state_file(&root, &id)).unwrap()).unwrap();
    assert_eq!(written["place"]["at"], "s1");
    assert!(
        written["place"].get("stop").is_none() && written["place"].get("position").is_none(),
        "the state file keeps the place only"
    );
    // the watcher reports the write: it is the core's own, and not a move
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [state_file(&root, &id)] }),
    );
    core.settle();
    assert_eq!(core.saw("vnoc/tourPlaceChanged"), 0, "an echo");
    assert_eq!(core.saw("vnoc/indexChanged"), 0);

    // another process moves it
    vnoc_ok(&root, &["tour", "go", &id, "s1.b1"], "");
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [state_file(&root, &id)] }),
    );
    let moved = core.wait_for("vnoc/tourPlaceChanged");
    assert_eq!(moved["id"], json!(id));
    assert_eq!(moved["place"]["at"], "s1.b1");
    assert_eq!(moved["place"]["by"], "command");
    assert_eq!(moved["place"]["stop"], "s1");
    assert_eq!(moved["place"]["position"]["step"], 2);

    // the same state file rewritten for a check (a change to the stop's file, then the next ask) is no move: the place is the one told
    let source = root.join("billing/charge.py");
    let text = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("{text}\n\ndef added_later():\n    pass\n")).unwrap();
    core.notify("vnoc/filesChanged", json!({ "paths": [source] }));
    core.wait_for("vnoc/indexChanged");
    assert_eq!(core.tour(&id)["check"]["from"], "check");
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [state_file(&root, &id)] }),
    );
    core.settle();
    assert_eq!(
        core.saw("vnoc/tourPlaceChanged"),
        0,
        "a check recorded is not a move"
    );
    // and the view has the place the reader is at
    let shown = core.tour(&id);
    assert_eq!(shown["place"]["at"], "s1.b1");
    assert_eq!(shown["place"]["stop"], "s1");
    assert_eq!(shown["writing"], Value::Null);
    // the list says each row's second line, and `tourOpen` answers a place with its stop too
    let row = core.request("vnoc/tours", json!({})).unwrap()["tours"][0].clone();
    assert!(
        row["words"].as_str().unwrap().starts_with("writing · "),
        "{row}"
    );
    let opened = core
        .request("vnoc/tourOpen", json!({ "id": id, "open": true }))
        .unwrap();
    assert_eq!(
        (opened["open"].clone(), opened["stop"].clone()),
        (json!(true), json!("s1"))
    );
    core.finish();
}

#[test]
fn tour_go_refuses_what_the_command_refuses_and_tour_open_sets_open() {
    let root = tour_project("open");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    add_retry(&root, &id);
    let nope = core
        .request("vnoc/tourGo", json!({ "id": id, "move": "s9" }))
        .unwrap_err();
    assert_eq!(nope["code"], -32602);
    assert!(nope["message"].as_str().unwrap().contains("no stop s9"));
    let nope = core
        .request("vnoc/tourGo", json!({ "id": id, "move": "sideways" }))
        .unwrap_err();
    assert_eq!(nope["code"], -32602);
    assert_eq!(
        core.request(
            "vnoc/tourGo",
            json!({ "id": "no-such-0000", "move": "next" })
        )
        .unwrap_err()["code"],
        -32602
    );

    let opened = core
        .request("vnoc/tourOpen", json!({ "id": id, "open": true }))
        .unwrap();
    assert_eq!(opened["open"], true);
    assert_eq!(opened["at"], "cover");
    let where_ = vnoc_ok(&root, &["tour", "where", &id, "--json"], "");
    assert_eq!(
        serde_json::from_str::<Value>(&where_).unwrap()["open"],
        true
    );
    let closed = core
        .request("vnoc/tourOpen", json!({ "id": id, "open": false }))
        .unwrap();
    assert_eq!(closed["open"], false);
    core.finish();
}

#[test]
fn tour_halt_and_then_an_add_from_the_command_is_refused_as_stopped() {
    let root = tour_project("halt");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    let halted = core.request("vnoc/tourHalt", json!({ "id": id })).unwrap();
    assert_eq!(halted, json!({ "halt": "written", "state": "stopping" }));
    let out = vnoc_tour(
        &root,
        &["tour", "add", &id, RETRY, "--body-file", "-"],
        RETRY_BODY,
    );
    assert!(!out.status.success(), "the add is refused");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_lowercase();
    assert!(said.contains("stopped"), "{said}");
    let again = core.request("vnoc/tourHalt", json!({ "id": id })).unwrap();
    assert_eq!(again["halt"], "already");
    core.finish();
}

#[test]
fn an_answer_of_vnoc_tour_has_the_steps_the_place_and_what_the_row_says() {
    let root = tour_project("view");
    let (mut core, _) = ready_core(&root);
    let id = start_tour(&root);
    add_retry(&root, &id);
    let shown = core.tour(&id);
    let stop = &shown["stops"][0];
    let steps: Vec<(&str, u64)> = stop["reading"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s["depth"].as_u64().unwrap()))
        .collect();
    assert_eq!(steps, [("s1", 0), ("s1.b1", 1), ("s1.b2", 2), ("s1.b3", 1)]);
    assert_eq!(stop["reading"][2]["parent"], "s1.b1");
    assert!(
        stop["reading"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["fresh"] == "fresh")
    );
    assert_eq!(shown["place"]["at"], "cover");
    assert_eq!(shown["minutes"], 2);
    let row = core.request("vnoc/tours", json!({})).unwrap()["tours"][0].clone();
    assert_eq!(
        row["root"],
        json!({ "kind": "function", "name": "retry_charge" })
    );
    assert_eq!(row["place"], Value::Null);
    core.request("vnoc/tourGo", json!({ "id": id, "move": "s1.b3" }))
        .unwrap();
    let row = core.request("vnoc/tours", json!({})).unwrap()["tours"][0].clone();
    // the last step of the only stop: it has been read
    assert_eq!(row["place"], json!({ "stop": 1, "at": "step", "seen": 1 }));
    core.finish();
}

// ---------------------------------------------------------------------------------------------------------------------------------- tours (TR15b)
//
// Tours on a node, a stop as written, and the count for a node (docs/26-tours/03-core.md §6, §8).

const PARSE: &str = "orders/service.py::OrderService.parse_order";
const PLACE: &str = "orders/service.py::OrderService.place";
/// A stop that is one step: one paragraph, so no blocks are asked for.
const ONE_STEP: &str = "It does one small thing, and the code around it relies on that: read it as a single step, with nothing hidden behind a second paragraph of explanation.\n";

fn git_in(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git is installed");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The sample project as a git repository with everything committed, so a finished tour is pinned.
fn tour_git_project(label: &str) -> PathBuf {
    let root = tour_project(label);
    git_in(&root, &["init", "-q"]);
    git_in(&root, &["add", "-A"]);
    git_in(&root, &["commit", "-qm", "sample"]);
    root
}

/// A finished tour with these stops (each one step); its id.
fn finished_tour(root: &Path, title: &str, stops: &[&str]) -> String {
    let out = vnoc_ok(root, &["tour", "start", "--title", title, "--json"], "");
    let id = serde_json::from_str::<Value>(&out).unwrap()["tour"]
        .as_str()
        .unwrap()
        .to_string();
    for stop in stops {
        vnoc_ok(
            root,
            &["tour", "add", &id, stop, "--body-file", "-"],
            ONE_STEP,
        );
    }
    vnoc_ok(root, &["tour", "finish", &id, "--no-ids"], "");
    id
}

fn on_rows(core: &mut Core, node: &str) -> Vec<Value> {
    core.request("vnoc/tours", json!({ "on": node })).unwrap()["tours"]
        .as_array()
        .unwrap()
        .clone()
}

fn tell_tour_changed(core: &mut Core, root: &Path, id: &str) {
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [tour_file(root, id)] }),
    );
    core.wait_for("vnoc/toursChanged");
}

#[test]
fn tours_on_a_node_lists_the_tours_through_it_with_line_2_and_a_file_lists_those_with_stops_in_it()
{
    let root = tour_git_project("on");
    let (mut core, _) = ready_core(&root);
    let first = finished_tour(&root, "parse first", &[PARSE]);
    let second = finished_tour(&root, "place then parse", &[PLACE, PARSE]);
    let apart = finished_tour(&root, "elsewhere", &["billing/charge.py::refund"]);
    for id in [&first, &second, &apart] {
        tell_tour_changed(&mut core, &root, id);
    }

    let rows = on_rows(&mut core, PARSE);
    let said: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap(),
                r["on"]["how"].as_str().unwrap(),
                r["on"]["words"].as_str().unwrap(),
            )
        })
        .collect();
    // the same order as the list, and only the tours that pass through the function
    let all = core.request("vnoc/tours", json!({})).unwrap()["tours"]
        .as_array()
        .unwrap()
        .clone();
    let order: Vec<&str> = all
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .filter(|id| *id != apart)
        .collect();
    assert_eq!(said.iter().map(|s| s.0).collect::<Vec<_>>(), order);
    for (id, how, words) in &said {
        if *id == first {
            assert_eq!((*how, *words), ("starts", "starts here"));
        } else {
            assert_eq!((*how, *words), ("stops", "stop 2 of 2"), "{id}");
        }
    }
    assert_eq!(rows.len(), 2);
    assert!(rows[0]["on"]["of"].as_u64().unwrap() >= 1);

    // the command says the same words for the same node
    let listed = vnoc_ok(&root, &["tour", "list", "--on", PARSE, "--json"], "");
    let listed: Value = serde_json::from_str(&listed).unwrap();
    let by_command: Vec<(String, String)> = listed["tours"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_string(),
                r["on"]["words"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let by_request: Vec<(String, String)> = rows
        .iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_string(),
                r["on"]["words"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(by_request, by_command);

    // a file: the tours with stops in it, the stops inside it said
    let in_file = on_rows(&mut core, "orders/service.py");
    let words: Vec<(&str, &str)> = in_file
        .iter()
        .map(|r| {
            (
                r["on"]["how"].as_str().unwrap(),
                r["on"]["words"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(in_file.len(), 2, "{in_file:?}");
    assert!(words.contains(&("inside", "1 stop inside")), "{words:?}");
    assert!(words.contains(&("inside", "2 stops inside")), "{words:?}");
    // a node nothing passes through, a node that is not there, and the root
    assert!(on_rows(&mut core, "api/app.py").is_empty());
    assert_eq!(
        core.request("vnoc/tours", json!({ "on": "no/such.py" }))
            .unwrap_err()["code"],
        -32602
    );
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    let root_key = structure["root"]["id"].as_str().unwrap().to_string();
    let refused = core
        .request("vnoc/tours", json!({ "on": root_key }))
        .unwrap_err();
    assert_eq!(
        refused["message"],
        "the project's root folder is every tour's: name a folder in it"
    );
    core.finish();
}

#[test]
fn tour_counts_counts_tours_not_stops_by_the_key_of_structure_and_a_tour_write_never_moves_the_revision()
 {
    let root = tour_git_project("counts");
    let (mut core, revision) = ready_core(&root);
    let empty = core.request("vnoc/tourCounts", Value::Null).unwrap();
    assert_eq!(empty, json!({ "counts": {} }));

    let one = finished_tour(&root, "one", &[PARSE, PARSE]);
    tell_tour_changed(&mut core, &root, &one);
    let counts = core.request("vnoc/tourCounts", Value::Null).unwrap();
    // two stops on one function are one tour; the file and the class that hold it count the tour too
    assert_eq!(counts["counts"][PARSE], 1, "{counts}");
    assert_eq!(counts["counts"]["orders/service.py"], 1, "{counts}");
    assert_eq!(
        counts["counts"]["orders/service.py::OrderService"], 1,
        "{counts}"
    );
    assert!(counts["counts"].get(PLACE).is_none(), "{counts}");
    // the key is the one `vnoc/structure` gives a node
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    let service = structure["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "orders")
        .unwrap()["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "service.py")
        .unwrap()
        .clone();
    assert_eq!(service["id"], "orders/service.py");

    let two = finished_tour(&root, "two", &[PARSE]);
    tell_tour_changed(&mut core, &root, &two);
    let counts = core.request("vnoc/tourCounts", Value::Null).unwrap();
    assert_eq!(counts["counts"][PARSE], 2, "{counts}");
    println!("tourCounts: {counts}");

    // one is deleted; the watcher says so; the count is 1
    vnoc_ok(&root, &["tour", "delete", &two], "");
    tell_tour_changed(&mut core, &root, &two);
    let counts = core.request("vnoc/tourCounts", Value::Null).unwrap();
    assert_eq!(counts["counts"][PARSE], 1, "{counts}");

    core.settle();
    assert_eq!(core.saw("vnoc/indexChanged"), 0, "a tour is not the index");
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(structure["revision"].as_u64().unwrap(), revision);
    core.finish();
}

#[test]
fn tour_code_gives_the_old_lines_of_a_pinned_tour_and_null_when_there_are_none_to_give() {
    let root = tour_git_project("code");
    let (mut core, _) = ready_core(&root);
    let id = finished_tour(&root, "parse", &[PARSE]);
    tell_tour_changed(&mut core, &root, &id);
    let shown = core.tour(&id);
    assert_eq!(shown["pinned"], true, "finish pinned it: {shown}");

    // not changed yet: any stop of a pinned tour has its code as written
    let now = core
        .request("vnoc/tourCode", json!({ "id": id, "stop": "s1" }))
        .unwrap();
    assert_eq!(now["path"], "orders/service.py");
    assert_eq!(now["lines"], json!([10, 13]));
    assert_eq!(now["lit"], json!([[10, 13]]));
    assert_eq!(now["commit"].as_str().unwrap().len(), 7);
    let written_first = now["code"][0].as_str().unwrap().to_string();
    assert_eq!(written_first, "    def parse_order(self, raw):");

    // the function is edited and committed: the stop is changed, and the code as written is still the old lines
    let path = root.join("orders/service.py");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("raw[\"number\"], lines", "raw[\"number\"], list(lines)"),
    )
    .unwrap();
    git_in(&root, &["commit", "-qam", "edit parse_order"]);
    let old = core
        .request("vnoc/tourCode", json!({ "id": id, "stop": "s1" }))
        .unwrap();
    assert_eq!(
        old["code"], now["code"],
        "the commit's lines, not the file's"
    );
    assert!(old["code"].as_array().unwrap().iter().any(|l| {
        l.as_str()
            .unwrap()
            .ends_with("Order(raw[\"number\"], lines)")
    }));
    assert_eq!(old, now);

    // a stop the tour does not have is the request's mistake, in the words `go` uses
    let bad = core
        .request("vnoc/tourCode", json!({ "id": id, "stop": "s9" }))
        .unwrap_err();
    assert_eq!(bad["code"], -32602);
    assert_eq!(bad["message"], "no stop s9 in this tour");
    assert_eq!(
        core.request("vnoc/tourCode", json!({ "id": id, "stop": "nonsense" }))
            .unwrap_err()["code"],
        -32602
    );

    // not pinned: a tour still being written
    let writing = start_tour(&root);
    add_retry(&root, &writing);
    tell_tour_changed(&mut core, &root, &writing);
    let none = core
        .request("vnoc/tourCode", json!({ "id": writing, "stop": "s1" }))
        .unwrap();
    assert_eq!(none, Value::Null);

    // pinned to a commit this repository does not have
    let file = tour_file(&root, &id);
    let record = std::fs::read_to_string(&file).unwrap();
    let pinned: Value = record
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|r| r["type"] == "pin")
        .unwrap();
    let commit = pinned["commit"].as_str().unwrap();
    std::fs::write(&file, record.replace(commit, &"a".repeat(40))).unwrap();
    tell_tour_changed(&mut core, &root, &id);
    let missing = core
        .request("vnoc/tourCode", json!({ "id": id, "stop": "s1" }))
        .unwrap();
    assert_eq!(missing, Value::Null);
    core.finish();
}

// ---------------------------------------------------------------------------------------------------------------------------------- tours (TR15c)
//
// The user's actions as requests (docs/26-tours/03-core.md §8): each is the function of the command, and says what the command says.

/// A change to the file of the tour that git has not been told of, and one to a file the tour does not touch.
fn leave_uncommitted(root: &Path) {
    let service = root.join("orders/service.py");
    let text = std::fs::read_to_string(&service).unwrap();
    std::fs::write(
        &service,
        format!("{text}\n# a note added after the last commit\n"),
    )
    .unwrap();
    let tax = root.join("billing/tax.py");
    let text = std::fs::read_to_string(&tax).unwrap();
    std::fs::write(&tax, format!("{text}\n# not the tour's\n")).unwrap();
    std::fs::write(root.join("scratch.py"), "x = 1\n").unwrap();
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn set_tour_setting(core: &mut Core, key: &str, value: &str) -> Value {
    core.request("vnoc/tourSettings", json!({ key: value }))
        .unwrap()
}

#[test]
fn tour_pin_on_uncommitted_work_is_refused_with_the_files_and_with_commit_commits_exactly_those() {
    let root = tour_git_project("pin");
    let (mut core, revision) = ready_core(&root);
    leave_uncommitted(&root);
    let id = finished_tour(&root, "parse", &[PARSE]);
    tell_tour_changed(&mut core, &root, &id);
    let row = &core.request("vnoc/tours", json!({})).unwrap()["tours"][0];
    assert_eq!(row["pinned"], false, "{row}");

    let refused = core.request("vnoc/tourPin", json!({ "id": id })).unwrap();
    println!("tourPin refusal: {refused}");
    assert_eq!(refused["state"], "not_pinned");
    assert_eq!(refused["reasons"][0]["code"], "uncommitted");
    assert_eq!(
        refused["reasons"][0]["files"],
        json!([{ "status": " M", "path": "orders/service.py" }])
    );
    assert!(refused.get("commit").is_none());
    assert_eq!(
        git_out(&root, &["log", "--oneline"]).lines().count(),
        1,
        "nothing was committed"
    );

    // with `commit`: exactly the tour's file, and the pin is made at the new commit
    let pinned = core
        .request(
            "vnoc/tourPin",
            json!({ "id": id, "commit": { "message": "Code for the parse tour" } }),
        )
        .unwrap();
    assert_eq!(pinned["state"], "pinned", "{pinned}");
    assert_eq!(pinned["committed"]["files"], json!(["orders/service.py"]));
    assert_eq!(pinned["committed"]["message"], "Code for the parse tour");
    let head = git_out(&root, &["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(pinned["commit"], json!(head));
    let shown = git_out(&root, &["show", "--stat", "--format=%s", "HEAD"]);
    assert!(shown.contains("Code for the parse tour"), "{shown}");
    assert!(shown.contains("orders/service.py"), "{shown}");
    assert!(
        !shown.contains("tax.py") && !shown.contains("scratch.py"),
        "{shown}"
    );
    let status = git_out(&root, &["status", "--porcelain"]);
    assert!(
        status.contains("billing/tax.py") && status.contains("scratch.py"),
        "{status}"
    );
    tell_tour_changed(&mut core, &root, &id);
    let row = &core.request("vnoc/tours", json!({})).unwrap()["tours"][0];
    assert_eq!(row["pinned"], true, "{row}");

    // pinned and HEAD has the commit behind it: nothing to do
    let again = core.request("vnoc/tourPin", json!({ "id": id })).unwrap();
    assert_eq!(
        (again["already"].clone(), again["state"].clone()),
        (json!(true), json!("pinned"))
    );
    // a tour that is not there
    assert_eq!(
        core.request("vnoc/tourPin", json!({ "id": "no-such-tour" }))
            .unwrap_err()["code"],
        -32602
    );
    // a pin, or a commit for it, is not the index changing
    core.settle();
    assert_eq!(core.saw("vnoc/indexChanged"), 0);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(structure["revision"].as_u64().unwrap(), revision);
    core.finish();
}

#[test]
fn tour_ids_writes_the_id_lines_and_the_tour_stays_fresh_because_the_edit_rebased_it() {
    let root = tour_git_project("ids");
    let (mut core, _) = ready_core(&root);
    let id = finished_tour(&root, "place", &[PLACE]);
    tell_tour_changed(&mut core, &root, &id);
    let before = std::fs::read_to_string(root.join("orders/service.py")).unwrap();
    assert!(!before.contains("def place(self, raw):\n        \"\"\"ID:"));

    let written = core.request("vnoc/tourIds", json!({ "id": id })).unwrap();
    assert_eq!(
        written["changed"],
        json!(["orders/service.py"]),
        "{written}"
    );
    assert_eq!(written["stops"][0]["state"], "written");
    assert_eq!(written["recorded"], true);
    let after = std::fs::read_to_string(root.join("orders/service.py")).unwrap();
    assert!(
        after.contains("def place(self, raw):\n        \"\"\"ID: "),
        "{after}"
    );
    assert!(
        core.edit_requests.is_empty(),
        "the edit is on disk, as the command's"
    );

    // the files the extension's watcher would report
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [
            root.join("orders/service.py"),
            tour_file(&root, &id),
            root.join(".vnoc/nodes"),
        ] }),
    );
    core.wait_for("vnoc/toursChanged");
    core.wait_for("vnoc/indexChanged");
    let shown = core.tour(&id);
    assert_eq!(
        shown["check"]["fresh"], true,
        "the edit rebased the tour: {shown}"
    );
    assert!(
        shown["stops"][0]["node"].as_str().is_some(),
        "the stop has its ID now: {shown}"
    );
    // asked again, there is nothing to write
    let again = core.request("vnoc/tourIds", json!({ "id": id })).unwrap();
    assert_eq!(again["changed"], json!([]), "{again}");
    assert_eq!(again["recorded"], false);
    core.finish();
}

#[test]
fn tour_edit_and_tour_share_there_and_back_and_tour_delete_then_restore_gives_the_same_file() {
    let root = tour_git_project("edit");
    let (mut core, revision) = ready_core(&root);
    let id = finished_tour(&root, "parse", &[PARSE]);
    tell_tour_changed(&mut core, &root, &id);

    let edited = core
        .request(
            "vnoc/tourEdit",
            json!({ "id": id, "title": "  How an order is parsed  ", "question": "why twice?" }),
        )
        .unwrap();
    assert_eq!(edited["title"], "How an order is parsed");
    tell_tour_changed(&mut core, &root, &id);
    let row = &core.request("vnoc/tours", json!({})).unwrap()["tours"][0];
    assert_eq!(
        (row["title"].clone(), row["question"].clone()),
        (json!("How an order is parsed"), json!("why twice?"))
    );
    // the command's own words for a title it refuses
    let bad = core
        .request("vnoc/tourEdit", json!({ "id": id, "title": "two\nlines" }))
        .unwrap_err();
    assert_eq!(bad["code"], -32602);
    let by_command = vnoc_tour(&root, &["tour", "edit", &id, "--title", "two\nlines"], "");
    let said = String::from_utf8_lossy(&by_command.stderr).to_string();
    assert!(
        said.contains(bad["message"].as_str().unwrap().lines().next().unwrap()),
        "{said} / {bad}"
    );

    // share, and back
    let private = tour_file(&root, &id);
    let project_file = root.join(format!(".vnoc/tours/{id}.jsonl"));
    assert!(private.is_file() && !project_file.exists());
    let shared = core
        .request("vnoc/tourShare", json!({ "id": id, "share": true }))
        .unwrap();
    assert_eq!(shared["folder"], "project");
    assert_eq!(shared["path"], format!(".vnoc/tours/{id}.jsonl"));
    assert!(!private.exists() && project_file.is_file());
    let refused = core
        .request("vnoc/tourShare", json!({ "id": id, "share": true }))
        .unwrap_err();
    assert_eq!(refused["code"], -32602, "already the project's: {refused}");
    let back = core
        .request("vnoc/tourShare", json!({ "id": id, "share": false }))
        .unwrap();
    assert_eq!(back["folder"], "private");
    assert!(private.is_file() && !project_file.exists());

    // delete, then restore: the same file, and its state with it
    core.request("vnoc/tourGo", json!({ "id": id, "move": "s1" }))
        .unwrap();
    let file_before = std::fs::read(&private).unwrap();
    let state_before = std::fs::read(state_file(&root, &id)).unwrap();
    let gone = core
        .request("vnoc/tourDelete", json!({ "id": id }))
        .unwrap();
    let token = gone["token"].as_str().unwrap().to_string();
    assert!(!private.exists() && !state_file(&root, &id).exists());
    let restored = core
        .request("vnoc/tourRestore", json!({ "token": token }))
        .unwrap();
    assert_eq!(restored, json!({ "tour": id, "folder": "private" }));
    assert_eq!(std::fs::read(&private).unwrap(), file_before);
    assert_eq!(std::fs::read(state_file(&root, &id)).unwrap(), state_before);
    assert_eq!(
        core.request("vnoc/tourRestore", json!({ "token": token }))
            .unwrap_err()["code"],
        -32602,
        "the entry is not in the trash any more"
    );
    // a tour being written is not deleted: the command's rule
    let writing = start_tour(&root);
    assert_eq!(
        core.request("vnoc/tourDelete", json!({ "id": writing }))
            .unwrap_err()["code"],
        -32602
    );
    core.settle();
    assert_eq!(core.saw("vnoc/indexChanged"), 0);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(structure["revision"].as_u64().unwrap(), revision);
    core.finish();
}

#[test]
fn tour_settings_reads_with_no_param_and_changes_only_the_key_it_is_given() {
    let root = tour_git_project("settings");
    let (mut core, _) = ready_core(&root);
    let toml = root.join(".vnoc/vnoc.toml");
    let before = std::fs::read_to_string(&toml).unwrap();
    for params in [Value::Null, json!({})] {
        let read = core.request("vnoc/tourSettings", params).unwrap();
        assert_eq!(read["ids"]["from"], "default", "{read}");
        assert_eq!(read["commit"]["from"], "default");
        assert!(read.get("changed").is_none());
    }
    assert_eq!(
        std::fs::read_to_string(&toml).unwrap(),
        before,
        "a read writes nothing"
    );

    let set = set_tour_setting(&mut core, "commit", "always");
    assert_eq!(
        set["commit"],
        json!({ "value": "always", "from": "file" }),
        "{set}"
    );
    assert_eq!(set["ids"]["from"], "default");
    assert_eq!(set["changed"], json!(["commit"]));
    let after = std::fs::read_to_string(&toml).unwrap();
    assert!(
        after.starts_with(&before),
        "nothing that was there moved: {after}"
    );
    assert!(
        after.contains("commit = \"always\"") && !after.contains("ids ="),
        "{after}"
    );

    let both = set_tour_setting(&mut core, "ids", "never");
    assert_eq!(both["commit"]["value"], "always");
    assert_eq!(both["ids"], json!({ "value": "never", "from": "file" }));
    // the command reads what the request wrote
    let by_command: Value =
        serde_json::from_str(&vnoc_ok(&root, &["tour", "settings", "--json"], "")).unwrap();
    assert_eq!(by_command["ids"], both["ids"]);
    assert_eq!(by_command["commit"], both["commit"]);

    let now = std::fs::read_to_string(&toml).unwrap();
    let bad = core
        .request("vnoc/tourSettings", json!({ "commit": "sometimes" }))
        .unwrap_err();
    assert_eq!(bad["code"], -32602);
    assert_eq!(std::fs::read_to_string(&toml).unwrap(), now, "{bad}");
    core.finish();
}

#[test]
fn offers_are_the_ids_and_the_files_only_under_ask_and_only_for_a_finished_tour() {
    let root = tour_git_project("offers");
    let (mut core, _) = ready_core(&root);
    leave_uncommitted(&root);
    // `place` has no ID; `parse_order` has one
    let id = finished_tour(&root, "both", &[PARSE, PLACE]);
    let writing = start_tour(&root);
    add_retry(&root, &writing);
    for t in [&id, &writing] {
        tell_tour_changed(&mut core, &root, t);
    }
    let offers = |core: &mut Core, tour: &str| -> (Value, Value) {
        let row = core.request("vnoc/tours", json!({})).unwrap()["tours"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == tour)
            .unwrap()["offers"]
            .clone();
        (row, core.tour(tour)["offers"].clone())
    };

    set_tour_setting(&mut core, "ids", "ask");
    set_tour_setting(&mut core, "commit", "ask");
    let (row, view) = offers(&mut core, &id);
    println!("offers: {row}");
    assert_eq!(row, json!({ "ids": 1, "commit": ["orders/service.py"] }));
    assert_eq!(row, view, "the row and the view say the same");
    // a tour that is still being written offers nothing
    assert_eq!(offers(&mut core, &writing), (Value::Null, Value::Null));

    // "always" was done at `finish`, and "never" is not offered
    set_tour_setting(&mut core, "ids", "always");
    assert_eq!(
        offers(&mut core, &id).0,
        json!({ "ids": 0, "commit": ["orders/service.py"] })
    );
    set_tour_setting(&mut core, "commit", "always");
    assert_eq!(offers(&mut core, &id), (Value::Null, Value::Null));
    set_tour_setting(&mut core, "ids", "never");
    set_tour_setting(&mut core, "commit", "never");
    assert_eq!(offers(&mut core, &id), (Value::Null, Value::Null));

    // ask again; the files are committed (pinned): only the IDs are left to offer
    set_tour_setting(&mut core, "ids", "ask");
    set_tour_setting(&mut core, "commit", "ask");
    core.request("vnoc/tourPin", json!({ "id": id, "commit": {} }))
        .unwrap();
    tell_tour_changed(&mut core, &root, &id);
    assert_eq!(offers(&mut core, &id).0, json!({ "ids": 1, "commit": [] }));
    // written, the IDs are not offered any more
    core.request("vnoc/tourIds", json!({ "id": id })).unwrap();
    core.notify(
        "vnoc/filesChanged",
        json!({ "paths": [root.join("orders/service.py"), tour_file(&root, &id)] }),
    );
    core.wait_for("vnoc/toursChanged");
    assert_eq!(offers(&mut core, &id), (Value::Null, Value::Null));
    core.finish();
}
