//! `vnoc/planIds` and `vnoc/planFileId` over the wire: the edits are applied here, as the editor would, and the text is outlined again.

mod common;

use common::Driver;
use serde_json::{Value, json};
use vnoc_domain::{LocationKey, RepoPath, first_free_id, occurrences};

const KINDS: &str = include_str!("fixtures/kinds.bend");

/// Applies a plan's edits to ASCII `text` (positions are line/character; for ASCII a character is a byte). The edits are all in the
/// coordinates of `text`, so they go in from the end.
fn apply_edits(text: &str, edits: &[Value]) -> String {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |p: &Value| {
        starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
    };
    let mut out = text.to_string();
    for e in edits.iter().rev() {
        let (start, end) = (offset(&e["range"]["start"]), offset(&e["range"]["end"]));
        out.replace_range(start..end, e["newText"].as_str().unwrap());
    }
    out
}

fn blake3_hex(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

fn ask(qualname: &str, occurrence: u32, id: &str) -> Value {
    json!({ "qualname": qualname, "occurrence": occurrence, "id": id, "replace_existing": false })
}

fn outline(d: &mut Driver, text: &str) -> Value {
    d.request("vnoc/outline", json!({ "text": text }))
        .expect("outline answers")
}

/// What `text` reads at `range` (ASCII text, positions as bytes).
fn slice<'a>(text: &'a str, range: &Value) -> &'a str {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |p: &Value| {
        starts[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
    };
    &text[offset(&range["start"])..offset(&range["end"])]
}

/// The IDs the core would pick for every def of `text`, one per (qualname, occurrence), in the order of the outline.
fn core_ids(file: &str, outline: &Value) -> Vec<(String, u32, String)> {
    let qualnames: Vec<&str> = outline["defs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["qualname"].as_str().unwrap())
        .collect();
    let path = RepoPath::new(file).unwrap();
    qualnames
        .iter()
        .zip(occurrences(qualnames.iter().copied()))
        .map(|(q, o)| {
            let key = LocationKey::new(path.clone(), *q, o).unwrap();
            let id = first_free_id(&key, |_| false).unwrap().to_string();
            ((*q).to_string(), o, id)
        })
        .collect()
}

#[test]
fn an_id_for_every_def_round_trips_and_a_law_and_its_proof_each_have_their_own() {
    let mut d = Driver::start();
    let before = outline(&mut d, KINDS);
    let ids = core_ids("kinds.bend", &before);
    assert_eq!(ids.len(), 7);
    let asks: Vec<Value> = ids.iter().map(|(q, o, id)| ask(q, *o, id)).collect();

    let plan = d
        .request(
            "vnoc/planIds",
            json!({ "text": KINDS, "version": 7, "ids": asks }),
        )
        .expect("planIds answers");
    assert_eq!(plan["version"], 7, "the version is echoed");
    assert_eq!(plan["skipped"], json!([]));
    assert_eq!(plan["unmatched"], json!([]));
    assert_eq!(plan["edits"].as_array().unwrap().len(), 7);

    let new_text = apply_edits(KINDS, plan["edits"].as_array().unwrap());
    assert_eq!(
        plan["text_hash"],
        blake3_hash(&new_text),
        "the hash is of the text the edits make"
    );
    // directly above the declaration, below the doc comment, above `@unsafe`
    assert!(
        new_text.contains(&format!(
            "# Adds two numbers.\n# ID: {}\ndef add(",
            ids[0].2
        )),
        "{new_text}"
    );
    assert!(
        new_text.contains(&format!("# ID: {}\n@unsafe def raw(", ids[1].2)),
        "{new_text}"
    );

    let after = outline(&mut d, &new_text);
    let read: Vec<(String, String)> = after["defs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["qualname"].as_str().unwrap().to_string(),
                x["id"].as_str().expect("every def has its ID").to_string(),
            )
        })
        .collect();
    let wanted: Vec<(String, String)> = ids
        .iter()
        .map(|(q, _, id)| (q.clone(), id.clone()))
        .collect();
    assert_eq!(read, wanted);
    assert_ne!(read[3].1, read[4].1, "the law and its proof, each its own");
    assert_eq!(read[3].0, read[4].0);

    // `written`: where each line is in the NEW text, and what it says
    let written = plan["written"].as_array().unwrap();
    assert_eq!(written.len(), 7);
    for (w, (q, o, id)) in written.iter().zip(&ids) {
        assert_eq!(
            (
                w["qualname"].as_str().unwrap(),
                w["occurrence"].as_u64().unwrap() as u32
            ),
            (q.as_str(), *o)
        );
        assert_eq!(w["id"], id.as_str());
        assert_eq!(
            slice(&new_text, &w["span"]),
            w["text"].as_str().unwrap(),
            "the span is the inserted text, in the new text"
        );
        assert_eq!(w["text"], format!("# ID: {id}\n"));
    }
    d.finish();
}

