//! The file marker: a `# FileID: <ID>` comment in the header of a module (`05-identity/03-files.md` §1).
//!
//! Placement: after the leading run of comment lines, which holds a shebang (the OS needs it first), an encoding line (Python honours
//! it on line 1 or 2 only) and a licence block (checkers expect it at the top), and before the first statement, the module
//! docstring included. A comment before the docstring does not change `__doc__`.
//!
//! This works on lines of text and not on the syntax tree: the header is made of comments, which the parser throws away.

use ruff_text_size::TextSize;
use vnoc_driver_protocol::{PlanFileIdParams, PlanFileIdResult, Range, TextEdit, Written};

use crate::positions::Lines;

/// The ID text of the first `FileID` comment in the header (comments and blank lines before the first statement), as written.
/// Reading is tolerant: `#FileID:` and extra spaces are fine.
pub fn read(text: &str) -> Option<String> {
    header(text).find_map(|(_, line)| id_in(line)).map(str::to_string)
}

/// `# FileID: X` → `X`; `None` for any other line.
fn id_in(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start().strip_prefix("FileID")?.trim_start().strip_prefix(':')?;
    rest.split_whitespace().next()
}

/// The lines before the first statement, with their numbers: only comments and blank lines.
fn header(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().take_while(|(_, l)| l.trim().is_empty() || l.trim_start().starts_with('#'))
}

/// The line the marker goes on: the one after the leading run of comment lines. A shebang and an encoding line are comment lines,
/// so they stay above it, and the marker never lands between them.
fn insertion_line(text: &str) -> usize {
    text.lines().take_while(|l| l.trim_start().starts_with('#')).count()
}

pub fn plan(params: PlanFileIdParams) -> PlanFileIdResult {
    let text = params.text.as_str();
    let done = |skipped: Option<String>| PlanFileIdResult { version: params.version, edits: vec![], written: None, skipped, text_hash: blake3::hash(text.as_bytes()).to_hex().to_string() };
    if read(text).is_some() {
        return done(Some("already has a FileID".into()));
    }

    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let line = insertion_line(text);
    let lines = Lines::new(text);
    // the byte offset where that line starts; past the end when the header runs to the end of the text
    let offset = text.split_inclusive('\n').take(line).map(str::len).sum::<usize>();
    let comment = format!("# FileID: {}", params.id);
    // the last line has no line break: the new line needs one in front of it, and has none behind
    let (inserted, ends_open) = if offset == text.len() && !text.is_empty() && !text.ends_with('\n') { (format!("{newline}{comment}"), true) } else { (format!("{comment}{newline}"), false) };

    let at = lines.position(TextSize::try_from(offset).expect("a text under 4 GB"));
    let new_text = format!("{}{inserted}{}", &text[..offset], &text[offset..]);
    let new = Lines::new(&new_text);
    let start = offset + if ends_open { newline.len() } else { 0 };
    let end = start + comment.len() + if ends_open { 0 } else { newline.len() };
    let position = |byte: usize| new.position(TextSize::try_from(byte).expect("a text under 4 GB"));
    PlanFileIdResult {
        version: params.version,
        edits: vec![TextEdit { range: Range { start: at, end: at }, new_text: inserted.clone() }],
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
        let offset = (0..=text.len()).filter(|&o| text.is_char_boundary(o)).find(|&o| lines.position(TextSize::try_from(o).unwrap()) == edit.range.start).unwrap();
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
