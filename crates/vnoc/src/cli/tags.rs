//! `vnoc tags`, `vnoc doc tag` and `vnoc tag describe` (`docs/21-agent-cli/04-write-commands.md` §4, "Listing, editing and describing tags"), and the
//! notice `doc new` and `doc tag` give for a tag nobody uses yet.
//!
//! The list of tags is **computed** from the docs every time: the doc texts carry each doc's tags and authors, so listing is a count over them, and it needs
//! no driver. `.vnoc/tags.toml` says what a tag means; people write it, by hand or with `vnoc tag describe`, and nothing else writes it.

use std::path::Path;

use serde::Serialize;

use super::args::{DocTagArgs, TagCommand, TagDescribeArgs, TagsArgs};
use super::doc::{choose_doc, node_docs};
use super::out::{self, Render};
use super::project::open_without_driver;
use super::relink::refused;
use super::write::{NO_VNOC, label, settings_failure, take_lock};
use super::{Failure, Global, style};
use crate::project::Project;
use crate::userdata::{
    DescribeTagError, Described, Descriptions, DocTexts, EditTagsError, TagCount, UserData,
    WriteError, author_counts, check_description, is_tag, tag_counts,
};

/// The doc texts of the whole project, read from `.vnoc/` alone (no index, no driver), and what `tags.toml` says.
fn read_project(project: &Project) -> (DocTexts, Descriptions) {
    let data = UserData::new(project.git_root());
    let records = data.load_all().records;
    let (texts, _) = data.refresh_doc_texts(&DocTexts::default(), &records);
    (texts, data.tag_descriptions())
}

// --------------------------------------------------------------------------------------------------------------------------------- tags

#[derive(Serialize)]
struct TagRow {
    tag: String,
    docs: usize,
    description: Option<String>,
}

#[derive(Serialize)]
struct AuthorRow {
    author: String,
    docs: usize,
}

#[derive(Serialize)]
struct Tags {
    schema: u32,
    command: &'static str,
    tags: Vec<TagRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authors: Option<Vec<AuthorRow>>,
    #[serde(skip)]
    authors_asked: bool,
}

fn docs_word(n: usize) -> String {
    format!("{n} doc{}", if n == 1 { "" } else { "s" })
}

impl Render for Tags {
    fn text(&self, quiet: bool) -> String {
        // the names, the counts and the descriptions, each in a column
        let rows: Vec<(String, usize, Option<&str>)> = if self.authors_asked {
            self.authors
                .iter()
                .flatten()
                .map(|r| (r.author.clone(), r.docs, None))
                .collect()
        } else {
            self.tags
                .iter()
                .map(|r| (r.tag.clone(), r.docs, r.description.as_deref()))
                .collect()
        };
        if rows.is_empty() {
            return format!(
                "{}\n",
                if self.authors_asked {
                    "no authors"
                } else {
                    "no tags"
                }
            );
        }
        let name_width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
        let count_width = rows.iter().map(|r| docs_word(r.1).len()).max().unwrap_or(0);
        let mut text = String::new();
        for (name, docs, description) in rows {
            let pad = " ".repeat(name_width - name.chars().count());
            text.push_str(&format!("{}{pad}  ", style::paint(style::title(), &name)));
            match description.filter(|d| !d.is_empty()) {
                // the count's column is padded only when something follows it: no trailing white space
                Some(description) => text.push_str(&format!(
                    "{:<count_width$}  {}",
                    docs_word(docs),
                    style::paint(style::dim(), description)
                )),
                None => text.push_str(&docs_word(docs)),
            }
            text.push('\n');
        }
        // nothing is described yet: say how, once (a hint is not an answer, so `--quiet` drops it)
        if !quiet
            && !self.authors_asked
            && self
                .tags
                .iter()
                .all(|r| r.description.as_deref().is_none_or(str::is_empty))
        {
            text.push_str(&format!(
                "{}\n",
                style::paint(
                    style::dim(),
                    "→ describe one: vnoc tag describe <tag> \"<what it means>\""
                )
            ));
        }
        text
    }
}

