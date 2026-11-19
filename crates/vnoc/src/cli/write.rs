//! The commands that write (`docs/21-agent-cli/04-write-commands.md`): `vnoc init` and `vnoc doc new`.
//!
//! `doc new` is the extension's attach (`identity::attach`) with the one thing that differs passed in: the disk applies the edit that puts an `ID:`
//! line into the code (`EditSink::Disk`), because no editor is on the other end. It takes the project's write lock for the whole flow (A7), so a
//! command and a running VS Code, or two commands, never write at the same moment.

use std::io::Read;

use serde::Serialize;
use serde_json::json;

use super::args::{DocNewArgs, InitArgs, Switch};
use super::nodes::{Lookup, NodeError, Target, doc_path, kind_word};
use super::out::{self, Render};
use super::project::{open_from, open_without_driver};
use super::show::head_line;
use super::{Failure, Global, style};
use crate::identity::folder_marker;
use crate::identity::{
    AttachError, AttachOptions, EditSink, Env, MAX_BODY_BYTES, MarkerWritten, attach, check_note,
};
use crate::index::Kind;
use crate::project::Project;
use crate::protocol::AttachParams;
use crate::userdata::{UserData, UserDataError, WriteLock};

/// 04 §3: what a command that needs `.vnoc/` says when the project has none. The user decides to start V-NOC, not the agent.
const NO_VNOC: &str = "this project has no .vnoc/ yet. Starting it adds ID: lines to code when a note is attached. Ask the user, then: vnoc init";

/// `.vnoc/vnoc.toml` that cannot be read, or is from a newer V-NOC: the environment's problem, not the request's.
fn settings_failure(e: UserDataError) -> Failure {
    Failure::Environment(e.to_string())
}

/// A label of the answer's lines, padded to one column.
fn label(word: &str) -> String {
    style::paint(style::dim(), format!("{word:<13}"))
}

// -------------------------------------------------------------------------------------------------------------------------------- init

#[derive(Serialize)]
struct Init {
    schema: u32,
    command: &'static str,
    /// the files this run made; none when the project was started already
    created: Vec<String>,
    /// `.vnoc/` was there: nothing was changed
    already: bool,
    /// `[ids] default` is not `none`: attaching writes `ID:` lines into the code
    markers: bool,
    #[serde(skip)]
    config: String,
}

impl Render for Init {
    fn text(&self, quiet: bool) -> String {
        let ids = if self.markers {
            "attaching a note writes an ID: line into the code"
        } else {
            "no ID: lines are written; notes are attached by name"
        };
        if self.already {
            return format!("{} already set up: {} ({ids})\n", ".vnoc/", self.config);
        }
        let mut text = String::new();
        for (at, path) in self.created.iter().enumerate() {
            let note = if !quiet && at + 1 == self.created.len() {
                format!("   {}", style::paint(style::dim(), format!("({ids})")))
            } else {
                String::new()
            };
            text.push_str(&format!("{}{path}{note}\n", label("created")));
        }
        text
    }
}

/// `vnoc init [--markers on|off]`: creates `.vnoc/`. It needs no index and so no driver.
pub async fn init(global: &Global, args: &InitArgs) -> Result<(), Failure> {
    let project = open_without_driver(global)?;
    let data = UserData::new(project.git_root());
    let settings = data.settings().map_err(settings_failure)?;
    let [gitignore, config] = data.init_files();
    let created = if settings.initialized {
        vec![]
    } else {
        data.initialize(args.markers == Switch::On)
            .map_err(settings_failure)?;
        vec![doc_path(&project, &gitignore), doc_path(&project, &config)]
    };
    // what the project says now: the first start decides, a later `--markers` is not a change
    let markers = data.settings().map_err(settings_failure)?.markers;
    out::render(
        &Init {
            schema: 1,
            command: "init",
            already: created.is_empty(),
            created,
            markers,
            config: doc_path(&project, &config),
        },
        global,
    )
}

// ---------------------------------------------------------------------------------------------------------------------------- doc new

