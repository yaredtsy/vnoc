//! Style for people in a terminal (docs/21-agent-cli/03-read-commands.md §11): one palette, 16-colour ANSI so the terminal's theme applies, and the
//! one place that says whether to use it.
//!
//! **Colour is on only for a stream that is a terminal** (or when `--color always` / `CLICOLOR_FORCE` say so). When it is off the helpers here return
//! their text **unchanged**, so a pipe gets byte for byte what the command printed before there was a palette, and an agent pays nothing. The
//! renderers never write an escape code themselves: they mark a piece of text with a named style from here.
//!
//! **Text from the project is shown, not obeyed, on a terminal** (03 §11, "Control characters"): a note, a code line or a file name can hold `ESC`, and on a
//! terminal that is a command to it. [`writer`] cleans what it writes when its stream is a terminal (`^[` for `ESC`, `^G` for `BEL`, …) and **keeps V-NOC's own
//! paint**: [`paint`] does not write `ESC`, it writes a stand-in that holds a random number made for this run, which nothing in a project can know, and the
//! writer turns the stand-ins into `ESC` after cleaning everything else. A pipe, `--json` and `--format` use [`raw_writer`]: the bytes as they are.
//!
//! Whether a stream is a terminal is `anstream`'s to say (`NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`, isatty); `--color` is the
//! user's word over it. Until [`init`] has run nothing is painted, which is what a unit test of a renderer wants.

use std::io::{self, IsTerminal, Write};
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
    TERMINAL_OUT.store(
        if io::stdout().is_terminal() { ON } else { OFF },
        Ordering::Relaxed,
    );
    TERMINAL_ERR.store(
        if io::stderr().is_terminal() { ON } else { OFF },
        Ordering::Relaxed,
    );
}

/// Whether the stream is a terminal, whatever `--color` says: the one thing that decides whether project text is cleaned. Until [`init`] has run: no.
static TERMINAL_OUT: AtomicU8 = AtomicU8::new(UNSET);
static TERMINAL_ERR: AtomicU8 = AtomicU8::new(UNSET);

fn is_terminal(stream: Stream) -> bool {
    match stream {
        Stream::Out => TERMINAL_OUT.load(Ordering::Relaxed) == ON,
        Stream::Err => TERMINAL_ERR.load(Ordering::Relaxed) == ON,
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
    let (open, close) = (
        standing_in(&style.to_string()),
        standing_in(&format!("{style:#}")),
    );
    format!("{open}{text}{close}")
}

/// `text` in `style` with real `ESC` codes, for a string that does **not** go through [`writer`] (a menu's label, a line written to the terminal directly):
/// nothing cleans it, so nothing but V-NOC's own text may be in it.
pub fn paint_direct(stream: Stream, style: Style, text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    if text.is_empty() || !on(stream) {
        return text.to_string();
    }
    format!("{style}{text}{style:#}")
}

// ── what a terminal is given: project text cleaned, V-NOC's paint kept ─────────────────────────────────────────────────

/// What stands for `ESC` in the paint of this run: private-use marks around 128 random bits. A file in a project cannot hold it, so cleaning the
/// finished text and then turning these back into `ESC` keeps V-NOC's paint and nothing else. (In a unit test it is `ESC` itself, so the renderers' tests
/// read the real codes; [`Shown`] is tested with a mark of its own.)
fn mark() -> &'static str {
    #[cfg(test)]
    {
        "\x1b"
    }
    #[cfg(not(test))]
    {
        static MARK: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        MARK.get_or_init(|| {
            use std::hash::{BuildHasher, Hasher};
            // `RandomState` is seeded by the operating system, differently for every hasher made
            let random = || {
                std::collections::hash_map::RandomState::new()
                    .build_hasher()
                    .finish()
            };
            format!("\u{E000}{:016x}{:016x}\u{E001}", random(), random())
        })
    }
}

/// `codes` (an `ESC [ … m` sequence, as `anstyle` writes it) with the mark in place of each `ESC`.
fn standing_in(codes: &str) -> String {
    codes.replace('\x1b', mark())
}

