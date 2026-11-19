//! Who applies the edits that put an ID into a file (`docs/21-agent-cli/04-write-commands.md` §2).
//!
//! The driver **plans** the `ID:` line as text edits; somebody has to apply them. In VS Code that is the editor (`workspace/applyEdit`, so
//! the change is one undo step in the buffer the user has open). In a terminal nobody is on the other end: the file on disk is the document.
//! The flows in `attach.rs`, `relink.rs` and `write_id.rs` do not know which: an `Err` from here is what they already handle when the
//! editor refuses (the doc is attached by name, with a notice).

use std::io;
use std::path::Path;

use async_lsp::lsp_types::{
    ApplyWorkspaceEditParams, DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier,
    TextDocumentEdit, Url, WorkspaceEdit,
};
use async_lsp::{ClientSocket, LanguageClient};
use vnoc_driver_protocol::{Position, TextEdit};

use crate::protocol::Document;
use crate::userdata::atomic::write_atomic;

pub enum EditSink {
    /// `workspace/applyEdit`, with the document's version
    Editor(ClientSocket),
    /// the file on disk is the document
    Disk,
}

impl EditSink {
    pub async fn apply(
        &mut self,
        document: &Document,
        label: &str,
        edits: &[TextEdit],
    ) -> Result<(), String> {
        match self {
            EditSink::Editor(client) => apply_in_editor(client, document, label, edits).await,
            EditSink::Disk => apply_on_disk(document, edits),
        }
    }

    pub fn is_disk(&self) -> bool {
        matches!(self, EditSink::Disk)
    }
}

async fn apply_in_editor(
    client: &mut ClientSocket,
    document: &Document,
    label: &str,
    edits: &[TextEdit],
) -> Result<(), String> {
    let edit = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier {
            uri: document.uri.clone(),
            version: Some(document.version),
        },
        edits: edits.iter().map(|e| OneOf::Left(text_edit(e))).collect(),
    };
    let workspace_edit = WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(vec![edit])),
        ..Default::default()
    };
    let answer = client
        .apply_edit(ApplyWorkspaceEditParams {
            label: Some(label.into()),
            edit: workspace_edit,
        })
        .await
        .map_err(|e| e.to_string())?;
    if answer.applied {
        Ok(())
    } else {
        Err(answer.failure_reason.unwrap_or_else(|| {
            "the editor did not apply the edit; the document changed meanwhile?".into()
        }))
    }
}

fn text_edit(e: &TextEdit) -> async_lsp::lsp_types::TextEdit {
    let position = |p: Position| async_lsp::lsp_types::Position {
        line: p.line,
        character: p.character,
    };
    async_lsp::lsp_types::TextEdit {
        range: async_lsp::lsp_types::Range {
            start: position(e.range.start),
            end: position(e.range.end),
        },
        new_text: e.new_text.clone(),
    }
}

/// The file is read again: the plan was made for `document.text`, and a plan for other bytes would put the line in the wrong place.
fn apply_on_disk(document: &Document, edits: &[TextEdit]) -> Result<(), String> {
    let path = document
        .uri
        .to_file_path()
        .map_err(|()| format!("{} is not a file", document.uri))?;
    let now = std::fs::read(&path).map_err(|e| e.to_string())?;
    if now != document.text.as_bytes() {
        return Err("the file changed while the ID was being written".into());
    }
    let new_text = apply_text_edits(&document.text, &past_the_bom(&document.text, edits))?;
    write_atomic(&path, new_text.as_bytes()).map_err(|e| e.to_string())
}

/// The driver plans for the text it is given, and has no notion of a byte-order mark: "the start of the file" is line 0, character 0. In an
/// editor that is right (its buffer has no mark), but the text of a file read from disk keeps U+FEFF as its first character, and a comment
/// put before it would leave the mark at the start of line 2, which Python refuses. So here a position at the very start is the place after the mark.
fn past_the_bom(text: &str, edits: &[TextEdit]) -> Vec<TextEdit> {
    let mut edits = edits.to_vec();
    if text.starts_with('\u{feff}') {
        for position in edits
            .iter_mut()
            .flat_map(|e| [&mut e.range.start, &mut e.range.end])
        {
            if (position.line, position.character) == (0, 0) {
                position.character = 1; // U+FEFF is one UTF-16 unit
            }
        }
    }
    edits
}