/// `vnoc tags [--authors]`
pub async fn tags(global: &Global, args: &TagsArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let (texts, descriptions) = read_project(&project);
    let tags = tag_counts(&texts, &descriptions, None)
        .into_iter()
        .map(
            |TagCount {
                 tag,
                 docs,
                 description,
             }| TagRow {
                tag,
                docs,
                description,
            },
        )
        .collect();
    let authors = args.authors.then(|| {
        author_counts(&texts)
            .into_iter()
            .map(|a| AuthorRow {
                author: a.author,
                docs: a.docs,
            })
            .collect()
    });
    out::render(
        &Tags {
            schema: 1,
            command: "tags",
            tags,
            authors,
            authors_asked: args.authors,
        },
        global,
    )
}

// ------------------------------------------------------------------------------------------------------------------------------ doc tag

#[derive(Serialize)]
struct Unchanged {
    tag: String,
    /// `has` (asked to add, and the doc has it) or `without` (asked to remove, and the doc has not)
    state: &'static str,
}

#[derive(Serialize)]
struct TagEdit {
    schema: u32,
    command: &'static str,
    doc: String,
    tags: Vec<String>,
    added: Vec<String>,
    removed: Vec<String>,
    unchanged: Vec<Unchanged>,
    /// the new-tag notices (one line each, joined by a line break), or null
    notice: Option<String>,
    #[serde(skip)]
    notices: Vec<String>,
}

impl Render for TagEdit {
    fn text(&self, _quiet: bool) -> String {
        // the tags are the fact; the doc's path, which is long, is a row of its own after them (03 §11)
        let mut text = format!(
            "{}{}\n{}{}\n",
            label("tagged"),
            style::paint(style::dim(), format!("[{}]", self.tags.join(", "))),
            label("in"),
            self.doc
        );
        for tag in &self.unchanged {
            text.push_str(&format!("already {} \"{}\"\n", tag.state, tag.tag));
        }
        for notice in &self.notices {
            text.push_str(&format!("{notice}\n"));
        }
        text
    }
}

/// What an `EditTagsError` is to a command: a request that cannot be met is `2`; a file that changed under the command is the environment's (`3`).
fn edit_failure(e: EditTagsError) -> Failure {
    match e {
        EditTagsError::Write(WriteError::Changed | WriteError::Io(_)) => {
            Failure::Environment(e.to_string())
        }
        other => Failure::Usage(other.to_string()),
    }
}

/// `vnoc doc tag <node> [file] [--add t]… [--remove t]…`
pub async fn tag(global: &Global, args: &DocTagArgs) -> Result<(), Failure> {
    if args.add.is_empty() && args.remove.is_empty() {
        return Err(Failure::Usage(
            "give at least one --add <tag> or --remove <tag>".into(),
        ));
    }
    // a tag to add is a tag, before anything is looked up
    if let Some(bad) = args.add.iter().find(|t| !is_tag(t)) {
        return Err(Failure::Usage(
            crate::userdata::TagError::BadTag(bad.clone()).to_string(),
        ));
    }
    let (project, found) = node_docs(global, &args.node).await?;
    let (name, docs) = match found {
        Ok(found) => found,
        Err(error) => return refused(global, "doc tag", error),
    };
    let doc = match choose_doc(&name, docs, args.file.as_deref()) {
        Ok(doc) => doc,
        Err(error) => return refused(global, "doc tag", error),
    };

    let _one_writer = take_lock(&project).await?;
    let data = UserData::new(project.git_root());
    let change = data
        .edit_doc_tags(&doc.abs, &args.add, &args.remove)
        .map_err(edit_failure)?;
    let notices = new_tag_notices(&project, &doc.abs, &change.added);
    let mut unchanged: Vec<Unchanged> = change
        .already_has
        .iter()
        .map(|tag| Unchanged {
            tag: tag.clone(),
            state: "has",
        })
        .collect();
    unchanged.extend(change.already_without.iter().map(|tag| Unchanged {
        tag: tag.clone(),
        state: "without",
    }));
    out::render(
        &TagEdit {
            schema: 1,
            command: "doc tag",
            doc: doc.path,
            tags: change.tags,
            added: change.added,
            removed: change.removed,
            unchanged,
            notice: (!notices.is_empty()).then(|| notices.join("\n")),
            notices,
        },
        global,
    )
}

// -------------------------------------------------------------------------------------------------------------------------- tag describe

const TAGS_FILE: &str = ".vnoc/tags.toml";

