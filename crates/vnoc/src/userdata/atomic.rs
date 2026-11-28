//! Every write under `.vnoc/` goes through here, so a crash never leaves a half-written file (`04-storage/03-writing-safely.md` §2).

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Counts the writes of this process, so no two writes of one file ever share a temporary file.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// How long a write waits before each retry (about two seconds in all).
const BACKOFF_MS: [u64; 8] = [10, 20, 40, 80, 160, 320, 640, 730];

/// Writes `bytes` to a temporary file in the same folder, then renames it over `target`. A reader sees the old file or the
/// new one, never a mix. (Same folder: a rename across disks is a copy, and a copy is not atomic.)
///
/// A `target` that is a symlink is written through: the bytes go to the file it points at and the link stays a link
/// (a `CLAUDE.md` that points at `AGENTS.md` is still that link afterwards).
///
/// On Windows a rename over a file that another process holds open without delete sharing (a scanner, an indexer) is refused with "Access is
/// denied" or a sharing violation. That is waited out, about two seconds, each retry said on standard error with the step Windows refused. A
/// write that still fails is an error naming `target`, the temporary file is removed, and the target keeps its old bytes: a save that failed
/// never looks like it worked.
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
        ".{}.tmp-{}-{}",
        target
            .file_name()
            .expect("a file has a name")
            .to_string_lossy(),
        std::process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let mut retries = 0;
    loop {
        let (step, error) = match attempt(target, &tmp, bytes) {
            Ok(()) => return Ok(()),
            Err(failed) => failed,
        };
        let _ = fs::remove_file(&tmp);
        if !is_busy(&error) {
            return Err(error);
        }
        let Some(wait) = BACKOFF_MS.get(retries).copied() else {
            return Err(io::Error::new(
                error.kind(),
                format!("could not replace {} ({step}): {error}", target.display()),
            ));
        };
        retries += 1;
        // standard error (the crate's `print_stderr` is denied, and a log line that cannot be written is not worth failing the write for)
        let _ = writeln!(
            io::stderr(),
            "vnoc: writing {}: {step} failed ({error}); retry {retries} in {wait} ms",
            target.display()
        );
        std::thread::sleep(Duration::from_millis(wait));
    }
}

/// One try: the temporary file made, written, and renamed over `target`. The step that failed comes with the error.
fn attempt(target: &Path, tmp: &Path, bytes: &[u8]) -> Result<(), (&'static str, io::Error)> {
    {
        let mut file = fs::File::create(tmp).map_err(|e| ("temp create", e))?;
        file.write_all(bytes).map_err(|e| ("write", e))?;
        file.sync_all().map_err(|e| ("write", e))?; // the bytes are really on the disk before the rename
    } // closed here: Windows needs that before a rename
    // a file that is replaced keeps its permissions (a script's executable bit): the code files V-NOC writes an ID into are not its own
    if let Ok(existing) = fs::metadata(target) {
        fs::set_permissions(tmp, existing.permissions()).map_err(|e| ("permissions", e))?;
    }
    fs::rename(tmp, target).map_err(|e| ("rename", e))
}

/// A refusal that another process may lift: only on Windows (on Unix a refused write stays refused). Rust reads "Access is denied" as
/// `PermissionDenied`, and leaves a sharing or lock violation as an unnamed kind, so those are named by their codes.
fn is_busy(error: &io::Error) -> bool {
    cfg!(windows)
        && (error.kind() == io::ErrorKind::PermissionDenied
            // ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
            || matches!(error.raw_os_error(), Some(5 | 32 | 33)))
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

    /// The first suspect of the Windows failures: two writes of one file at once, which used to share one temporary file.
    #[test]
    fn two_threads_writing_one_file_never_fail_and_leave_no_temp_file() {
        let d = fresh("race");
        let file = d.join("state.json");
        write_atomic(&file, b"start").unwrap();
        let writers: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|text| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for _ in 0..200 {
                        write_atomic(&file, text.as_bytes()).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let text = fs::read_to_string(&file).unwrap();
        assert!(text == "one" || text == "two", "{text:?}");
        let names: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["state.json"], "no temporary file is left");
    }

    #[cfg(windows)]
    mod held {
        use super::*;
        use std::os::windows::fs::OpenOptionsExt;

        /// A reader that shares read and write but not delete: what an unknown scanner does. (Node and Rust both share delete.)
        fn hold(target: &Path) -> fs::File {
            const FILE_SHARE_READ: u32 = 0x1;
            const FILE_SHARE_WRITE: u32 = 0x2;
            OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(target)
                .unwrap()
        }

        #[test]
        fn a_reader_that_lets_go_soon_is_waited_for() {
            let d = fresh("held-short");
            let file = d.join("state.json");
            write_atomic(&file, b"old").unwrap();
            let handle = hold(&file);
            let letting_go = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                drop(handle);
            });
            write_atomic(&file, b"new").unwrap();
            letting_go.join().unwrap();
            assert_eq!(fs::read_to_string(&file).unwrap(), "new");
            assert_eq!(fs::read_dir(&d).unwrap().count(), 1, "no temporary file");
        }

        #[test]
        fn a_reader_that_never_lets_go_is_an_error_naming_the_file() {
            let d = fresh("held-long");
            let file = d.join("state.json");
            write_atomic(&file, b"old").unwrap();
            let handle = hold(&file);
            let error = write_atomic(&file, b"new").unwrap_err();
            assert!(error.to_string().contains("state.json"), "{error}");
            assert_eq!(fs::read_to_string(&file).unwrap(), "old");
            drop(handle);
            assert_eq!(fs::read_dir(&d).unwrap().count(), 1, "no temporary file");
        }
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
