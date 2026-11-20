//! One writer at a time in a project, across processes (`docs/21-agent-cli/04-write-commands.md` §7).
//!
//! The server's `attaching` mutex keeps two attaches in one process apart; this keeps a command apart from the server (or from another command).
//! The lock is held by the open file, so the system lets go of it when the process dies: a crash never leaves a stale lock.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use super::layout;

/// How long a writer waits for the lock before it gives up.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(5);

/// `std` has no lock with a timeout: `try_lock` in a loop, with this much sleep between tries.
const RETRY: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("another V-NOC is writing in this project")]
    Busy,
    #[error("could not lock .vnoc/local/write.lock: {0}")]
    Io(#[from] io::Error),
}

/// The lock, for as long as this value lives.
#[derive(Debug)]
pub struct WriteLock {
    _file: File,
}

impl WriteLock {
    /// Takes the lock on `<vnoc_root>/local/write.lock` (the folder is made if it is missing), waiting up to `wait` for another holder.
    /// This blocks the thread it runs on: from async code, run it with `spawn_blocking`.
    pub fn take(vnoc_root: &Path, wait: Duration) -> Result<WriteLock, LockError> {
        let path = layout::write_lock_file(vnoc_root);
        std::fs::create_dir_all(path.parent().expect("a file has a parent folder"))?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(WriteLock { _file: file }),
                Err(TryLockError::Error(e)) => return Err(LockError::Io(e)),
                Err(TryLockError::WouldBlock) => {
                    if started.elapsed() >= wait {
                        return Err(LockError::Busy);
                    }
                    std::thread::sleep(RETRY);
                }
            }
        }
    }

    /// As `take`, but only in a project that already has a `.vnoc/`: the first attach creates it, and a flow that fails early must not leave
    /// a `local/` behind. `Ok(None)`: nothing to lock yet.
    pub fn take_if_initialized(
        vnoc_root: &Path,
        wait: Duration,
    ) -> Result<Option<WriteLock>, LockError> {
        if !vnoc_root.is_dir() {
            return Ok(None);
        }
        WriteLock::take(vnoc_root, wait).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-lock-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_second_taker_waits_and_gets_the_lock_once_the_first_lets_go() {
        let root = fresh("wait");
        let first = WriteLock::take(&root, Duration::ZERO).unwrap();
        assert!(root.join("local/write.lock").is_file());
        let other = root.clone();
        let waiter = std::thread::spawn(move || {
            let started = Instant::now();
            let lock = WriteLock::take(&other, Duration::from_secs(10));
            (lock.is_ok(), started.elapsed())
        });
        std::thread::sleep(Duration::from_millis(300));
        drop(first);
        let (got_it, waited) = waiter.join().unwrap();
        assert!(got_it);
        assert!(
            waited >= Duration::from_millis(250),
            "it waited: {waited:?}"
        );
    }

    #[test]
    fn a_short_wait_ends_busy_with_the_message() {
        let root = fresh("busy");
        let _held = WriteLock::take(&root, Duration::ZERO).unwrap();
        let started = Instant::now();
        let err = WriteLock::take(&root, Duration::from_millis(120)).unwrap_err();
        assert!(matches!(err, LockError::Busy));
        assert_eq!(err.to_string(), "another V-NOC is writing in this project");
        assert!(started.elapsed() >= Duration::from_millis(120));
        assert!(
            WriteLock::take(&root, Duration::ZERO).is_err(),
            "still held by the first"
        );
    }

    #[test]
    fn it_is_free_again_after_a_drop() {
        let root = fresh("again");
        drop(WriteLock::take(&root, Duration::ZERO).unwrap());
        WriteLock::take(&root, Duration::ZERO).unwrap();
    }

    #[test]
    fn with_no_vnoc_folder_there_is_nothing_to_lock_and_nothing_is_created() {
        let project = fresh("none");
        let root = project.join(".vnoc");
        assert!(
            WriteLock::take_if_initialized(&root, Duration::ZERO)
                .unwrap()
                .is_none()
        );
        assert!(!root.exists(), "no .vnoc/ and no local/ left behind");

        std::fs::create_dir_all(&root).unwrap();
        assert!(
            WriteLock::take_if_initialized(&root, Duration::ZERO)
                .unwrap()
                .is_some()
        );
    }
}
