//! `vnoc/environmentInputs { root } → { files }` through the real binary: the analyzer's `Project::environment_inputs` of the kept project. The
//! environment is named in `ty.toml` (`environment.python`) so that the machine running the tests does not choose it: without one the engine
//! looks at `VIRTUAL_ENV`, `<root>/.venv`, then `python3` on `PATH`.

mod common;

use common::{Driver, Project};
use serde_json::{Value, json};

const APP: &str = "def run():\n    pass\n";
const PYVENV: &str = "home = /usr/bin\nversion = 3.12.0\n";

fn files(d: &mut Driver, p: &Project) -> Vec<String> {
    let answer = d
        .request(
            "vnoc/environmentInputs",
            json!({ "root": p.root.to_str().unwrap() }),
        )
        .unwrap();
    answer["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect()
}

fn no_backslash(files: &[String]) {
    assert!(
        files.iter().all(|f| !f.contains('\\')),
        "every entry is written with `/`, on every OS: {files:?}"
    );
}

#[test]
fn the_handshake_announces_it() {
    let d = Driver::start();
    assert_eq!(d.init["capabilities"]["environmentInputs"], true);
    d.finish();
}

#[test]
fn the_config_and_the_lock_files_are_always_listed_sorted() {
    let p = Project::new(&[("app.py", APP)]);
    let mut d = Driver::start();
    let l = files(&mut d, &p);
    for fixed in [
        ".venv/pyvenv.cfg",
        "Pipfile.lock",
        "poetry.lock",
        "pyproject.toml",
        "requirements*.txt",
        "ty.toml",
        "uv.lock",
    ] {
        assert!(l.contains(&fixed.to_string()), "{fixed} is in {l:?}");
    }
    assert!(
        l.windows(2).all(|w| w[0] < w[1]),
        "sorted, no duplicates: {l:?}"
    );
    no_backslash(&l);
    d.finish();
}

#[test]
fn an_environment_under_the_root_is_relative() {
    let p = Project::new(&[
        ("app.py", APP),
        ("env/pyvenv.cfg", PYVENV),
        ("ty.toml", "[environment]\npython = \"env\"\n"),
    ]);
    let mut d = Driver::start();
    let l = files(&mut d, &p);
    assert!(l.contains(&"env/pyvenv.cfg".to_string()), "{l:?}");
    no_backslash(&l);
    d.finish();
}

#[test]
fn an_environment_outside_the_root_is_absolute_and_kept() {
    let env = Project::new(&[("pyvenv.cfg", PYVENV)]);
    let env_path = env.root.to_str().unwrap().replace('\\', "/");
    let p = Project::new(&[
        ("app.py", APP),
        (
            "ty.toml",
            &format!(
                "[environment]\npython = {}\n",
                Value::from(env_path.clone())
            ),
        ),
    ]);
    let mut d = Driver::start();
    let l = files(&mut d, &p);
    let outside: Vec<&String> = l
        .iter()
        .filter(|f| f.ends_with("/pyvenv.cfg") && *f != ".venv/pyvenv.cfg")
        .collect();
    assert_eq!(outside.len(), 1, "one entry for the environment: {l:?}");
    assert!(
        std::path::Path::new(outside[0]).is_absolute(),
        "absolute: {l:?}"
    );
    if cfg!(unix) {
        assert_eq!(
            outside[0],
            &format!("{env_path}/pyvenv.cfg"),
            "the environment's own file"
        );
    }
    no_backslash(&l);
    d.finish();
}

#[test]
fn an_environment_named_after_the_first_request_is_in_the_next_list() {
    let p = Project::new(&[("app.py", APP), ("env/pyvenv.cfg", PYVENV)]);
    let mut d = Driver::start();
    d.tree(&p.root, "app.py", "run").unwrap();
    assert!(
        !files(&mut d, &p).contains(&"env/pyvenv.cfg".to_string()),
        "nothing names it yet"
    );
    p.write("ty.toml", "[environment]\npython = \"env\"\n");
    assert!(
        files(&mut d, &p).contains(&"env/pyvenv.cfg".to_string()),
        "the list is asked after the refresh, which reopened the project"
    );
    let stats = d.stats();
    assert_eq!(
        (
            stats["opens"].as_u64(),
            stats["reopens"].as_u64(),
            stats["solves"].as_u64()
        ),
        (Some(1), Some(1), Some(1)),
        "one project, no solve for the list: {stats}"
    );
    d.finish();
}

#[test]
fn a_root_that_is_not_there_is_an_error_and_the_driver_goes_on() {
    let p = Project::new(&[("app.py", APP)]);
    let mut d = Driver::start();
    let missing = p.root.join("no-such-folder");
    let err = d
        .request(
            "vnoc/environmentInputs",
            json!({ "root": missing.to_str().unwrap() }),
        )
        .unwrap_err();
    assert_eq!(err["code"], -32603, "the project cannot be opened: {err}");
    assert!(
        files(&mut d, &p).contains(&"pyproject.toml".to_string()),
        "the next request is answered"
    );
    d.finish();
}