fn blake3_hash(text: &str) -> Value {
    Value::String(blake3_hex(text))
}

#[test]
fn a_def_that_has_an_id_a_syntax_error_or_no_name_in_the_text_is_not_written() {
    let mut d = Driver::start();
    let text = "# ID: first-1\ndef first() -> Nat: 0\n\ndef fine() -> Nat: 1\n\ndef broken(a: Nat, , b) -> Nat:\n  body\n\ndef last() -> Nat: 1\n";
    let plan = d
        .request(
            "vnoc/planIds",
            json!({ "text": text, "version": 3, "ids": [
                ask("first", 1, "AAAA"), ask("fine", 1, "BBBB"), ask("broken", 1, "CCCC"),
                ask("nowhere", 1, "DDDD"), ask("fine", 2, "EEEE"),
                json!({ "qualname": "first", "occurrence": 1, "id": "FFFF", "replace_existing": true }),
            ] }),
        )
        .unwrap();
    let new_text = apply_edits(text, plan["edits"].as_array().unwrap());
    assert_eq!(
        new_text.matches("# ID: ").count(),
        2,
        "first-1 and BBBB:\n{new_text}"
    );
    assert!(new_text.contains("# ID: BBBB\ndef fine("));
    let skipped: Vec<(&str, &str)> = plan["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["qualname"].as_str().unwrap(),
                s["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(skipped.len(), 3, "{skipped:?}");
    assert_eq!(skipped[0], ("first", "already has an ID"));
    assert_eq!(skipped[1].0, "broken");
    assert!(skipped[1].1.contains("syntax error"), "{skipped:?}");
    assert_eq!(skipped[2], ("first", "replacing an ID is not supported"));
    let unmatched: Vec<(&str, u64)> = plan["unmatched"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["qualname"].as_str().unwrap(),
                s["occurrence"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(unmatched, [("nowhere", 1), ("fine", 2)]);
    assert_eq!(plan["text_hash"], blake3_hash(&new_text));
    d.finish();
}

#[test]
fn a_def_marked_by_the_plan_is_skipped_the_second_time() {
    let mut d = Driver::start();
    let text = "def f() -> Nat: 0\n";
    let plan = d
        .request(
            "vnoc/planIds",
            json!({ "text": text, "version": 1, "ids": [ask("f", 1, "AAAA")] }),
        )
        .unwrap();
    let new_text = apply_edits(text, plan["edits"].as_array().unwrap());
    assert_eq!(new_text, "# ID: AAAA\ndef f() -> Nat: 0\n");
    let again = d
        .request(
            "vnoc/planIds",
            json!({ "text": new_text, "version": 2, "ids": [ask("f", 1, "ZZZZ")] }),
        )
        .unwrap();
    assert_eq!(again["edits"], json!([]));
    assert_eq!(again["skipped"][0]["reason"], "already has an ID");
    assert_eq!(
        again["text_hash"],
        blake3_hash(&new_text),
        "nothing changed"
    );
    d.finish();
}

#[test]
fn plan_ids_with_occurrence_zero_is_invalid_params() {
    // the protocol counts same-named defs from 1: 0 is a malformed request, and no def is touched for it
    let mut d = Driver::start();
    let err = d
        .request(
            "vnoc/planIds",
            json!({ "text": "def f() -> Nat: 0\n", "version": 1, "ids": [ask("f", 1, "AAAA"), ask("f", 0, "AAAA")] }),
        )
        .unwrap_err();
    assert_eq!(err["code"], -32602);
    assert_eq!(err["message"], "occurrence of f starts at 1, not 0");
    d.finish();
}

/// Plans a `FileID` for `text` and applies it. Returns the plan and the new text.
fn plan_file_id(d: &mut Driver, text: &str, id: &str) -> (Value, String) {
    let plan = d
        .request(
            "vnoc/planFileId",
            json!({ "text": text, "version": 4, "id": id }),
        )
        .expect("planFileId answers");
    assert_eq!(plan["version"], 4, "the version is echoed");
    let new_text = apply_edits(text, plan["edits"].as_array().unwrap());
    assert_eq!(plan["text_hash"], blake3_hash(&new_text));
    (plan, new_text)
}

#[test]
fn a_file_id_goes_first_and_before_the_imports() {
    let mut d = Driver::start();
    let text = include_str!("fixtures/imports.bend");
    assert!(outline(&mut d, text)["file_id"].is_null());
    let (plan, new_text) = plan_file_id(&mut d, text, "AAAA");
    assert_eq!(new_text, format!("# FileID: AAAA\n{text}"));
    assert_eq!(plan["written"]["text"], "# FileID: AAAA\n");
    assert_eq!(
        plan["written"]["span"]["start"],
        json!({ "line": 0, "character": 0 })
    );
    assert_eq!(
        plan["written"]["span"]["end"],
        json!({ "line": 1, "character": 0 })
    );
    assert_eq!(
        slice(&new_text, &plan["written"]["span"]),
        "# FileID: AAAA\n"
    );
    assert_eq!(outline(&mut d, &new_text)["file_id"], "AAAA");
    d.finish();
}

#[test]
fn a_file_without_imports_does_not_hand_its_file_id_to_its_first_def() {
    let mut d = Driver::start();
    let text = include_str!("fixtures/no_imports.bend");
    let (plan, new_text) = plan_file_id(&mut d, text, "AAAA");
    // an empty line keeps the comment apart from the doc comment of `f`
    assert_eq!(new_text, format!("# FileID: AAAA\n\n{text}"));
    assert_eq!(plan["written"]["text"], "# FileID: AAAA\n");
    assert_eq!(
        slice(&new_text, &plan["written"]["span"]),
        "# FileID: AAAA\n",
        "the empty line after it is not part of the span"
    );
    let out = outline(&mut d, &new_text);
    assert_eq!(out["file_id"], "AAAA");
    let defs = out["defs"].as_array().unwrap();
    assert!(defs[0]["id"].is_null(), "the first def took no ID: {out}");
    assert_eq!(defs[0]["qualname"], "f");
    d.finish();
}

#[test]
fn a_file_without_imports_and_a_def_on_its_first_line() {
    let mut d = Driver::start();
    let text = "def f() -> Nat: 0\n";
    let (_, new_text) = plan_file_id(&mut d, text, "AAAA");
    assert_eq!(new_text, "# FileID: AAAA\n\ndef f() -> Nat: 0\n");
    let out = outline(&mut d, &new_text);
    assert_eq!(out["file_id"], "AAAA");
    assert!(out["defs"][0]["id"].is_null());
    d.finish();
}

#[test]
fn the_file_id_goes_above_a_licence_comment_and_is_read_back() {
    let mut d = Driver::start();
    let text = include_str!("fixtures/licence.bend");
    let (_, new_text) = plan_file_id(&mut d, text, "AAAA");
    assert_eq!(new_text, format!("# FileID: AAAA\n{text}"));
    assert_eq!(outline(&mut d, &new_text)["file_id"], "AAAA");
    d.finish();
}

#[test]
fn a_byte_order_mark_stays_first() {
    let mut d = Driver::start();
    let text = "\u{feff}import Base\n\ndef main() -> Nat: 0\n";
    let plan = d
        .request(
            "vnoc/planFileId",
            json!({ "text": text, "version": 1, "id": "AAAA" }),
        )
        .unwrap();
    let edit = &plan["edits"][0];
    // the BOM is one UTF-16 unit: the comment goes after it
    assert_eq!(edit["range"]["start"], json!({ "line": 0, "character": 1 }));
    assert_eq!(edit["newText"], "# FileID: AAAA\n");
    let new_text = "\u{feff}# FileID: AAAA\nimport Base\n\ndef main() -> Nat: 0\n";
    assert_eq!(plan["text_hash"], blake3_hash(new_text));
    assert_eq!(outline(&mut d, new_text)["file_id"], "AAAA");
    d.finish();
}

#[test]
fn crlf_text_keeps_crlf() {
    let mut d = Driver::start();
    let text = "import Base\r\n\r\ndef main() -> Nat: 0\r\n";
    let (plan, new_text) = plan_file_id(&mut d, text, "AAAA");
    assert_eq!(new_text, format!("# FileID: AAAA\r\n{text}"));
    assert_eq!(plan["written"]["text"], "# FileID: AAAA\r\n");
    // and a def's ID, too
    let ids = d
        .request(
            "vnoc/planIds",
            json!({ "text": text, "version": 2, "ids": [ask("main", 1, "BBBB")] }),
        )
        .unwrap();
    assert_eq!(ids["written"][0]["text"], "# ID: BBBB\r\n");
    d.finish();
}

#[test]
fn a_file_that_has_one_is_left_alone() {
    let mut d = Driver::start();
    let text = "#FileID:   AAAA  \nimport Base\n\ndef main() -> Nat: 0\n";
    assert_eq!(outline(&mut d, text)["file_id"], "AAAA");
    let plan = d
        .request(
            "vnoc/planFileId",
            json!({ "text": text, "version": 3, "id": "BBBB" }),
        )
        .unwrap();
    assert_eq!(plan["edits"], json!([]));
    assert!(plan["written"].is_null());
    assert_eq!(plan["skipped"], "already has a FileID");
    assert_eq!(plan["version"], 3);
    assert_eq!(plan["text_hash"], blake3_hash(text));
    d.finish();
}

#[test]
fn a_def_id_directly_above_the_first_def_of_a_file_without_imports_is_not_the_file_id() {
    let mut d = Driver::start();
    let out = outline(&mut d, "# ID: first-1\ndef first() -> Nat: 0\n");
    assert!(out["file_id"].is_null());
    assert_eq!(out["defs"][0]["id"], "first-1");
    d.finish();
}
