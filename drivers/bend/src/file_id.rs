//! The file marker: a `# FileID: <ID>` comment at the top of a file, before the first `import` (`05-identity/03-files.md` §1).
//!
//! Where it goes and how it is read are `bend_syntax`'s (`header_marker`, `plan_header_marker`); this file puts the answer in the wire's shape.

use analyzer_core::edit::apply;
use bend_syntax::{MarkerStyle, Skip};
use vnoc_driver_protocol::{PlanFileIdParams, PlanFileIdResult, Range, TextEdit, Written};

use crate::positions::Lines;

/// How the file's marker is written: the label is `FileID`, a comment line like a def's.
fn style() -> MarkerStyle<'static> {
    MarkerStyle::above("FileID")
}

/// The ID text of the first `FileID` comment in the header (the comments before the first `import`, or the first declaration), as written.
pub fn read(text: &str) -> Option<String> {
    bend_syntax::header_marker(text, style())
}

pub fn plan(params: PlanFileIdParams) -> PlanFileIdResult {
    let text = params.text.as_str();
    let done = |skipped: Option<String>| PlanFileIdResult {
        version: params.version,
        edits: vec![],
        written: None,
        skipped,
        text_hash: blake3::hash(text.as_bytes()).to_hex().to_string(),
    };
    let edit = match bend_syntax::plan_header_marker(text, &params.id, style()) {
        Ok(edit) => edit,
        Err(Skip::AlreadyMarked { .. }) => return done(Some("already has a FileID".into())),
        Err(other) => return done(Some(other.to_string())),
    };
    let Some(new_text) = apply(text, std::slice::from_ref(&edit)) else {
        return done(Some("the planned edit does not fit the text".into()));
    };

    // The written span is the comment with its own line break. A break that only separates it from the line above is not part of it, and neither
    // is the empty line the analyzer adds after it when the first def sits where it goes.
    let inserted = edit.text;
    let leading = inserted.len() - inserted.trim_start_matches(['\r', '\n']).len();
    let comment_end = inserted.trim_end_matches(['\r', '\n']).len();
    let rest = &inserted[comment_end..];
    let own_break = if rest.starts_with("\r\n") {
        2
    } else {
        rest.len().min(1)
    };
    let written_end = comment_end + own_break;
    let new = Lines::new(&new_text);
    let position = |byte: usize| new.position(u32::try_from(byte).unwrap_or(u32::MAX));
    let start = edit.at.start as usize + leading;
    let end = edit.at.start as usize + written_end;
    let written_text = inserted[leading..written_end].to_string();
    PlanFileIdResult {
        version: params.version,
        edits: vec![TextEdit {
            range: Lines::new(text).range(edit.at),
            new_text: inserted,
        }],
        written: Some(Written {
            id: params.id,
            qualname: String::new(),
            occurrence: 1,
            span: Range {
                start: position(start),
                end: position(end),
            },
            text: written_text,
        }),
        skipped: None,
        text_hash: blake3::hash(new_text.as_bytes()).to_hex().to_string(),
    }
}
