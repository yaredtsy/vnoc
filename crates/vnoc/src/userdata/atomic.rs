//! Every write under `.vnoc/` goes through here, so a crash never leaves a half-written file (`04-storage/03-writing-safely.md` §2).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to a temporary file in the same folder, then renames it over `target`. A reader sees the old file or the
/// new one, never a mix. (Same folder: a rename across disks is a copy, and a copy is not atomic.)
///
/// A `target` that is a symlink is written through: the bytes go to the file it points at and the link stays a link
/// (a `CLAUDE.md` that points at `AGENTS.md` is still that link afterwards).
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let linked;
    let target = if fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink()) {
        // a link that points nowhere is replaced by a file, as before
        linked = fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
        linked.as_path()
    } else {
        target
    };
    let dir = target.parent().expect("a file has a parent folder");
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .expect("a file has a name")
            .to_string_lossy(),
        std::process::id()
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?; // the bytes are really on the disk before the rename
    } // closed here: Windows needs that before a rename
    // a file that is replaced keeps its permissions (a script's executable bit): the code files V-NOC writes an ID into are not its own
    if let Ok(existing) = fs::metadata(target) {
        fs::set_permissions(&tmp, existing.permissions())?;
    }
    fs::rename(&tmp, target)
}

/// Writes a file that must not exist yet (a new doc): an existing one is an `AlreadyExists` error, never overwritten.
pub fn create_new(target: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(target.parent().expect("a file has a parent folder"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-atomic-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        crate::testdir::remove_at_end(&d);
        d
    }

    #[test]
    fn write_atomic_creates_folders_replaces_and_leaves_no_temp_file() {
        let d = fresh("write");
        let file = d.join("a/b/node.toml");
        write_atomic(&file, b"one").unwrap();
        write_atomic(&file, b"two").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "two");
        assert_eq!(
            fs::read_dir(file.parent().unwrap()).unwrap().count(),
            1,
            "only the file itself is left"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_written_through_and_stays_a_link() {
        let d = fresh("link");
        let real = d.join("AGENTS.md");
        let link = d.join("CLAUDE.md");
        write_atomic(&real, b"one").unwrap();
        std::os::unix::fs::symlink("AGENTS.md", &link).unwrap();
        write_atomic(&link, b"two").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "two");
    }

    #[cfg(unix)]
    #[test]
    fn a_replaced_file_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let d = fresh("mode");
        let file = d.join("run.py");
        write_atomic(&file, b"one").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        write_atomic(&file, b"two").unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn create_new_never_overwrites() {
        let d = fresh("new");
        let file = d.join("doc.md");
        create_new(&file, b"mine").unwrap();
        assert_eq!(
            create_new(&file, b"theirs").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    }
}