/// `text` as a terminal may be given it. Every piece of it that is not `mark` is cleaned; each `mark` is turned into `ESC` (V-NOC's own paint).
///
/// - a C0 control other than tab and line feed becomes caret notation (`ESC` → `^[`, `BEL` → `^G`, `NUL` → `^@`), and so does DEL (`^?`);
/// - a carriage return right before a line feed stays (a CRLF file), any other becomes `^M`;
/// - a C1 control (U+0080 to U+009F: the 8-bit forms of the same commands) becomes `<U+009B>`;
/// - everything else, `é` and `日本` included, is as it was.
pub fn sanitize(text: &str, mark: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if !mark.is_empty() && rest.starts_with(mark) {
            out.push('\x1b');
            rest = &rest[mark.len()..];
            continue;
        }
        rest = &rest[c.len_utf8()..];
        match c {
            '\t' | '\n' => out.push(c),
            '\r' if rest.starts_with('\n') => out.push(c),
            '\x7f' => out.push_str("^?"),
            c if c.is_ascii_control() => {
                out.push('^');
                out.push(char::from(c as u8 ^ 0x40));
            }
            c if ('\u{80}'..='\u{9f}').contains(&c) => {
                out.push_str(&format!("<U+{:04X}>", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

/// What a stream's writer does: nothing (a pipe that is not painted), turn V-NOC's marks into `ESC` (a painted pipe: `--color always`), or clean the
/// project's text as well (a terminal, painted or not).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shaping {
    AsIs,
    Marks,
    Clean,
}

/// **The decision**: a terminal's text is cleaned whatever `--color` says; otherwise only the paint's marks are turned back, and only when something was painted.
pub fn shaping(terminal: bool, painted: bool) -> Shaping {
    match (terminal, painted) {
        (true, _) => Shaping::Clean,
        (false, true) => Shaping::Marks,
        (false, false) => Shaping::AsIs,
    }
}

/// A writer that collects what it is given and, when it is flushed (or dropped), writes it to `inner` shaped as `shaping` says. A command writes a
/// whole answer and flushes, so a mark is never cut in two; a byte that is not UTF-8 is shown as U+FFFD (see `pass_on`).
pub struct Shown<W: Write> {
    inner: W,
    mark: String,
    clean: bool,
    held: Vec<u8>,
}

impl<W: Write> Shown<W> {
    pub fn new(inner: W, mark: &str, clean: bool) -> Self {
        Shown {
            inner,
            mark: mark.to_string(),
            clean,
            held: Vec::new(),
        }
    }

    fn pass_on(&mut self) -> io::Result<()> {
        if self.held.is_empty() {
            return Ok(());
        }
        let held = std::mem::take(&mut self.held);
        // A byte that is not UTF-8 is read as U+FFFD, never written as it is: a raw byte such as 0x9B is a terminal's own command (the 8-bit CSI), and the
        // text could not be cleaned around it. (A file that is not UTF-8 is read lossily everywhere else V-NOC shows it.)
        let text = String::from_utf8_lossy(&held);
        let shaped = if self.clean {
            sanitize(&text, &self.mark)
        } else if self.mark.is_empty() {
            text.into_owned()
        } else {
            text.replace(&self.mark, "\x1b")
        };
        self.inner.write_all(shaped.as_bytes())
    }
}

impl<W: Write> Write for Shown<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.held.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.pass_on()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for Shown<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
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

/// What a writer to `stream` is, as it was before project text was cleaned: a pipe gets the bytes as they are, a painted stream goes through `anstream`
/// (which also turns codes into console calls on an old Windows terminal). For what is not project text (clap's own help and errors, which carry real
/// `ESC` codes) and for what must stay the bytes (`--json`, `--format`).
pub fn raw_writer(stream: Stream) -> Box<dyn Write> {
    match (stream, on(stream)) {
        (Stream::Out, true) => Box::new(anstream::AutoStream::always(io::stdout())),
        (Stream::Out, false) => Box::new(io::stdout().lock()),
        (Stream::Err, true) => Box::new(anstream::AutoStream::always(io::stderr())),
        (Stream::Err, false) => Box::new(io::stderr().lock()),
    }
}

/// The writer of everything a command shows people: text that came from the project is cleaned when the stream is a terminal (see the top of this file).
pub fn writer(stream: Stream) -> Box<dyn Write> {
    match shaping(is_terminal(stream), on(stream)) {
        Shaping::AsIs => raw_writer(stream),
        shaping => Box::new(Shown::new(
            raw_writer(stream),
            mark(),
            shaping == Shaping::Clean,
        )),
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

    // ── project text on a terminal (03 §11) ──

    #[test]
    fn each_class_of_control_character_is_shown_and_not_obeyed() {
        let clean = |t: &str| sanitize(t, "");
        assert_eq!(clean("a\x1b[2Jb"), "a^[[2Jb", "ESC");
        assert_eq!(
            clean("\x1b]0;owned\x07"),
            "^[]0;owned^G",
            "ESC ] … BEL: the title, and BEL"
        );
        assert_eq!(clean("x\x07y"), "x^Gy", "BEL");
        assert_eq!(clean("x\x00y"), "x^@y", "NUL");
        assert_eq!(clean("x\x08y"), "x^Hy", "backspace");
        assert_eq!(clean("x\x7fy"), "x^?y", "DEL");
        assert_eq!(clean("a\r\nb\r\n"), "a\r\nb\r\n", "CRLF stays");
        assert_eq!(clean("a\rb"), "a^Mb", "a lone CR");
        assert_eq!(clean("a\r"), "a^M", "a CR at the very end is lone");
        assert_eq!(clean("a\r\rb\n"), "a^M^Mb\n");
        assert_eq!(
            clean("a\r\r\n"),
            "a^M\r\n",
            "the CR before a CR is lone; the one before the LF stays"
        );
        assert_eq!(clean("a\u{9b}31m"), "a<U+009B>31m", "C1: the 8-bit CSI");
        assert_eq!(clean("\u{80}\u{9f}"), "<U+0080><U+009F>");
        assert_eq!(clean("a\tb\nc"), "a\tb\nc", "tab and line feed stay");
        assert_eq!(
            clean("é 日本 ✔ 𝄞 \u{a0}"),
            "é 日本 ✔ 𝄞 \u{a0}",
            "text in other scripts, and the first character after the C1 range"
        );
        assert_eq!(clean(""), "");
    }

    #[test]
    fn the_cleaner_keeps_the_marks_of_our_own_paint_and_nothing_that_looks_like_it() {
        let mark = "\u{E000}abcd\u{E001}";
        // our own: `ESC` again
        assert_eq!(
            sanitize(&format!("{mark}[36mx{mark}[0m"), mark),
            "\x1b[36mx\x1b[0m"
        );
        // the project's own `ESC [ 36 m`, and a forged mark with another number: cleaned
        assert_eq!(sanitize("\x1b[36mx\x1b[0m", mark), "^[[36mx^[[0m");
        assert_eq!(
            sanitize("\u{E000}0000\u{E001}[2J", mark),
            "\u{E000}0000\u{E001}[2J",
            "not our mark: only text (private-use characters are not controls)"
        );
        // both in one line
        assert_eq!(
            sanitize(&format!("{mark}[1mt{mark}[0m \x1b[2J"), mark),
            "\x1b[1mt\x1b[0m ^[[2J"
        );
    }

    #[test]
    fn paint_writes_the_mark_where_esc_would_be() {
        // what `paint` writes is `mark()` in place of each `ESC`; in a unit test the mark is `ESC`, so this checks the relation and not the number
        let painted = with_color(true, || paint(location(), "a.py::f"));
        let standing = standing_in("\x1b[36m");
        assert!(painted.starts_with(&standing));
        let shown = sanitize(&painted, mark());
        assert_eq!(shown, "\x1b[36ma.py::f\x1b[0m");
    }

    #[test]
    fn a_terminal_cleans_whatever_colour_says_and_a_pipe_never_does() {
        assert_eq!(shaping(true, true), Shaping::Clean);
        assert_eq!(
            shaping(true, false),
            Shaping::Clean,
            "`--color never` on a terminal: still cleaned"
        );
        assert_eq!(
            shaping(false, true),
            Shaping::Marks,
            "`--color always` into a pipe: V-NOC's colours, the project's bytes"
        );
        assert_eq!(shaping(false, false), Shaping::AsIs, "a pipe");
    }

    #[test]
    fn the_writer_turns_marks_into_esc_and_cleans_only_when_told() {
        let mark = "\u{E000}1234\u{E001}";
        let text = format!("{mark}[1mtitle{mark}[0m\nbody \x1b]0;owned\x07 and \x1b[2J\n");
        let through = |clean: bool| {
            let mut out = Vec::new();
            {
                let mut w = Shown::new(&mut out, mark, clean);
                // pieces, as `writeln!` hands them over
                w.write_all(&text.as_bytes()[..7]).unwrap();
                w.write_all(&text.as_bytes()[7..]).unwrap();
                w.flush().unwrap();
            }
            String::from_utf8(out).unwrap()
        };
        assert_eq!(
            through(true),
            "\x1b[1mtitle\x1b[0m\nbody ^[]0;owned^G and ^[[2J\n"
        );
        assert_eq!(
            through(false),
            "\x1b[1mtitle\x1b[0m\nbody \x1b]0;owned\x07 and \x1b[2J\n",
            "a painted pipe: marks only; the file's bytes as they are"
        );
        // dropped without a flush: still written
        let mut out = Vec::new();
        drop({
            let mut w = Shown::new(&mut out, mark, true);
            w.write_all(b"x\x1b").unwrap();
            w
        });
        assert_eq!(out, b"x^[");
        // not UTF-8: shown as U+FFFD, and the ESC beside it is cleaned like any other (a raw 0xff, or a raw 0x9b, is never written)
        for clean in [true, false] {
            let mut out = Vec::new();
            {
                let mut w = Shown::new(&mut out, mark, clean);
                w.write_all(&[0x66, 0xff, 0x1b, 0x9b, 0x67]).unwrap();
            }
            let shown = String::from_utf8(out).expect("what is written is UTF-8");
            if clean {
                assert_eq!(shown, "f\u{fffd}^[\u{fffd}g", "a terminal");
            } else {
                assert_eq!(
                    shown, "f\u{fffd}\x1b\u{fffd}g",
                    "a painted pipe: lossy, the file's ESC as it is, only the marks turned back"
                );
            }
        }
    }
}
