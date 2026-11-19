//! The one module that prints. In `vnoc --stdio` standard output is the protocol, so the rest of the library has `print_stdout` and
//! `print_stderr` denied (`lib.rs`) and every command prints through here: text, one JSON value, an error line.
//!
//! A reader that goes away (`vnoc tree | head`) is not an error: Rust ignores SIGPIPE, so a write to a closed pipe returns `BrokenPipe`
//! (a `println!` would panic), and it ends the command quietly with `0`.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{self, Write};

use serde::Serialize;

use super::Failure;

/// Writes `text` to standard output, locked, and flushes.
pub fn text(text: &str) -> Result<(), Failure> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(closed_or)
}

/// Writes one JSON value and a newline. The value is the command's own struct, with `schema` and `command` first.
pub fn json<T: Serialize>(value: &T) -> Result<(), Failure> {
    let mut line = serde_json::to_string(value)
        .map_err(|e| Failure::Environment(format!("could not write JSON: {e}")))?;
    line.push('\n');
    text(&line)
}

/// A failure's line on standard error. Nothing else is written there by a command (logs only with `VNOC_LOG`).
pub fn error(message: &str) {
    // a closed standard error has nowhere to complain to
    let _ = writeln!(io::stderr().lock(), "error: {message}");
}

/// clap's own answer to `--help`, `--version` or a mistake: its text goes to standard output for the first two and to standard error for the
/// last, as clap decides. Returns clap's exit code (0, or 2 for a usage error).
pub fn clap_error(e: &clap::Error) -> i32 {
    let rendered = e.render().to_string();
    let written = if e.use_stderr() {
        io::stderr().lock().write_all(rendered.as_bytes())
    } else {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(rendered.as_bytes())
            .and_then(|()| stdout.flush())
    };
    match written {
        Err(w) if w.kind() != io::ErrorKind::BrokenPipe => 3,
        _ => e.exit_code(),
    }
}

fn closed_or(e: io::Error) -> Failure {
    if e.kind() == io::ErrorKind::BrokenPipe {
        Failure::Closed
    } else {
        Failure::Environment(format!("could not write to standard output: {e}"))
    }
}
