//! What a stop's text has to be, against the node it is about (`docs/26-tours/02-commands.md` §4, the two tables): budgets, lines, nesting,
//! the inline rule. Pure: it is given the parsed [`Body`] and the node's lines, and answers with every refusal of the call, in text order, or none
//! (D9: every reason at once, nothing is written).
//!
//! **Not here, because they need the tour or the index (TR4):** `--from` is a stop of this tour; `--line` is inside that stop's node; `--same-as`
//! names a written stop on the same node; at most 40 stops on the route; the node is in the index.
//!
//! **Characters.** A budget counts `chars().count()` of the paragraph after its lines are joined by a space. That is Unicode scalar values: an
//! emoji written with a variation selector or as a family of joined emoji is more than one.

use std::fmt;

use vnoc_domain::NodeKind;

use super::body::{Body, BodyBlock};
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
        budget((120, 120, 160), detail)
    } else {
        budget((160, 280, 480), detail)
    };
    over(&mut out, "intro", &body.intro, intro_budget);
    inline(
        &mut out,
        "intro",
        &body.intro_lines,
        "A step is one paragraph",
    );

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
            &block.text,
            budget((160, 280, 600), detail),
        );
        inline(
            &mut out,
            &text_place,
            &block.text_lines,
            "A step is one paragraph",
        );
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
        let long = "x".repeat(300);
        assert_eq!(
            normal(&format!("{long}\n## 1 T\nt"), &node("f", 1, 4)),
            [
                "intro: 300 of 280 characters",
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
            ("intro", [160, 280, 480]),
            ("block 1 text", [160, 280, 600]),
            ("block 1 title", [60, 60, 60]),
            ("same-as", [120, 120, 160]),
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
        let long = "x".repeat(412);
        assert_eq!(
            normal(&format!("I.\n## 1-2 T\nok\n## 3-4 U\n{long}"), &big()),
            ["block 2 text: 412 of 280 characters"]
        );
    }

    #[test]
    fn characters_are_counted_not_bytes() {
        // `é` is two bytes, an emoji four: 280 of them are 280 characters
        for ch in ['é', '😀', '字'] {
            let at = ch.to_string().repeat(280);
            let over = ch.to_string().repeat(281);
            assert!(normal(&at, &big()).is_empty(), "{ch}");
            assert_eq!(
                normal(&over, &big()),
                ["intro: 281 of 280 characters"],
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
        // the paragraph is counted joined: lines are trimmed and glued by one space
        let line = "a".repeat(139);
        assert!(
            normal(&format!("{line}\n{line}"), &big()).is_empty(),
            "139 + 1 + 139 = 279"
        );
        assert_eq!(
            normal(&format!("{line}  \n   {line}bb"), &big()),
            ["intro: 281 of 280 characters"]
        );
        // an emoji with a variation selector is two scalar values: the count is of those
        let heart = "\u{2764}\u{fe0f}".repeat(141);
        assert_eq!(normal(&heart, &big()), ["intro: 282 of 280 characters"]);
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

    #[test]
    fn the_inline_rule_names_what_it_refuses() {
        let refused = |text: &str| normal(&format!("I.\n## 1-2 T\n{text}"), &big());
        let cases: [(&str, &str); 14] = [
            ("- one\n- two", "a list"),
            ("fine\n* star", "a list"),
            ("+ plus", "a list"),
            ("1. first\n2. second", "a list"),
            ("12. twelfth", "a list"),
            ("| a | b |\n|---|---|", "a table"),
            ("```\ncode\n```", "a code fence"),
            ("see ![diagram](d.png) here", "an image"),
            ("see [the docs](http://x) here", "a link"),
            ("see [x](y) and `ok`", "a link"),
            ("a <b>bold</b> word", "HTML"),
            ("closing </div> only", "HTML"),
            ("a <br/> break", "HTML"),
            ("an <a href=x>anchor</a>", "HTML"),
        ];
        for (text, what) in cases {
            assert_eq!(
                refused(text),
                [format!("block 1 text: {what}. A step is one paragraph")],
                "{text}"
            );
        }
        // the same words for the intro and for a title
        assert_eq!(
            normal("- a\n- b\n## 1-2 T\nt", &big()),
            ["intro: a list. A step is one paragraph"]
        );
        assert_eq!(
            normal("I.\n## 1-2 A <b>title</b>\nt", &big()),
            ["block 1 title: HTML. A title is a short phrase"]
        );
        // each named once per part, in a fixed order: list, table, fence, image, link, HTML
        assert_eq!(
            refused("<i>x</i> [a](b) ![c](d) ``` | t |\n- item"),
            [
                "block 1 text: a list. A step is one paragraph",
                "block 1 text: an image. A step is one paragraph",
                "block 1 text: a link. A step is one paragraph",
                "block 1 text: HTML. A step is one paragraph",
            ]
        );
        assert_eq!(
            refused("| t |\n```\n- x"),
            [
                "block 1 text: a list. A step is one paragraph",
                "block 1 text: a table. A step is one paragraph",
                "block 1 text: a code fence. A step is one paragraph",
            ]
        );
        // an image is not also a link
        assert_eq!(
            refused("![a](b)"),
            ["block 1 text: an image. A step is one paragraph"]
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
            ["block 1 text: HTML. A step is one paragraph"]
        );
        // a span of two backticks is not closed by one
        assert_eq!(
            normal("I.\n## 1-2 T\n`` a ` <b> and more", &big()),
            ["block 1 text: HTML. A step is one paragraph"]
        );
        // a span does not join what is around it into a link
        assert!(normal("I.\n## 1-2 T\n[`a`] and (`b`)", &big()).is_empty());
        // a link is still refused when only part of it is in a span
        assert_eq!(
            normal("I.\n## 1-2 T\n[`a`](http://x)", &big()),
            ["block 1 text: a link. A step is one paragraph"].map(String::from)
        );
    }

    #[test]
    fn several_refusals_in_one_call_in_text_order() {
        let long = "w".repeat(300);
        let text = format!(
            "- not a paragraph\n\n## 40-44 A title.\n{long}\n### 12-14 Detail\nd\n### 13-15 Other\n[x](y)\n## 2-3 Last <b>one</b>\nfine"
        );
        let got = normal(&text, &node("reserve_stock", 11, 31));
        assert_eq!(
            got,
            [
                "intro: a list. A step is one paragraph",
                "block 1: lines 40-44 are outside reserve_stock (11-31)",
                "block 1 title ends with a full stop",
                "block 1 text: 300 of 280 characters",
                "block 2 (12-14) is not inside block 1 (40-44)",
                "block 3 (13-15) is not inside block 1 (40-44)",
                "blocks 2 and 3 both light line 13",
                "block 3 text: a link. A step is one paragraph",
                "block 4: lines 2-3 are outside reserve_stock (11-31)",
                "block 4 title: HTML. A title is a short phrase",
            ]
        );
    }

    #[test]
    fn the_refusals_of_the_shape_come_first_and_the_numbers_are_the_ones_the_agent_wrote() {
        // heading 1 is a `####` and is refused; the `##` after it is block 2, in the shape's and in the rules' sentences alike
        let long = "w".repeat(300);
        let text = format!("One.\n\ntwo.\n#### 1-2 X\nx\n## 90-91 Y\n{long}");
        let got = normal(&text, &node("f", 1, 30));
        assert_eq!(
            got,
            [
                "intro: a blank line starts a second paragraph. A step is one paragraph",
                "block 1: blocks nest two deep: make this a ### or its own ##",
                "block 2: lines 90-91 are outside f (1-30)",
                "block 2 text: 300 of 280 characters",
            ]
        );
    }

    #[test]
    fn a_refusal_has_a_place_and_prints_as_its_sentence() {
        let (body, _) = body::read("- x\n## 1-2 Bad.\n- y");
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
