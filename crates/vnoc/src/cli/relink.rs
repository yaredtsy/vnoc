//! `vnoc relink <ID> <node>` and `vnoc repair [<node>…] [--all]` (`docs/21-agent-cli/04-write-commands.md` §6).
//!
//! Both are the extension's flows (`identity::relink`, `identity::write_id`) with the disk applying the `ID:` edit (`EditSink::Disk`), under the write lock
//! (A7). `repair` takes the lock **once** for the whole run and writes one node at a time: each call reads its file again just before it plans, so a second
//! node of the same file is planned against the text the first one left, never against the text from before.

use serde::Serialize;
use serde_json::{Value, json};
use vnoc_domain::NodeId;

use super::args::{RelinkArgs, RepairArgs};
use super::nodes::{Lookup, NodeError, Target, expand, id_of, kind_word};
use super::out::{self, Render};
use super::project::open_from;
use super::show::head_line;
use super::write::{attach_failure, label, marker_words, take_lock};
use super::{Failure, Global, style};
use crate::identity::{
    EditSink, Env, MarkerWritten, RelinkError, WriteIdError, relink as relink_flow,
    write_id as write_id_flow,
};
use crate::index::Kind;
use crate::protocol::{RelinkParams, WriteIdParams, Written};
use crate::userdata::UserData;

/// An argument that cannot be the target: the sentence, or with `--json` a value on standard output. Exit `2`.
fn refused(global: &Global, command: &str, error: NodeError) -> Result<(), Failure> {
    if global.json {
        out::json(&json!({ "schema": 1, "command": command, "error": error.json() }))?;
    } else {
        error.print();
    }
    Err(Failure::Reported)
}

// ----------------------------------------------------------------------------------------------------------------------------- relink

#[derive(Serialize)]
struct NodeSummary {
    kind: &'static str,
    location: String,
    node_id: String,
}

#[derive(Serialize)]
struct Relinked {
    schema: u32,
    command: &'static str,
    /// the node's permanent ID
    id: String,
    /// where its code was last known
    was: String,
    /// where it is now
    node: NodeSummary,
    /// a folder's `.vnoc-id`, when the relink made one
    created: Vec<String>,
    /// the file of the code that got an `ID:` line
    changed: Vec<String>,
    /// the ID is in the code at the new place: the notes follow the code from now on
    id_in_code: bool,
    notice: Option<String>,
    #[serde(skip)]
    text: RelinkedText,
}

#[derive(Default)]
struct RelinkedText {
    /// the file and what was added to it
    marker: Option<(String, String)>,
}

