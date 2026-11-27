//! The look of the tour commands (`docs/26-tours/05-on-screen.md` §5, "the tour commands' text is hard to read"): one voice for people on a terminal.
//!
//! Every answer starts with a mark and the outcome (`✓` done, green · `!` needs the user, yellow · `✗` refused, red), and its details are indented two spaces
//! under it. Labels are dim, ids cyan, titles bold, nodes cyan (`location`). `list` is a block a tour ([`list`]).
//!
//! **Only the look changes.** The commands still build the words (`written`, `finished: …`, `ask the user …`, a refusal's text); [`dress`] puts a mark, an
//! indent and colour on them and cuts nothing. The colour is `style`'s (off in a pipe, with `--color never`, and in a test), so a pipe gets the same layout
//! with no colour.
//!
//! **Width.** On a terminal a row is cut at the terminal's width with `…`, never wrapped in the middle of a column; a pipe has no width and nothing is cut.
//! Text of the project or of the tour (a step's prose, a line of code) is never cut: the terminal wraps it as it does any text.

use std::sync::OnceLock;

use anstyle::Style;
use regex::{Captures, Regex};
use serde::Serialize;

use crate::cli::style::{self, bold, dim, good, location, paint, warn};
use crate::tours::view::{self, ListRowView};

/// A pipe has no width; a terminal that does not say gets this.
pub const FALLBACK_WIDTH: usize = 100;

/// What an answer is, as its first line says: done, waiting for the user, refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Done,
    Needs,
    Refused,
}

impl Mark {
    fn glyph(self) -> &'static str {
        match self {
            Mark::Done => "✓",
            Mark::Needs => "!",
            Mark::Refused => "✗",
        }
    }

    fn style(self) -> Style {
        match self {
            Mark::Done => good(),
            Mark::Needs => warn(),
            Mark::Refused => style::bad(),
        }
    }

    /// The mark, painted, with the space after it.
    pub fn painted(self) -> String {
        format!("{} ", paint(self.style(), self.glyph()))
    }
}

/// How a command's text is laid out, beyond the mark and the indent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// lines of words: ids, pins and labels are coloured
    Plain,
    /// `show`: the title, the facts, a row a stop, a row a block
    Outline,
    /// `route`: the sizes as a small table, then a row a stop
    Route,
    /// `where` and `go`: a step's words and code are shown as they are
    Where,
}

/// The tour ids a value names (`tour`, and `id` of a row), for the ink: they are what a person copies, so they are cyan.
pub fn ids_in<T: Serialize>(value: &T) -> Vec<String> {
    fn walk(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, inner) in map {
                    match (key.as_str(), inner) {
                        ("tour" | "id", serde_json::Value::String(id)) if !out.contains(id) => {
                            out.push(id.clone());
                        }
                        _ => walk(inner, out),
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }
    let mut ids = Vec::new();
    if let Ok(value) = serde_json::to_value(value) {
        walk(&value, &mut ids);
    }
    ids
}

// ── the answer of a command ────────────────────────────────────────────────────────────────────────────────────────────

/// What mark an answer's text has: `!` for an answer that asks the user (or that a command could not act on), `✗` for a tour that cannot be read, else `✓`.
fn mark_of(text: &str) -> Mark {
    let asks = text.lines().any(|l| l.starts_with("ask the user"));
    let first = text.lines().next().unwrap_or("");
    if text.lines().any(|l| l.starts_with("cannot be read")) {
        Mark::Refused
    } else if asks
        || [
            "stopped:",
            "nothing to go back to",
            "no tour has been opened",
            "nothing to pin",
        ]
        .iter()
        .any(|w| first.starts_with(w))
    {
        Mark::Needs
    } else {
        Mark::Done
    }
}

/// `text` as a person reads it: the mark before the first line, every other line indented two spaces, ids and labels in colour, and (on a terminal) the
/// rows cut at the width.
pub fn dress(text: &str, ids: &[String], kind: Kind) -> String {
    dress_at(text, ids, kind, style::terminal_width())
}

/// [`dress`] with the terminal's width given (`None`: a pipe, nothing is cut).
pub fn dress_at(text: &str, ids: &[String], kind: Kind, width: Option<usize>) -> String {
    let mark = mark_of(text);
    let ink = Ink::new(ids);
    let mut out = String::new();
    // route: the first block (the sizes) ends at the first blank line
    let mut table = kind == Kind::Route;
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() {
            out.push('\n');
            table = false;
            continue;
        }
        let room = width.map(|w| w.saturating_sub(2).max(20));
        let line = match (kind, room) {
            // a step's prose and its code are the tour's and the project's: shown whole
            (Kind::Outline, _) if line.starts_with("      ") => line.to_string(),
            (Kind::Where, _) if n > 2 => line.to_string(),
            (_, Some(room)) => cut(line, room),
            (_, None) => line.to_string(),
        };
        let painted = match kind {
            Kind::Outline => outline_line(&ink, n, &line, width.unwrap_or(FALLBACK_WIDTH)),
            Kind::Route => route_line(&ink, &line, table),
            Kind::Where => where_line(&ink, n, &line, text.lines().count()),
            Kind::Plain => ink.line(&line),
        };
        // `show` is a page of its own (the title, then the outline), with no mark and no indent
        match (kind, n) {
            (Kind::Outline, _) => {}
            (_, 0) => out.push_str(&mark.painted()),
            _ => out.push_str("  "),
        }
        out.push_str(&painted);
        out.push('\n');
    }
    out
}

