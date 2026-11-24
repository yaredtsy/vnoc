//! The hash of some lines: what says a stop or a block is still the code it was written about (`docs/26-tours/01-on-disk.md` §7).
//!
//! `of` is the whole rule: a line that holds an ID marker is left out, white space at the end of a line is left out, the lines are joined with
//! `\n` (so the line end of the file never counts), BLAKE3, the first 16 hexadecimal characters. Everything else counts, the indent too.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::source;

/// 16 hexadecimal characters: the first eight bytes of a BLAKE3 hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 8]);

/// What is not a hash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a hash: 16 hexadecimal characters")]
pub struct HashError(String);

impl Hash {
    /// Exactly 16 hexadecimal characters; capitals are read, and written back in lower case.
    pub fn parse(text: &str) -> Result<Hash, HashError> {
        let bad = || HashError(text.to_string());
        if text.len() != 16 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let mut bytes = [0u8; 8];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
        }
        Ok(Hash(bytes))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl FromStr for Hash {
    type Err = HashError;
    fn from_str(text: &str) -> Result<Hash, HashError> {
        Hash::parse(text)
    }
}

/// A JSON string of 16 hexadecimal characters; a line with any other `hash` is not a record (`record.rs`).
impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Hash::parse(&text).map_err(de::Error::custom)
    }
}

