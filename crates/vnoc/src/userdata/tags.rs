//! Tags (`docs/21-agent-cli/04-write-commands.md` §4, "Listing, editing and describing tags").
//!
//! A tag lives only in the frontmatter of the docs that carry it: the list of tags is **computed** from the doc texts, never kept. `.vnoc/tags.toml`
//! (optional, written by people) says what a tag means. This file reads it, counts tags and authors, rewrites one doc's `tags:` line, and changes one
//! key of `tags.toml` for `vnoc tag describe` (nothing else in V-NOC writes that file).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Value};

use super::doc_text::DocTexts;
use super::layout;
use super::read::{LoadProblem, frontmatter};

/// A tag is lower-case ASCII letters, digits and `-`, and not empty.
pub fn is_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// What each tag means, from `.vnoc/tags.toml`: a description, not a whitelist. Sorted by tag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Descriptions(BTreeMap<String, String>);

impl Descriptions {
    pub fn get(&self, tag: &str) -> Option<&str> {
        self.0.get(tag).map(String::as_str)
    }

    pub fn contains(&self, tag: &str) -> bool {
        self.0.contains_key(tag)
    }

    /// The described tags, by name.
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Reads `<root>/tags.toml` (`root` is the `.vnoc/` folder). Not there: no descriptions and no problem. A file that is not TOML is one problem and
/// nothing is read; a key that is not a valid tag, or a value that is not a string, is a problem **each** and the rest is read.
pub fn read_descriptions(root: &Path) -> (Descriptions, Vec<LoadProblem>) {
    let file = layout::tags_file(root);
    let shown = format!("{}/tags.toml", layout::FOLDER);
    let problem = |text: String| LoadProblem {
        text: format!("{shown}: {text}"),
        file: file.clone(),
    };
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (Descriptions::default(), vec![]),
        Err(e) => return (Descriptions::default(), vec![problem(e.to_string())]),
    };
    let doc: DocumentMut = match text.parse() {
        Ok(doc) => doc,
        Err(e) => {
            let e: toml_edit::TomlError = e;
            return (
                Descriptions::default(),
                vec![problem(e.message().to_string())],
            );
        }
    };
    let (mut described, mut problems) = (BTreeMap::new(), Vec::new());
    for (key, item) in doc.iter() {
        if !is_tag(key) {
            problems.push(problem(format!(
                "`{key}` is not a tag: lower-case letters, digits and `-` (skipped)"
            )));
            continue;
        }
        match item.as_str() {
            Some(description) => {
                described.insert(key.to_string(), description.trim().to_string());
            }
            None => problems.push(problem(format!(
                "the description of `{key}` is not a string (skipped)"
            ))),
        }
    }
    (Descriptions(described), problems)
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DescribeError {
    #[error("`{0}` is not a tag: lower-case letters, digits and `-`")]
    BadTag(String),
    #[error("a description is one line: it has a line break")]
    LineBreak,
    #[error("a description cannot be empty: use --clear to take it away")]
    Empty,
    #[error(".vnoc/tags.toml is not valid TOML ({0}): fix the file by hand first")]
    NotToml(String),
}

/// What `vnoc tag describe` did to the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Described {
    /// the key now holds the description
    Set,
    /// the key was there and is gone
    Cleared,
    /// `--clear` of a tag that had no description: nothing changed
    NotDescribed,
}

/// The rules of a description, checked before anything is read: a tag, and for a text one non-empty line (`None` is `--clear`, which takes any tag).
pub fn check_description(tag: &str, description: Option<&str>) -> Result<(), DescribeError> {
    if !is_tag(tag) {
        return Err(DescribeError::BadTag(tag.to_string()));
    }
    if let Some(text) = description {
        if text.contains(['\n', '\r']) {
            return Err(DescribeError::LineBreak);
        }
        if text.trim().is_empty() {
            return Err(DescribeError::Empty);
        }
    }
    Ok(())
}

