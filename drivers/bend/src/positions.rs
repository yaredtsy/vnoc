//! Positions on the wire: **0-based line, UTF-16 code-unit `character`** (as LSP and VS Code), converted from byte offsets.
//! A copy of drivers/python's, not a shared crate: each driver is its own workspace.

use bend_syntax::Span;
use vnoc_driver_protocol::{Position, Range};

pub struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Lines { text, starts }
    }

    pub fn position(&self, offset: u32) -> Position {
        let off = (offset as usize).min(self.text.len());
        let line = self.starts.partition_point(|&s| s <= off) - 1;
        let character = self.text[self.starts[line]..off].encode_utf16().count();
        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    pub fn range(&self, span: Span) -> Range {
        Range {
            start: self.position(span.start),
            end: self.position(span.end),
        }
    }
}
