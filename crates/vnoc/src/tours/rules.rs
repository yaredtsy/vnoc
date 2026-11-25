//! What a stop's text has to be, against the node it is about (`docs/26-tours/02-commands.md` §4, the two tables): budgets, lines, nesting,
//! the inline rule. Pure: it is given the parsed [`Body`] and the node's lines, and answers with every refusal of the call, in text order, or none
//! (D9: every reason at once, nothing is written).
//!
//! **Not here, because they need the tour or the index (TR4):** `--from` is a stop of this tour; `--line` is inside that stop's node; `--same-as`
//! names a written stop on the same node; at most 40 stops on the route; the node is in the index.
//!
//! **Characters.** A budget counts `chars().count()` of the **prose** of a text (what is outside its fences and its tables) after its lines are joined by
//! a space. That is Unicode scalar values: an emoji written with a variation selector or as a family of joined emoji is more than one.
//!
//! **What a text may hold** (`05-on-screen.md` §7): paragraphs, `code`, *emphasis*, **strong**, lists, tables, block quotes, code fences, one Mermaid fence,
//! links. Refused, each named once per part: raw HTML, images, a Markdown heading, a second Mermaid fence, a fence over [`MAX_FENCE`] lines, a table over
//! [`MAX_TABLE_ROWS`] rows (and a fence that is never closed, which would swallow the blocks after it).

use std::fmt;

use vnoc_domain::NodeKind;

use super::body::{self, Body, BodyBlock};
use super::record::Detail;

/// One thing wrong with the text, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Where it is, as 02 §4 numbers things: `intro`, `block 2 title`, `block 2 text`, `block 3`, or `text` for the whole of it. A block is the n-th
    /// heading of the text, at any depth.
    pub place: String,
    /// The sentence the agent reads (02 §4's, with the numbers filled in). It carries its own place where 02's does (`block 2 text: 412 of 280 characters`).
    pub problem: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.problem)
    }
}

/// The node a stop is about, as the index has it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLines {
    /// What the refusals call it: `reserve_stock`, `app/`. A folder's name has `/` added if it has none.
    pub name: String,
    /// 1-based, as `vnoc code` prints them. Not looked at for a folder.
    pub first: u32,
    pub last: u32,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// `--same-as`: the text is one sentence, and has no blocks.
    pub same_as: bool,
}

/// A node shorter than this many lines is one step (W9).
pub const SHORT: u32 = 8;
/// Blocks in a stop, details counted.
const MAX_BLOCKS: usize = 12;
const TITLE_BUDGET: usize = 60;
/// The most lines in one fence, code or Mermaid.
pub const MAX_FENCE: usize = 30;
/// The most rows in one table (its header counts).
pub const MAX_TABLE_ROWS: usize = 12;

/// (quick, normal, detailed)
fn budget(by_detail: (usize, usize, usize), detail: Detail) -> usize {
    match detail {
        Detail::Quick => by_detail.0,
        Detail::Normal => by_detail.1,
        Detail::Detailed => by_detail.2,
    }
}

fn refusal(place: impl Into<String>, problem: impl Into<String>) -> Refusal {
    Refusal {
        place: place.into(),
        problem: problem.into(),
    }
}

/// Every refusal of this text for this node, in text order; empty when it is good. Run `body::read`'s refusals first: they are the shape of the text.
pub fn check(body: &Body, node: &NodeLines, detail: Detail, options: Options) -> Vec<Refusal> {
    let mut out = Vec::new();

    let intro_budget = if options.same_as {
        budget((180, 180, 240), detail)
    } else {
        budget((240, 420, 720), detail)
    };
    over(&mut out, "intro", &scan(&body.intro).prose, intro_budget);
    markdown(&mut out, "intro", &body.intro);

    // the blocks, flat, in the order they were written, each with the block it is a detail of
    let mut flat: Vec<(&BodyBlock, Option<&BodyBlock>)> = Vec::new();
    for top in &body.blocks {
        flat.push((top, None));
        flat.extend(top.details.iter().map(|d| (d, Some(top))));
    }
    flat.sort_by_key(|(b, _)| b.number);

    let is_folder = node.kind == NodeKind::Folder;
    let node_lines = node.last.saturating_sub(node.first) + 1;
    let short = !is_folder && node_lines < SHORT;
    // a node that takes no blocks: said once, at the first block; its blocks' lines are not looked at (there is nothing to be inside)
    let no_blocks = if is_folder {
        let name = node.name.trim_end_matches('/');
        Some(format!("{name}/ is a folder: it has no lines to light"))
    } else if options.same_as {
        Some("a --same-as stop is one sentence: it has no blocks".to_string())
    } else if short {
        let lines = if node_lines == 1 {
            "1 line".to_string()
        } else {
            format!("{node_lines} lines")
        };
        Some(format!(
            "{} is {lines}: a stop this short is one step. Put it all in the first paragraph",
            node.name
        ))
    } else {
        None
    };

    for (i, (block, parent)) in flat.iter().enumerate() {
        let n = block.number;
        let place = format!("block {n}");
        if i == 0
            && let Some(sentence) = &no_blocks
        {
            out.push(refusal(&place, sentence.clone()));
        }
        if i == MAX_BLOCKS {
            out.push(refusal(
                &place,
                format!(
                    "this stop has {} blocks, the most one stop holds is {MAX_BLOCKS} (details count). Make the rest a second stop",
                    flat.len()
                ),
            ));
        }
        let (first, last) = block.lines;
        if no_blocks.is_none() {
            if first < node.first || last > node.last {
                let what = if first == last {
                    format!("line {first} is")
                } else {
                    format!("lines {first}-{last} are")
                };
                out.push(refusal(
                    &place,
                    format!(
                        "block {n}: {what} outside {} ({}-{})",
                        node.name, node.first, node.last
                    ),
                ));
            }
            if let Some(parent) = parent
                && (first < parent.lines.0 || last > parent.lines.1)
            {
                out.push(refusal(
                    &place,
                    format!(
                        "block {n} ({first}-{last}) is not inside block {} ({}-{})",
                        parent.number, parent.lines.0, parent.lines.1
                    ),
                ));
            }
            // the first earlier block beside it (the same parent, or both at the top) that shares a line
            let beside: &[BodyBlock] = parent.map_or(&body.blocks, |p| &p.details);
            if let Some(other) = beside
                .iter()
                .filter(|o| o.number < n)
                .find(|o| o.lines.0 <= last && first <= o.lines.1)
            {
                out.push(refusal(
                    &place,
                    format!(
                        "blocks {} and {n} both light line {}",
                        other.number,
                        first.max(other.lines.0)
                    ),
                ));
            }
        }
        let title_place = format!("block {n} title");
        over(&mut out, &title_place, &block.title, TITLE_BUDGET);
        if block.title.ends_with('.') {
            out.push(refusal(
                &title_place,
                format!("block {n} title ends with a full stop"),
            ));
        }
        inline(
            &mut out,
            &title_place,
            std::slice::from_ref(&block.title),
            "A title is a short phrase",
        );
        let text_place = format!("block {n} text");
        over(
            &mut out,
            &text_place,
            &scan(&block.text).prose,
            budget((240, 420, 900), detail),
        );
        markdown(&mut out, &text_place, &block.text);
    }
    out
}

