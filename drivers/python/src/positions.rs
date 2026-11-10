//! Positions on the wire: **0-based line, UTF-16 code-unit `character`** (as LSP and VS Code), converted from byte offsets.

use ruff_text_size::{TextRange, TextSize};
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

    pub fn position(&self, offset: TextSize) -> Position {
        let off = offset.to_usize().min(self.text.len());
        let line = self.starts.partition_point(|&s| s <= off) - 1;
        let character = self.text[self.starts[line]..off].encode_utf16().count();
        Position { line: line as u32, character: character as u32 }
    }

    pub fn range(&self, range: TextRange) -> Range {
        Range { start: self.position(range.start()), end: self.position(range.end()) }
    }
}
