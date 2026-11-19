//! The one module that prints. In `vnoc --stdio` standard output is the protocol, so the rest of the library has `print_stdout` and
//! `print_stderr` denied (`lib.rs`) and every command prints through here: text, one JSON value, an error line.
//!
//! A reader that goes away (`vnoc tree | head`) is not an error: Rust ignores SIGPIPE, so a write to a closed pipe returns `BrokenPipe`
//! (a `println!` would panic), and it ends the command quietly with `0`.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::{self, Write};

use serde::Serialize;

use super::style::{self, Stream};
use super::{Failure, Global};

/// What a command answers, as a value that can be written as JSON (`Serialize`: the shape is the promise, C5) and as text. A command builds this
/// and hands it to [`render`]; it formats nothing as it computes, so a new format is a new method here and a new branch in `render`, and no command
/// has to change (03 §11). `quiet` is the one flag a text rendering reads: the hints and totals it leaves out.
pub trait Render: Serialize {
    fn text(&self, quiet: bool) -> String;
}

/// Writes `value` as the flags ask: one JSON value with `--json`, else its text.
pub fn render<T: Render>(value: &T, global: &Global) -> Result<(), Failure> {
    if global.json {
        json(value)
    } else {
        text(&value.text(global.quiet))
    }
}

/// Writes `text` to standard output, locked, and flushes. A terminal gets it through `anstream`; a pipe gets the bytes as they are (the renderers
/// painted nothing: `style`).
pub fn text(text: &str) -> Result<(), Failure> {
    let mut stdout = style::writer(Stream::Out);
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
    let _ = writeln!(
        style::writer(Stream::Err),
        "{} {message}",
        style::paint_err(style::alert(), "error:")
    );
}

/// A remark about the request that did not stop it (a filter of a query that means nothing): one line on standard error, `note: …`.
pub fn note(message: &str) {
    let _ = writeln!(
        style::writer(Stream::Err),
        "{} {message}",
        style::paint_err(style::warn(), "note:")
    );
}

/// clap's own answer to `--help`, `--version` or a mistake: its text goes to standard output for the first two and to standard error for the
/// last, as clap decides. Returns clap's exit code (0, or 2 for a usage error).
pub fn clap_error(e: &clap::Error) -> i32 {
    let (stream, to_stderr) = if e.use_stderr() {
        (Stream::Err, true)
    } else {
        (Stream::Out, false)
    };
    // clap's own styling (bold headings, the usage line) follows `--color` as everything else does
    let rendered = if style::on(stream) {
        e.render().ansi().to_string()
    } else {
        e.render().to_string()
    };
    let written = {
        let mut out = style::writer(if to_stderr { Stream::Err } else { Stream::Out });
        out.write_all(rendered.as_bytes())
            .and_then(|()| out.flush())
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