/// The hash of `lines`. A line that contains any of `marker_ids` (the ID text of the def itself, of a def inside its lines, or the file's `FileID`:
/// the caller names them) is dropped; white space at the end of every other line is dropped; what is left is joined with `\n`. An empty id in
/// `marker_ids` is ignored: it would be in every line.
pub fn of(lines: &[&str], marker_ids: &[&str]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    let mut first = true;
    for line in lines {
        if marker_ids
            .iter()
            .any(|id| !id.is_empty() && line.contains(id))
        {
            continue;
        }
        if !first {
            hasher.update(b"\n");
        }
        first = false;
        hasher.update(line.trim_end().as_bytes());
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
    Hash(bytes)
}

/// Lines `first` to `last` (1-based, both included) of a file's text, split as `source.rs` splits it (`\r\n` ends a line like `\n`). A byte order mark at the
/// start of `text` is not part of the first line, as `source::read_text` has it. A range past the end gives the lines that are there, and
/// `first` of 0 is read as 1; a range that is backwards, or starts after the last line, gives none.
pub fn lines_of(text: &str, first: u32, last: u32) -> Vec<&str> {
    let all = source::split_lines(text.strip_prefix('\u{feff}').unwrap_or(text));
    let from = first.max(1) as usize - 1;
    let to = (last as usize).min(all.len());
    if from >= to {
        return Vec::new();
    }
    all[from..to].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
    const NESTED: &str = "3M1X9C7B5N2V8K4J6H0G1F3D5S";

    fn text_hash(text: &str) -> String {
        let joined = source::split_lines(text).join("\n");
        blake3::hash(joined.as_bytes()).to_hex()[..16].to_string()
    }

    #[test]
    fn it_is_the_first_16_hex_characters_of_the_blake3_of_the_lines_joined_with_a_newline() {
        let cases: [(&[&str], &str); 4] = [
            (&["def f():", "    return 1"], "def f():\n    return 1"),
            (&["one"], "one"),
            (&["", ""], "\n"),
            (&[], ""),
        ];
        for (lines, joined) in cases {
            let want = blake3::hash(joined.as_bytes()).to_hex()[..16].to_string();
            assert_eq!(of(lines, &[]).to_string(), want, "{lines:?}");
        }
        // pinned: the same on every machine, now and later
        assert_eq!(of(&["a", "b"], &[]).to_string(), text_hash("a\nb"));
        assert_eq!(of(&[], &[]).to_string(), "af1349b9f5f9a1a6");
        assert_eq!(of(&["a", "b"], &[]).to_string().len(), 16);
    }

    #[test]
    fn the_line_end_and_the_white_space_at_the_end_of_a_line_do_not_count() {
        let lf = "def f(x):\n    y = x + 1\n    return y\n";
        let crlf = "def f(x):\r\n    y = x + 1\r\n    return y\r\n";
        let trailing = "def f(x):   \n    y = x + 1\t\n    return y \n";
        let no_final_end = "def f(x):\n    y = x + 1\n    return y";
        let bom = "\u{feff}def f(x):\n    y = x + 1\n    return y\n";
        let hash = |text: &str| of(&lines_of(text, 1, 3), &[]);
        let want = hash(lf);
        for (what, text) in [
            ("crlf", crlf),
            ("trailing white space", trailing),
            ("no final line end", no_final_end),
            ("a byte order mark", bom),
        ] {
            assert_eq!(hash(text), want, "{what}");
        }
        // lines handed over with their `\r`, as a careless caller would: the same
        assert_eq!(
            of(&["def f(x):\r", "    y = x + 1\r", "    return y\r"], &[]),
            want
        );
    }

    #[test]
    fn a_changed_character_and_a_changed_indent_change_it() {
        let base = ["def f(x):", "    y = x + 1", "    return y"];
        let want = of(&base, &[]);
        let changed: [(&str, [&str; 3]); 6] = [
            (
                "one character",
                ["def f(x):", "    y = x + 2", "    return y"],
            ),
            (
                "a renamed variable",
                ["def f(x):", "    z = x + 1", "    return y"],
            ),
            (
                "a deeper indent",
                ["def f(x):", "     y = x + 1", "    return y"],
            ),
            (
                "a shallower indent",
                ["def f(x):", "    y = x + 1", "   return y"],
            ),
            (
                "a tab for spaces",
                ["def f(x):", "\ty = x + 1", "    return y"],
            ),
            (
                "a comment",
                ["def f(x):  # new", "    y = x + 1", "    return y"],
            ),
        ];
        for (what, lines) in changed {
            assert_ne!(of(&lines, &[]), want, "{what}");
        }
        // white space inside a line, and a blank line in the middle, count
        assert_ne!(of(&["a  b"], &[]), of(&["a b"], &[]));
        assert_ne!(of(&["a", "", "b"], &[]), of(&["a", "b"], &[]));
        // order counts
        assert_ne!(of(&["a", "b"], &[]), of(&["b", "a"], &[]));
        // and so does where a line ends: two lines are not one
        assert_ne!(of(&["ab"], &[]), of(&["a", "b"], &[]));
    }

    #[test]
    fn a_line_with_an_id_marker_is_left_out() {
        let marker = format!("    \"\"\"ID: {ID}\"\"\"");
        let with = ["def f():", marker.as_str(), "    return 1"];
        let without = ["def f():", "    return 1"];
        assert_eq!(of(&with, &[ID]), of(&without, &[ID]));
        assert_eq!(of(&without, &[ID]), of(&without, &[]));
        // the marker line counts when its ID is not named: the caller says which ids are markers
        assert_ne!(of(&with, &[]), of(&without, &[]));
        // the ID of a def nested in the lines: its line is left out as well
        let nested_marker = format!("        \"\"\"ID: {NESTED}\"\"\"");
        let outer = [
            "def outer():",
            marker.as_str(),
            "    def inner():",
            nested_marker.as_str(),
            "        return 1",
            "    return inner()",
        ];
        let bare = [
            "def outer():",
            "    def inner():",
            "        return 1",
            "    return inner()",
        ];
        assert_eq!(of(&outer, &[ID, NESTED]), of(&bare, &[]));
        assert_ne!(
            of(&outer, &[ID]),
            of(&bare, &[]),
            "the nested id was not named"
        );
        // the line is left out whatever else is on it, and at the end of the lines too
        assert_eq!(
            of(&["a", &format!("# FileID: {ID} (x)")], &[ID]),
            of(&["a"], &[])
        );
        assert_eq!(of(&[marker.as_str()], &[ID]), of(&[], &[]));
        // an empty id is nothing: it is not in every line
        assert_eq!(of(&["a", "b"], &[""]), of(&["a", "b"], &[]));
    }

    #[test]
    fn lines_are_split_as_source_rs_splits_them() {
        let text = "one\r\ntwo\nthree\r\n\r\nfive";
        assert_eq!(lines_of(text, 1, 5), ["one", "two", "three", "", "five"]);
        assert_eq!(lines_of(text, 2, 3), ["two", "three"]);
        assert_eq!(lines_of(text, 3, 3), ["three"]);
        // the same lines `source::excerpt` gives for the same range (0-based there)
        let dir = std::env::temp_dir().join(format!("vnoc-tours-hash-{}", std::process::id()));
        crate::testdir::remove_at_end(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.py");
        let bytes = b"\xef\xbb\xbfone\r\ntwo\xff\nthree\r\n";
        std::fs::write(&file, bytes).unwrap();
        let range = |a, b| {
            let at = |line| vnoc_driver_protocol::Position { line, character: 0 };
            vnoc_driver_protocol::Range {
                start: at(a),
                end: at(b),
            }
        };
        let excerpt = source::excerpt(&file, &range(0, 2), 0).unwrap();
        let read = source::read_text(&file).unwrap();
        assert_eq!(lines_of(&read, 1, 3), excerpt.lines);
        // a file's text that still has its byte order mark (from `git show`): the same first line
        let raw = String::from_utf8_lossy(bytes);
        assert_eq!(lines_of(&raw, 1, 3), excerpt.lines);
        assert_eq!(lines_of(&raw, 1, 1), ["one"]);
        assert_eq!(excerpt.lines[1], "two\u{fffd}");
        let _ = std::fs::remove_dir_all(&dir);
        // out of range
        assert_eq!(lines_of(text, 4, 99), ["", "five"]);
        assert_eq!(lines_of(text, 0, 2), ["one", "two"]);
        assert!(lines_of(text, 6, 9).is_empty());
        assert!(lines_of(text, 3, 2).is_empty());
        assert!(lines_of("", 1, 3).is_empty());
    }

    #[test]
    fn a_hash_prints_in_lower_case_and_parses_back() {
        let hash = of(&["x"], &[]);
        let text = hash.to_string();
        assert_eq!(Hash::parse(&text).unwrap(), hash);
        assert_eq!(text.parse::<Hash>().unwrap(), hash);
        assert_eq!(Hash::parse(&text.to_uppercase()).unwrap(), hash);
        // capitals are read, and what is kept and printed is lower case
        let from_capitals = Hash::parse("0123456789ABCDEF").unwrap();
        assert_eq!(from_capitals.to_string(), "0123456789abcdef");
        assert_eq!(from_capitals, Hash::parse("0123456789abcdef").unwrap());
        assert_eq!(
            Hash::parse("AbCdEf0123456789").unwrap().to_string(),
            "abcdef0123456789"
        );
        assert_eq!(Hash::parse(&text.to_uppercase()).unwrap().to_string(), text);
        assert_eq!(
            Hash::parse("0123456789abcdef").unwrap().to_string(),
            "0123456789abcdef"
        );
        for bad in [
            "",
            "abc",
            "0123456789abcde",
            "0123456789abcdef0",
            "0123456789abcdeg",
            "0123456789abcde ",
            "-123456789abcdef",
            "0123456789abcdé",
        ] {
            assert!(Hash::parse(bad).is_err(), "{bad:?}");
        }
    }
}
