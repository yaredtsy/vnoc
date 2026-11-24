//! The text an agent sends for a stop (`docs/26-tours/02-commands.md` §4): an intro, then `##` blocks, each with `###` details, every part one
//! paragraph. This file reads the **shape** of the text and refuses what is wrong with it; the rules about the node and the budgets are `rules.rs`.
//!
//! ```text
//! <the intro: one paragraph>
//!
//! ## 11-24 Hold the stock, then charge
//! <one paragraph>
//!
//! ### 11-16 Check the stock, then hold it
//! <one paragraph>
//! ```
//!
//! A [`Body`] is a tree because it is how a person wrote it; it is never stored (the records are flat, `record.rs`). Blocks are numbered in the order
//! their headings are written, at any depth (`block 3` is the third heading), whether or not the heading was accepted: the numbers in a refusal are
//! the ones the agent can count in its own text.

use super::rules::Refusal;

/// The most text one stop takes, in bytes. Over it, one refusal and nothing else is read.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// One block: a `##`, or a `###` detail of the block before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyBlock {
    /// Which heading of the text this is, from 1, at any depth.
    pub number: usize,
    /// `[first, last]`, as the heading wrote them.
    pub lines: (u32, u32),
    pub title: String,
    /// The text as one paragraph: its lines trimmed and joined by one space.
    pub text: String,
    /// The same text as the lines it was written in (trimmed, none blank): what a rule about the start of a line looks at.
    pub text_lines: Vec<String>,
    pub details: Vec<BodyBlock>,
}

/// The parse of what an agent typed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Body {
    /// Everything before the first heading, as one paragraph.
    pub intro: String,
    /// The intro as the lines it was written in (trimmed, none blank).
    pub intro_lines: Vec<String>,
    pub blocks: Vec<BodyBlock>,
}

/// Reads the text, or says everything that is wrong with its shape.
pub fn parse(text: &str) -> Result<Body, Vec<Refusal>> {
    let (body, refusals) = read(text);
    if refusals.is_empty() {
        Ok(body)
    } else {
        Err(refusals)
    }
}

/// As [`parse`], but gives the body as far as it could be read **and** the refusals, so that the caller can run `rules::check` on what there is and
/// list every problem of one call together (02 §4: "every problem of one call is listed"). A heading that was refused, and what is under it, is
/// not in the body. Over [`MAX_BODY_BYTES`] the body is empty.
pub fn read(text: &str) -> (Body, Vec<Refusal>) {
    read_as(text, Text::Stop)
}

/// What the text is for: it decides whether an intro is wanted and whether a `###` is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// a stop: an intro (required), `##` blocks and `###` details of them
    Stop,
    /// `add --to`: blocks only. An intro is refused, since the stop has its own
    Blocks,
    /// `add --to --under`: the `##` blocks are details of one block. An intro is refused, and so is a `###`: details nest one level
    Details,
}

