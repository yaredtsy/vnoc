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

    /// pylspt's own positions: 1-based line, 1-based column counted in characters. A column past the end of its line stops at the end.
    pub fn position_1based(&self, (line, column): (u32, u32)) -> Position {
        let line = (line.saturating_sub(1) as usize).min(self.starts.len() - 1);
        let rest = self.text[self.starts[line]..].chars().take_while(|&c| c != '\n');
        let character = rest.take(column.saturating_sub(1) as usize).map(char::len_utf16).sum::<usize>();
        Position { line: line as u32, character: character as u32 }
    }

    pub fn range(&self, range: TextRange) -> Range {
        Range { start: self.position(range.start()), end: self.position(range.end()) }
    }
}
