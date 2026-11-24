//! The temp folders a test makes go away when the test ends (a Drop guard in a thread-local, so the helpers keep returning a `PathBuf`).
//!
//! A helper that makes a folder calls [`remove_at_end`] with it: the folder is removed when the test's thread ends, whether the test passed or
//! failed (a panic unwinds the thread, and the guard drops). A folder that tests share (made once, in a `static`) is [`remove_at_exit`]: it goes
//! when the test binary ends. Anything still registered at exit goes then too (a test run on the main thread has no thread end to wait for).
//!
//! `VNOC_KEEP_TEMP=1` keeps every folder, for looking at what a test left. One file for the crate's unit tests (`#[path]`) and every integration
//! test (`mod testdir`, with `#[path = "common/testdir.rs"]`).
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::thread::{self, ThreadId};

/// What is waiting to be removed, by the thread that made it (`None`: at exit only).
static FOLDERS: Mutex<Vec<(Option<ThreadId>, PathBuf)>> = Mutex::new(Vec::new());

fn keep() -> bool {
    std::env::var_os("VNOC_KEEP_TEMP").is_some_and(|v| v == "1")
}

fn remove(path: &Path) {
    if !keep() && std::fs::remove_dir_all(path).is_err() {
        let _ = std::fs::remove_file(path); // a file a test made
    }
}

unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> i32;
}

extern "C" fn at_exit() {
    let left = FOLDERS.lock().map(|mut f| std::mem::take(&mut *f));
    for (_, path) in left.unwrap_or_default() {
        remove(&path);
    }
}

fn register(owner: Option<ThreadId>, path: &Path) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: `at_exit` is an `extern "C" fn()` that does not unwind (a poisoned lock is read as empty).
        unsafe {
            atexit(at_exit);
        }
    });
    if let Ok(mut folders) = FOLDERS.lock() {
        folders.push((owner, path.to_path_buf()));
    }
}

/// Removes what the thread registered, when the thread ends.
struct Guard(ThreadId);

impl Drop for Guard {
    fn drop(&mut self) {
        let mine: Vec<PathBuf> = match FOLDERS.lock() {
            Ok(mut folders) => {
                let mut mine = Vec::new();
                folders.retain(|(owner, path)| {
                    if *owner == Some(self.0) {
                        mine.push(path.clone());
                        false
                    } else {
                        true
                    }
                });
                mine
            }
            Err(_) => Vec::new(),
        };
        for path in mine {
            remove(&path);
        }
    }
}

thread_local! {
    static GUARD: Guard = Guard(thread::current().id());
}

/// `dir` is removed when the calling test ends.
pub fn remove_at_end(dir: &Path) {
    GUARD.with(|_| ()); // makes the guard for this thread, which drops when the thread does
    register(Some(thread::current().id()), dir);
}

/// `dir` is removed when the test binary ends (a folder every test of the binary shares).
pub fn remove_at_exit(dir: &Path) {
    register(None, dir);
}
