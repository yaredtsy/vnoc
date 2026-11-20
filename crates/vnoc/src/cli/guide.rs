//! `vnoc guide`, `vnoc guide <topic>` and `vnoc skill` (docs/21-agent-cli/06-skill.md §6): the how-to an agent reads from the binary it is running,
//! so it cannot be older than the flags it describes. The texts are files in the repository, compiled in: `skills/vnoc/` at the root (the skill and
//! the rules block, which the wider skill installers read there too) and `crates/vnoc/guide/` (the topics).
//!
//! Nothing here needs a project or a driver. A terminal gets the headings and the command lines painted; the words are never touched.

use super::args::GuideArgs;
use super::{Failure, Global, out, style};

/// The skill, as `vnoc skill` prints it and `vnoc agent install` will write it.
pub const SKILL: &str = include_str!("../../../../skills/vnoc/SKILL.md");

/// The rules block for `AGENTS.md` and its kin, markers included (06 §4).
pub const RULES: &str = include_str!("../../../../skills/vnoc/rules.md");

/// A topic of `vnoc guide`: its name, the line the list gives it, and its page.
pub struct Topic {
    pub name: &'static str,
    pub about: &'static str,
    pub text: &'static str,
}

pub const TOPICS: [Topic; 5] = [
    Topic {
        name: "read",
        about: "what a node is and what notes it has: show, code, doc read; naming a node; --json",
        text: include_str!("../../guide/read.md"),
    },
    Topic {
        name: "tree",
        about: "the structure of a folder, file or class; what has no note yet; keeping it small",
        text: include_str!("../../guide/tree.md"),
    },
    Topic {
        name: "find",
        about: "find by name or by a word in a note; grep for callers; the query language",
        text: include_str!("../../guide/find.md"),
    },
    Topic {
        name: "write",
        about: "what a good note is; doc new, tags, doc tag; what a write prints",
        text: include_str!("../../guide/write.md"),
    },
    Topic {
        name: "check",
        about: "check, repair, relink; what each problem means; the exit codes",
        text: include_str!("../../guide/check.md"),
    },
];

/// The topics, one line each.
fn list() -> String {
    let width = TOPICS.iter().map(|t| t.name.len()).max().unwrap_or(0);
    let mut text = String::from(
        "vnoc guide <topic> prints one, with examples that were run on a sample project:\n\n",
    );
    for topic in &TOPICS {
        let pad = " ".repeat(width - topic.name.len());
        text.push_str(&format!(
            "  {}{pad}  {}\n",
            style::paint(style::bold(), topic.name),
            topic.about
        ));
    }
    text.push_str("\n`vnoc <command> --help` has every flag of a command.\n");
    text
}

/// A page with its headings (`# …`) and its command lines (`vnoc …`) painted, when a terminal is asked to paint. Every line keeps its words.
fn page(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, end) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let painted = if body.starts_with('#') {
            style::paint(style::bold(), body)
        } else if body.starts_with("vnoc ") {
            style::paint(style::subject(), body)
        } else {
            body.to_string()
        };
        out.push_str(&painted);
        out.push_str(end);
    }
    out
}

fn text_only(global: &Global, command: &str) -> Result<(), Failure> {
    if global.json || global.format.is_some() {
        return Err(Failure::Usage(format!(
            "{command} prints text; --json and --format are for the commands that answer about a project"
        )));
    }
    Ok(())
}

/// `vnoc guide [<topic>]`
pub fn guide(global: &Global, args: &GuideArgs) -> Result<(), Failure> {
    text_only(global, "guide")?;
    let Some(name) = &args.topic else {
        return out::text(&list());
    };
    match TOPICS.iter().find(|t| t.name == name.as_str()) {
        Some(topic) => out::text(&page(topic.text)),
        None => Err(Failure::Usage(format!(
            "no guide topic `{name}`; the topics are:\n{}",
            list()
                .lines()
                .filter(|l| l.starts_with("  "))
                .collect::<Vec<_>>()
                .join("\n")
        ))),
    }
}

/// `vnoc skill`: the file as it is, for a pipe or a person.
pub fn skill(global: &Global) -> Result<(), Failure> {
    text_only(global, "skill")?;
    out::text(SKILL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_keeps_every_word_when_it_is_painted() {
        let text = "# read: a title\n\nprose\nvnoc show x\nplain vnoc show\n";
        assert_eq!(page(text), text, "plain through a pipe");
        let painted = style::with_color(true, || page(text));
        assert_ne!(painted, text);
        // the escape codes are the only difference
        let mut stripped = String::new();
        let mut chars = painted.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                stripped.push(c);
            }
        }
        assert_eq!(stripped, text);
        // headings and command lines are painted, prose is not
        assert!(painted.contains("\nprose\n"));
        assert!(painted.starts_with('\x1b'));
    }

    #[test]
    fn every_topic_has_a_page_and_one_line_for_the_list() {
        for topic in &TOPICS {
            assert!(topic.text.starts_with(&format!("# {}:", topic.name)));
            assert!(!topic.about.contains('\n'));
        }
    }

    #[test]
    fn the_embedded_texts_have_no_carriage_return() {
        let texts = [SKILL, RULES]
            .into_iter()
            .chain(TOPICS.iter().map(|t| t.text));
        for text in texts {
            assert!(
                !text.contains('\r'),
                "a checkout turned these into CRLF: .gitattributes must keep them eol=lf"
            );
        }
    }
}
