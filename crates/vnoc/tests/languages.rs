//! The core with two real drivers behind it (docs/23 §5): Python's and Bend's, each in its own build.
//! Needs both built first: `cd drivers/python && cargo build` and `cd drivers/bend && cargo build`.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const PY_ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
const BEND_ID: &str = "3RGJ87CMS6TK775XRJV8BW977Y";

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// `drivers/<language>/target/debug/vnoc-driver-<language>`, or a clear message about how to build it.
fn driver(language: &str) -> PathBuf {
    let path = repo_root()
        .join("drivers")
        .join(language)
        .join("target/debug")
        .join(format!(
            "vnoc-driver-{language}{}",
            std::env::consts::EXE_SUFFIX
        ));
    assert!(
        path.exists(),
        "the {language} driver is not built at {}: run `cd drivers/{language} && cargo build`",
        path.display()
    );
    path
}

/// A temp folder (not inside this repository: its git repo would put its path in every ID) with a Python file and a Bend file, one def with an ID
/// in each, and a record for each ID so the tree can show that the core read it.
fn project(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("vnoc-languages-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("a.py"),
        format!("def py_one():\n    \"\"\"ID: {PY_ID}\"\"\"\n    return 1\n\n\ndef py_two():\n    return 2\n"),
    )
    .unwrap();
    std::fs::write(
        root.join("b.bend"),
        format!(
            "# ID: {BEND_ID}\ndef bend_one() -> U32: bend_two()\n\ndef bend_two() -> U32: 1\n\ndef Nat.double(n: U32) -> U32: n\n"
        ),
    )
    .unwrap();
    for (id, path, qualname) in [(PY_ID, "a.py", "py_one"), (BEND_ID, "b.bend", "bend_one")] {
        let dir = root.join(".vnoc/nodes").join(&id[24..]).join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("node.toml"),
            format!("path = \"{path}\"\nqualname = \"{qualname}\"\nid_in_code = \"yes\"\nkind = \"function\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join("notes.md"), "# Notes\n").unwrap();
    }
    root
}

struct Core {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next: u64,
}

impl Core {
    /// Starts the core on `root`. `options` are the `initializationOptions` besides `root`; `env` are set for the process (the driver variables
    /// are removed first, so nothing of the machine leaks in).
    fn start(root: &Path, options: Value, env: &[(&str, std::ffi::OsString)]) -> Core {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vnoc"));
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_remove("VNOC_DRIVER")
            .env_remove("VNOC_DRIVERS");
        for (key, value) in env {
            cmd.env(key, value);
        }
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
        };
        let mut options = options;
        options["root"] = json!(root);
        core.request(
            "initialize",
            json!({ "processId": null, "rootUri": null, "capabilities": {}, "initializationOptions": options }),
        )
        .expect("initialize answers");
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
            if msg["method"].is_string() {
                continue; // a notification (`vnoc/indexChanged`)
            }
            assert_eq!(msg["id"], json!(id), "{msg}");
            return match msg.get("error") {
                Some(e) => Err(e.clone()),
                None => Ok(msg["result"].clone()),
            };
        }
    }

    fn read_frame(&mut self) -> Value {
        let mut len = 0usize;
        loop {
            let mut line = String::new();
            self.out.read_line(&mut line).unwrap();
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(n) = line.strip_prefix("Content-Length:") {
                len = n.trim().parse().unwrap();
            }
        }
        let mut buf = vec![0u8; len];
        self.out.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    fn finish(mut self) {
        self.request("shutdown", Value::Null).unwrap();
        self.notify("exit", Value::Null);
        let _ = self.child.wait();
    }
}

fn find<'a>(node: &'a Value, wanted: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
    if wanted(node) {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|c| find(c, wanted))
}

fn both(root: &Path) -> Core {
    Core::start(
        root,
        json!({ "drivers": [driver("python"), driver("bend")] }),
        &[],
    )
}

fn names_under<'a>(structure: &'a Value, file: &str) -> Vec<&'a str> {
    let file = find(&structure["root"], &|n| n["id"] == file)
        .unwrap_or_else(|| panic!("no file {file} in {structure}"));
    file["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect()
}

#[test]
fn the_tree_has_the_files_and_defs_of_both_languages() {
    let root = project("tree");
    let mut core = both(&root);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert_eq!(names_under(&structure, "a.py"), ["py_one", "py_two"]);
    assert_eq!(
        names_under(&structure, "b.bend"),
        ["bend_one", "bend_two", "Nat.double"],
        "a Bend name with a dot is one def at the top of its file, named as written"
    );
    assert!(structure["failed"].as_array().unwrap().is_empty());
    // the old single answer is the union of what the drivers can do
    assert_eq!(structure["capabilities"]["callTree"], true);
    core.finish();
}

#[test]
fn an_id_is_read_from_a_python_docstring_and_from_a_bend_comment() {
    let root = project("ids");
    let mut core = both(&root);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    for (name, id) in [("py_one", PY_ID), ("bend_one", BEND_ID)] {
        let def = find(&structure["root"], &|n| n["name"] == name).unwrap();
        assert_eq!(
            (def["node_id"].as_str(), def["link"].as_str()),
            (Some(id), Some("marker")),
            "{name}: found by the ID written in its code"
        );
    }
    core.finish();
}