/// `text` cut to `room` columns, with `…` where it was cut. A column is a character.
pub fn cut(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    let kept: String = text.chars().take(room.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

// ── the ink: which words are cyan, dim, green ──────────────────────────────────────────────────────────────────────────

/// What colours a line of words: the tour ids it knows, a stop's or block's id (`s3`, `s2.b1`), the pin, and the labels that start a line.
struct Ink {
    words: Regex,
}

fn label() -> &'static str {
    r"^(?:next|note|before|after|ask the user|stopped|not pinned):?"
}

impl Ink {
    fn new(ids: &[String]) -> Ink {
        let mut known: Vec<&String> = ids.iter().filter(|i| !i.is_empty()).collect();
        known.sort_by_key(|i| std::cmp::Reverse(i.len()));
        let known = known
            .iter()
            .map(|i| regex::escape(i))
            .collect::<Vec<_>>()
            .join("|");
        let ids = if known.is_empty() {
            String::new()
        } else {
            format!("(?P<id>{known})|")
        };
        let words = Regex::new(&format!(
            r"{ids}(?P<stop>\bs\d+(?:\.b\d+)*\b)|(?P<pin>\bpinned at [0-9a-f]{{4,40}}\b)|(?P<not>\bnot pinned(?: any more)?\b)|(?P<label>{})",
            label()
        ))
        .expect("the ink's pattern is fixed");
        Ink { words }
    }

    /// One line of words, coloured.
    fn line(&self, line: &str) -> String {
        // code and quoted lines are shown as they are
        if line.trim_start().starts_with('>') {
            return line.to_string();
        }
        let mut out = String::new();
        let mut at = 0;
        for found in self.words.captures_iter(line) {
            let whole = found.get(0).expect("a match has a whole");
            if whole.start() < at {
                continue;
            }
            out.push_str(&line[at..whole.start()]);
            out.push_str(&paint(Self::style_of(&found), whole.as_str()));
            at = whole.end();
        }
        out.push_str(&line[at..]);
        out
    }

    fn style_of(found: &Captures<'_>) -> Style {
        if found.name("id").is_some() || found.name("stop").is_some() {
            location()
        } else if found.name("pin").is_some() {
            good()
        } else if found.name("not").is_some() {
            dim()
        } else {
            match found.name("label").map(|m| m.as_str()) {
                Some(l) if l.starts_with("ask the user") => warn(),
                _ => dim(),
            }
        }
    }
}

fn regex_of(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("a fixed pattern"))
}

/// The columns of a row (two spaces or more between them), the first in `first` and the others dim.
fn columns(rest: &str, first: Style) -> String {
    static GAP: OnceLock<Regex> = OnceLock::new();
    let gap = regex_of(&GAP, r"\s{2,}");
    let mut out = String::new();
    let mut at = 0;
    let mut n = 0;
    for between in gap.find_iter(rest) {
        out.push_str(&paint(
            if n == 0 { first } else { dim() },
            &rest[at..between.start()],
        ));
        out.push_str(between.as_str());
        at = between.end();
        n += 1;
    }
    out.push_str(&paint(if n == 0 { first } else { dim() }, &rest[at..]));
    out
}

