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

/// The tours lock is held for microseconds and a reader's Next waits on it, so it tries far more often (TR0, check 3: a 50 ms sleep was every wait).
const TOURS_RETRY: Duration = Duration::from_millis(5);

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("another V-NOC is writing in this project")]
    Busy,
    #[error("could not lock .vnoc/local/write.lock: {0}")]
    Io(#[from] io::Error),
}

/// Why a lock was not taken: busy past the wait, or the file itself failed.
enum Failed {
    Busy,
    Io(io::Error),
}

impl From<io::Error> for Failed {
    fn from(e: io::Error) -> Failed {
        Failed::Io(e)
    }
}

/// Opens `path` (its folder is made if it is missing) and tries to lock it, sleeping `retry` between tries, for `wait` at most.
/// The lock is held by the returned file.
fn lock_file(path: &Path, wait: Duration, retry: Duration) -> Result<File, Failed> {
    std::fs::create_dir_all(path.parent().expect("a file has a parent folder"))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::Error(e)) => return Err(Failed::Io(e)),
            Err(TryLockError::WouldBlock) => {
                if started.elapsed() >= wait {
                    return Err(Failed::Busy);
                }
                std::thread::sleep(retry);
            }
        }
    }
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
        match lock_file(&layout::write_lock_file(vnoc_root), wait, RETRY) {
            Ok(file) => Ok(WriteLock { _file: file }),
            Err(Failed::Busy) => Err(LockError::Busy),
            Err(Failed::Io(e)) => Err(LockError::Io(e)),
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

#[derive(Debug, thiserror::Error)]
pub enum ToursLockError {
    #[error("another V-NOC is writing this tour")]
    Busy,
    #[error("could not lock .vnoc/local/tours.lock: {0}")]
    Io(#[from] io::Error),
}

/// The lock of one write to a tour's file or its state file (`docs/26-tours/01-on-disk.md` §1), on `local/tours.lock` and not on the tour's file:
/// on Windows a lock on a file stops other processes reading it. Held by the open file, so a crash never leaves it stale.
#[derive(Debug)]
pub struct ToursLock {
    _file: File,
}

impl ToursLock {
    /// Takes the lock (the folder is made if it is missing), trying every 5 ms for `DEFAULT_WAIT` at most. Blocks the thread.
    pub fn take(vnoc_root: &Path) -> Result<ToursLock, ToursLockError> {
        ToursLock::take_for(vnoc_root, DEFAULT_WAIT)
    }

    /// As `take`, with the wait given.
    pub fn take_for(vnoc_root: &Path, wait: Duration) -> Result<ToursLock, ToursLockError> {
        match lock_file(&layout::tours_lock_file(vnoc_root), wait, TOURS_RETRY) {
            Ok(file) => Ok(ToursLock { _file: file }),
            Err(Failed::Busy) => Err(ToursLockError::Busy),
            Err(Failed::Io(e)) => Err(ToursLockError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-lock-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        crate::testdir::remove_at_end(&d);
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

    #[test]
    fn the_tours_lock_is_its_own_file_and_does_not_block_the_write_lock() {
        let root = fresh("tours");
        let held = ToursLock::take(&root).unwrap();
        assert!(root.join("local/tours.lock").is_file());
        WriteLock::take(&root, Duration::ZERO).unwrap();
        let started = Instant::now();
        let err = ToursLock::take_for(&root, Duration::from_millis(60)).unwrap_err();
        assert!(matches!(err, ToursLockError::Busy));
        assert_eq!(err.to_string(), "another V-NOC is writing this tour");
        assert!(started.elapsed() >= Duration::from_millis(60));
        drop(held);
        ToursLock::take_for(&root, Duration::ZERO).unwrap();
    }
}