#[test]
fn the_languages_are_listed_with_what_each_driver_said() {
    let root = project("languages");
    let mut core = both(&root);
    let languages = core.request("vnoc/languages", Value::Null).unwrap();
    let languages = languages.as_array().unwrap();
    let names: Vec<&str> = languages
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["bend", "python"], "by name");
    let (bend, python) = (&languages[0], &languages[1]);
    assert_eq!(bend["extensions"], json!(["bend"]));
    assert_eq!(python["extensions"], json!(["py", "pyi"]));
    for language in languages {
        assert_eq!(language["state"], "running", "{language}");
        assert!(language.get("error").is_none());
        assert_eq!(language["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(language["capabilities"]["outline"], true);
    }
    assert_eq!(bend["capabilities"]["environmentInputs"], false);
    assert_eq!(python["capabilities"]["environmentInputs"], true);
    assert!(
        bend["path"].as_str().unwrap().contains("vnoc-driver-bend"),
        "{bend}"
    );
    core.finish();
}

#[test]
fn a_call_tree_of_a_bend_def_is_answered_by_the_bend_driver() {
    let root = project("calltree");
    let mut core = both(&root);
    let tree = core
        .request("vnoc/callTree", json!({ "key": "b.bend::bend_one" }))
        .expect(
            "the call tree answers (is the Bend driver built? `cd drivers/bend && cargo build`)",
        );
    let top = &tree["root"];
    assert_eq!(
        (
            top["key"].as_str(),
            top["name"].as_str(),
            top["path"].as_str()
        ),
        (Some("b.bend::bend_one"), Some("bend_one"), Some("b.bend"))
    );
    let [child] = &top["children"].as_array().unwrap()[..] else {
        panic!("one call: {top}")
    };
    assert_eq!(child["key"], "b.bend::bend_two");
    // and a Python one still goes to Python
    let py = core
        .request("vnoc/callTree", json!({ "key": "a.py::py_one" }))
        .unwrap();
    assert_eq!(py["root"]["key"], "a.py::py_one");
    core.finish();
}

#[test]
fn a_bend_file_is_not_scanned_when_only_the_python_driver_is_given() {
    let root = project("python-only");
    let mut core = Core::start(&root, json!({ "driverPath": driver("python") }), &[]);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert!(find(&structure["root"], &|n| n["id"] == "a.py").is_some());
    assert!(
        find(&structure["root"], &|n| n["id"] == "b.bend").is_none(),
        "no driver owns `.bend`: the file is not in the tree"
    );
    let languages = core.request("vnoc/languages", Value::Null).unwrap();
    assert_eq!(languages.as_array().unwrap().len(), 1);
    assert_eq!(structure["capabilities"]["callTree"], true);
    core.finish();
}

#[test]
fn the_drivers_can_be_given_by_the_environment_as_a_list_or_one() {
    let root = project("env");
    let list = std::env::join_paths([driver("python"), driver("bend")]).unwrap();
    let mut core = Core::start(&root, json!({}), &[("VNOC_DRIVERS", list)]);
    let languages = core.request("vnoc/languages", Value::Null).unwrap();
    assert_eq!(languages.as_array().unwrap().len(), 2);
    core.finish();

    let mut core = Core::start(
        &root,
        json!({}),
        &[("VNOC_DRIVER", driver("bend").into_os_string())],
    );
    let languages = core.request("vnoc/languages", Value::Null).unwrap();
    assert_eq!(languages[0]["name"], "bend");
    assert_eq!(languages.as_array().unwrap().len(), 1);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert!(find(&structure["root"], &|n| n["id"] == "b.bend").is_some());
    assert!(find(&structure["root"], &|n| n["id"] == "a.py").is_none());
    core.finish();
}

#[test]
fn a_driver_that_is_not_there_is_listed_with_its_reason_and_the_other_language_works() {
    let root = project("one-missing");
    let missing = repo_root().join("no/such/vnoc-driver-python");
    let mut core = Core::start(&root, json!({ "drivers": [missing, driver("bend")] }), &[]);
    let structure = core.request("vnoc/structure", Value::Null).unwrap();
    assert!(find(&structure["root"], &|n| n["id"] == "b.bend").is_some());
    let languages = core.request("vnoc/languages", Value::Null).unwrap();
    let languages = languages.as_array().unwrap();
    assert_eq!(languages.len(), 2);
    let python = languages.iter().find(|l| l["name"] == "python").unwrap();
    assert_eq!(python["state"], "failed");
    assert!(
        python["error"].as_str().unwrap().contains("is not at"),
        "{python}"
    );
    assert_eq!(python["extensions"], json!([]));
    let bend = languages.iter().find(|l| l["name"] == "bend").unwrap();
    assert_eq!(bend["state"], "running");
    core.finish();
}
