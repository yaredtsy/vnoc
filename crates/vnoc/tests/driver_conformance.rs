//! One test every driver must pass (docs/06-drivers.md §6, docs/23 §5). A driver is a folder `drivers/<x>/` with a package `vnoc-driver-<x>` (the rule
//! of `scripts/drivers.mjs`), found here and not listed: a new language is a new folder and is checked with no edit to this file.
//!
//! Each driver needs its binary built (`cd drivers/<x> && cargo build`) and a folder `conformance/` with `input.<ext>` (a file of its language: a
//! character outside the BMP before a column the outline reports, a def with an ID and some without, the nested or same-name case the language has) and
//! `expected-outline.json` (the `vnoc/outline` result for it, checked by hand).
//!
//! The checks run on the real binary over its stdio. One `#[test]` loops over the drivers and reports every failing check of every driver at the end,
//! so a new driver's first run shows all that is wrong with it.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use vnoc_domain::{LocationKey, RepoPath, first_free_id, occurrences};

const WAIT: Duration = Duration::from_secs(60);
/// A valid ID for a file that has none (any text the driver writes as is).
const FILE_ID: &str = "7FJ3K9Q2M5X8B1D4H6N0R2T4V6";

type Check = Result<(), String>;

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

// ---- finding the drivers ----------------------------------------------------------------------------------------------------------------------

struct Found {
    name: String,
    dir: PathBuf,
}

/// Every folder of `drivers/`, by name. A folder that is not a driver (no `Cargo.toml`, or a package not named after the folder) is a failure of its
/// own, as it is for `scripts/drivers.mjs`.
fn find_drivers() -> Vec<Result<Found, String>> {
    let base = repo_root().join("drivers");
    let mut dirs: Vec<_> = std::fs::read_dir(&base)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", base.display()))
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().unwrap().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "drivers/ holds no driver");
    dirs.into_iter()
        .map(|name| {
            let dir = base.join(&name);
            let manifest = std::fs::read_to_string(dir.join("Cargo.toml"))
                .map_err(|_| format!("drivers/{name}/ has no Cargo.toml: every folder in drivers/ is a driver"))?;
            let package = manifest
                .split("\n[")
                .find(|section| section.trim_start_matches('[').starts_with("package]"))
                .unwrap_or("");
            let declared = package.lines().find_map(|line| {
                let value = line.strip_prefix("name")?.trim_start().strip_prefix('=')?;
                Some(value.trim().trim_matches('"').to_string())
            });
            if declared.as_deref() != Some(&format!("vnoc-driver-{name}")) {
                return Err(format!(
                    "drivers/{name}/Cargo.toml: the package is {declared:?}, and must be \"vnoc-driver-{name}\" (the folder's name)"
                ));
            }
            Ok(Found { name, dir })
        })
        .collect()
}

fn binary(driver: &Found) -> Result<PathBuf, String> {
    let path = driver.dir.join("target/debug").join(format!(
        "vnoc-driver-{}{}",
        driver.name,
        std::env::consts::EXE_SUFFIX
    ));
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "the {0} driver is not built at {1}: run `cd drivers/{0} && cargo build`",
            driver.name,
            path.display()
        ))
    }
}

/// What a driver brings: its input file and the outline it must give for it.
struct Fixture {
    /// `input.<ext>`
    file: String,
    ext: String,
    text: String,
    expected: Value,
}

fn fixture(driver: &Found) -> Result<Fixture, String> {
    let dir = driver.dir.join("conformance");
    let name = driver.name.as_str();
    if !dir.is_dir() {
        return Err(format!(
            "drivers/{name}/conformance/ is missing: a driver brings an input file and the outline it must give (docs/06 §6)"
        ));
    }
    let inputs: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|f| f.starts_with("input."))
        .collect();
    let [file] = inputs.as_slice() else {
        return Err(format!(
            "drivers/{name}/conformance/ must hold exactly one `input.<ext>`, it holds {inputs:?}"
        ));
    };
    let text = std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    let expected = std::fs::read_to_string(dir.join("expected-outline.json"))
        .map_err(|e| format!("drivers/{name}/conformance/expected-outline.json: {e}"))?;
    let expected = serde_json::from_str(&expected)
        .map_err(|e| format!("drivers/{name}/conformance/expected-outline.json: {e}"))?;
    Ok(Fixture {
        ext: file["input.".len()..].to_string(),
        file: file.clone(),
        text,
        expected,
    })
}

