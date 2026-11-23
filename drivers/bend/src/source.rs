//! A file read from disk as the core reads it (`crates/vnoc/src/source.rs`): bytes that are not UTF-8 become U+FFFD and a leading byte-order mark
//! is dropped. A copy, not a shared crate: each driver is its own workspace. The core sends text with the mark already gone, so a position
//! computed on this text is the same on both paths.

use std::io;
use std::path::Path;

pub fn read_text(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(label: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vnoc-driver-source-{label}-{}.txt",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn the_mark_is_dropped_and_bad_bytes_become_the_replacement_character() {
        assert_eq!(read_text(&file("bom", b"\xef\xbb\xbfab")).unwrap(), "ab");
        assert_eq!(read_text(&file("bad", b"a\xffb")).unwrap(), "a\u{fffd}b");
        assert_eq!(read_text(&file("plain", b"ab")).unwrap(), "ab");
    }

    #[test]
    fn a_file_that_is_not_there_is_an_io_error() {
        assert!(read_text(Path::new("/no/such/file")).is_err());
    }
}