/// [`read`] for a text of the kind it is.
pub fn read_as(text: &str, kind: Text) -> (Body, Vec<Refusal>) {
    if text.len() > MAX_BODY_BYTES {
        let refusal = Refusal {
            place: "text".to_string(),
            problem: format!(
                "the text is {} bytes: the most one stop takes is {MAX_BODY_BYTES}",
                text.len()
            ),
        };
        return (Body::default(), vec![refusal]);
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut refusals = Vec::new();
    let mut body = Body::default();

    // the parts: the intro, then one part per heading
    let mut parts: Vec<(Option<Heading>, Vec<&str>)> = vec![(None, Vec::new())];
    let mut headings = 0;
    for line in text.lines() {
        match heading(line) {
            Some((hashes, rest)) => {
                headings += 1;
                parts.push((
                    Some(Heading {
                        number: headings,
                        hashes,
                        rest,
                    }),
                    Vec::new(),
                ));
            }
            None => parts.last_mut().expect("there is a part").1.push(line),
        }
    }

    /// Where the text under a heading goes.
    #[derive(PartialEq)]
    enum Open {
        /// no `##` yet
        Nothing,
        /// the last `##` was kept: the index of its block
        Block(usize),
        /// the last `##` was refused: what is under it is not read
        Refused,
    }
    let mut open = Open::Nothing;
    for (head, lines) in parts {
        let Some(head) = head else {
            let (intro, intro_lines) = paragraph("intro", &lines, &mut refusals);
            if kind != Text::Stop {
                if !intro_lines.is_empty() {
                    refusals.push(Refusal {
                        place: "intro".to_string(),
                        problem:
                            "intro: --to adds blocks: leave out the first paragraph, start with a ## heading"
                                .to_string(),
                    });
                }
            } else if intro_lines.is_empty() {
                refusals.push(Refusal {
                    place: "intro".to_string(),
                    problem: "intro: there is none. Start with one paragraph on what this is for, before the first heading".to_string(),
                });
            }
            body.intro = intro;
            body.intro_lines = intro_lines;
            continue;
        };
        let n = head.number;
        let refuse = |problem: String, refusals: &mut Vec<Refusal>| {
            refusals.push(Refusal {
                place: format!("block {n}"),
                problem: format!("block {n}: {problem}"),
            });
        };
        match head.hashes {
            2 | 3 => {
                let detail = head.hashes == 3;
                if detail && kind == Text::Details {
                    refuse(
                        "details nest one level: with --under the blocks of the text are the details, write them as ##"
                            .to_string(),
                        &mut refusals,
                    );
                    continue;
                }
                if detail && open == Open::Nothing {
                    refuse(
                        "a detail needs a block before it: write a ## block first".to_string(),
                        &mut refusals,
                    );
                    continue;
                }
                if detail && open == Open::Refused {
                    continue; // its block was refused: say that, not this
                }
                let (lines_of, title) = match numbers_and_title(head.rest, head.hashes) {
                    Ok(read) => read,
                    Err(problem) => {
                        refuse(problem, &mut refusals);
                        if !detail {
                            open = Open::Refused;
                        }
                        continue;
                    }
                };
                let (text, text_lines) =
                    paragraph(&format!("block {n} text"), &lines, &mut refusals);
                if text_lines.is_empty() {
                    refusals.push(Refusal {
                        place: format!("block {n} text"),
                        problem: format!(
                            "block {n} text: there is none. Every block is one paragraph on why those lines are there"
                        ),
                    });
                }
                let block = BodyBlock {
                    number: n,
                    lines: lines_of,
                    title,
                    text,
                    text_lines,
                    details: Vec::new(),
                };
                match (detail, &open) {
                    (true, Open::Block(top)) => body.blocks[*top].details.push(block),
                    _ => {
                        body.blocks.push(block);
                        open = Open::Block(body.blocks.len() - 1);
                    }
                }
            }
            _ => refuse(
                "blocks nest two deep: make this a ### or its own ##".to_string(),
                &mut refusals,
            ),
        }
    }
    (body, refusals)
}

struct Heading<'a> {
    number: usize,
    hashes: usize,
    rest: &'a str,
}

/// A heading is a line that starts with `#`s and then a space (or ends there). `#hashtag` is text.
fn heading(line: &str) -> Option<(usize, &str)> {
    let line = line.trim_start();
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    let rest = &line[hashes..];
    if hashes == 0 || !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    Some((hashes, rest.trim()))
}

