//! Byte offsets into UTF-16 offsets.
//!
//! Rust indexes a string by byte, JavaScript and LSP by UTF-16 code unit. A name with `é` (two bytes, one unit) or an emoji (four bytes, two
//! units) would draw the wrong letters bold if a byte range were sent as it is (README Q4). The conversion is done once, here.

/// The UTF-16 range of the byte range `start..end` of `text`. Both ends must lie on a char boundary.
pub(crate) fn range(text: &str, start: usize, end: usize) -> (u32, u32) {
    let units = |s: &str| s.encode_utf16().count() as u32;
    let from = units(&text[..start]);
    (from, from + units(&text[start..end]))
}

#[cfg(test)]
mod tests {
    use super::range;

    #[test]
    fn ascii_is_unchanged() {
        assert_eq!(range("checkout", 2, 5), (2, 5));
    }

    #[test]
    fn two_bytes_are_one_unit() {
        // "café_order": `é` is bytes 3..5, `order` starts at byte 6 and unit 5
        assert_eq!(range("café_order", 6, 8), (5, 7));
    }

    #[test]
    fn four_bytes_are_two_units() {
        // "😀ab": the emoji is bytes 0..4 and units 0..2
        assert_eq!(range("😀ab", 4, 6), (2, 4));
        assert_eq!(range("😀ab", 0, 4), (0, 2));
    }
}