impl Document {
    /// The file as the driver will plan against it: version `0`, the text decoded **verbatim** (line ends as they are, a byte-order mark kept
    /// as U+FEFF). A file that is not UTF-8 is `InvalidData`: no document can be made of it.
    pub fn from_disk(path: &Path) -> io::Result<Document> {
        let uri = Url::from_file_path(path).map_err(|()| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not an absolute path", path.display()),
            )
        })?;
        let text = String::from_utf8(std::fs::read(path)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "the file is not UTF-8 text")
        })?;
        Ok(Document {
            uri,
            version: 0,
            text,
        })
    }
}

/// Applies `edits` (all against `text`) and returns the new text. The driver's position rule, exactly (`drivers/python/src/positions.rs`):
/// 0-based `line`, lines split on `\n` (so a `\r` is part of its line's text), and `character` in UTF-16 code units from the line's start.
///
/// An edit outside the text, one that ends before it starts, or two that overlap are refused, and nothing is applied. Inserts at one place
/// keep their order in `edits`.
pub fn apply_text_edits(text: &str, edits: &[TextEdit]) -> Result<String, String> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let offset = |p: Position| -> Result<usize, String> {
        let outside = || {
            format!(
                "an edit at line {}, character {} is outside the text",
                p.line, p.character
            )
        };
        let from = *starts.get(p.line as usize).ok_or_else(outside)?;
        let line = &text[from..];
        let line = &line[..line.find('\n').unwrap_or(line.len())];
        let mut units = 0u32;
        for (at, c) in line.char_indices() {
            if units == p.character {
                return Ok(from + at);
            }
            units += c.len_utf16() as u32;
            if units > p.character {
                return Err(outside()); // inside a character that takes two units
            }
        }
        if units == p.character {
            Ok(from + line.len())
        } else {
            Err(outside())
        }
    };

    let mut spans: Vec<(usize, usize, &str)> = Vec::with_capacity(edits.len());
    for e in edits {
        let (start, end) = (offset(e.range.start)?, offset(e.range.end)?);
        if end < start {
            return Err("an edit ends before it starts".into());
        }
        spans.push((start, end, &e.new_text));
    }
    spans.sort_by_key(|&(start, end, _)| (start, end)); // stable: inserts at one place keep their order
    if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("two edits overlap".into());
    }
    let mut new_text = text.to_string();
    for &(start, end, replacement) in spans.iter().rev() {
        new_text.replace_range(start..end, replacement);
    }
    Ok(new_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::Range;

    fn edit(from: (u32, u32), to: (u32, u32), new_text: &str) -> TextEdit {
        TextEdit {
            range: Range {
                start: Position {
                    line: from.0,
                    character: from.1,
                },
                end: Position {
                    line: to.0,
                    character: to.1,
                },
            },
            new_text: new_text.into(),
        }
    }

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-edits-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn positions_follow_the_drivers_rule() {
        // (text, edits, the new text); "é" is one UTF-16 unit, the emoji is two
        let table: Vec<(&str, Vec<TextEdit>, &str)> = vec![
            (
                "caf\u{e9}\nx = 1\n",
                vec![edit((1, 0), (1, 0), "# new\n")],
                "caf\u{e9}\n# new\nx = 1\n",
            ),
            (
                "caf\u{e9} \u{1F600} end\n",
                vec![edit((0, 7), (0, 7), "!")],
                "caf\u{e9} \u{1F600}! end\n", // 4 + space + 2 units = 7: right after the emoji
            ),
            (
                "caf\u{e9} \u{1F600}\nnext\n",
                vec![edit((1, 4), (1, 4), "?")],
                "caf\u{e9} \u{1F600}\nnext?\n",
            ),
            (
                "a\r\nb\r\n",
                vec![edit((1, 0), (1, 0), "# x\r\n")],
                "a\r\n# x\r\nb\r\n", // the inserted text is the edit's: nothing is converted
            ),
            (
                "a\r\nb\r\n",
                vec![edit((0, 2), (0, 2), "!")],
                "a\r!\nb\r\n", // the '\r' is part of line 0: its end is after it
            ),
            (
                "\u{feff}def f():\n    pass\n",
                vec![edit((0, 1), (0, 1), "# FileID: X\n")],
                "\u{feff}# FileID: X\ndef f():\n    pass\n", // the mark is the first unit of line 0
            ),
            (
                "one\ntwo\nthree\n",
                vec![edit((2, 0), (2, 5), "3"), edit((0, 0), (0, 3), "1")],
                "1\ntwo\n3\n", // given last-first, applied without moving each other
            ),
            (
                "ab\n",
                vec![edit((0, 1), (0, 1), "X"), edit((0, 1), (0, 1), "Y")],
                "aXYb\n", // inserts at one place keep their order
            ),
            ("ab\n", vec![edit((1, 0), (1, 0), "tail")], "ab\ntail"), // the empty last line
            ("ab\n", vec![], "ab\n"),
        ];
        for (text, edits, expected) in table {
            assert_eq!(
                apply_text_edits(text, &edits).as_deref(),
                Ok(expected),
                "{text:?}"
            );
        }
    }

    #[test]
    fn an_edit_outside_the_text_or_two_that_overlap_are_refused() {
        let text = "ab\ncd\n";
        for (what, edits) in [
            ("a line past the end", vec![edit((3, 0), (3, 0), "x")]),
            (
                "a character past the line's end",
                vec![edit((0, 3), (0, 3), "x")],
            ),
            ("the end before the start", vec![edit((1, 1), (0, 0), "x")]),
            (
                "overlapping",
                vec![edit((0, 0), (0, 2), "x"), edit((0, 1), (0, 1), "y")],
            ),
            (
                "overlapping ranges",
                vec![edit((0, 0), (1, 1), "x"), edit((1, 0), (1, 2), "y")],
            ),
        ] {
            assert!(apply_text_edits(text, &edits).is_err(), "{what}");
        }
        assert!(
            apply_text_edits("\u{1F600}\n", &[edit((0, 1), (0, 1), "x")]).is_err(),
            "the middle of a character is not a place"
        );
        assert_eq!(
            apply_text_edits(
                text,
                &[edit((0, 0), (0, 1), "x"), edit((0, 1), (0, 2), "y")]
            )
            .as_deref(),
            Ok("xy\ncd\n"),
            "ranges that only touch are fine"
        );
    }

    #[test]
    fn disk_applies_the_edit_atomically() {
        let dir = fresh("apply");
        let file = dir.join("a.py");
        std::fs::write(&file, "def f():\n    pass\n").unwrap();
        let document = Document::from_disk(&file).unwrap();
        assert_eq!(document.version, 0);
        apply_on_disk(
            &document,
            &[edit((1, 0), (1, 0), "    \"\"\"ID: X\"\"\"\n")],
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "def f():\n    \"\"\"ID: X\"\"\"\n    pass\n"
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temp file is left"
        );
    }

    #[test]
    fn disk_puts_an_insert_at_the_start_of_a_file_after_its_byte_order_mark() {
        let dir = fresh("bom");
        let file = dir.join("a.py");
        std::fs::write(&file, b"\xef\xbb\xbfx = 1\n").unwrap();
        let document = Document::from_disk(&file).unwrap();
        apply_on_disk(&document, &[edit((0, 0), (0, 0), "# FileID: X\n")]).unwrap();
        assert_eq!(
            std::fs::read(&file).unwrap(),
            b"\xef\xbb\xbf# FileID: X\nx = 1\n"
        );
    }

    #[test]
    fn disk_refuses_a_file_that_changed_after_the_plan_and_leaves_it_alone() {
        let dir = fresh("changed");
        let file = dir.join("a.py");
        std::fs::write(&file, "def f():\n    pass\n").unwrap();
        let document = Document::from_disk(&file).unwrap();
        std::fs::write(&file, "def f():\n    return 1\n").unwrap();
        let err = apply_on_disk(&document, &[edit((1, 0), (1, 0), "x\n")]).unwrap_err();
        assert_eq!(err, "the file changed while the ID was being written");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            b"def f():\n    return 1\n",
            "byte for byte as the other writer left it"
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn a_file_that_is_not_utf8_makes_no_document() {
        let dir = fresh("latin1");
        let file = dir.join("a.py");
        std::fs::write(&file, b"# caf\xe9\n").unwrap();
        let err = Document::from_disk(&file).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_byte_order_mark_and_line_ends_are_kept_in_the_document() {
        let dir = fresh("verbatim");
        let file = dir.join("a.py");
        std::fs::write(&file, b"\xef\xbb\xbfa\r\nb\r\n").unwrap();
        assert_eq!(
            Document::from_disk(&file).unwrap().text,
            "\u{feff}a\r\nb\r\n"
        );
    }
}