/// The text of `tags.toml` (`""` for a file that is not there) with the key `tag` set to `description`, or taken out for `None` (pure). **Only that key
/// changes**: comments, order, spacing and every other key stay as they are. A new key goes after the last one; a key that is there keeps its place and
/// the comment at the end of its line.
pub fn describe_in(
    text: &str,
    tag: &str,
    description: Option<&str>,
) -> Result<(String, Described), DescribeError> {
    check_description(tag, description)?;
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| DescribeError::NotToml(e.message().to_string()))?;
    let done = match description {
        Some(description) => {
            let mut new = Value::from(description.trim());
            match doc.get_mut(tag) {
                // the key stays as it is (its comment above, its spacing), and the value keeps what trails it on the line
                Some(old) => {
                    if let Some(old) = old.as_value() {
                        *new.decor_mut() = old.decor().clone();
                    }
                    *old = Item::Value(new);
                }
                None => {
                    doc.insert(tag, Item::Value(new));
                }
            }
            Described::Set
        }
        None if doc.remove(tag).is_some() => Described::Cleared,
        None => Described::NotDescribed,
    };
    Ok((doc.to_string(), done))
}

/// One tag, how many docs carry it, and what it means when `tags.toml` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub docs: usize,
    pub description: Option<String>,
}

/// Every tag some doc carries, with its count (a doc counts once for a tag), and every described tag no doc carries, with `0`. Most docs first, then by
/// name. `except`: a doc path to leave out of the counts (the doc being written, when asking whether a tag is new).
pub fn tag_counts(
    texts: &DocTexts,
    descriptions: &Descriptions,
    except: Option<&Path>,
) -> Vec<TagCount> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, _, doc) in texts.iter() {
        if except.is_some_and(|path| path == doc.path) {
            continue;
        }
        let mut seen: Vec<&String> = Vec::new();
        for tag in &doc.tags {
            if !seen.contains(&tag) {
                seen.push(tag);
                *counts.entry(tag.clone()).or_default() += 1;
            }
        }
    }
    for tag in descriptions.tags() {
        counts.entry(tag.to_string()).or_default();
    }
    let mut rows: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, docs)| TagCount {
            description: descriptions.get(&tag).map(str::to_string),
            tag,
            docs,
        })
        .collect();
    rows.sort_by(|a, b| b.docs.cmp(&a.docs).then_with(|| a.tag.cmp(&b.tag)));
    rows
}

/// One author and how many docs they wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorCount {
    pub author: String,
    pub docs: usize,
}

/// Every author some doc names, with its count. Two spellings that differ only in case are one author (a filter ignores case too), shown as the first
/// one met. Most docs first, then by name.
pub fn author_counts(texts: &DocTexts) -> Vec<AuthorCount> {
    let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (_, _, doc) in texts.iter() {
        if let Some(author) = &doc.author {
            let key: String = author.chars().flat_map(char::to_lowercase).collect();
            counts.entry(key).or_insert_with(|| (author.clone(), 0)).1 += 1;
        }
    }
    let mut rows: Vec<AuthorCount> = counts
        .into_values()
        .map(|(author, docs)| AuthorCount { author, docs })
        .collect();
    rows.sort_by(|a, b| b.docs.cmp(&a.docs).then_with(|| a.author.cmp(&b.author)));
    rows
}

/// What a rewrite of one doc's `tags:` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// the doc's tags after, in order
    pub tags: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// asked to add, and the doc had it
    pub already_has: Vec<String>,
    /// asked to remove, and the doc did not have it
    pub already_without: Vec<String>,
}

