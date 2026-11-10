//! Every write under `.vnoc/` goes through here, so a crash never leaves a half-written file (`04-storage/03-writing-safely.md` §2).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to a temporary file in the same folder, then renames it over `target`. A reader sees the old file or the
/// new one, never a mix. (Same folder: a rename across disks is a copy, and a copy is not atomic.)
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = target.parent().expect("a file has a parent folder");
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp-{}", target.file_name().expect("a file has a name").to_string_lossy(), std::process::id()));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?; // the bytes are really on the disk before the rename
    } // closed here: Windows needs that before a rename
    fs::rename(&tmp, target)
}

/// Writes a file that must not exist yet (a new doc): an existing one is an `AlreadyExists` error, never overwritten.
pub fn create_new(target: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(target.parent().expect("a file has a parent folder"))?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(target)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-atomic-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn write_atomic_creates_folders_replaces_and_leaves_no_temp_file() {
        let d = fresh("write");
        let file = d.join("a/b/node.toml");
        write_atomic(&file, b"one").unwrap();
        write_atomic(&file, b"two").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "two");
        assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1, "only the file itself is left");
    }

    #[test]
    fn create_new_never_overwrites() {
        let d = fresh("new");
        let file = d.join("doc.md");
        create_new(&file, b"mine").unwrap();
        assert_eq!(create_new(&file, b"theirs").unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    }
}