#[derive(Serialize)]
struct TagDescribed {
    schema: u32,
    command: &'static str,
    tag: String,
    /// the description now, null after `--clear`
    description: Option<String>,
    file: &'static str,
    #[serde(skip)]
    done: Described,
}

impl Render for TagDescribed {
    fn text(&self, _quiet: bool) -> String {
        let tag = style::paint(style::title(), &self.tag);
        match (self.done, &self.description) {
            (Described::Set, Some(text)) => format!(
                "{}{tag}  \"{text}\"\n{}{TAGS_FILE}\n",
                label("described"),
                label("in")
            ),
            (Described::Cleared, _) => format!("{}{tag}\n", label("cleared")),
            _ => format!("{}  {tag}\n", label("not described")),
        }
    }
}

/// What a `DescribeTagError` is to a command: a request that breaks a rule is `2`; a file that is not TOML, or cannot be written, is the environment's (`3`).
fn describe_failure(e: DescribeTagError) -> Failure {
    match e {
        DescribeTagError::Describe(crate::userdata::DescribeError::NotToml(_))
        | DescribeTagError::Write(_) => Failure::Environment(e.to_string()),
        DescribeTagError::Describe(_) => Failure::Usage(e.to_string()),
    }
}

/// `vnoc tag …`
pub async fn tag_command(global: &Global, command: &TagCommand) -> Result<(), Failure> {
    match command {
        TagCommand::Describe(args) => describe(global, args).await,
    }
}

/// `vnoc tag describe <tag> "<text>"` and `vnoc tag describe <tag> --clear`
async fn describe(global: &Global, args: &TagDescribeArgs) -> Result<(), Failure> {
    let text = args.text.as_deref().filter(|_| !args.clear);
    // the rules of the request, before the project is looked at
    check_description(&args.tag, text).map_err(|e| Failure::Usage(e.to_string()))?;
    let project = open_without_driver(global)?;
    let data = UserData::new(project.git_root());
    if !data.settings().map_err(settings_failure)?.initialized {
        return Err(Failure::Usage(NO_VNOC.into()));
    }
    let _one_writer = take_lock(&project).await?;
    let done = data
        .describe_tag(&args.tag, text)
        .map_err(describe_failure)?;
    out::render(
        &TagDescribed {
            schema: 1,
            command: "tag describe",
            tag: args.tag.clone(),
            description: text.map(|t| t.trim().to_string()),
            file: TAGS_FILE,
            done,
        },
        global,
    )
}

// ------------------------------------------------------------------------------------------------------------------------- the new-tag notice

/// For each of `added` that no OTHER doc carries and `tags.toml` does not describe: `note: "<t>" is a new tag in this project (described: …); did you
/// mean "<nearest>"?`. The doc at `doc` is left out of the counts (it carries the tag now). The parenthesis is left out when `tags.toml` describes
/// nothing, the question when nothing known is near (`nearest_word`: the closest, a described tag winning a tie). A tag nobody uses yet is accepted
/// either way: this is a note, never a refusal.
pub(super) fn new_tag_notices(project: &Project, doc: &Path, added: &[String]) -> Vec<String> {
    if added.is_empty() {
        return Vec::new();
    }
    let data = UserData::new(project.git_root());
    let records = data.load_all().records;
    let (texts, _) = data.refresh_doc_texts(&DocTexts::default(), &records);
    let descriptions = data.tag_descriptions();
    let others = tag_counts(&texts, &descriptions, Some(doc));
    // the known tags: the described ones first, then the used ones, most used first
    let described: Vec<&str> = descriptions.tags().collect();
    let mut known: Vec<&str> = described.clone();
    known.extend(
        others
            .iter()
            .filter(|row| row.docs > 0 && !descriptions.contains(&row.tag))
            .map(|row| row.tag.as_str()),
    );
    added
        .iter()
        .filter(|tag| !known.contains(&tag.as_str()))
        .map(|tag| {
            let mut note = format!("note: \"{tag}\" is a new tag in this project");
            if !described.is_empty() {
                note.push_str(&format!(" (described: {})", described.join(", ")));
            }
            if let Some(near) = vnoc_search::nearest_word(&known, tag) {
                note.push_str(&format!("; did you mean \"{near}\"?"));
            }
            note
        })
        .collect()
}