// ---- the driver on its wire -------------------------------------------------------------------------------------------------------------------

/// A driver process. A thread reads its stdout frame by frame (so a driver that stops answering is a timeout and not a hang), and keeps every byte
/// that is not inside a frame: the check at the end of a run reads them.
struct Wire {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: Receiver<Value>,
    stray: Arc<Mutex<Vec<String>>>,
    reader: Option<std::thread::JoinHandle<()>>,
    next: u64,
}

impl Wire {
    fn spawn(program: &Path) -> Wire {
        let mut child = Command::new(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", program.display()));
        let stdin = child.stdin.take();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let (send, frames) = channel();
        let stray: Arc<Mutex<Vec<String>>> = Arc::default();
        let found = stray.clone();
        let reader = std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                if out.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let Some(len) = line
                    .strip_prefix("Content-Length:")
                    .and_then(|n| n.trim().parse::<usize>().ok())
                else {
                    found.lock().unwrap().push(line.clone());
                    continue;
                };
                // the rest of the header, up to the blank line
                loop {
                    line.clear();
                    if out.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    if line == "\r\n" {
                        break;
                    }
                    if !line.starts_with("Content-Type:") {
                        found.lock().unwrap().push(line.clone());
                    }
                }
                let mut body = vec![0u8; len];
                if out.read_exact(&mut body).is_err() {
                    found.lock().unwrap().push("a frame cut short".into());
                    return;
                }
                match serde_json::from_slice(&body) {
                    Ok(value) => {
                        if send.send(value).is_err() {
                            return;
                        }
                    }
                    Err(e) => found
                        .lock()
                        .unwrap()
                        .push(format!("a frame that is not JSON: {e}")),
                }
            }
        });
        Wire {
            child,
            stdin,
            frames,
            stray,
            reader: Some(reader),
            next: 0,
        }
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_vec(&message).unwrap();
        let stdin = self.stdin.as_mut().expect("stdin is open");
        // a driver that has already stopped is seen by the next read, not by a broken pipe here
        let _ = write!(stdin, "Content-Length: {}\r\n\r\n", body.len());
        let _ = stdin.write_all(&body);
        let _ = stdin.flush();
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// `Ok(result)` or `Err(error object)`; a driver that does not answer is an `Err` of its own.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let until = Instant::now() + WAIT;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            match self.frames.recv_timeout(left) {
                Ok(frame) if frame["id"] == json!(id) => {
                    return match frame.get("error") {
                        Some(error) => Err(error.clone()),
                        None => Ok(frame["result"].clone()),
                    };
                }
                Ok(_) => {} // not the answer to this request
                Err(RecvTimeoutError::Timeout) => {
                    return Err(json!({ "message": format!("no answer to {method} in {WAIT:?}") }));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(
                        json!({ "message": format!("the driver stopped before it answered {method}") }),
                    );
                }
            }
        }
    }

    /// Like `request`, as a `Result` with a message: for the checks that expect an answer.
    fn ask(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request(method, params)
            .map_err(|e| format!("{method} was answered with an error: {e}"))
    }

    /// Closes stdin and waits for the process to end: its exit status (`None` if it did not end in time, and is then killed).
    fn end(&mut self) -> Option<std::process::ExitStatus> {
        self.stdin = None;
        let until = Instant::now() + WAIT;
        let status = loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                break Some(status);
            }
            if Instant::now() > until {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        status
    }

    /// Every byte that was outside a frame, over the whole run (call after `end`).
    fn stray(&self) -> Vec<String> {
        self.stray.lock().unwrap().clone()
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---- text, positions, IDs ---------------------------------------------------------------------------------------------------------------------

/// The byte offset of an LSP position (0-based line, UTF-16 `character`) in `text`.
fn offset(text: &str, position: &Value) -> usize {
    let line = position["line"].as_u64().unwrap() as usize;
    let mut character = position["character"].as_u64().unwrap() as usize;
    let start = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .nth(line)
        .unwrap_or_else(|| panic!("no line {line}"));
    let mut at = start;
    for c in text[start..].chars() {
        if character == 0 {
            break;
        }
        character = character
            .checked_sub(c.len_utf16())
            .unwrap_or_else(|| panic!("a position inside a character: {position}"));
        at += c.len_utf8();
    }
    at
}

/// The edits of a plan, applied as an editor would: they are all in the coordinates of `text`, so they go in from the end.
fn apply(text: &str, edits: &Value) -> String {
    let mut edits: Vec<(usize, usize, String)> = edits
        .as_array()
        .expect("`edits` is a list")
        .iter()
        .map(|e| {
            (
                offset(text, &e["range"]["start"]),
                offset(text, &e["range"]["end"]),
                e["newText"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut out = text.to_string();
    for (start, end, new) in edits {
        out.replace_range(start..end, &new);
    }
    out
}

fn hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

fn defs(outline: &Value) -> Vec<&Value> {
    outline["defs"]
        .as_array()
        .map(|d| d.iter().collect())
        .unwrap_or_default()
}

fn outline_of_text(wire: &mut Wire, text: &str) -> Result<Value, String> {
    wire.ask("vnoc/outline", json!({ "text": text }))
}

/// `(qualname, occurrence)` of every def, in the outline's order: the protocol counts same-named defs from 1.
fn keys(outline: &Value) -> Vec<(String, u32)> {
    let qualnames: Vec<&str> = defs(outline)
        .iter()
        .map(|d| d["qualname"].as_str().unwrap())
        .collect();
    qualnames
        .iter()
        .zip(occurrences(qualnames.iter().copied()))
        .map(|(q, o)| ((*q).to_string(), o))
        .collect()
}

/// A temp folder of its own for a driver's run, gone with the value. Not inside this repository.
struct Folder(PathBuf);

impl Folder {
    fn new(driver: &str) -> Folder {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vnoc-conformance-{driver}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).unwrap();
    }

    fn root(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---- the checks -------------------------------------------------------------------------------------------------------------------------------

fn is(condition: bool, message: impl FnOnce() -> String) -> Check {
    if condition { Ok(()) } else { Err(message()) }
}

fn same(what: &str, got: &Value, expected: &Value) -> Check {
    is(got == expected, || {
        format!(
            "{what}\n  got:      {}\n  expected: {}",
            serde_json::to_string(got).unwrap(),
            serde_json::to_string(expected).unwrap()
        )
    })
}

/// An outline against the expected one: on a difference, the first def that differs (a whole outline is too long to read).
fn same_outline(what: &str, got: &Value, expected: &Value) -> Check {
    if got == expected {
        return Ok(());
    }
    let (got_defs, expected_defs) = (defs(got), defs(expected));
    for at in 0..got_defs.len().max(expected_defs.len()) {
        let (a, b) = (got_defs.get(at).copied(), expected_defs.get(at).copied());
        if a != b {
            let show = |d: Option<&Value>| {
                d.map_or("no def".to_string(), |d| serde_json::to_string(d).unwrap())
            };
            return Err(format!(
                "{what}: def {at} differs\n  got:      {}\n  expected: {}",
                show(a),
                show(b)
            ));
        }
    }
    same(
        &format!("{what}: the outline differs outside its defs"),
        got,
        expected,
    )
}

/// A wrong protocol number is refused, with a reason, and the driver stops (checklist: "refuses an unknown protocol number").
fn refuses_a_wrong_protocol(program: &Path) -> Check {
    let mut wire = Wire::spawn(program);
    let refused = wire
        .request("initialize", json!({ "protocol": 999 }))
        .err()
        .ok_or("initialize with protocol 999 was answered, not refused")?;
    is(
        refused["code"].is_i64() && refused["message"].as_str().is_some_and(|m| !m.is_empty()),
        || format!("the refusal is not an error with a code and a reason: {refused}"),
    )?;
    wire.end();
    let stray = wire.stray();
    is(stray.is_empty(), || {
        format!("bytes on stdout outside a frame: {stray:?}")
    })
}

fn handshake(wire: &mut Wire, name: &str, ext: &str) -> Result<Value, String> {
    let init = wire.ask("initialize", json!({ "protocol": 1 }))?;
    wire.notify("initialized", json!({}));
    same("initialize.protocol", &init["protocol"], &json!(1))?;
    same(
        "initialize.name is the folder's name",
        &init["name"],
        &json!(name),
    )?;
    let extensions = init["files"]["extensions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    is(!extensions.is_empty(), || {
        "initialize.files.extensions is empty or missing".into()
    })?;
    is(extensions.contains(&json!(ext)), || {
        format!(
            "initialize.files.extensions {extensions:?} does not hold the input's extension `{ext}`"
        )
    })?;
    same(
        "initialize.capabilities.outline",
        &init["capabilities"]["outline"],
        &json!(true),
    )?;
    Ok(init)
}

fn unknown_messages(wire: &mut Wire, fixture: &Fixture) -> Check {
    wire.notify("$/aNotificationNobodyKnows", json!({ "x": 1 }));
    let error = wire
        .request("vnoc/aRequestNobodyKnows", json!({}))
        .err()
        .ok_or("an unknown request was answered with a result")?;
    same(
        "an unknown request's error code",
        &error["code"],
        &json!(-32601),
    )?;
    // still alive, after the notification and the request
    outline_of_text(wire, &fixture.text)
        .map(|_| ())
        .map_err(|e| format!("no answer after an unknown message: {e}"))
}

fn outline_by_text(wire: &mut Wire, fixture: &Fixture) -> Check {
    let got = outline_of_text(wire, &fixture.text)?;
    same_outline("outline by text", &got, &fixture.expected)
}

fn outline_by_file(wire: &mut Wire, fixture: &Fixture, folder: &Folder) -> Check {
    let got = wire.ask(
        "vnoc/outline",
        json!({ "root": folder.root(), "file": fixture.file }),
    )?;
    same_outline("outline by root + file", &got, &fixture.expected)
}

fn outline_of_a_marked_file(wire: &mut Wire, fixture: &Fixture, folder: &Folder) -> Check {
    let marked = format!("bom.{}", fixture.ext);
    folder.put(&marked, format!("\u{feff}{}", fixture.text).as_bytes());
    let got = wire.ask(
        "vnoc/outline",
        json!({ "root": folder.root(), "file": marked }),
    )?;
    same_outline(
        "outline of a file with a byte-order mark",
        &got,
        &fixture.expected,
    )
}

/// The same defs with the same names and lines when every line ends in CRLF.
fn outline_with_crlf(wire: &mut Wire, fixture: &Fixture) -> Check {
    let crlf = fixture.text.replace('\n', "\r\n");
    let got = outline_of_text(wire, &crlf)?;
    let shape = |outline: &Value| -> Vec<Value> {
        defs(outline)
            .iter()
            .map(|d| {
                json!([
                    d["qualname"],
                    d["name"],
                    d["kind"],
                    d["parent"],
                    d["id"],
                    d["range"]["start"]["line"],
                    d["range"]["end"]["line"]
                ])
            })
            .collect()
    };
    same(
        "with CRLF line ends: the defs, names, ids and lines",
        &json!(shape(&got)),
        &json!(shape(&fixture.expected)),
    )
}

/// `capabilities.ids.plan`: an ID for every def without one goes in, comes out of the next outline, and is not planned twice.
fn plans_ids(wire: &mut Wire, fixture: &Fixture) -> Check {
    let before = outline_of_text(wire, &fixture.text)?;
    let path = RepoPath::new(&fixture.file).map_err(|e| format!("{}: {e}", fixture.file))?;
    let mut given = Vec::new();
    let mut asks = Vec::new();
    for (def, (qualname, occurrence)) in defs(&before).into_iter().zip(keys(&before)) {
        if !def["id"].is_null() {
            continue;
        }
        let key =
            LocationKey::new(path.clone(), &qualname, occurrence).map_err(|e| e.to_string())?;
        let id = first_free_id(&key, |_| false)
            .map_err(|e| e.to_string())?
            .to_string();
        asks.push(json!({ "qualname": qualname, "occurrence": occurrence, "id": id, "replace_existing": false }));
        given.push((qualname, occurrence, id));
    }
    is(!given.is_empty(), || {
        "the input has no def without an ID: the check has nothing to plan".into()
    })?;

    let plan = wire.ask(
        "vnoc/planIds",
        json!({ "text": fixture.text, "version": 7, "ids": asks }),
    )?;
    same("planIds echoes the version", &plan["version"], &json!(7))?;
    same("planIds skipped", &plan["skipped"], &json!([]))?;
    same("planIds unmatched", &plan["unmatched"], &json!([]))?;
    let applied = apply(&fixture.text, &plan["edits"]);
    same(
        "planIds.text_hash is the blake3 of the applied text",
        &plan["text_hash"],
        &json!(hash(&applied)),
    )?;

    let after = outline_of_text(wire, &applied)?;
    let got: Vec<(String, u32, Option<String>)> = defs(&after)
        .into_iter()
        .zip(keys(&after))
        .map(|(d, (q, o))| (q, o, d["id"].as_str().map(str::to_string)))
        .collect();
    let want: Vec<(String, u32, Option<String>)> = defs(&before)
        .into_iter()
        .zip(keys(&before))
        .map(|(d, (q, o))| {
            let new = given
                .iter()
                .find(|(gq, go, _)| *gq == q && *go == o)
                .map(|(_, _, id)| id.clone());
            (q, o, new.or_else(|| d["id"].as_str().map(str::to_string)))
        })
        .collect();
    is(got == want, || {
        format!(
            "after applying the plan, each def has its ID\n  got:      {got:?}\n  expected: {want:?}"
        )
    })?;

    // planning again, for every def, writes nothing
    let all: Vec<Value> = got
        .iter()
        .map(|(q, o, id)| json!({ "qualname": q, "occurrence": o, "id": id, "replace_existing": false }))
        .collect();
    let again = wire.ask(
        "vnoc/planIds",
        json!({ "text": applied, "version": 8, "ids": all }),
    )?;
    same("planning again: no edit", &again["edits"], &json!([]))?;
    is(
        again["skipped"]
            .as_array()
            .is_some_and(|s| s.len() == got.len()),
        || {
            format!(
                "planning again skips every def ({}): {}",
                got.len(),
                again["skipped"]
            )
        },
    )
}

/// `capabilities.ids.file`: the file's ID goes in, is read back by the outline, and is not planned twice.
fn plans_a_file_id(wire: &mut Wire, fixture: &Fixture) -> Check {
    let plan = wire.ask(
        "vnoc/planFileId",
        json!({ "text": fixture.text, "version": 4, "id": FILE_ID }),
    )?;
    same("planFileId echoes the version", &plan["version"], &json!(4))?;
    let applied = apply(&fixture.text, &plan["edits"]);
    same(
        "planFileId.text_hash is the blake3 of the applied text",
        &plan["text_hash"],
        &json!(hash(&applied)),
    )?;
    let after = outline_of_text(wire, &applied)?;
    same(
        "after applying the plan, the outline's file_id",
        &after["file_id"],
        &json!(FILE_ID),
    )?;
    let again = wire.ask(
        "vnoc/planFileId",
        json!({ "text": applied, "version": 5, "id": "ANOTHERID" }),
    )?;
    same(
        "planning a file ID again: no edit",
        &again["edits"],
        &json!([]),
    )?;
    is(
        again["skipped"].as_str().is_some_and(|s| !s.is_empty()),
        || {
            format!(
                "planning a file ID again says why it did nothing: {}",
                again["skipped"]
            )
        },
    )
}

/// `capabilities.callTree`: the first function of the input, asked for as the entry, is the root of the tree.
fn call_tree(wire: &mut Wire, fixture: &Fixture, folder: &Folder) -> Check {
    let (_, (qualname, occurrence)) = defs(&fixture.expected)
        .into_iter()
        .zip(keys(&fixture.expected))
        .find(|(d, _)| d["kind"] == "function")
        .ok_or("the expected outline has no function to start a call tree from")?;
    let tree = wire.ask(
        "vnoc/callTree",
        json!({ "root": folder.root(), "file": fixture.file, "entry": { "qualname": qualname, "occurrence": occurrence } }),
    )?;
    let target = &tree["root"]["target"];
    same(
        "callTree root.target.qualname",
        &target["qualname"],
        &json!(qualname),
    )?;
    same(
        "callTree root.target.occurrence",
        &target["occurrence"],
        &json!(occurrence),
    )?;
    same(
        "callTree root.target.file",
        &target["file"],
        &json!(fixture.file),
    )
}

fn shuts_down(wire: &mut Wire) -> Check {
    wire.ask("shutdown", Value::Null)?;
    wire.notify("exit", Value::Null);
    let status = wire
        .end()
        .ok_or("the driver did not end after shutdown + exit")?;
    is(status.success(), || {
        format!("the driver ended with {status} after shutdown + exit, not 0")
    })?;
    let stray = wire.stray();
    is(stray.is_empty(), || {
        format!("bytes on stdout outside a frame: {stray:?}")
    })
}

/// Every check of one driver; each failure is a line of the report.
fn run(driver: &Found) -> Vec<String> {
    let mut failures = Vec::new();
    let mut record = |name: &str, result: Check| {
        if let Err(why) = result {
            failures.push(format!("{name}: {why}"));
        }
    };
    let program = match binary(driver) {
        Ok(program) => program,
        Err(why) => return vec![why],
    };
    let fixture = match fixture(driver) {
        Ok(fixture) => fixture,
        Err(why) => return vec![why],
    };

    record(
        "a wrong protocol is refused",
        refuses_a_wrong_protocol(&program),
    );

    let folder = Folder::new(&driver.name);
    folder.put(&fixture.file, fixture.text.as_bytes());
    let mut wire = Wire::spawn(&program);
    match handshake(&mut wire, &driver.name, &fixture.ext) {
        Err(why) => {
            record("handshake", Err(why));
            return failures;
        }
        Ok(init) => {
            let can = |capability: &str| init["capabilities"][capability] == json!(true);
            record(
                "an unknown notification is ignored, an unknown request is -32601",
                unknown_messages(&mut wire, &fixture),
            );
            record("outline by text", outline_by_text(&mut wire, &fixture));
            record(
                "outline by root + file",
                outline_by_file(&mut wire, &fixture, &folder),
            );
            record(
                "outline of a file with a byte-order mark",
                outline_of_a_marked_file(&mut wire, &fixture, &folder),
            );
            record(
                "outline with CRLF line ends",
                outline_with_crlf(&mut wire, &fixture),
            );
            if can("ids.plan") {
                record("ids.plan", plans_ids(&mut wire, &fixture));
            }
            if can("ids.file") {
                record("ids.file", plans_a_file_id(&mut wire, &fixture));
            }
            if can("callTree") {
                record("callTree", call_tree(&mut wire, &fixture, &folder));
            }
        }
    }
    record(
        "shutdown + exit end the process with 0, and stdout held only frames",
        shuts_down(&mut wire),
    );
    failures
}

#[test]
fn every_driver_passes_the_conformance_test() {
    let mut report = Vec::new();
    let mut checked = Vec::new();
    for found in find_drivers() {
        match found {
            Err(why) => report.push(format!("drivers/: {why}")),
            Ok(driver) => {
                let failures = run(&driver);
                if failures.is_empty() {
                    checked.push(driver.name);
                } else {
                    let list: Vec<String> = failures
                        .iter()
                        .map(|f| format!("    - {}", f.replace('\n', "\n      ")))
                        .collect();
                    report.push(format!(
                        "driver `{}` fails:\n{}",
                        driver.name,
                        list.join("\n")
                    ));
                }
            }
        }
    }
    assert!(
        report.is_empty(),
        "\n{}\n(passed: {checked:?})",
        report.join("\n")
    );
}