/// `show`'s lines: line 0 is the title (bold) and the facts at the right (the state, the size, the pin), line 1 the id, where it is kept and the question, then
/// a row a stop (`number  id  name  place  steps  ← from`, a nested stop's name starting with `└`) and under it the rows of its blocks (`id  lines  title`,
/// the lines dim); a step's text under them is shown as it is.
fn outline_line(ink: &Ink, n: usize, line: &str, width: usize) -> String {
    static STOP: OnceLock<Regex> = OnceLock::new();
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    if let Some(c) = regex_of(
        &BLOCK,
        r"^(\s*)(s\d+(?:\.b\d+)+)(\s{2})(\d+(?:-\d+)?)(\s{2})(.*)$",
    )
    .captures(line)
    {
        return format!(
            "{}{}{}{}{}{}",
            &c[1],
            paint(location(), &c[2]),
            &c[3],
            paint(dim(), &c[4]),
            &c[5],
            paint(bold(), &c[6])
        );
    }
    if line.starts_with("      ") {
        return line.to_string();
    }
    if n == 0 {
        let (title, facts) = line.rsplit_once("   ").unwrap_or((line, ""));
        let gap = width
            .saturating_sub(title.chars().count() + facts.chars().count())
            .max(3);
        return format!(
            "{}{}{}",
            paint(bold(), title),
            " ".repeat(gap),
            facts_line(facts)
        );
    }
    if n == 1 {
        let mut parts = line.splitn(3, " · ");
        let id = parts.next().unwrap_or("");
        let sep = paint(dim(), " · ");
        let mut row = ink.line(id);
        for part in parts {
            row.push_str(&sep);
            row.push_str(&paint(dim(), part));
        }
        return row;
    }
    if let Some(c) = regex_of(&STOP, r"^(\s*)(\d+)(\s{2,})(s\d+)(\s{2,})(.*)$").captures(line) {
        return format!(
            "{}{}{}{}{}{}",
            &c[1],
            paint(dim(), &c[2]),
            &c[3],
            paint(location(), &c[4]),
            &c[5],
            columns(&c[6], location())
        );
    }
    ink.line(line)
}

/// `writing · 3 of 4 stops · 8 steps · by claude-code · not pinned (being written)`: the state, the pin and the check each in their own colour.
fn facts_line(line: &str) -> String {
    let sep = paint(dim(), " · ");
    line.split(" · ")
        .map(|part| {
            let style = match part {
                "writing" | "changed" => warn(),
                "stopping" | "stopped" => style::bad(),
                p if p.starts_with("pinned at") => good(),
                _ => dim(),
            };
            paint(style, part)
        })
        .collect::<Vec<_>>()
        .join(&sep)
}

/// `route`'s lines: the first block is the sizes (a small table: the name bold, the rest dim), then the stops (`number  name  place  line 16`).
fn route_line(ink: &Ink, line: &str, table: bool) -> String {
    static STOP: OnceLock<Regex> = OnceLock::new();
    if table {
        return columns(line, bold());
    }
    if let Some(c) = regex_of(&STOP, r"^(\s*)(\d+)(\s{2,})(.*)$").captures(line) {
        return format!(
            "{}{}{}{}",
            &c[1],
            paint(dim(), &c[2]),
            &c[3],
            columns(&c[4], location())
        );
    }
    ink.line(line)
}

/// `where` and `go`: the tour and where the reader is (line 0 and 1), the step's node (line 2), then the step's words and code as they are, and the lines
/// before and after at the end.
fn where_line(ink: &Ink, n: usize, line: &str, lines: usize) -> String {
    match n {
        0 | 1 => ink.line(line),
        2 if line.starts_with(['ƒ', '◆', '▪', '□', '◌', '?']) => columns(line, location()),
        2 => ink.line(line),
        n if n + 1 == lines && (line.starts_with("before:") || line.starts_with("after:")) => {
            ink.line(line)
        }
        _ => line.to_string(),
    }
}

// ── list ───────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// Columns before the title: ` ● `, the state's word padded to 10, a space.
const LEAD: usize = 14;