impl Change {
    /// Whether the doc's text is different after.
    pub fn changes_text(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TagError {
    #[error("`{0}` is not a tag: lower-case letters, digits and `-`")]
    BadTag(String),
    #[error("this doc has no frontmatter (the lines between two `---` at its top) to put tags in")]
    NoFrontmatter,
}

/// `text` with the tags of its frontmatter changed: **only the `tags:` line** (pure; every other byte stays).
///
/// * The doc's tags as they are (read as `frontmatter` reads them, so lower case), the `remove`d ones taken out, the `add`ed ones appended in the order
///   given, none twice. A tag to add that the doc has, or one to remove that it has not, changes nothing and is reported.
/// * A `tags:` written as a block list (`tags:` then `- plan` lines) becomes the one line `tags: [a, b]`.
/// * When the last tag goes, the line goes with it. A doc with no `tags:` line gets one after its `author:` line, else after `hint:`, else at the end
///   of the frontmatter.
/// * The line ends of the doc (`\n` or `\r\n`) are kept: the new line ends as the one it replaces, or the one it follows.
///
/// A tag to add must be a tag. A tag to remove must be one too, unless the doc has it (a hand-written one that is not).
pub fn rewrite_tags(
    text: &str,
    add: &[String],
    remove: &[String],
) -> Result<(String, Change), TagError> {
    let current = frontmatter(text).tags;
    if let Some(bad) = add.iter().find(|t| !is_tag(t)) {
        return Err(TagError::BadTag(bad.clone()));
    }
    if let Some(bad) = remove.iter().find(|t| !is_tag(t) && !current.contains(t)) {
        return Err(TagError::BadTag(bad.clone()));
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let bare = |line: &str| line.trim_end_matches(['\r', '\n']).to_string();
    if lines.first().map(|l| bare(l)) != Some("---".to_string()) {
        return Err(TagError::NoFrontmatter);
    }
    // the closing `---` must end with a line break, as the reader needs
    let close = (1..lines.len())
        .find(|&i| bare(lines[i]) == "---" && lines[i].ends_with('\n'))
        .ok_or(TagError::NoFrontmatter)?;

    let mut change = Change::default();
    let mut next = current.clone();
    for tag in remove {
        if let Some(at) = next.iter().position(|t| t == tag) {
            next.remove(at);
            change.removed.push(tag.clone());
        } else if !change.already_without.contains(tag) {
            change.already_without.push(tag.clone());
        }
    }
    for tag in add {
        if next.contains(tag) {
            if !change.already_has.contains(tag) {
                change.already_has.push(tag.clone());
            }
        } else {
            next.push(tag.clone());
            change.added.push(tag.clone());
        }
    }
    change.tags = next.clone();
    if !change.changes_text() {
        return Ok((text.to_string(), change));
    }

    // the key lines of the frontmatter, at the top level: the first of each wins, as the reader has it
    let key_at = |name: &str| {
        (1..close).find(|&i| {
            bare(lines[i])
                .split_once(':')
                .is_some_and(|(key, _)| key == name)
        })
    };
    let eol = |line: &str| if line.ends_with("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len() + 32);
    let rendered = |eol: &str| format!("tags: [{}]{eol}", next.join(", "));
    match key_at("tags") {
        Some(at) => {
            // the lines the key owns: itself, and when its value is empty the `- x` lines after it (blank lines between them go with them)
            let value = bare(lines[at])
                .split_once(':')
                .map(|(_, v)| v.trim().to_string());
            let mut end = at + 1;
            if value.as_deref() == Some("") {
                let mut scan = at + 1;
                while scan < close {
                    let trimmed = bare(lines[scan]).trim().to_string();
                    if trimmed.is_empty() {
                        scan += 1;
                    } else if trimmed.starts_with('-') {
                        scan += 1;
                        end = scan;
                    } else {
                        break;
                    }
                }
            }
            lines[..at].iter().for_each(|l| out.push_str(l));
            if !next.is_empty() {
                out.push_str(&rendered(eol(lines[at])));
            }
            lines[end..].iter().for_each(|l| out.push_str(l));
        }
        None => {
            // no `tags:` line: one goes after `author:`, else `hint:`, else at the end of the frontmatter
            let after = key_at("author")
                .or_else(|| key_at("hint"))
                .unwrap_or(close - 1);
            lines[..=after].iter().for_each(|l| out.push_str(l));
            if !lines[after].ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&rendered(eol(lines[after])));
            lines[after + 1..].iter().for_each(|l| out.push_str(l));
        }
    }
    Ok((out, change))
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("the file changed while the tags were being written")]
    Changed,
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Writes `new` over the file at `path` only if the file still holds exactly `was` (the text the rewrite was made from): read again, compared,
/// and written atomically. A file that changed in between is left as the other writer left it.
pub fn write_if_unchanged(path: &Path, was: &str, new: &str) -> Result<(), WriteError> {
    if fs::read(path)? != was.as_bytes() {
        return Err(WriteError::Changed);
    }
    super::atomic::write_atomic(path, new.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| t.to_string()).collect()
    }

    /// A doc with this frontmatter between the `---` lines.
    fn doc(frontmatter: &str) -> String {
        format!(
            "---\nnode: 4KF0\nhint: a.py::f\n{frontmatter}---\n# Title\n\ntags: [not, here]\n- a body line\n"
        )
    }

    fn rewritten(text: &str, add: &[&str], remove: &[&str]) -> (String, Change) {
        rewrite_tags(text, &tags(add), &tags(remove)).unwrap()
    }

    #[test]
    fn the_table_of_04_section_4() {
        // (frontmatter before, add, remove, frontmatter after)
        let table: Vec<(&str, Vec<&str>, Vec<&str>, &str)> = vec![
            // add and remove at once: removed taken out, added appended
            (
                "tags: [plan, billing]\n",
                vec!["done"],
                vec!["plan"],
                "tags: [billing, done]\n",
            ),
            // added in the order given
            ("tags: [a]\n", vec!["c", "b"], vec![], "tags: [a, c, b]\n"),
            // an add that the doc has is not added twice, a remove that it does not have changes nothing
            ("tags: [a]\n", vec!["a", "b"], vec!["z"], "tags: [a, b]\n"),
            // the last tag removed: the line goes
            ("tags: [a]\n", vec![], vec!["a"], ""),
            ("tags: [a, b]\n", vec![], vec!["a", "b"], ""),
            // removed and added in one go: removals first
            ("tags: [a]\n", vec!["a"], vec!["a"], "tags: [a]\n"),
            // no tags line: after author:, else after hint:
            (
                "author: claude-code\n",
                vec!["plan"],
                vec![],
                "author: claude-code\ntags: [plan]\n",
            ),
            ("", vec!["plan"], vec![], "tags: [plan]\n"),
            // a block list is folded into one line
            (
                "tags:\n  - plan\n  - Billing\n",
                vec!["done"],
                vec!["plan"],
                "tags: [billing, done]\n",
            ),
            (
                "tags:\n- plan\n\n- x\nauthor: z\n",
                vec!["y"],
                vec![],
                "tags: [plan, x, y]\nauthor: z\n",
            ),
            // the last of a block list removed: all of its lines go
            (
                "tags:\n  - plan\nauthor: z\n",
                vec![],
                vec!["plan"],
                "author: z\n",
            ),
            // `tags: plan` is one tag; quotes and capitals as the reader has them
            ("tags: plan\n", vec!["done"], vec![], "tags: [plan, done]\n"),
            (
                "tags: [\"Plan\", 'x']\n",
                vec!["y"],
                vec![],
                "tags: [plan, x, y]\n",
            ),
            // a key between the others stays where it is
            (
                "author: a\ntags: [x]\nzzz: 1\n",
                vec!["y"],
                vec![],
                "author: a\ntags: [x, y]\nzzz: 1\n",
            ),
        ];
        for (before, add, remove, after) in table {
            let text = doc(before);
            let (got, _) = rewritten(&text, &add, &remove);
            assert_eq!(got, doc(after), "{before:?} +{add:?} -{remove:?}");
            // every other line is byte for byte as it was: the body has a `tags:` line of its own, and it is not touched
            assert!(got.ends_with("# Title\n\ntags: [not, here]\n- a body line\n"));
        }
    }

    #[test]
    fn what_a_change_says() {
        let text = doc("tags: [plan, billing]\n");
        let (_, change) = rewritten(
            &text,
            &["done", "plan", "plan"],
            &["billing", "nope", "nope"],
        );
        assert_eq!(
            change,
            Change {
                tags: tags(&["plan", "done"]),
                added: tags(&["done"]),
                removed: tags(&["billing"]),
                already_has: tags(&["plan"]),
                already_without: tags(&["nope"]),
            }
        );
        assert!(change.changes_text());
        // nothing to change: the text comes back as it was, formatting and all
        let odd = doc("tags:\n  - plan\n\n  - billing\n");
        let (same, change) = rewritten(&odd, &["plan"], &["nope"]);
        assert_eq!(same, odd);
        assert!(!change.changes_text());
        assert_eq!(change.already_has, tags(&["plan"]));
        assert_eq!(change.already_without, tags(&["nope"]));
    }

    #[test]
    fn crlf_and_a_missing_last_line_break_are_kept() {
        let crlf = "---\r\nnode: X\r\nhint: a::b\r\ntags: [a]\r\n---\r\n# T\r\n";
        let (got, _) = rewritten(crlf, &["b"], &[]);
        assert_eq!(
            got,
            "---\r\nnode: X\r\nhint: a::b\r\ntags: [a, b]\r\n---\r\n# T\r\n"
        );
        let (got, _) = rewritten(
            "---\r\nnode: X\r\nhint: a::b\r\n---\r\n# T\r\n",
            &["b"],
            &[],
        );
        assert_eq!(
            got,
            "---\r\nnode: X\r\nhint: a::b\r\ntags: [b]\r\n---\r\n# T\r\n"
        );
        // no author, no hint: the end of the frontmatter
        let (got, _) = rewritten("---\nnode: X\n---\n# T\n", &["b"], &[]);
        assert_eq!(got, "---\nnode: X\ntags: [b]\n---\n# T\n");
    }

    #[test]
    fn a_bad_tag_is_refused_and_a_doc_with_no_frontmatter_has_nowhere_to_put_tags() {
        let text = doc("tags: [a]\n");
        for bad in ["Plan", "two words", "", "a_b", "caf\u{e9}", "a,b"] {
            assert_eq!(
                rewrite_tags(&text, &tags(&[bad]), &[]).unwrap_err(),
                TagError::BadTag(bad.to_string()),
                "add {bad:?}"
            );
            assert_eq!(
                rewrite_tags(&text, &[], &tags(&[bad])).unwrap_err(),
                TagError::BadTag(bad.to_string()),
                "remove {bad:?}"
            );
        }
        // a hand-written tag that is not valid can still be taken off, by the name the reader gives it
        let odd = doc("tags: [caf\u{e9}, a]\n");
        let (got, _) = rewritten(&odd, &[], &["caf\u{e9}"]);
        assert_eq!(got, doc("tags: [a]\n"));
        for text in [
            "# only a title\n",
            "---\nnode: X\n# never closed\n",
            "---\nnode: X\n---",
        ] {
            assert_eq!(
                rewrite_tags(text, &tags(&["a"]), &[]).unwrap_err(),
                TagError::NoFrontmatter,
                "{text:?}"
            );
        }
    }

    fn fresh(label: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vnoc-tags-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        crate::testdir::remove_at_end(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tags_toml_one_bad_key_and_one_good_and_the_rest_is_read() {
        let root = fresh("toml");
        assert_eq!(read_descriptions(&root), (Descriptions::default(), vec![]));
        fs::write(
            root.join("tags.toml"),
            "# what a tag means\nplan = \"  A change we intend to make.  \"\nBad_Key = \"x\"\nnumber = 3\ndone = \"A plan that shipped.\"\n[table]\nx = \"y\"\n",
        )
        .unwrap();
        let (described, problems) = read_descriptions(&root);
        assert_eq!(
            described.tags().collect::<Vec<_>>(),
            ["done", "plan"],
            "the good ones are read, trimmed"
        );
        assert_eq!(described.get("plan"), Some("A change we intend to make."));
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert_eq!(
            problems[0].text,
            ".vnoc/tags.toml: `Bad_Key` is not a tag: lower-case letters, digits and `-` (skipped)"
        );
        assert_eq!(
            problems[1].text,
            ".vnoc/tags.toml: the description of `number` is not a string (skipped)"
        );
        assert!(problems[2].text.contains("`table`"), "{problems:?}");
        assert!(problems.iter().all(|p| p.file == root.join("tags.toml")));
        // a file that is not TOML is one problem and nothing is read
        fs::write(root.join("tags.toml"), "plan = [oops\n").unwrap();
        let (described, problems) = read_descriptions(&root);
        assert!(described.is_empty());
        assert_eq!(problems.len(), 1);
        assert!(problems[0].text.starts_with(".vnoc/tags.toml: "));
    }

    #[test]
    fn a_write_is_refused_when_the_file_changed_after_it_was_read() {
        let root = fresh("write");
        let file = root.join("a.md");
        let was = doc("tags: [a]\n");
        fs::write(&file, &was).unwrap();
        let (new, _) = rewritten(&was, &["b"], &[]);
        // another writer got there first
        fs::write(&file, doc("tags: [a, z]\n")).unwrap();
        assert!(matches!(
            write_if_unchanged(&file, &was, &new),
            Err(WriteError::Changed)
        ));
        assert_eq!(fs::read_to_string(&file).unwrap(), doc("tags: [a, z]\n"));
        // as it was read: written, atomically (no temp file left)
        fs::write(&file, &was).unwrap();
        write_if_unchanged(&file, &was, &new).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), new);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn describe_in_changes_one_key_and_nothing_else() {
        let text = "# header\nplan = \"old\"  # why\n\n# about done\ndone = 'A'\n";
        let (new, done) = describe_in(text, "plan", Some(" new ")).unwrap();
        assert_eq!(done, Described::Set);
        assert_eq!(
            new,
            "# header\nplan = \"new\"  # why\n\n# about done\ndone = 'A'\n"
        );
        let (new, _) = describe_in(text, "fresh-1", Some("x")).unwrap();
        assert_eq!(new, format!("{text}fresh-1 = \"x\"\n"));
        let (new, done) = describe_in(text, "done", None).unwrap();
        assert_eq!(done, Described::Cleared);
        assert!(new.starts_with("# header\nplan = \"old\"  # why\n"));
        assert!(!new.contains("done"));
        assert_eq!(
            describe_in(text, "other", None).unwrap(),
            (text.to_string(), Described::NotDescribed)
        );
        assert_eq!(
            describe_in("", "plan", Some("a")).unwrap().0,
            "plan = \"a\"\n"
        );
    }

    #[test]
    fn describe_in_refuses_what_the_rules_forbid_and_a_file_that_is_not_toml() {
        let refused = |tag, text| describe_in("", tag, text).unwrap_err();
        assert_eq!(
            refused("Plan", Some("x")),
            DescribeError::BadTag("Plan".into())
        );
        assert_eq!(refused("plan", Some("a\nb")), DescribeError::LineBreak);
        assert_eq!(refused("plan", Some("a\rb")), DescribeError::LineBreak);
        assert_eq!(refused("plan", Some("  ")), DescribeError::Empty);
        assert!(matches!(
            describe_in("plan = [oops", "plan", Some("x")),
            Err(DescribeError::NotToml(_))
        ));
    }
}