impl Render for Relinked {
    fn text(&self, _quiet: bool) -> String {
        let mut text = format!(
            "{}{}  {} {}\n",
            label("relinked"),
            self.id,
            style::paint(style::dim(), "was"),
            style::paint(style::location(), &self.was)
        );
        text.push_str(&format!(
            "{}{}\n",
            label("to"),
            head_line(self.node.kind, &self.node.location, None, None::<&str>)
        ));
        for file in &self.created {
            text.push_str(&format!("{}{file}\n", label("created")));
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

/// What a `RelinkError` is to a command: a refusal of the library is exit `2` with its words; a file that could not be written is the environment's.
fn relink_failure(e: RelinkError, project: &crate::project::Project) -> Failure {
    match e {
        RelinkError::Attach(a) => attach_failure(a, project),
        RelinkError::UserData(_) => Failure::Environment(e.to_string()),
        other => Failure::Usage(other.to_string()),
    }
}

/// `vnoc relink <ID> <node>`
pub async fn relink(global: &Global, args: &RelinkArgs) -> Result<(), Failure> {
    let id = NodeId::parse(&args.id)
        .map_err(|_| Failure::Usage(RelinkError::BadId(args.id.clone()).to_string()))?;
    let opened = open_from(global).await?;
    // an ID no record has: say so as `doc path` does, rather than "not Missing"
    if UserData::new(opened.project.git_root())
        .node_toml(&id)
        .is_none()
    {
        return Err(Failure::NoSuchNode(format!("no node has the ID {id}")));
    }
    let lookup = Lookup::new(&opened, 1);
    let was = lookup.describe_missing(id).was;
    let node = match lookup.resolve(&args.node) {
        Ok(Target::Node(node)) => node,
        Ok(Target::Missing(missing)) => {
            return refused(
                global,
                "relink",
                NodeError::new(
                    "missing",
                    format!(
                        "relink to code that is there: {} is Missing too (its code is gone)",
                        missing.was
                    ),
                ),
            );
        }
        Err(error) => return refused(global, "relink", error),
    };

    let _one_writer = take_lock(&opened.project).await?;
    let env = Env {
        project: &opened.project,
        index: &opened.index,
        host: &opened.host,
        edits: EditSink::Disk,
    };
    let done = relink_flow(
        env,
        RelinkParams {
            id: id.to_string(),
            to: node.id.clone(),
            document: None,
        },
    )
    .await
    .map_err(|e| relink_failure(e, &opened.project))?;

    let (mut created, mut changed, mut marker) = (vec![], vec![], None);
    match done.marker {
        Some(MarkerWritten::Line(line)) => {
            changed.push(node.path.clone());
            marker = Some((node.path.clone(), marker_words(node.kind, line)));
        }
        Some(MarkerWritten::FolderFile) => created.push(if node.path.is_empty() {
            crate::identity::folder_marker::FILE_NAME.to_string()
        } else {
            format!(
                "{}/{}",
                node.path,
                crate::identity::folder_marker::FILE_NAME
            )
        }),
        None => {}
    }
    out::render(
        &Relinked {
            schema: 1,
            command: "relink",
            id: id.to_string(),
            was,
            node: NodeSummary {
                kind: kind_word(node.kind),
                location: node.location(),
                node_id: id.to_string(),
            },
            created,
            changed,
            id_in_code: done.result.written == Written::Id,
            notice: done.result.notice,
            text: RelinkedText { marker },
        },
        global,
    )
}

// ----------------------------------------------------------------------------------------------------------------------------- repair

/// A node to write the ID of: the key `write_id` takes, how to show it, and whether the user named it.
struct Todo {
    key: String,
    location: String,
    node_id: Option<String>,
    kind: Kind,
    named: bool,
    /// the line its def starts at (0-based; 0 for a file or a folder): the order of writing inside a file
    line: u32,
}

/// What happened to one node.
enum Outcome {
    Repaired { line: Option<u32>, what: String },
    Left { reason: String },
}

struct Row {
    todo: Todo,
    outcome: Outcome,
}

#[derive(Serialize)]
struct Repaired {
    location: String,
    node_id: Option<String>,
    /// the line (1-based) the `ID:` line is at now; none for a folder, which has a `.vnoc-id` file instead
    line: Option<u32>,
}

#[derive(Serialize)]
struct Left {
    location: String,
    node_id: Option<String>,
    reason: String,
}

#[derive(Serialize)]
struct Repair {
    schema: u32,
    command: &'static str,
    repaired: Vec<Repaired>,
    left: Vec<Left>,
    /// arguments that named no node, each with the error the other commands give
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<Value>,
    #[serde(skip)]
    rows: Vec<String>,
}

impl Render for Repair {
    fn text(&self, _quiet: bool) -> String {
        if self.rows.is_empty() {
            return "nothing to repair\n".to_string();
        }
        self.rows.iter().map(|row| format!("{row}\n")).collect()
    }
}

/// The file a node's key is in: what comes before `::`.
fn file_of(key: &str) -> &str {
    key.split("::").next().unwrap_or(key)
}

/// The 0-based line a def starts at, from the index; 0 for a file or a folder, which have no def.
fn def_line(index: &crate::index::Index, key: &str) -> u32 {
    key.parse::<vnoc_domain::LocationKey>()
        .ok()
        .and_then(|location| index.def_at(&location))
        .map_or(0, |def| def.range.start.line)
}

/// The words of what was written, in the `repaired` line.
fn written_words(kind: Kind, written: Option<MarkerWritten>) -> (Option<u32>, String) {
    match (kind, written) {
        (_, Some(MarkerWritten::Line(line))) => (
            Some(line),
            if kind == Kind::File {
                format!("# FileID: comment written at line {line}")
            } else {
                format!("ID: line written at line {line}")
            },
        ),
        (_, Some(MarkerWritten::FolderFile)) => (None, ".vnoc-id written".to_string()),
        (_, None) => (None, "ID written".to_string()),
    }
}

/// `vnoc repair [<node>…] [--all]`
pub async fn repair(global: &Global, args: &RepairArgs) -> Result<(), Failure> {
    let opened = open_from(global).await?;
    let lookup = Lookup::new(&opened, args.nodes.len());

    // what to do: the sweep from the index, or the nodes the user named (an argument that names nothing is an error in its place)
    let mut todo: Vec<Todo> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    let mut failed = false;
    if args.nodes.is_empty() {
        let index = opened.index.read().unwrap();
        let mut swept = index.lost_markers();
        if args.all {
            swept.extend(index.attached_by_name());
        }
        todo.extend(swept.into_iter().map(|m| Todo {
            line: def_line(&index, &m.key),
            location: m.key.clone(),
            key: m.key,
            node_id: Some(m.id),
            kind: m.kind,
            named: false,
        }));
    } else {
        for arg in expand(&args.nodes)? {
            match lookup.resolve(&arg) {
                Ok(Target::Node(node)) => todo.push(Todo {
                    line: def_line(&opened.index.read().unwrap(), &node.id),
                    key: node.id.clone(),
                    location: node.location(),
                    node_id: id_of(node).map(|id| id.to_string()),
                    kind: node.kind,
                    named: true,
                }),
                Ok(Target::Missing(missing)) => {
                    failed = true;
                    let error = NodeError::new(
                        "missing",
                        format!(
                            "{} is Missing: its code is gone, so there is no ID to write. Relink it: vnoc relink {} <node>",
                            missing.was, missing.id
                        ),
                    );
                    if global.json {
                        errors.push(json!({ "arg": arg, "error": error.json() }));
                    } else {
                        error.print();
                    }
                }
                Err(error) => {
                    failed = true;
                    if global.json {
                        errors.push(json!({ "arg": arg, "error": error.json() }));
                    } else {
                        error.print();
                    }
                }
            }
        }
    }

    // A file's nodes are written from the top: an `ID:` line put in above the others shifts them down, so doing the upper one first means every line
    // reported is where it stays (the other way round, the first line reported would be one off by the end).
    todo.sort_by(|a, b| (file_of(&a.key), a.line).cmp(&(file_of(&b.key), b.line)));

    // one lock for the whole run; the nodes are written one at a time, each flow reading its file again
    let mut rows: Vec<Row> = Vec::new();
    if !todo.is_empty() {
        let _one_writer = take_lock(&opened.project).await?;
        for item in todo {
            let env = Env {
                project: &opened.project,
                index: &opened.index,
                host: &opened.host,
                edits: EditSink::Disk,
            };
            let outcome = match write_id_flow(
                env,
                WriteIdParams {
                    key: item.key.clone(),
                    document: None,
                },
            )
            .await
            {
                Ok(done) if done.result.written => {
                    let (line, what) = written_words(item.kind, done.marker);
                    Outcome::Repaired { line, what }
                }
                Ok(done) => Outcome::Left {
                    reason: done.result.notice.unwrap_or_default(),
                },
                Err(WriteIdError::UserData(e)) => {
                    return Err(Failure::Environment(e.to_string()));
                }
                Err(WriteIdError::Attach(a)) => match attach_failure(a, &opened.project) {
                    Failure::Environment(message) => return Err(Failure::Environment(message)),
                    Failure::Usage(reason) | Failure::NoSuchNode(reason) => {
                        Outcome::Left { reason }
                    }
                    other => Outcome::Left {
                        reason: other.to_string(),
                    },
                },
                Err(refusal) => Outcome::Left {
                    reason: refusal.to_string(),
                },
            };
            rows.push(Row {
                todo: item,
                outcome,
            });
        }
    }

    let mut answer = Repair {
        schema: 1,
        command: "repair",
        repaired: vec![],
        left: vec![],
        errors,
        rows: vec![],
    };
    let mut named_left = false;
    for row in &rows {
        match &row.outcome {
            Outcome::Repaired { line, what } => {
                answer.rows.push(format!(
                    "{}{}  {}",
                    label("repaired"),
                    style::paint(style::location(), &row.todo.location),
                    style::paint(style::dim(), format!("({what})"))
                ));
                answer.repaired.push(Repaired {
                    location: row.todo.location.clone(),
                    node_id: row.todo.node_id.clone(),
                    line: *line,
                });
            }
            Outcome::Left { reason } => {
                named_left |= row.todo.named;
                answer.rows.push(format!(
                    "{}{}  {}",
                    label("left"),
                    style::paint(style::location(), &row.todo.location),
                    reason
                ));
                answer.left.push(Left {
                    location: row.todo.location.clone(),
                    node_id: row.todo.node_id.clone(),
                    reason: reason.clone(),
                });
            }
        }
    }
    // text: "nothing to repair" only when there was nothing to do and no argument failed (those said their error already)
    if answer.rows.is_empty() && failed && !global.json {
        // nothing more to say: the errors were printed
    } else {
        out::render(&answer, global)?;
    }
    if failed || named_left {
        Err(Failure::Reported)
    } else {
        Ok(())
    }
}