/// `block 2 text: 412 of 280 characters`
fn over(out: &mut Vec<Refusal>, place: &str, text: &str, budget: usize) {
    let length = text.chars().count();
    if length > budget {
        out.push(refusal(
            place,
            format!("{place}: {length} of {budget} characters"),
        ));
    }
}

/// What a text is made of, for the rules: its prose (outside fences and tables, the lines trimmed and joined by a space), everything outside fences (for
/// the tags and images a table may hold), its fences and its tables.
struct Scan {
    prose: String,
    outside_fences: String,
    fences: Vec<FenceFound>,
    tables: Vec<usize>,
    heading: bool,
}

struct FenceFound {
    mermaid: bool,
    /// the lines between the opening and the closing line
    lines: usize,
    closed: bool,
}

/// A delimiter row of a table: `|---|:-:|`, `--- | ---`: dashes, colons, bars and spaces, with a dash and a bar.
fn is_delimiter(line: &str) -> bool {
    let line = line.trim();
    line.contains('-')
        && line.contains('|')
        && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn scan(text: &str) -> Scan {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Scan {
        prose: String::new(),
        outside_fences: String::new(),
        fences: Vec::new(),
        tables: Vec::new(),
        heading: false,
    };
    // which lines are in a fence, and which are in a table
    let mut in_fence = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let Some(open) = body::opens(lines[i]) else {
            i += 1;
            continue;
        };
        let info = lines[i]
            .trim_start()
            .trim_start_matches(['`', '~'])
            .trim()
            .to_lowercase();
        let mut end = i + 1;
        while end < lines.len() && !body::closes(lines[end], open) {
            end += 1;
        }
        let closed = end < lines.len();
        found.fences.push(FenceFound {
            mermaid: info.split_whitespace().next() == Some("mermaid"),
            lines: end - i - 1,
            closed,
        });
        for flag in &mut in_fence[i..(end + 1).min(lines.len())] {
            *flag = true;
        }
        i = end + 1;
    }
    let mut in_table = vec![false; lines.len()];
    let mut j = 1;
    while j < lines.len() {
        if !in_fence[j] && !in_fence[j - 1] && is_delimiter(lines[j]) && lines[j - 1].contains('|')
        {
            let mut end = j + 1;
            while end < lines.len()
                && !in_fence[end]
                && lines[end].contains('|')
                && !lines[end].trim().is_empty()
            {
                end += 1;
            }
            found.tables.push(end - (j - 1) - 1); // the header and the rows, not the delimiter
            for flag in &mut in_table[j - 1..end] {
                *flag = true;
            }
            j = end;
        } else {
            j += 1;
        }
    }
    let mut prose = Vec::new();
    let mut outside = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        if in_fence[n] {
            continue;
        }
        outside.push(line.trim());
        if in_table[n] {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if is_heading(line) {
            found.heading = true;
        }
        prose.push(line.trim());
    }
    found.prose = prose.join(" ");
    found.outside_fences = outside.join(" ");
    found
}