/// `list`: a block a tour, a blank line between them. First the tours being written, then the one read last (the tour `where` answers for), then the
/// rest in the order the rows came (newest first).
///
/// ```text
///  ● writing    How a stock hold is released                     2 of 5 stops
///               knowledgebase-init-cw6v · private · 3 min ago
///               “why does init call the loader twice?”
/// ```
///
/// `last_read` is the id of the tour whose place moved last. `now` is the time the ages are told from. `width` is the terminal's (`None` for a pipe:
/// the right edge is at [`FALLBACK_WIDTH`] and nothing is cut).
pub fn list(
    rows: &[ListRowView],
    last_read: Option<&str>,
    now: &str,
    width: Option<usize>,
) -> String {
    let writing = |r: &ListRowView| matches!(r.state, Some("writing" | "stopping"));
    let last = last_read.and_then(|id| {
        rows.iter()
            .position(|r| r.readable && r.id == id && !writing(r))
    });
    let mut order: Vec<(usize, bool)> = Vec::new();
    order.extend(
        rows.iter()
            .enumerate()
            .filter(|(_, r)| writing(r))
            .map(|(i, _)| (i, false)),
    );
    order.extend(last.map(|i| (i, true)));
    order.extend(
        rows.iter()
            .enumerate()
            .filter(|(i, r)| !writing(r) && Some(*i) != last)
            .map(|(i, _)| (i, false)),
    );
    let blocks: Vec<String> = order
        .into_iter()
        .map(|(i, is_last)| block(&rows[i], is_last, now, width))
        .collect();
    blocks.join("\n")
}

/// One tour's block of two or three lines.
fn block(row: &ListRowView, is_last: bool, now: &str, width: Option<usize>) -> String {
    let total = width.unwrap_or(FALLBACK_WIDTH);
    let lead = " ".repeat(LEAD);
    if !row.readable {
        let mut first = format!(" ✗ {:<10} {}", "unreadable", row.id);
        let folder = view_folder(row);
        let room = total.saturating_sub(first.chars().count() + 2);
        if width.is_some() && folder.chars().count() > room {
            first = cut(&first, total);
        } else {
            let pad = total
                .saturating_sub(first.chars().count() + folder.chars().count())
                .max(2);
            first = format!("{first}{}{folder}", " ".repeat(pad));
        }
        let why = row.problem.clone().unwrap_or_default();
        let why = match width {
            Some(w) => cut(&format!("{lead}{why}"), w),
            None => format!("{lead}{why}"),
        };
        return format!(
            "{}\n{}\n",
            paint(style::bad(), first),
            paint(style::bad(), why)
        );
    }
    // line 1: the state, the title, and the size or the place
    let (glyph, word, state_style) = match row.state {
        Some("writing") => ("●", "writing", warn()),
        Some("stopping") => ("■", "stopping", style::bad()),
        Some("stopped") => ("■", "stopped", style::bad()),
        _ if is_last => ("▶", "last read", good()),
        _ => (" ", "finished", dim()),
    };
    let right = match &row.on {
        Some(on) => on.words.clone(),
        None if is_last => place_words(row),
        None => view::stops_words(row.written.unwrap_or(0), row.planned.unwrap_or(0)),
    };
    let title = row.title.clone().unwrap_or_default();
    let title = match width {
        Some(_) => cut(
            &title,
            total
                .saturating_sub(LEAD + 2 + right.chars().count())
                .max(8),
        ),
        None => title,
    };
    let used = LEAD + title.chars().count() + right.chars().count();
    let pad = total.saturating_sub(used).max(2);
    let mut text = format!(
        " {} {} {}{}{}\n",
        paint(state_style, glyph),
        paint(state_style, format!("{word:<10}")),
        paint(bold(), &title),
        " ".repeat(pad),
        paint(dim(), &right)
    );
    // line 2: the id, where it is kept, the pin, whether it changed, when
    let sep = paint(dim(), " · ");
    let mut parts = vec![paint(location(), &row.id)];
    parts.push(paint(
        dim(),
        match (row.folder, row.private_copy) {
            ("project", _) => "shared",
            (_, true) => "private copy",
            _ => "private",
        },
    ));
    let being_written = matches!(row.state, Some("writing" | "stopping"));
    if !being_written {
        match (&row.pin, row.pinned) {
            (Some(pin), Some(true)) => {
                let short = &pin[..pin.len().min(7)];
                let held = if row.pin_in_repository == Some(false) {
                    " (not in this repository)"
                } else {
                    ""
                };
                parts.push(paint(good(), format!("pinned at {short}{held}")));
            }
            _ => parts.push(paint(dim(), "not pinned")),
        }
    }
    match row.check {
        Some("changed") => parts.push(paint(warn(), "changed")),
        Some("fresh") if row.say_fresh => parts.push(paint(good(), "fresh")),
        _ => {}
    }
    let when = view::when_word(&row.started, now);
    if !when.is_empty() {
        parts.push(paint(dim(), when));
    }
    let second = parts.join(&sep);
    // on a terminal the second line is cut by its plain text, so it is cut before it is painted
    let second = match width {
        Some(w) if plain_len(&second) + LEAD > w => {
            cut_painted(&parts, &sep, w.saturating_sub(LEAD))
        }
        _ => second,
    };
    text.push_str(&format!("{lead}{second}\n"));
    // line 3: the question
    if let Some(question) = &row.question {
        let line = format!("“{question}”");
        let line = match width {
            Some(w) => cut(&line, w.saturating_sub(LEAD)),
            None => line,
        };
        text.push_str(&format!("{lead}{}\n", paint(dim(), line)));
    }
    text
}

