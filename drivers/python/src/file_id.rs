//! The file marker: a `# FileID: <ID>` comment in the header of a module (`05-identity/03-files.md` §1).
//!
//! Where it goes (after the leading comments, before the first statement) and how it is read are `python_syntax`'s
//! (`header_marker`, `plan_header_marker`); this file puts the answer in the wire's shape.

use analyzer_core::edit::apply;
use python_syntax::{MarkerStyle, Skip};
use vnoc_driver_protocol::{PlanFileIdParams, PlanFileIdResult, Range, TextEdit, Written};

use crate::positions::Lines;

/// How the file's marker is written: the label is `FileID`, and the header is a comment whatever the placement of a def's marker.
fn style() -> MarkerStyle<'static> {
    MarkerStyle::docstring("FileID")
}

/// The ID text of the first `FileID` comment in the header (comments and blank lines before the first statement), as written.
/// Reading is tolerant: `#FileID:` and extra spaces are fine.
pub fn read(text: &str) -> Option<String> {
    python_syntax::header_marker(text, style())
}

pub fn plan(params: PlanFileIdParams) -> PlanFileIdResult {
    let text = params.text.as_str();
    let done = |skipped: Option<String>| PlanFileIdResult { version: params.version, edits: vec![], written: None, skipped, text_hash: blake3::hash(text.as_bytes()).to_hex().to_string() };
    let edit = match python_syntax::plan_header_marker(text, &params.id, style()) {
        Ok(edit) => edit,
        Err(Skip::AlreadyMarked { .. }) => return done(Some("already has a FileID".into())),
        Err(other) => return done(Some(other.to_string())),
    };
    let Some(new_text) = apply(text, std::slice::from_ref(&edit)) else {
        return done(Some("the planned edit does not fit the text".into()));
    };

    // The comment is the written span, with its own line break when it has one: a break that only separates it from the line above is not.
    let inserted = edit.text;
    let leading = inserted.len() - inserted.trim_start_matches(['\r', '\n']).len();
    let new = Lines::new(&new_text);
    let position = |byte: usize| new.position(u32::try_from(byte).unwrap_or(u32::MAX));
    let start = edit.at.start as usize + leading;
    let end = edit.at.start as usize + inserted.len();
    PlanFileIdResult {
        version: params.version,
        edits: vec![TextEdit { range: Lines::new(text).range(edit.at), new_text: inserted.clone() }],
        written: Some(Written { id: params.id, qualname: String::new(), occurrence: 1, span: Range { start: position(start), end: position(end) }, text: inserted }),
        skipped: None,
        text_hash: blake3::hash(new_text.as_bytes()).to_hex().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    fn apply(text: &str) -> String {
        let result = plan(PlanFileIdParams { text: text.into(), version: 1, id: ID.into() });
        let [edit] = &result.edits[..] else { panic!("one edit, got {:?} ({:?})", result.edits, result.skipped) };
        let lines = Lines::new(text);
        let offset = (0..=text.len()).filter(|&o| text.is_char_boundary(o)).find(|&o| lines.position(u32::try_from(o).unwrap()) == edit.range.start).unwrap();
        let new = format!("{}{}{}", &text[..offset], edit.new_text, &text[offset..]);
        assert_eq!(result.text_hash, blake3::hash(new.as_bytes()).to_hex().to_string(), "the hash is of the new text");
        new
    }

    #[test]
    fn goes_before_the_docstring_and_after_the_header() {
        let before = "#!/usr/bin/env python3\n# -*- coding: utf-8 -*-\n# Copyright 2026 Acme.\n\"\"\"Orders module.\"\"\"\n\nimport json\n";
        let after = format!("#!/usr/bin/env python3\n# -*- coding: utf-8 -*-\n# Copyright 2026 Acme.\n# FileID: {ID}\n\"\"\"Orders module.\"\"\"\n\nimport json\n");
        assert_eq!(apply(before), after);
        assert_eq!(read(&after).as_deref(), Some(ID));
    }

    #[test]
    fn an_empty_file_gets_it_as_the_first_line() {
        assert_eq!(apply(""), format!("# FileID: {ID}\n"));
    }

    #[test]
    fn a_header_without_a_final_line_break_still_works() {
        assert_eq!(apply("# only a comment"), format!("# only a comment\n# FileID: {ID}"));
    }

    #[test]
    fn crlf_files_keep_crlf() {
        assert_eq!(apply("x = 1\r\n"), format!("# FileID: {ID}\r\nx = 1\r\n"));
    }

    #[test]
    fn a_file_that_has_one_is_left_alone() {
        let text = format!("#FileID:   {ID}  \nx = 1\n");
        assert_eq!(read(&text).as_deref(), Some(ID));
        let result = plan(PlanFileIdParams { text, version: 3, id: "other".into() });
        assert!(result.edits.is_empty() && result.skipped.is_some() && result.version == 3);
    }

    #[test]
    fn a_comment_after_the_first_statement_is_not_the_marker() {
        assert_eq!(read(&format!("x = 1\n# FileID: {ID}\n")), None);
    }
}