/// `#` to `######`, then a space or the end of the line: a Markdown heading. `#hashtag` is text.
fn is_heading(line: &str) -> bool {
    let line = line.trim_start();
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    (1..=6).contains(&hashes)
        && line[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
}

/// The rule for what Markdown a text may hold: paragraphs, `code`, *emphasis*, **strong**, lists, tables, block quotes, links and fences (one of them
/// Mermaid) pass. Refused, each named once per part, in this order: raw HTML, an image, a heading, a second Mermaid fence, a fence over [`MAX_FENCE`] lines, a
/// table over [`MAX_TABLE_ROWS`] rows, a fence that is not closed. Inside a `code` span nothing is refused; inside a fence nothing is looked at but its length.
fn markdown(out: &mut Vec<Refusal>, place: &str, text: &str) {
    let found = scan(text);
    let outside = outside_code_spans(&found.outside_fences);
    let mut say = |problem: String| out.push(refusal(place, format!("{place}: {problem}")));
    if has_html(&outside) {
        say("HTML. A text is Markdown: tags are not drawn, write the words".to_string());
    }
    if outside.contains("![") {
        say(
            "an image. A text has no images: say it in words, or draw it as a Mermaid diagram"
                .to_string(),
        );
    }
    if found.heading {
        say("a heading. `##` and `###` are the stop's blocks and details; inside a text there are no headings: use **bold** for a line of its own".to_string());
    }
    if found.fences.iter().filter(|f| f.mermaid).count() > 1 {
        say(
            "a second Mermaid diagram. One diagram a step: the other goes in another step"
                .to_string(),
        );
    }
    if let Some(long) = found.fences.iter().find(|f| f.lines > MAX_FENCE) {
        say(format!(
            "a fence of {} lines. The most is {MAX_FENCE}: show only the lines that matter",
            long.lines
        ));
    }
    if let Some(rows) = found.tables.iter().find(|r| **r > MAX_TABLE_ROWS) {
        say(format!(
            "a table of {rows} rows. The most is {MAX_TABLE_ROWS}: keep the rows that matter"
        ));
    }
    if found.fences.iter().any(|f| !f.closed) {
        say("a code fence that is not closed. Close it with ``` on a line of its own".to_string());
    }
}

/// The inline rule: `code` and *emphasis* pass; a list, table, code fence, image, link or HTML is refused, each named, each once per part, in that
/// order. `lines` are the lines of the part as they were written: a list, a table and a fence are about how a line starts. Inside a `code` span
/// nothing is refused.
fn inline(out: &mut Vec<Refusal>, place: &str, lines: &[String], tail: &str) {
    let mut found: Vec<&str> = Vec::new();
    let starts = |test: &dyn Fn(&str) -> bool| lines.iter().any(|l| test(l.trim_start()));
    if starts(&is_list_item) {
        found.push("a list");
    }
    if starts(&|l| l.starts_with('|')) {
        found.push("a table");
    }
    // a fence opens a line with three backticks and does not close them on it (a run of three backticks that closes is a code span)
    if starts(&|l| l.starts_with("```") && !l[3..].contains("```")) {
        found.push("a code fence");
    }
    let outside = outside_code_spans(&lines.join(" "));
    if outside.contains("![") {
        found.push("an image");
    }
    if has_link(&outside) {
        found.push("a link");
    }
    if has_html(&outside) {
        found.push("HTML");
    }
    for what in found {
        out.push(refusal(place, format!("{place}: {what}. {tail}")));
    }
}

/// `- x`, `* x`, `+ x`, or a number and `. `: `1. x`.
fn is_list_item(line: &str) -> bool {
    if ["- ", "* ", "+ "]
        .iter()
        .any(|bullet| line.starts_with(bullet))
    {
        return true;
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && line[digits..].starts_with(". ")
}

/// `[` that is not an image's, with `](` somewhere after it.
fn has_link(text: &str) -> bool {
    text.char_indices()
        .any(|(i, c)| c == '[' && !text[..i].ends_with('!') && text[i..].contains("]("))
}

/// `<` followed by a letter or `/`.
fn has_html(text: &str) -> bool {
    text.char_indices().any(|(i, c)| {
        c == '<'
            && text[i + 1..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_alphabetic() || next == '/')
    })
}

/// The text with every `code` span taken out (a span is a run of backticks, what is inside, and a run of as many: ``` `a` ``` or ` `` a`b `` `).
/// A run with no match is just backticks.
fn outside_code_spans(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let run = after.bytes().take_while(|b| *b == b'`').count();
        let body = &after[run..];
        match find_run(body, run) {
            Some(close) => {
                out.push(' '); // the span is one thing, not a gap that joins its two sides
                rest = &body[close + run..];
            }
            None => {
                out.push_str(&after[..run]);
                rest = body;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Where in `text` a run of exactly `run` backticks starts.
fn find_run(text: &str, run: usize) -> Option<usize> {
    let mut at = 0;
    while let Some(found) = text[at..].find('`') {
        let start = at + found;
        let length = text[start..].bytes().take_while(|b| *b == b'`').count();
        if length == run {
            return Some(start);
        }
        at = start + length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::body;

    fn node(name: &str, first: u32, last: u32) -> NodeLines {
        NodeLines {
            name: name.to_string(),
            first,
            last,
            kind: NodeKind::Function,
        }
    }

    /// What a call to `add` would say: the shape's refusals, then the rules'.
    fn say(text: &str, node: &NodeLines, detail: Detail, options: Options) -> Vec<String> {
        let (body, mut said) = body::read(text);
        said.extend(check(&body, node, detail, options));
        said.into_iter().map(|r| r.problem).collect()
    }

    fn normal(text: &str, node: &NodeLines) -> Vec<String> {
        say(text, node, Detail::Normal, Options::default())
    }

    /// A node with room for any block of these tests.
    fn big() -> NodeLines {
        node("f", 1, 100)
    }

    const MINICART: &str = "checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records\nthe sale, in that order.\n\n## 11-24 Hold the stock, then charge\nNothing is charged until the items are set aside, and nothing stays set aside if the charge fails.\n\n### 11-16 Check the stock, then hold it\ncheck_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them\nwhile this payment runs.\n\n### 18-24 Charge, and give the stock back if it times out\nA timeout is the one failure it knows how to undo: cancel_checkout puts the units back, then the error goes up.\n\n## 26-30 Record the sale\nThe order is created only after the money is taken, so there is never an order nobody paid for.\n";

    #[test]
    fn the_minicart_text_of_the_design_passes_against_lines_11_to_30() {
        for detail in [Detail::Quick, Detail::Normal, Detail::Detailed] {
            let said = say(
                MINICART,
                &node("checkout_order", 11, 30),
                detail,
                Options::default(),
            );
            assert_eq!(said, Vec::<String>::new(), "{detail:?}");
        }
    }

    #[test]
    fn a_block_whose_lines_are_outside_the_node() {
        let cases: [(&str, u32, u32, &str); 6] = [
            (
                "## 40-44 T\nt",
                19,
                31,
                "block 1: lines 40-44 are outside reserve_stock (19-31)",
            ),
            (
                "## 18-25 T\nt",
                19,
                31,
                "block 1: lines 18-25 are outside reserve_stock (19-31)",
            ),
            (
                "## 25-32 T\nt",
                19,
                31,
                "block 1: lines 25-32 are outside reserve_stock (19-31)",
            ),
            (
                "## 40 T\nt",
                19,
                31,
                "block 1: line 40 is outside reserve_stock (19-31)",
            ),
            (
                "## 18 T\nt",
                19,
                31,
                "block 1: line 18 is outside reserve_stock (19-31)",
            ),
            (
                "## 1-99 T\nt",
                19,
                31,
                "block 1: lines 1-99 are outside reserve_stock (19-31)",
            ),
        ];
        for (text, first, last, want) in cases {
            let got = normal(
                &format!("Intro.\n{text}"),
                &node("reserve_stock", first, last),
            );
            assert_eq!(got, [want], "{text}");
        }
        // the whole node, and one line of it, are inside
        for text in [
            "## 19-31 T\nt",
            "## 19 T\nt",
            "## 31 T\nt",
            "## 19-20 T\nt\n## 30-31 U\nu",
        ] {
            assert!(
                normal(&format!("Intro.\n{text}"), &node("reserve_stock", 19, 31)).is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn a_detail_outside_its_block() {
        let text = "I.\n## 11-24 A\na\n### 12-14 B\nb\n### 25-26 C\nc\n";
        assert_eq!(
            normal(text, &node("f", 11, 40)),
            ["block 3 (25-26) is not inside block 1 (11-24)"]
        );
        // either end
        assert_eq!(
            normal("I.\n## 11-24 A\na\n### 10-14 B\nb", &node("f", 1, 40)),
            ["block 2 (10-14) is not inside block 1 (11-24)"]
        );
        assert_eq!(
            normal("I.\n## 11-24 A\na\n### 20-30 B\nb", &node("f", 1, 40)),
            ["block 2 (20-30) is not inside block 1 (11-24)"]
        );
        // equal to its block is inside; a one-line detail at the end is inside
        assert_eq!(
            normal(
                "I.\n## 11-24 A\na\n### 11-24 B\nb\n### 24 C\nc",
                &node("f", 1, 40)
            ),
            ["blocks 2 and 3 both light line 24"],
            "inside the block, and overlapping each other"
        );
        assert!(normal("I.\n## 11-24 A\na\n### 11-24 B\nb", &node("f", 1, 40)).is_empty());
        // a detail outside the node and outside its block: both are said
        assert_eq!(
            normal("I.\n## 11-24 A\na\n### 90-95 B\nb", &node("f", 1, 40)),
            [
                "block 2: lines 90-95 are outside f (1-40)",
                "block 2 (90-95) is not inside block 1 (11-24)",
            ]
        );
    }

    #[test]
    fn blocks_beside_each_other_do_not_overlap() {
        assert_eq!(
            normal("I.\n## 1-3 A\na\n## 15-17 B\nb\n## 17-20 C\nc", &big()),
            ["blocks 2 and 3 both light line 17"]
        );
        // the first line they share
        assert_eq!(
            normal("I.\n## 10-30 A\na\n## 20-40 B\nb", &big()),
            ["blocks 1 and 2 both light line 20"]
        );
        assert_eq!(
            normal("I.\n## 20-40 A\na\n## 10-30 B\nb", &big()),
            ["blocks 1 and 2 both light line 20"]
        );
        // one inside the other, equal, one line
        assert_eq!(
            normal("I.\n## 10-30 A\na\n## 15 B\nb", &big()),
            ["blocks 1 and 2 both light line 15"]
        );
        assert_eq!(
            normal("I.\n## 10 A\na\n## 10 B\nb", &big()),
            ["blocks 1 and 2 both light line 10"]
        );
        // two details of one block
        assert_eq!(
            normal("I.\n## 10-30 A\na\n### 12-14 B\nb\n### 13-15 C\nc", &big()),
            ["blocks 2 and 3 both light line 13"]
        );
        // each later block names the first earlier one it meets, once
        assert_eq!(
            normal("I.\n## 10-20 A\na\n## 15-25 B\nb\n## 18-30 C\nc", &big()),
            [
                "blocks 1 and 2 both light line 15",
                "blocks 1 and 3 both light line 18"
            ]
        );
        // beside each other is what is checked: end and start on neighbouring lines, a detail and its block, details of two blocks
        assert!(normal("I.\n## 10-16 A\na\n## 17-20 B\nb", &big()).is_empty());
        assert!(normal("I.\n## 10-20 A\na\n### 10-20 B\nb", &big()).is_empty());
        assert!(
            normal(
                "I.\n## 10-20 A\na\n### 12-14 B\nb\n## 30-40 C\nc\n### 31-35 D\nd\n### 36-40 E\ne",
                &big()
            )
            .is_empty()
        );
    }

    #[test]
    fn a_node_under_8_lines_has_no_blocks() {
        let short = |first, last, name: &str| {
            normal("I.\n## 1-2 T\nt\n## 3-4 U\nu", &node(name, first, last))
        };
        // said once, at the first block, and the lines of the blocks are not looked at
        assert_eq!(
            short(12, 15, "check_stock"),
            [
                "check_stock is 4 lines: a stop this short is one step. Put it all in the first paragraph"
            ]
        );
        assert_eq!(
            short(5, 5, "f"),
            ["f is 1 line: a stop this short is one step. Put it all in the first paragraph"]
        );
        // 7 lines is short, 8 is not
        assert_eq!(
            short(10, 16, "seven"),
            ["seven is 7 lines: a stop this short is one step. Put it all in the first paragraph"]
        );
        let eight = normal("I.\n## 10-12 T\nt\n## 13-17 U\nu", &node("eight", 10, 17));
        assert!(eight.is_empty(), "{eight:?}");
        // a short node with no blocks is fine
        assert!(normal("Just the intro.", &node("check_stock", 12, 15)).is_empty());
        // a short node still has its budgets and its text rules
        let long = "x".repeat(500);
        assert_eq!(
            normal(&format!("{long}\n## 1 T\nt"), &node("f", 1, 4)),
            [
                "intro: 500 of 420 characters",
                "f is 4 lines: a stop this short is one step. Put it all in the first paragraph",
            ]
        );
    }

    #[test]
    fn a_folder_has_no_blocks_and_neither_has_a_same_as_stop() {
        let folder = |name: &str| NodeLines {
            name: name.to_string(),
            first: 0,
            last: 0,
            kind: NodeKind::Folder,
        };
        for name in ["app", "app/"] {
            assert_eq!(
                normal("I.\n## 1-2 T\nt\n## 3-4 U\nu", &folder(name)),
                ["app/ is a folder: it has no lines to light"],
                "{name}"
            );
        }
        assert!(normal("A folder holds the checkout code.", &folder("app")).is_empty());
        let same = Options { same_as: true };
        assert_eq!(
            say(
                "Called again here.\n## 1-2 T\nt",
                &node("f", 1, 30),
                Detail::Normal,
                same
            ),
            ["a --same-as stop is one sentence: it has no blocks"]
        );
        assert!(
            say(
                "Called again here.",
                &node("f", 1, 30),
                Detail::Normal,
                same
            )
            .is_empty()
        );
        // a file is a node with lines: it may have blocks
        let file = NodeLines {
            name: "app/checkout.py".to_string(),
            first: 1,
            last: 120,
            kind: NodeKind::File,
        };
        assert!(normal("The checkout code.\n## 10-20 T\nt", &file).is_empty());
    }

    #[test]
    fn a_title_is_a_phrase_with_no_full_stop() {
        assert_eq!(
            normal("I.\n## 1-2 Hold the stock.\nt", &big()),
            ["block 1 title ends with a full stop"]
        );
        assert_eq!(
            normal("I.\n## 1-2 Wait...\nt", &big()),
            ["block 1 title ends with a full stop"]
        );
        for title in [
            "Hold the stock",
            "Why `x.y` first",
            "Is it 1.5?",
            "A. B",
            "Done!",
        ] {
            assert!(
                normal(&format!("I.\n## 1-2 {title}\nt"), &big()).is_empty(),
                "{title}"
            );
        }
        // a detail's title says its own number
        assert_eq!(
            normal("I.\n## 1-9 A\na\n### 2-3 B.\nb", &big()),
            ["block 2 title ends with a full stop"]
        );
    }

    #[test]
    fn the_budgets_at_their_limit_and_one_over_by_the_detail_of_the_tour() {
        // (field, quick, normal, detailed)
        let rows: [(&str, [usize; 3]); 4] = [
            ("intro", [240, 420, 720]),
            ("block 1 text", [240, 420, 900]),
            ("block 1 title", [60, 60, 60]),
            ("same-as", [180, 180, 240]),
        ];
        let details = [Detail::Quick, Detail::Normal, Detail::Detailed];
        for (field, budgets) in rows {
            for (detail, budget) in details.into_iter().zip(budgets) {
                let say_with = |length: usize| {
                    let words = "a".repeat(length);
                    let (text, options) = match field {
                        "intro" => (words, Options::default()),
                        "same-as" => (words, Options { same_as: true }),
                        "block 1 text" => (format!("I.\n## 1-2 T\n{words}"), Options::default()),
                        _ => (format!("I.\n## 1-2 {words}\nt"), Options::default()),
                    };
                    say(&text, &big(), detail, options)
                };
                assert!(
                    say_with(budget).is_empty(),
                    "{field} {detail:?} at {budget}"
                );
                let place = if field == "same-as" { "intro" } else { field };
                assert_eq!(
                    say_with(budget + 1),
                    [format!("{place}: {} of {budget} characters", budget + 1)],
                    "{field} {detail:?}"
                );
            }
        }
        // the numbers in the sentence are the lengths
        let long = "x".repeat(512);
        assert_eq!(
            normal(&format!("I.\n## 1-2 T\nok\n## 3-4 U\n{long}"), &big()),
            ["block 2 text: 512 of 420 characters"]
        );
    }

    #[test]
    fn characters_are_counted_not_bytes() {
        // `é` is two bytes, an emoji four: 420 of them are 420 characters
        for ch in ['é', '😀', '字'] {
            let at = ch.to_string().repeat(420);
            let over = ch.to_string().repeat(421);
            assert!(normal(&at, &big()).is_empty(), "{ch}");
            assert_eq!(
                normal(&over, &big()),
                ["intro: 421 of 420 characters"],
                "{ch}"
            );
            let title_at = ch.to_string().repeat(60);
            assert!(
                normal(&format!("I.\n## 1-2 {title_at}\nt"), &big()).is_empty(),
                "{ch}"
            );
            assert_eq!(
                normal(&format!("I.\n## 1-2 {title_at}x\nt"), &big()),
                ["block 1 title: 61 of 60 characters"],
                "{ch}"
            );
        }
        // the prose is counted joined: lines are trimmed and glued by one space
        let line = "a".repeat(209);
        assert!(
            normal(&format!("{line}\n{line}"), &big()).is_empty(),
            "209 + 1 + 209 = 419"
        );
        assert_eq!(
            normal(&format!("{line}  \n   {line}bb"), &big()),
            ["intro: 421 of 420 characters"]
        );
        // an emoji with a variation selector is two scalar values: the count is of those
        let heart = "\u{2764}\u{fe0f}".repeat(211);
        assert_eq!(normal(&heart, &big()), ["intro: 422 of 420 characters"]);
    }

    #[test]
    fn at_most_12_blocks_in_a_stop_and_details_count() {
        let blocks = |tops: u32, details_each: u32| {
            let mut text = String::from("I.\n");
            let mut line = 1;
            for _ in 0..tops {
                text.push_str(&format!("## {}-{} T\nt\n", line, line + details_each));
                for d in 0..details_each {
                    text.push_str(&format!("### {} D\nd\n", line + d));
                }
                line += details_each + 1;
            }
            text
        };
        let node = node("f", 1, 1000);
        assert!(normal(&blocks(12, 0), &node).is_empty());
        assert_eq!(
            normal(&blocks(13, 0), &node),
            [
                "this stop has 13 blocks, the most one stop holds is 12 (details count). Make the rest a second stop"
            ]
        );
        assert!(
            normal(&blocks(4, 2), &node).is_empty(),
            "4 blocks and 8 details: 12"
        );
        assert_eq!(normal(&blocks(5, 2), &node).len(), 1, "5 and 10: 15");
        // said at the first block over the limit
        let (body, _) = body::read(&blocks(13, 0));
        let said = check(&body, &node, Detail::Normal, Options::default());
        assert_eq!(said[0].place, "block 13");
    }

    const HTML: &str = "HTML. A text is Markdown: tags are not drawn, write the words";
    const IMAGE: &str =
        "an image. A text has no images: say it in words, or draw it as a Mermaid diagram";
    const HEADING: &str = "a heading. `##` and `###` are the stop's blocks and details; inside a text there are no headings: use **bold** for a line of its own";

    #[test]
    fn every_construct_a_text_may_hold_passes() {
        let allowed = [
            "A paragraph.\n\nAnd a second one, with `code`, *emphasis* and **strong**.",
            "- one\n- two\n  - nested\n\n1. first\n2. second\n\n+ plus\n* star",
            "| a | b |\n|---|---|\n| 1 | 2 |",
            "a | b\n--|--\n1 | 2",
            "> a block quote\n> over two lines\n\n> and another",
            "```python\nprint(1)\n```",
            "~~~\ntilde fence\n~~~",
            "```mermaid\nflowchart LR\n  A --> B\n```",
            "see [the docs](https://example.com/a) and [x](y \"title\")",
            "a longer fence holds a shorter one:\n````\n```\ninside\n```\n````",
            "a fence holds what a text may not: \n```html\n<b>bold</b> ![i](j)\n# heading\n## 1-2 block\n- x\n```",
            "---\n\na thematic break above",
        ];
        for text in allowed {
            for place in ["I.\n## 1-2 T\n", "I.\n## 1-9 T\nt\n### 1-2 D\n"] {
                let got = normal(&format!("{place}{text}"), &big());
                assert!(got.is_empty(), "{text}: {got:?}");
            }
            let got = normal(text, &big());
            assert!(got.is_empty(), "as an intro: {text}: {got:?}");
        }
    }

    #[test]
    fn what_a_text_may_not_hold_is_named_once_per_part_with_its_own_sentence() {
        let refused = |text: &str| normal(&format!("I.\n## 1-2 T\n{text}"), &big());
        let at = |what: &str| vec![format!("block 1 text: {what}")];
        // raw HTML, in every form
        for text in [
            "a <b>bold</b> word",
            "closing </div> only",
            "a <br/> break",
            "an <a href=x>anchor</a>",
            "<script>x</script>",
        ] {
            assert_eq!(refused(text), at(HTML), "{text}");
        }
        // an image, and an image is not also a link
        assert_eq!(refused("see ![diagram](d.png) here"), at(IMAGE));
        assert_eq!(refused("![a](b)"), at(IMAGE));
        // a heading: `#` and `####` to `######` (a `##` or `###` line is a block, and never gets here)
        for text in [
            "# One",
            "#### Four",
            "##### Five",
            "###### Six",
            "text\n#\nmore",
        ] {
            assert_eq!(refused(text), at(HEADING), "{text}");
        }
        assert!(
            refused("#hashtag and #1").is_empty(),
            "a hash that is not a heading is text"
        );
        // a second Mermaid fence, and one is fine
        assert_eq!(
            refused(
                "```mermaid\nflowchart LR\n A-->B\n```\ntext\n```mermaid\nflowchart LR\n C-->D\n```"
            ),
            at("a second Mermaid diagram. One diagram a step: the other goes in another step")
        );
        assert!(
            refused("```mermaid\nflowchart LR\n A-->B\n```\n```python\nx\n```\n```text\ny\n```")
                .is_empty()
        );
        // a fence over 30 lines, code or Mermaid; 30 is fine
        let lines = |n: usize| {
            (0..n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(refused(&format!("```\n{}\n```", lines(30))).is_empty());
        assert_eq!(
            refused(&format!("```\n{}\n```", lines(31))),
            at("a fence of 31 lines. The most is 30: show only the lines that matter")
        );
        assert_eq!(
            refused(&format!("```mermaid\n{}\n```", lines(45))),
            at("a fence of 45 lines. The most is 30: show only the lines that matter")
        );
        // a table over 12 rows (its header counts); 12 is fine
        let table = |rows: usize| {
            format!(
                "| a | b |\n|---|---|\n{}",
                (1..rows)
                    .map(|i| format!("| {i} | x |"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        assert!(refused(&table(12)).is_empty());
        assert_eq!(
            refused(&table(13)),
            at("a table of 13 rows. The most is 12: keep the rows that matter")
        );
        // a fence that is never closed would swallow the blocks after it
        assert_eq!(
            refused("```python\nx = 1"),
            at("a code fence that is not closed. Close it with ``` on a line of its own")
        );
        // each is named once per part, in a fixed order: HTML, image, heading, second diagram, long fence, long table, unclosed fence
        let mermaid = "```mermaid\nflowchart LR\n A-->B\n```";
        assert_eq!(
            refused(&format!(
                "<i>x</i> ![c](d)\n# H\n{mermaid}\n{mermaid}\n```\n{}\n```\n{}\n<b>y</b> ![e](f)\n```\nopen",
                lines(40),
                table(20)
            )),
            [
                HTML,
                IMAGE,
                HEADING,
                "a second Mermaid diagram. One diagram a step: the other goes in another step",
                "a fence of 40 lines. The most is 30: show only the lines that matter",
                "a table of 20 rows. The most is 12: keep the rows that matter",
                "a code fence that is not closed. Close it with ``` on a line of its own"
            ]
            .map(|w| format!("block 1 text: {w}"))
        );
        // the same words for the intro and for a detail
        assert_eq!(
            normal("<i>x</i>\n## 1-2 T\nt", &big()),
            [format!("intro: {HTML}")]
        );
        assert_eq!(
            normal("I.\n## 1-9 T\nt\n### 1-2 D\n![a](b)", &big()),
            [format!("block 2 text: {IMAGE}")]
        );
        // a title is a short phrase: HTML, an image, a link, a list are still refused there
        assert_eq!(
            normal("I.\n## 1-2 A <b>title</b>\nt", &big()),
            ["block 1 title: HTML. A title is a short phrase"]
        );
    }

    #[test]
    fn only_the_prose_is_counted_against_the_budget() {
        let prose = "p".repeat(400);
        // 400 characters of prose are inside the normal 420, whatever the fences and the tables hold
        let fence = format!("```\n{}\n```", "x".repeat(2000));
        let table = format!(
            "| a | b |\n|---|---|\n{}",
            (1..10)
                .map(|i| format!("| {} | {} |", "y".repeat(50), i))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(normal(&format!("{prose}\n\n{fence}\n\n{table}"), &big()).is_empty());
        assert!(
            normal(
                &format!("I.\n## 1-2 T\n{prose}\n\n{fence}\n\n{table}"),
                &big()
            )
            .is_empty()
        );
        // a diagram does not count either
        assert!(
            normal(
                &format!(
                    "{prose}\n```mermaid\nflowchart LR\n{}\n```",
                    "A --> B\n".repeat(20)
                ),
                &big()
            )
            .is_empty()
        );
        // the prose does: 421 is one over, with the same fences around it
        assert_eq!(
            normal(&format!("{prose}ppppp\n{fence}"), &big()),
            ["intro: 405 of 420 characters"]
                .iter()
                .filter(|_| false)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            normal(&format!("{}\n{fence}", "p".repeat(421)), &big()),
            ["intro: 421 of 420 characters"]
        );
        // the budget by detail: block (240, 420, 900), intro (240, 420, 720), same-as (180, 180, 240)
        let text = format!("I.\n## 1-2 T\n{}", "b".repeat(241));
        assert_eq!(
            say(&text, &big(), Detail::Quick, Options::default()),
            ["block 1 text: 241 of 240 characters"]
        );
        assert!(say(&text, &big(), Detail::Normal, Options::default()).is_empty());
        assert_eq!(
            say(
                &"s".repeat(181),
                &big(),
                Detail::Normal,
                Options { same_as: true }
            ),
            ["intro: 181 of 180 characters"]
        );
        assert!(
            say(
                &"s".repeat(240),
                &big(),
                Detail::Detailed,
                Options { same_as: true }
            )
            .is_empty()
        );
    }

    #[test]
    fn code_and_emphasis_pass_and_nothing_is_refused_inside_a_code_span() {
        let passes = [
            "Use `reserve_stock` and *then* check _it_ and **twice**.",
            "*emphasis* at the start of a line\n*and* the next",
            "`- x` is not a list",
            "the tag `<b>` is shown as text",
            "the markup `[a](b)` and `![i](j)` are shown as text",
            "```inline``` is a span too: `<div>`",
            "`` a `<i>` b `` holds a backtick and a tag",
            "a `span\nacross lines <b>` ends here",
            "`- one`\n`| two |`",
            "1.5 is a number, version 2. is not at a line start, and a - b is a minus",
            "a < b and x<3 and a <- b and 5 > 3",
            "a long run of ``` is fine when `` ` `` holds it",
            "price is $5 [approx] (maybe)",
            "[note] and (then) text",
            "the sign - at the end -",
            "-1 is not a list",
            "*not a list*",
        ];
        for text in passes {
            let got = normal(&format!("I.\n## 1-2 T\n{text}"), &big());
            assert!(got.is_empty(), "{text}: {got:?}");
        }
        // a backtick that closes nothing is just a character
        assert_eq!(
            normal("I.\n## 1-2 T\nan open ` backtick then <b>tag</b>", &big()),
            [format!("block 1 text: {HTML}")]
        );
        // a span of two backticks is not closed by one
        assert_eq!(
            normal("I.\n## 1-2 T\n`` a ` <b> and more", &big()),
            [format!("block 1 text: {HTML}")]
        );
        // a span does not join what is around it into a link
        assert!(normal("I.\n## 1-2 T\n[`a`] and (`b`)", &big()).is_empty());
        // a link is allowed, a span in it too
        assert!(normal("I.\n## 1-2 T\n[`a`](http://x)", &big()).is_empty());
    }

    #[test]
    fn several_refusals_in_one_call_in_text_order() {
        let long = "w".repeat(500);
        let text = format!(
            "<i>not text</i>\n\n## 40-44 A title.\n{long}\n### 12-14 Detail\nd\n### 13-15 Other\n![x](y)\n## 2-3 Last <b>one</b>\nfine"
        );
        let got = normal(&text, &node("reserve_stock", 11, 31));
        assert_eq!(
            got.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                &format!("intro: {HTML}"),
                "block 1: lines 40-44 are outside reserve_stock (11-31)",
                "block 1 title ends with a full stop",
                "block 1 text: 500 of 420 characters",
                "block 2 (12-14) is not inside block 1 (40-44)",
                "block 3 (13-15) is not inside block 1 (40-44)",
                "blocks 2 and 3 both light line 13",
                &format!("block 3 text: {IMAGE}"),
                "block 4: lines 2-3 are outside reserve_stock (11-31)",
                "block 4 title: HTML. A title is a short phrase",
            ]
        );
    }

    #[test]
    fn the_refusals_of_the_shape_come_first_and_the_numbers_are_the_ones_the_agent_wrote() {
        // a `####` is no block: it stays in the intro, and a heading is refused there. The `##` after it is block 1
        let long = "w".repeat(500);
        let text = format!("One.\n\ntwo.\n#### 1-2 X\nx\n## 90-91 Y\n{long}");
        let got = normal(&text, &node("f", 1, 30));
        assert_eq!(
            got,
            [
                format!("intro: {HEADING}"),
                "block 1: lines 90-91 are outside f (1-30)".to_string(),
                "block 1 text: 500 of 420 characters".to_string(),
            ]
        );
        // a bad heading of a block is the shape's, and comes before the rules'
        let got = normal("One.\n## x T\nx\n## 1-2 Y\n<b>y</b>", &node("f", 1, 30));
        assert_eq!(
            got,
            [
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)".to_string(),
                format!("block 2 text: {HTML}"),
            ]
        );
    }

    #[test]
    fn a_refusal_has_a_place_and_prints_as_its_sentence() {
        let (body, _) = body::read("<i>x</i>\n## 1-2 Bad.\n<b>y</b>");
        let said = check(&body, &node("f", 1, 30), Detail::Normal, Options::default());
        let places: Vec<&str> = said.iter().map(|r| r.place.as_str()).collect();
        assert_eq!(places, ["intro", "block 1 title", "block 1 text"]);
        assert_eq!(said[1].to_string(), "block 1 title ends with a full stop");
        // a block-wide problem is at `block n`
        let (body, _) = body::read("I.\n## 50-60 T\nt");
        let said = check(&body, &node("f", 1, 30), Detail::Normal, Options::default());
        assert_eq!(said[0].place, "block 1");
    }
}