fn view_folder(row: &ListRowView) -> &'static str {
    if row.folder == "project" {
        "shared"
    } else {
        "private"
    }
}

/// `stop 2 of 3`, `cover`, `end`: where the reader is in a tour.
fn place_words(row: &ListRowView) -> String {
    let total = row.written.unwrap_or(0) + row.planned.unwrap_or(0);
    match &row.place {
        Some(place) => match (place.stop, place.at) {
            (Some(n), _) => format!("stop {n} of {total}"),
            (None, at) => at.to_string(),
        },
        None => view::stops_words(row.written.unwrap_or(0), row.planned.unwrap_or(0)),
    }
}

/// The length of what a person sees: the text without the marks of the paint.
fn plain_len(painted: &str) -> usize {
    static ESC: OnceLock<Regex> = OnceLock::new();
    // the paint's own marks are private-use characters around hex digits, then `ESC [ … m`'s body (see `style`); in a test they are real escapes
    let strip = regex_of(&ESC, r"(?:\x1b|\u{E000}[0-9a-f]{32}\u{E001})\[[0-9;]*m");
    strip.replace_all(painted, "").chars().count()
}

/// The parts of line 2 joined, and cut to `room` columns: the parts that fit whole, then the one that does not, cut with `…`.
fn cut_painted(parts: &[String], sep: &str, room: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for (i, part) in parts.iter().enumerate() {
        let sep_len = if i == 0 { 0 } else { 3 };
        let len = plain_len(part);
        if used + sep_len + len > room {
            if room > used + sep_len + 1 {
                if i > 0 {
                    out.push_str(sep);
                }
                out.push_str(&cut(&strip_paint(part), room - used - sep_len));
            }
            break;
        }
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(part);
        used += sep_len + len;
    }
    out
}

fn strip_paint(painted: &str) -> String {
    static ESC: OnceLock<Regex> = OnceLock::new();
    regex_of(&ESC, r"(?:\x1b|\u{E000}[0-9a-f]{32}\u{E001})\[[0-9;]*m")
        .replace_all(painted, "")
        .into_owned()
}

// ── check ──────────────────────────────────────────────────────────────────────────────────────────────────────────────

/// What `check` says of a tour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    UpToDate,
    Changed,
    Unreadable,
}

/// The first line of `check`: the mark, and the count in the words of today (`3 tours · 1 with stale stops`).
pub fn check_header(words: &str, mark: Mark) -> String {
    format!("{}{}\n\n", mark.painted(), words)
}