/// `11-24 A title`, or `24 A title`: the lines and the title; or why not (the sentence after `block n: `).
fn numbers_and_title(rest: &str, hashes: usize) -> Result<((u32, u32), String), String> {
    let marks = "#".repeat(hashes);
    let no_numbers = || {
        format!(
            "the heading has no line numbers: write `{marks} 11-24 Title` (`{marks} 24 Title` for one line)"
        )
    };
    let token = rest.split_whitespace().next().ok_or_else(no_numbers)?;
    let number = |digits: &str| {
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .then(|| digits.parse::<u32>().ok())
            .flatten()
    };
    let (first, last) = match token.split_once('-') {
        Some((a, b)) => (number(a), number(b)),
        None => (number(token), number(token)),
    };
    let (Some(first), Some(last)) = (first, last) else {
        return Err(no_numbers());
    };
    if first == 0 || last == 0 {
        return Err("lines start at 1, not 0".to_string());
    }
    if last < first {
        return Err(format!(
            "lines {first}-{last} are backwards: the first line comes first"
        ));
    }
    let title = rest[token.len()..].trim();
    if title.is_empty() {
        return Err(format!(
            "the heading has no title: write `{marks} {first}-{last} Title`"
        ));
    }
    Ok(((first, last), title.to_string()))
}

