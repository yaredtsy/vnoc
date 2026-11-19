//! Style for people in a terminal (docs/21-agent-cli/03-read-commands.md §11): one palette, 16-colour ANSI so the terminal's theme applies, and the
//! one place that says whether to use it.
//!
//! **Colour is on only for a stream that is a terminal** (or when `--color always` / `CLICOLOR_FORCE` say so). When it is off the helpers here return
//! their text **unchanged**, so a pipe gets byte for byte what the command printed before there was a palette, and an agent pays nothing. The
//! renderers never write an escape code themselves: they mark a piece of text with a named style from here.
//!
//! Whether a stream is a terminal is `anstream`'s to say (`NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`, isatty); `--color` is the
//! user's word over it. Until [`init`] has run nothing is painted, which is what a unit test of a renderer wants.

use std::sync::atomic::{AtomicU8, Ordering};

use anstream::ColorChoice;
use anstyle::{AnsiColor, Effects, Style};

/// What `--color` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum When {
    /// colour when the stream is a terminal, and neither `NO_COLOR` nor `TERM=dumb` says no (or `CLICOLOR_FORCE` says yes)
    Auto,
    /// colour whatever the stream is
    Always,
    /// plain text whatever the stream is
    Never,
}

/// The two streams a command writes to; each is a terminal or not on its own (`vnoc show … 2>/dev/null` keeps its colour on standard output).
#[derive(Clone, Copy)]
pub enum Stream {
    Out,
    Err,
}

const UNSET: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

static OUT: AtomicU8 = AtomicU8::new(UNSET);
static ERR: AtomicU8 = AtomicU8::new(UNSET);

#[cfg(test)]
thread_local! {
    /// A test's own answer, so renderers can be tried with colour on and off in parallel threads.
    static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with colour forced on or off for this thread's `stdout` painting.
#[cfg(test)]
pub fn with_color<T>(on: bool, f: impl FnOnce() -> T) -> T {
    FORCED.with(|c| c.set(Some(on)));
    let out = f();
    FORCED.with(|c| c.set(None));
    out
}

/// What `when` means for a stream, given what `anstream` finds out about it (`resolved`: its answer for `Auto`).
pub fn resolve(when: When, auto: ColorChoice) -> bool {
    match when {
        When::Always => true,
        When::Never => false,
        When::Auto => !matches!(auto, ColorChoice::Never),
    }
}

/// `anstream`'s own answer for a stream: the terminal check and the environment (`NO_COLOR`, `CLICOLOR_FORCE`, …).
fn auto_of(stream: Stream) -> ColorChoice {
    match stream {
        Stream::Out => anstream::AutoStream::choice(&std::io::stdout()),
        Stream::Err => anstream::AutoStream::choice(&std::io::stderr()),
    }
}

/// Decides, once, for both streams. Called by the command line before anything is printed (and before `--help`, whose colour follows it).
pub fn init(when: When) {
    for (slot, stream) in [(&OUT, Stream::Out), (&ERR, Stream::Err)] {
        let on = resolve(when, auto_of(stream));
        slot.store(if on { ON } else { OFF }, Ordering::Relaxed);
    }
}

/// `--color` as written in the arguments, read **before** clap parses them: `--help` is clap's own output, and its colour must follow the flag too.
/// An unknown value is left for clap to refuse.
pub fn when_in(args: &[std::ffi::OsString]) -> When {
    let mut found = When::Auto;
    let mut args = args.iter().map(|a| a.to_string_lossy());
    while let Some(arg) = args.next() {
        let value = match arg.strip_prefix("--color=") {
            Some(value) => Some(value.to_string()),
            None if arg == "--color" => args.next().map(|v| v.to_string()),
            None => None,
        };
        if arg == "--" {
            break;
        }
        match value.as_deref() {
            Some("always") => found = When::Always,
            Some("never") => found = When::Never,
            Some("auto") => found = When::Auto,
            _ => {}
        }
    }
    found
}

/// Whether `stream` is painted.
pub fn on(stream: Stream) -> bool {
    // a unit test is plain unless it says otherwise (`with_color`): `cli::run` in one test sets the global from the real terminal, and the renderer
    // tests running beside it must not see that
    #[cfg(test)]
    {
        let _ = stream;
        FORCED.with(std::cell::Cell::get).unwrap_or(false)
    }
    #[cfg(not(test))]
    {
        let slot = match stream {
            Stream::Out => &OUT,
            Stream::Err => &ERR,
        };
        slot.load(Ordering::Relaxed) == ON
    }
}

// ── the palette (03 §11) ───────────────────────────────────────────────────────────────────────────────────────────────

fn plain(color: AnsiColor) -> Style {
    Style::new().fg_color(Some(color.into()))
}

/// the program's name and version, a header's subject
pub fn bold() -> Style {
    Style::new().effects(Effects::BOLD)
}

/// locations (`billing/charge.py::retry_charge`), the root path
pub fn location() -> Style {
    plain(AnsiColor::Cyan)
}

/// the location a header is about: the subject of the answer, so bold as well
pub fn subject() -> Style {
    plain(AnsiColor::Cyan).effects(Effects::BOLD)
}

/// kind words, labels, line numbers in code, hints (`→ …`), paths of doc files
pub fn dim() -> Style {
    Style::new().effects(Effects::DIMMED)
}

/// doc titles
pub fn title() -> Style {
    plain(AnsiColor::Green)
}

/// a failing count above 0
pub fn bad() -> Style {
    plain(AnsiColor::Red)
}

/// `MISSING`, `error:`
pub fn alert() -> Style {
    plain(AnsiColor::Red).effects(Effects::BOLD)
}

/// `note:`, warnings, `none yet`
pub fn warn() -> Style {
    plain(AnsiColor::Yellow)
}

/// `all good`
pub fn good() -> Style {
    plain(AnsiColor::Green)
}

/// a search hit
pub fn hit() -> Style {
    plain(AnsiColor::Yellow).effects(Effects::BOLD)
}

/// `text` in `style` on standard output when it is painted, else `text` as it is.
pub fn paint(style: Style, text: impl AsRef<str>) -> String {
    paint_on(Stream::Out, style, text)
}

/// The same for a line of standard error (`error:`, `note:`).
pub fn paint_err(style: Style, text: impl AsRef<str>) -> String {
    paint_on(Stream::Err, style, text)
}

fn paint_on(stream: Stream, style: Style, text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if text.is_empty() || !on(stream) {
        return text.to_string();
    }
    format!("{style}{text}{style:#}")
}

// ── numbers ────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// `1805` as `1,805`: the number of a count. Not for a line number or an ID, which are positions and names.
pub fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A count's number, with separators, in bold.
pub fn num(n: usize) -> String {
    paint(bold(), grouped(n))
}

/// A count that fails when above 0: red then, bold otherwise.
pub fn failing(n: usize) -> String {
    paint(if n > 0 { bad() } else { bold() }, grouped(n))
}

/// `1 file`, `3 files`: the number in bold, with separators, then the word.
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", num(n), if n == 1 { one } else { many })
}