/// One tour of `check`: its state, its title and, at the right, its id; under it the details (what changed, one line each). A tour that is up to date has
/// a line and nothing more, unless there is something to say of it.
///
/// ```text
///  ● changed     Creating a knowledge base                creating-a-knowledge-base-j8f6
///                1 stop changed · (checked now)
///                s2 changed: KnowledgeBasesResource.put knowledge_base/base_resource.py:99-148
///    up to date  knowledge_base/base_resource.py          knowledge-base-base-resource-py-4xre
/// ```
pub fn check_block(
    state: Checked,
    title: &str,
    id: &str,
    details: &[String],
    ids: &[String],
    width: Option<usize>,
) -> String {
    let total = width.unwrap_or(FALLBACK_WIDTH);
    let (glyph, word, style) = match state {
        Checked::Changed => ("●", "changed", warn()),
        Checked::UpToDate => (" ", "up to date", dim()),
        Checked::Unreadable => ("✗", "unreadable", style::bad()),
    };
    let title = match width {
        Some(_) => cut(
            title,
            total.saturating_sub(LEAD + 2 + id.chars().count()).max(8),
        ),
        None => title.to_string(),
    };
    let pad = total
        .saturating_sub(LEAD + title.chars().count() + id.chars().count())
        .max(2);
    let mut text = format!(
        " {} {} {}{}{}\n",
        paint(style, glyph),
        paint(style, format!("{word:<10}")),
        paint(bold(), &title),
        " ".repeat(pad),
        paint(location(), id)
    );
    let ink = Ink::new(ids);
    for detail in details {
        let detail = match width {
            Some(w) => cut(detail, w.saturating_sub(LEAD)),
            None => detail.clone(),
        };
        let painted = if state == Checked::Unreadable {
            paint(style::bad(), &detail)
        } else {
            ink.line(&detail)
        };
        text.push_str(&format!("{}{painted}\n", " ".repeat(LEAD)));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str, kind: Kind) -> String {
        style::with_color(false, || {
            dress_at(text, &["a-1234".to_string()], kind, None)
        })
    }

    #[test]
    fn an_answer_has_a_mark_and_its_details_are_indented() {
        assert_eq!(
            plain(
                "finished: 2 stops, 4 steps\nnot pinned: this is not a git repository\n",
                Kind::Plain
            ),
            "✓ finished: 2 stops, 4 steps\n  not pinned: this is not a git repository\n"
        );
    }

    #[test]
    fn an_answer_that_asks_the_user_is_marked_with_a_bang() {
        let text = "finished: 2 stops, 4 steps\nask the user  1 stop has no ID        vnoc tour ids a-1234\n";
        assert!(plain(text, Kind::Plain).starts_with("! finished: 2 stops"));
        assert!(plain("stopped: the reader stopped this tour\n", Kind::Plain).starts_with("! "));
        assert!(plain("a-1234   private\ncannot be read: x\n", Kind::Plain).starts_with("✗ "));
        assert!(plain("a-1234   private\n", Kind::Plain).starts_with("✓ "));
    }

    #[test]
    fn a_blank_line_stays_blank_and_the_route_table_lines_up() {
        let text = "a    1 stop   about 1 min\nb    2 stops  about 2 min\n\n 1  f  x.py:1\n";
        assert_eq!(
            plain(text, Kind::Route),
            "✓ a    1 stop   about 1 min\n  b    2 stops  about 2 min\n\n   1  f  x.py:1\n"
        );
    }

    #[test]
    fn colour_is_the_ink_of_ids_pins_and_labels() {
        let painted = style::with_color(true, || {
            dress_at(
                "started a-1234   private\nnext: vnoc tour add a-1234 s1\n",
                &["a-1234".to_string()],
                Kind::Plain,
                None,
            )
        });
        assert_eq!(
            painted,
            "\x1b[32m✓\x1b[0m started \x1b[36ma-1234\x1b[0m   private\n  \x1b[2mnext:\x1b[0m vnoc tour add \x1b[36ma-1234\x1b[0m \x1b[36ms1\x1b[0m\n"
        );
    }

    #[test]
    fn a_terminal_cuts_a_row_with_an_ellipsis_and_a_pipe_never() {
        let row = "x".repeat(60);
        let cut_at_40 = style::with_color(false, || dress_at(&row, &[], Kind::Plain, Some(40)));
        assert_eq!(cut_at_40.trim_end().chars().count(), 40);
        assert!(cut_at_40.trim_end().ends_with('…'));
        assert_eq!(plain(&row, Kind::Plain), format!("✓ {row}\n"));
    }
}