#[derive(Serialize)]
struct NodeSummary {
    kind: &'static str,
    location: String,
    node_id: String,
}

#[derive(Serialize)]
struct DocNew {
    schema: u32,
    command: &'static str,
    node: NodeSummary,
    /// the new doc's path
    doc: String,
    /// every file this created: the doc, and a folder's `.vnoc-id`
    created: Vec<String>,
    /// every file of the code this changed, and only those: an `ID:` line was added to it
    changed: Vec<String>,
    id_in_code: bool,
    notice: Option<String>,
    #[serde(skip)]
    text: Lines,
}

/// What the text of the answer says that the JSON says in fields.
struct Lines {
    head: String,
    /// the doc has its title only: the agent writes the note into it
    empty: bool,
    /// the file of the code that was changed, and what was added to it (04 §8)
    marker: Option<(String, String)>,
    /// a folder's `.vnoc-id`
    folder_marker: Option<String>,
}

impl Render for DocNew {
    fn text(&self, quiet: bool) -> String {
        let mut text = format!("{}{}\n", label("attached to"), self.text.head);
        let arrow = if self.text.empty && !quiet {
            format!("   {}", style::paint(style::dim(), "← write the note here"))
        } else {
            String::new()
        };
        text.push_str(&format!("{}{}{arrow}\n", label("created"), self.doc));
        if let Some(folder_marker) = &self.text.folder_marker {
            text.push_str(&format!("{}{folder_marker}\n", label("created")));
        }
        if let Some((file, what)) = &self.text.marker {
            text.push_str(&format!(
                "{}{file}   {}\n",
                label("changed"),
                style::paint(
                    style::dim(),
                    format!("({what}; read the file again before you edit it)")
                )
            ));
        }
        if let Some(notice) = &self.notice {
            text.push_str(&format!("{}{notice}\n", label("notice")));
        }
        text
    }
}

/// The body to attach: the file, or standard input for `-`, as UTF-8, and no more than the library accepts (reading stops one byte past it).
fn read_body(source: Option<&str>) -> Result<String, Failure> {
    let Some(source) = source else {
        return Ok(String::new());
    };
    let limit = MAX_BODY_BYTES as u64 + 1;
    let mut bytes = Vec::new();
    let read = if source == "-" {
        std::io::stdin().lock().take(limit).read_to_end(&mut bytes)
    } else {
        std::fs::File::open(source).and_then(|file| file.take(limit).read_to_end(&mut bytes))
    };
    read.map_err(|e| Failure::Usage(format!("could not read the body from {source}: {e}")))?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(Failure::Usage(AttachError::BodyTooLarge.to_string()));
    }
    String::from_utf8(bytes)
        .map_err(|_| Failure::Usage(format!("the body in {source} is not UTF-8 text")))
}

/// The write lock, waited for off the runtime's thread (it sleeps between tries). `None`: no `.vnoc/` yet, which `doc new` has already refused.
async fn take_lock(project: &Project) -> Result<Option<WriteLock>, Failure> {
    let data = UserData::new(project.git_root());
    tokio::task::spawn_blocking(move || data.write_lock())
        .await
        .map_err(|e| Failure::Environment(e.to_string()))?
        .map_err(|e| Failure::Environment(e.to_string()))
}

/// An argument that cannot be attached to: the sentence, or with `--json` a value on standard output. Exit `2`, as `calls` does.
fn refused(global: &Global, error: NodeError) -> Result<(), Failure> {
    if global.json {
        out::json(&json!({ "schema": 1, "command": "doc new", "error": error.json() }))?;
    } else {
        error.print();
    }
    Err(Failure::Reported)
}