/// The lines of one part as a paragraph: blank lines at its start and its end are nothing, a blank line inside it is a second paragraph (refused,
/// once). Returns the lines joined by one space, and the lines (trimmed, none blank).
fn paragraph(place: &str, lines: &[&str], refusals: &mut Vec<Refusal>) -> (String, Vec<String>) {
    let blank = |line: &&str| line.trim().is_empty();
    let start = lines.iter().position(|l| !blank(l));
    let end = lines.iter().rposition(|l| !blank(l));
    let (Some(start), Some(end)) = (start, end) else {
        return (String::new(), Vec::new());
    };
    let used = &lines[start..=end];
    if used.iter().any(blank) {
        refusals.push(Refusal {
            place: place.to_string(),
            problem: format!(
                "{place}: a blank line starts a second paragraph. A step is one paragraph"
            ),
        });
    }
    let kept: Vec<String> = used
        .iter()
        .filter(|l| !blank(l))
        .map(|l| l.trim().to_string())
        .collect();
    (kept.join(" "), kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(text: &str) -> Vec<String> {
        parse(text)
            .expect_err("a refusal")
            .into_iter()
            .map(|r| r.problem)
            .collect()
    }

    fn block(b: &BodyBlock) -> (usize, (u32, u32), &str, &str) {
        (b.number, b.lines, b.title.as_str(), b.text.as_str())
    }

    #[test]
    fn an_intro_only() {
        let body = parse("checkout_order turns an order into a sale.").unwrap();
        assert_eq!(body.intro, "checkout_order turns an order into a sale.");
        assert!(body.blocks.is_empty());
        // the lines of a paragraph are joined by one space, each trimmed; the text around it is trimmed
        let body = parse("\n\n  one two  \n\tthree\r\nfour   \n\n\n").unwrap();
        assert_eq!(body.intro, "one two three four");
        assert_eq!(body.intro_lines, ["one two", "three", "four"]);
    }

    #[test]
    fn three_blocks() {
        let body = parse(
            "Intro.\n\n## 3-5 First\nText one.\n\n## 7-9 Second\nText two,\nin two lines.\n## 10 Third\nText three.\n",
        )
        .unwrap();
        assert_eq!(body.intro, "Intro.");
        let got: Vec<_> = body.blocks.iter().map(block).collect();
        assert_eq!(
            got,
            [
                (1, (3, 5), "First", "Text one."),
                (2, (7, 9), "Second", "Text two, in two lines."),
                (3, (10, 10), "Third", "Text three."),
            ]
        );
        assert!(body.blocks.iter().all(|b| b.details.is_empty()));
        assert_eq!(body.blocks[1].text_lines, ["Text two,", "in two lines."]);
    }

    #[test]
    fn a_block_with_two_details() {
        let body = parse("I.\n## 11-24 Whole\nT.\n### 11-16 First part\nA.\n### 18-24 Second part\nB.\n## 26-30 Next\nN.").unwrap();
        assert_eq!(body.blocks.len(), 2);
        let first = &body.blocks[0];
        assert_eq!(block(first), (1, (11, 24), "Whole", "T."));
        let details: Vec<_> = first.details.iter().map(block).collect();
        assert_eq!(
            details,
            [
                (2, (11, 16), "First part", "A."),
                (3, (18, 24), "Second part", "B."),
            ]
        );
        assert_eq!(block(&body.blocks[1]), (4, (26, 30), "Next", "N."));
    }

    #[test]
    fn the_minicart_text_of_the_design() {
        let text = "checkout_order turns an order request into a completed order: it holds the stock, takes the money, and records\nthe sale, in that order.\n\n## 11-24 Hold the stock, then charge\nNothing is charged until the items are set aside, and nothing stays set aside if the charge fails.\n\n### 11-16 Check the stock, then hold it\ncheck_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them\nwhile this payment runs.\n\n### 18-24 Charge, and give the stock back if it times out\nA timeout is the one failure it knows how to undo: cancel_checkout puts the units back, then the error goes up.\n\n## 26-30 Record the sale\nThe order is created only after the money is taken, so there is never an order nobody paid for.\n";
        let body = parse(text).unwrap();
        assert!(
            body.intro
                .starts_with("checkout_order turns an order request")
        );
        assert!(body.intro.ends_with("records the sale, in that order."));
        assert!(!body.intro.contains('\n'));
        assert_eq!(body.blocks.len(), 2, "two ## blocks");
        assert_eq!(
            body.blocks[0].details.len(),
            2,
            "two ### details under the first"
        );
        assert_eq!(body.blocks[1].details.len(), 0);
        assert_eq!(body.blocks[0].lines, (11, 24));
        assert_eq!(body.blocks[0].details[0].lines, (11, 16));
        assert_eq!(body.blocks[0].details[1].lines, (18, 24));
        assert_eq!(body.blocks[1].lines, (26, 30));
        assert_eq!(
            body.blocks[0].details[0].text,
            "check_stock only looks; reserve_stock takes the units out of the count, so a second buyer cannot get them while this payment runs."
        );
        assert_eq!(
            body.blocks.iter().map(|b| b.number).collect::<Vec<_>>(),
            [1, 4]
        );
    }

    #[test]
    fn a_heading_with_one_number_is_one_line() {
        let body = parse("I.\n## 24 Title\nT.").unwrap();
        assert_eq!(body.blocks[0].lines, (24, 24));
        assert_eq!(body.blocks[0].title, "Title");
        let body = parse("I.\n##   24-24   A  longer   title \nT.").unwrap();
        assert_eq!(body.blocks[0].lines, (24, 24));
        assert_eq!(body.blocks[0].title, "A  longer   title");
    }

    #[test]
    fn a_text_with_no_intro_is_refused() {
        for text in ["", "   \n\n", "## 1-2 T\nt", "\n\n## 1-2 T\nt"] {
            let got = problems(text);
            assert!(
                got[0].starts_with("intro: there is none."),
                "{text:?}: {got:?}"
            );
        }
    }

    #[test]
    fn a_blank_line_inside_a_part_is_a_second_paragraph() {
        assert_eq!(
            problems("One.\n\nTwo.\n## 1-2 T\nt"),
            ["intro: a blank line starts a second paragraph. A step is one paragraph"]
        );
        assert_eq!(
            problems("I.\n## 1-2 T\nfirst\n\nsecond"),
            ["block 1 text: a blank line starts a second paragraph. A step is one paragraph"]
        );
        // a line with only white space is blank
        assert_eq!(
            problems("I.\n## 1-2 T\nfirst\n  \t \nsecond"),
            ["block 1 text: a blank line starts a second paragraph. A step is one paragraph"]
        );
        // a blank line between the heading and its text, or after the text, is nothing
        let body = parse("I.\n\n## 1-2 T\n\nthe text\n\n\n## 3-4 U\n\nmore\n\n").unwrap();
        assert_eq!(body.blocks[0].text, "the text");
        assert_eq!(body.blocks[1].text, "more");
        // several parts with one: each is named, in text order
        assert_eq!(
            problems("a\n\nb\n## 1-2 T\nc\n\nd\n### 1-1 U\ne\n\nf").len(),
            3
        );
    }

    #[test]
    fn a_block_with_no_text_is_refused() {
        assert_eq!(
            problems("I.\n## 1-2 T\n## 3-4 U\nu"),
            [
                "block 1 text: there is none. Every block is one paragraph on why those lines are there"
            ]
        );
        assert_eq!(problems("I.\n## 1-2 T").len(), 1);
    }

    #[test]
    fn a_detail_before_any_block_is_refused() {
        assert_eq!(
            problems("I.\n### 1-2 T\nt"),
            ["block 1: a detail needs a block before it: write a ## block first"]
        );
        // each one, and the ## after them still reads
        let (body, refusals) = read("I.\n### 1-2 T\nt\n### 3-4 U\nu\n## 5-6 V\nv");
        assert_eq!(refusals.len(), 2);
        assert_eq!(body.blocks.len(), 1);
        assert_eq!(body.blocks[0].number, 3);
    }

    #[test]
    fn a_heading_that_nests_too_deep_or_not_at_all_is_refused() {
        let sentence = "blocks nest two deep: make this a ### or its own ##";
        assert_eq!(
            problems("I.\n## 1-9 T\nt\n### 2-3 U\nu\n#### 2-2 V\nv"),
            [format!("block 3: {sentence}")]
        );
        assert_eq!(
            problems("I.\n##### 1-2 T\nt"),
            [format!("block 1: {sentence}")]
        );
        assert_eq!(problems("I.\n# Title\nt"), [format!("block 1: {sentence}")]);
        assert_eq!(
            problems("# Title\nI.\n"),
            [
                "intro: there is none. Start with one paragraph on what this is for, before the first heading".to_string(),
                format!("block 1: {sentence}"),
            ]
        );
        // the text under a refused heading is not read as the block's: the block before keeps its text, and its details go on
        let (body, refusals) = read("I.\n## 1-9 T\nt\n#### 2-3 X\nx\n### 4-5 U\nu");
        assert_eq!(refusals.len(), 1);
        assert_eq!(body.blocks[0].text, "t");
        assert_eq!(body.blocks[0].details.len(), 1);
        assert_eq!(body.blocks[0].details[0].number, 3);
    }

    #[test]
    fn a_hash_that_is_not_a_heading_is_text() {
        let body = parse("I'd say #1 is #hashtag.\n#!/bin/sh stays text\n## 1-2 T\nsee #3 and ##4")
            .unwrap();
        assert!(body.intro.contains("#hashtag"));
        assert!(body.intro.contains("#!/bin/sh"));
        assert_eq!(body.blocks[0].text, "see #3 and ##4");
    }

    #[test]
    fn a_heading_with_bad_line_numbers_is_refused() {
        let cases: [(&str, &str); 12] = [
            (
                "## Hold the stock",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "##",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## 11- Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## -24 Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## a-b Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## 11–24 Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## 1-2-3 Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## +5 Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            (
                "## 99999999999 Title",
                "block 1: the heading has no line numbers: write `## 11-24 Title` (`## 24 Title` for one line)",
            ),
            ("## 0 Title", "block 1: lines start at 1, not 0"),
            (
                "## 24-11 Title",
                "block 1: lines 24-11 are backwards: the first line comes first",
            ),
            (
                "## 11-24",
                "block 1: the heading has no title: write `## 11-24 Title`",
            ),
        ];
        for (heading, want) in cases {
            assert_eq!(problems(&format!("I.\n{heading}\nt")), [want], "{heading}");
        }
        assert_eq!(
            problems("I.\n## 0-5 T\nt"),
            ["block 1: lines start at 1, not 0"]
        );
        // a detail's heading says its own marks
        assert_eq!(
            problems("I.\n## 1-9 T\nt\n### x U\nu"),
            [
                "block 2: the heading has no line numbers: write `### 11-24 Title` (`### 24 Title` for one line)"
            ]
        );
        // a refused block takes what is under it: its details are not refused a second time
        let (body, refusals) = read("I.\n## x T\nt\n### 1-2 U\nu\n### 3-4 V\nv\n## 5-6 W\nw");
        assert_eq!(refusals.len(), 1);
        assert_eq!(body.blocks.len(), 1);
        assert_eq!(body.blocks[0].number, 4);
    }

    #[test]
    fn every_problem_of_the_shape_is_listed_in_text_order() {
        let text = "One.\n\nTwo.\n## 1-2\nt\n### 3-4 U\nu\n\nv\n#### 5 X\nx\n## 24-11 Y\ny";
        let got = problems(text);
        assert_eq!(
            got,
            [
                "intro: a blank line starts a second paragraph. A step is one paragraph",
                "block 1: the heading has no title: write `## 1-2 Title`",
                "block 3: blocks nest two deep: make this a ### or its own ##",
                "block 4: lines 24-11 are backwards: the first line comes first",
            ]
        );
        // the detail under the refused `## 1-2` is not read, so its own blank line is not listed
    }

    #[test]
    fn a_text_over_the_limit_is_one_refusal_before_anything_else_is_read() {
        let big = format!("{}\n\n## x\n", "a".repeat(MAX_BODY_BYTES));
        let refusals = parse(&big).unwrap_err();
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].place, "text");
        assert_eq!(
            refusals[0].problem,
            format!(
                "the text is {} bytes: the most one stop takes is 65536",
                big.len()
            )
        );
        // at the limit it is read
        let at = "a".repeat(MAX_BODY_BYTES);
        assert!(parse(&at).is_ok());
        let (body, refusals) = read(&big);
        assert_eq!(refusals.len(), 1);
        assert_eq!(body, Body::default());
    }

    #[test]
    fn crlf_and_a_byte_order_mark() {
        let body = parse("\u{feff}Intro line one\r\nline two\r\n\r\n## 1-2 T\r\ntext\r\n").unwrap();
        assert_eq!(body.intro, "Intro line one line two");
        assert_eq!(body.blocks[0].text, "text");
    }

    fn said(text: &str, kind: Text) -> Vec<String> {
        read_as(text, kind)
            .1
            .into_iter()
            .map(|r| r.problem)
            .collect()
    }

    #[test]
    fn a_text_for_to_has_blocks_and_no_intro_and_one_that_has_an_intro_is_refused() {
        let (body, refusals) = read_as(
            "## 3-5 First\nIt starts.\n\n### 4 Detail\nInside.\n",
            Text::Blocks,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(body.intro, "");
        assert_eq!(body.blocks.len(), 1);
        assert_eq!(body.blocks[0].details.len(), 1);
        // an intro is the stop's own: refused, with the sentence of 02 §4
        assert_eq!(
            said("An intro.\n\n## 3-5 First\nIt starts.\n", Text::Blocks),
            ["intro: --to adds blocks: leave out the first paragraph, start with a ## heading"]
        );
        // and the same text is a stop's with no refusal about the intro the other way round
        assert_eq!(
            said("## 3-5 First\nIt starts.\n", Text::Stop),
            [
                "intro: there is none. Start with one paragraph on what this is for, before the first heading"
            ]
        );
        // nothing at all is no blocks and no refusal here: the caller says that
        assert!(said("", Text::Blocks).is_empty());
    }

    #[test]
    fn with_under_the_blocks_are_details_and_a_heading_of_three_is_refused() {
        let (body, refusals) = read_as("## 3-4 One\nA.\n\n## 5-6 Two\nB.\n", Text::Details);
        assert!(refusals.is_empty());
        assert_eq!(body.blocks.len(), 2);
        assert_eq!(
            said("## 3-4 One\nA.\n\n### 4 Deeper\nB.\n", Text::Details),
            [
                "block 2: details nest one level: with --under the blocks of the text are the details, write them as ##"
            ]
        );
        // the other refusals of the shape are the same
        assert_eq!(
            said("## 0-4 One\nA.\n", Text::Details),
            ["block 1: lines start at 1, not 0"]
        );
    }
}
