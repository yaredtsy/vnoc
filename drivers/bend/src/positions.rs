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

    /// The analyzer's own positions: 1-based line, 1-based column counted in characters, a leading byte-order mark not counted (no editor shows
    /// it). A column past the end of its line stops at the end.
    pub fn position_1based(&self, (line, column): (u32, u32)) -> Position {
        let line = (line.saturating_sub(1) as usize).min(self.starts.len() - 1);
        let from = self.starts[line];
        let from = if line == 0 && self.text.starts_with('\u{feff}') {
            from + '\u{feff}'.len_utf8()
        } else {
            from
        };
        let rest = self.text[from..].chars().take_while(|&c| c != '\n');
        let character = rest
            .take(column.saturating_sub(1) as usize)
            .map(char::len_utf16)
            .sum::<usize>();
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
