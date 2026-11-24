//! A def's text, from the file on disk: what `vnoc show` and `vnoc code` print.
//!
//! **What the driver's range is** (measured on the Python driver in A3, with a decorated method and a class): `range.start.line` is the line of the
//! first decorator, when the def has any (`@property` above `def total`), and the line of `def` or `class` when it has none; `range.end.line` is
//! the **last line of the def itself** (inclusive, 0-based), and `end.character` is the end of that line, **not** column 0 of the next one. The blank
//! lines after a def are not in it, and a class ends where its last method does. So the lines of a def are `start.line..=end.line`, and the
//! numbers a person reads are one more.

use std::io;
use std::path::Path;

use vnoc_driver_protocol::Range;

/// Some lines of a file, with where they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excerpt {
    /// the 1-based number of `lines[0]`
    pub first: u32,
    pub lines: Vec<String>,
    /// how many lines the whole file has
    pub total: u32,
}

impl Excerpt {
    /// The 1-based number of the last line, or `first - 1` when there are none.
    pub fn last(&self) -> u32 {
        self.first + self.lines.len() as u32 - u32::from(!self.lines.is_empty())
    }
}

/// A file's text as every reader of source here sees it: bytes that are not UTF-8 become U+FFFD and a byte order mark is dropped.
pub fn read_text(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// A file's text as lines, the way every reader of source here splits it: `\r\n` ends a line like `\n`, and the last line needs no end. (The text is
/// what [`read_text`] returned: a byte order mark and bytes that are not UTF-8 are dealt with there.)
pub fn split_lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

/// The lines of `range` (0-based and inclusive, as the driver gives it) with `context` lines more on each side, read from `path`. The context stops
/// at the file's first and last line, and a range past the end of the file (it was shortened since the index was built) gives the lines that are there.
/// A UTF-8 byte order mark is dropped, `\r\n` ends a line like `\n`, and bytes that are not UTF-8 become U+FFFD.
pub fn excerpt(path: &Path, range: &Range, context: u32) -> io::Result<Excerpt> {
    let text = read_text(path)?;
    let all = split_lines(&text);
    let total = all.len() as u32;
    let start = range.start.line.saturating_sub(context);
    let end = range
        .end
        .line
        .saturating_add(context)
        .min(total.saturating_sub(1));
    let lines: Vec<String> = if total == 0 || start > end {
        Vec::new()
    } else {
        all[start as usize..=end as usize]
            .iter()
            .map(|line| line.to_string())
            .collect()
    };
    Ok(Excerpt {
        first: start + 1,
        lines,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::Position;

    fn lines(start: u32, end: u32) -> Range {
        let at = |line| Position { line, character: 0 };
        Range {
            start: at(start),
            end: at(end),
        }
    }

    fn file(label: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("vnoc-source-{label}-{}.py", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn the_range_is_inclusive_and_the_numbers_start_at_one() {
        let path = file("inclusive", b"a\nb\nc\nd\ne\n");
        let got = excerpt(&path, &lines(1, 2), 0).unwrap();
        assert_eq!((got.first, got.last(), got.total), (2, 3, 5));
        assert_eq!(got.lines, ["b", "c"]);
    }

    #[test]
    fn context_stops_at_the_ends_of_the_file() {
        let path = file("context", b"a\nb\nc\nd\ne\n");
        let got = excerpt(&path, &lines(1, 2), 5).unwrap();
        assert_eq!((got.first, got.last()), (1, 5));
        assert_eq!(got.lines.len(), 5);
        let got = excerpt(&path, &lines(2, 2), 1).unwrap();
        assert_eq!(
            (got.first, got.lines),
            (2, vec!["b".into(), "c".into(), "d".into()])
        );
    }

    #[test]
    fn crlf_and_a_byte_order_mark_are_not_part_of_the_lines() {
        let path = file("crlf", b"\xef\xbb\xbfone\r\ntwo\r\nthree");
        let got = excerpt(&path, &lines(0, 2), 0).unwrap();
        assert_eq!(got.lines, ["one", "two", "three"]);
        assert_eq!(got.total, 3);
    }

    #[test]
    fn a_range_past_the_end_gives_what_is_there() {
        let path = file("past", b"a\nb\n");
        let got = excerpt(&path, &lines(1, 9), 0).unwrap();
        assert_eq!((got.first, got.lines), (2, vec!["b".to_string()]));
        let got = excerpt(&path, &lines(7, 9), 0).unwrap();
        assert!(got.lines.is_empty());
        let empty = excerpt(&file("empty", b""), &lines(0, 3), 0).unwrap();
        assert!(empty.lines.is_empty() && empty.total == 0);
    }

    #[test]
    fn a_file_that_is_not_there_is_an_io_error() {
        assert!(excerpt(Path::new("/no/such/file.py"), &lines(0, 1), 0).is_err());
    }
}