/// What an `AttachError` is to a command: a rule broken by the request is exit `2`, anything else is the environment's (`3`).
fn attach_failure(e: AttachError, project: &Project) -> Failure {
    match e {
        AttachError::TitleTaken { title, path } => Failure::Usage(format!(
            "this node already has \"{title}\" → {} (edit that file, or --again)",
            doc_path(project, &path)
        )),
        AttachError::BadTitle
        | AttachError::BodyTooLarge
        | AttachError::BodyHasFrontmatter
        | AttachError::BadTag(_)
        | AttachError::BadAuthor
        | AttachError::BadKey(_) => Failure::Usage(e.to_string()),
        AttachError::NotInIndex(_) => Failure::NoSuchNode(e.to_string()),
        _ => Failure::Environment(e.to_string()),
    }
}

/// `vnoc doc new <node> --title "…" [--body-file <file>|-] [--no-marker] [--again] [--author <name>] [--tag <t>]…`
pub async fn doc_new(global: &Global, args: &DocNewArgs) -> Result<(), Failure> {
    // what needs no driver, and nothing is read from standard input before the project says it is started
    let project = open_without_driver(global)?;
    if !UserData::new(project.git_root())
        .settings()
        .map_err(settings_failure)?
        .initialized
    {
        return Err(Failure::Usage(NO_VNOC.into()));
    }
    let options = AttachOptions {
        body: read_body(args.body_file.as_deref())?,
        author: args.author.clone(),
        tags: args.tags.clone(),
        again: args.again,
        no_marker: args.no_marker,
    };
    check_note(&args.title, &options).map_err(|e| attach_failure(e, &project))?;

    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, 1);
    let node = match lookup.resolve(&args.node) {
        Ok(Target::Node(node)) => node,
        Ok(Target::Missing(missing)) => {
            return refused(
                global,
                NodeError::new(
                    "missing",
                    format!(
                        "a note is attached to code that is there: {} is Missing (its code is gone). Relink it to where the code is now (`vnoc relink` is not built yet: ask the user to relink it in VS Code)",
                        missing.was
                    ),
                ),
            );
        }
        Err(error) => return refused(global, error),
    };

    // the lock for the whole flow. What the index above knows may be a moment old by the time it is held: `attach` asks the disk which ID
    // the node has, so a node another writer just attached is reused, and no second ID line is written (04 §7).
    let _one_writer = take_lock(&opened.project).await?;
    let env = Env {
        project: &opened.project,
        index: &opened.index,
        host: &opened.host,
        edits: EditSink::Disk,
    };
    let params = AttachParams {
        key: node.id.clone(),
        title: args.title.trim().to_string(),
        markers: None,
        document: None,
    };
    let attached = attach(env, params, options)
        .await
        .map_err(|e| attach_failure(e, &opened.project))?;

    let doc = doc_path(&opened.project, std::path::Path::new(&attached.result.doc));
    let id = attached.result.id.clone();
    let location = node.location();
    let mut created = vec![doc.clone()];
    let mut changed = vec![];
    let (mut marker, mut folder_marker) = (None, None);
    match attached.marker {
        Some(MarkerWritten::Line(line)) => {
            changed.push(node.path.clone());
            let what = match node.kind {
                Kind::File => format!("a # FileID: comment was added at line {line}"),
                _ => format!("an ID: line was added at line {line}"),
            };
            marker = Some((node.path.clone(), what));
        }
        Some(MarkerWritten::FolderFile) => {
            let file = if node.path.is_empty() {
                folder_marker::FILE_NAME.to_string()
            } else {
                format!("{}/{}", node.path, folder_marker::FILE_NAME)
            };
            created.push(file.clone());
            folder_marker = Some(file);
        }
        None => {}
    }
    out::render(
        &DocNew {
            schema: 1,
            command: "doc new",
            node: NodeSummary {
                kind: kind_word(node.kind),
                location: location.clone(),
                node_id: id.clone(),
            },
            doc,
            created,
            changed,
            id_in_code: attached.result.id_in_code,
            notice: attached.result.notice,
            text: Lines {
                head: head_line(kind_word(node.kind), &location, None, Some(&id)),
                empty: args.body_file.is_none(),
                marker,
                folder_marker,
            },
        },
        global,
    )?;
    Ok(())
}