/// What a writer to `stream` is: a pipe gets the bytes as they are, a painted stream goes through `anstream` (which also turns codes into console
/// calls on an old Windows terminal).
pub fn writer(stream: Stream) -> Box<dyn std::io::Write> {
    match (stream, on(stream)) {
        (Stream::Out, true) => Box::new(anstream::AutoStream::always(std::io::stdout())),
        (Stream::Out, false) => Box::new(std::io::stdout().lock()),
        (Stream::Err, true) => Box::new(anstream::AutoStream::always(std::io::stderr())),
        (Stream::Err, false) => Box::new(std::io::stderr().lock()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_grouped_by_threes() {
        for (n, shown) in [
            (0, "0"),
            (7, "7"),
            (999, "999"),
            (1000, "1,000"),
            (1805, "1,805"),
            (12_345, "12,345"),
            (123_456, "123,456"),
            (1_234_567, "1,234,567"),
            (20_000, "20,000"),
        ] {
            assert_eq!(grouped(n), shown);
        }
    }

    #[test]
    fn unpainted_text_is_returned_as_it_is_and_painted_text_is_wrapped() {
        assert_eq!(
            with_color(false, || paint(location(), "a.py::f")),
            "a.py::f"
        );
        assert_eq!(
            with_color(true, || paint(location(), "a.py::f")),
            "\x1b[36ma.py::f\x1b[0m"
        );
        assert_eq!(with_color(true, || paint(bold(), "3")), "\x1b[1m3\x1b[0m");
        assert_eq!(
            with_color(true, || paint(dim(), "function")),
            "\x1b[2mfunction\x1b[0m"
        );
        assert_eq!(
            with_color(true, || paint(alert(), "MISSING")),
            "\x1b[1m\x1b[31mMISSING\x1b[0m"
        );
        assert_eq!(
            with_color(true, || paint(hit(), "retry")),
            "\x1b[1m\x1b[33mretry\x1b[0m"
        );
        assert_eq!(
            with_color(true, || paint(title(), "")),
            "",
            "nothing to paint, nothing written"
        );
    }

    #[test]
    fn counts_are_grouped_and_the_number_is_the_bold_part() {
        assert_eq!(
            with_color(false, || count(1805, "file", "files")),
            "1,805 files"
        );
        assert_eq!(with_color(false, || count(1, "file", "files")), "1 file");
        assert_eq!(
            with_color(true, || count(2, "class", "classes")),
            "\x1b[1m2\x1b[0m classes"
        );
        assert_eq!(with_color(true, || failing(0)), "\x1b[1m0\x1b[0m");
        assert_eq!(with_color(true, || failing(2)), "\x1b[31m2\x1b[0m");
    }

    #[test]
    fn the_flag_decides_and_auto_is_what_anstream_found() {
        assert!(resolve(When::Always, ColorChoice::Never));
        assert!(!resolve(When::Never, ColorChoice::Always));
        assert!(resolve(When::Auto, ColorChoice::Always));
        assert!(resolve(When::Auto, ColorChoice::AlwaysAnsi));
        assert!(!resolve(When::Auto, ColorChoice::Never));
    }

    #[test]
    fn the_flag_is_read_before_clap_does() {
        let args = |list: &[&str]| {
            list.iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(when_in(&args(&["status"])), When::Auto);
        assert_eq!(
            when_in(&args(&["--color", "always", "status"])),
            When::Always
        );
        assert_eq!(when_in(&args(&["status", "--color=never"])), When::Never);
        assert_eq!(
            when_in(&args(&["status", "--color", "sometimes"])),
            When::Auto
        );
        assert_eq!(
            when_in(&args(&["find", "--", "--color", "always"])),
            When::Auto,
            "after `--` it is a word"
        );
        assert_eq!(
            when_in(&args(&["--color", "never", "--color", "always"])),
            When::Always,
            "the last one wins, as in clap"
        );
    }
}
