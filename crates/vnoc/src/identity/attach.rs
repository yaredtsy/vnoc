//! Attach a doc to a function, class, file or folder: ID → marker → `node.toml` → doc file (`04-storage/03-writing-safely.md` §3).
//!
//! The order is chosen so that stopping after any step leaves something harmless: a marker without a node folder is an unused ID,
//! and a node folder without a doc is a node with no docs. The marker is written first, and a marker that cannot be written
//! does not stop the attach: the doc is then attached by name (`id_in_code = "no"`).
//!
//! The marker is a docstring line for a function or class (the driver plans it), a header comment for a file (the driver plans it),
//! and a `.vnoc-id` file for a folder (the core writes it: no language is involved).
//!
//! The refusals that are **rules** about a note live here, before anything is written, so every caller has them: a title the node already has
//! (unless `again`), a body that is too big or brings its own frontmatter, a tag or an author V-NOC cannot write (`AttachOptions`).

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::RwLock;

use vnoc_domain::{
    IdInCode, LocationKey, LocationKeyError, NoFreeId, NodeId, NodeKind, NodeToml, RepoPath,
    first_free_id,
};
use vnoc_driver_protocol::{DefKind, PlanFileIdParams, PlanId, PlanIdsParams, TextEdit};

use super::edits::EditSink;
use super::folder_marker;
use crate::driver_host::DriverHost;
use crate::index::Index;
use crate::project::Project;
use crate::protocol::{AttachParams, AttachResult, Document};
use crate::userdata::{NewDoc, UserData, UserDataError, after_frontmatter};

/// The longest body a doc is made with: a note is a note.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum AttachError {
    #[error(transparent)]
    BadKey(#[from] LocationKeyError),
    #[error("`{0}` is not in the index (is the file saved, and is it Python?)")]
    NotInIndex(String),
    #[error("the document is not the file of `{0}`")]
    WrongDocument(String),
    #[error("`{0}` needs the open document: its ID goes into the file")]
    MissingDocument(String),
    #[error(transparent)]
    NoFreeId(#[from] NoFreeId),
    #[error(transparent)]
    UserData(#[from] UserDataError),
    #[error("a doc needs a title: one line of text")]
    BadTitle,
    #[error("the body is over 256 KB: a note is at most that")]
    BodyTooLarge,
    #[error(
        "the body starts with its own --- frontmatter block: leave the frontmatter out, V-NOC writes it (node, hint, author, tags)"
    )]
    BodyHasFrontmatter,
    #[error("`{0}` is not a tag: lower-case letters, digits and `-`")]
    BadTag(String),
    #[error("the author is not one line of text")]
    BadAuthor,
    /// The node has a doc with this title already: `path` is that doc's file.
    #[error("this node already has \"{title}\" → {} (edit that file, or --again)", .path.display())]
    TitleTaken { title: String, path: PathBuf },
}

/// What an attach is asked for beyond what the editor's request says (`AttachParams`).
#[derive(Debug, Default)]
pub struct AttachOptions {
    /// the Markdown after the heading; empty for a doc of the heading alone
    pub body: String,
    /// the frontmatter's `author:`
    pub author: Option<String>,
    /// the frontmatter's `tags:`: lower-case letters, digits and `-`
    pub tags: Vec<String>,
    /// allow a second doc with a title the node already has (it becomes `<name>-2.md`)
    pub again: bool,
    /// write no ID into the code for this attach, whatever `vnoc.toml` says: the doc is attached by name
    pub no_marker: bool,
}

impl AttachOptions {
    /// What the extension's attach has always done: a second attach with the same title is a second doc, and the project decides on the ID.
    pub fn editor() -> AttachOptions {
        AttachOptions {
            again: true,
            ..AttachOptions::default()
        }
    }
}

/// What a marker wrote, for the caller that says so to its user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerWritten {
    /// an `ID:` line or a `# FileID:` comment, now at this line (1-based) of the file
    Line(u32),
    /// a `.vnoc-id` file in the folder
    FolderFile,
}

/// What an attach did: the answer the editor gets, and what was written into the code.
#[derive(Debug)]
pub struct Attached {
    pub result: AttachResult,
    /// `None` when the code was not touched (the node had its ID, or none was allowed, or the marker could not be written)
    pub marker: Option<MarkerWritten>,
}

/// What an attach works with.
pub struct Env<'a> {
    pub project: &'a Project,
    pub index: &'a RwLock<Index>,
    pub host: &'a DriverHost,
    /// who applies the edit that puts an ID into a file: the editor, or the disk
    pub edits: EditSink,
}

/// The document an ID is written into, as `document_for` found it.
pub(super) enum MarkerDoc<'a> {
    /// a folder: its marker is a file the core writes, no document is involved
    Folder,
    Open(Cow<'a, Document>),
    /// the file could not be read as text (`EditSink::Disk` only): the reason, for the notice. The attach goes on by name.
    Unreadable(String),
}

/// What the doc is attached to, with what the index knows of its ID.
pub(super) enum Target {
    Def {
        in_code: Option<NodeId>,
        kind: DefKind,
    },
    File {
        in_code: Option<NodeId>,
    },
    Folder {
        in_code: Option<NodeId>,
    },
}

impl Target {
    pub(super) fn kind(&self) -> NodeKind {
        match self {
            Target::Def {
                kind: DefKind::Class,
                ..
            } => NodeKind::Class,
            Target::Def {
                kind: DefKind::Function,
                ..
            } => NodeKind::Function,
            Target::File { .. } => NodeKind::File,
            Target::Folder { .. } => NodeKind::Folder,
        }
    }

    pub(super) fn in_code(&self) -> Option<NodeId> {
        match self {
            Target::Def { in_code, .. } | Target::File { in_code } | Target::Folder { in_code } => {
                *in_code
            }
        }
    }
}

/// A key with `::` names a def; a bare repo path names a file or a folder (the tree's ids, `index::Node::id`).
pub(super) fn find_target(index: &Index, key: &str) -> Result<(LocationKey, Target), AttachError> {
    let not_in_index = || AttachError::NotInIndex(key.to_string());
    if key.contains("::") {
        let location: LocationKey = key.parse()?;
        let def = index.def_at(&location).ok_or_else(not_in_index)?;
        let in_code = def.id.as_deref().and_then(|text| NodeId::parse(text).ok());
        return Ok((
            location,
            Target::Def {
                in_code,
                kind: def.kind,
            },
        ));
    }
    let path = RepoPath::new(key).map_err(|_| not_in_index())?;
    let whole = LocationKey::new(path.clone(), "", 1).expect("occurrences are 1-based");
    if index.is_file(&path) {
        Ok((
            whole,
            Target::File {
                in_code: index.file_id(&path),
            },
        ))
    } else if index.is_folder(&path) {
        Ok((
            whole,
            Target::Folder {
                in_code: index.folder_id(&path),
            },
        ))
    } else {
        Err(not_in_index())
    }
}

pub async fn attach(
    mut env: Env<'_>,
    params: AttachParams,
    options: AttachOptions,
) -> Result<Attached, AttachError> {
    check_note(&params.title, &options)?;
    let (location, target) = find_target(&env.index.read().unwrap(), &params.key)?;
    let document = document_for(
        &env,
        &target,
        &location,
        &params.key,
        params.document.as_ref(),
    )?;

    let data = UserData::new(env.project.git_root());
    let settings = data.settings()?;
    // the question is asked once per project: afterwards `vnoc.toml` answers it; `no_marker` is this attach's own "no"
    let project_says = if settings.initialized {
        settings.markers
    } else {
        params.markers.unwrap_or(true)
    };
    let markers_on = project_says && !options.no_marker;

    // 1. which ID: the one in the code, else the one this location already has, else a new one
    let in_code = target.in_code();
    let existing = if in_code.is_some() {
        None
    } else {
        data.find_node_at(&location)
    };
    let (id, is_new) = match (in_code, existing) {
        (Some(id), _) | (None, Some(id)) => (id, false),
        (None, None) => (first_free_id(&location, |id| data.is_taken(id))?, true),
    };
    // a title the node has is refused before anything is written
    if !options.again
        && let Some(held) = data
            .docs_of(&id)
            .into_iter()
            .find(|doc| doc.title.trim().to_lowercase() == params.title.trim().to_lowercase())
    {
        return Err(AttachError::TitleTaken {
            title: held.title,
            path: held.path,
        });
    }

    // 2. the marker, for a node that has none and is allowed one
    let mut notice = None;
    let mut marker = None;
    let mut id_in_code = in_code.is_some();
    if is_new && markers_on {
        match write_marker(&mut env, &target, &location, &id, &document).await {
            Ok(written) => {
                id_in_code = true;
                marker = Some(written);
            }
            Err(reason) => {
                notice = Some(format!(
                    "The ID could not be written ({reason}); this doc is attached by name."
                ))
            }
        }
    }

    // 3. `.vnoc/`: first time set up, then the node, then the doc
    data.initialize(params.markers.unwrap_or(true))?;
    let mut toml = NodeToml::new(&location);
    toml.set_id_in_code(if id_in_code {
        IdInCode::Yes
    } else {
        IdInCode::No
    });
    toml.set_kind(&target.kind());
    data.create_node(&id, &toml)?;
    let hint = if location.qualname().is_empty() {
        location.path().to_string()
    } else {
        location.to_string()
    };
    let doc: PathBuf = data.create_doc(
        &id,
        &NewDoc {
            title: &params.title,
            hint: &hint,
            body: &options.body,
            author: options.author.as_deref().map(str::trim),
            tags: &once_each(&options.tags),
        },
    )?;

    Ok(Attached {
        result: AttachResult {
            id: id.to_string(),
            doc: doc.to_string_lossy().into_owned(),
            id_in_code,
            notice,
        },
        marker,
    })
}

/// The rules about the note itself, asked before anything is read or written.
pub fn check_note(title: &str, options: &AttachOptions) -> Result<(), AttachError> {
    let one_line = |text: &str| !text.trim().is_empty() && !text.chars().any(char::is_control);
    if !one_line(title) {
        return Err(AttachError::BadTitle);
    }
    if options.author.as_deref().is_some_and(|a| !one_line(a)) {
        return Err(AttachError::BadAuthor);
    }
    if let Some(bad) = options.tags.iter().find(|tag| !is_tag(tag)) {
        return Err(AttachError::BadTag(bad.clone()));
    }
    if options.body.len() > MAX_BODY_BYTES {
        return Err(AttachError::BodyTooLarge);
    }
    // blank lines before it do not hide it; a lone `---` (a rule) with no closing line is Markdown, not a frontmatter block
    if after_frontmatter(options.body.trim_start()).is_some() {
        return Err(AttachError::BodyHasFrontmatter);
    }
    Ok(())
}

/// `tags` with a tag given twice kept once, in the order first given.
fn once_each(tags: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        if !seen.contains(tag) {
            seen.push(tag.clone());
        }
    }
    seen
}

/// A tag is lower-case ASCII letters, digits and `-`, and not empty.
fn is_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A function, class or file marker is an edit of the node's own file: the open document (the editor), or the file itself (`Disk`, when no
/// document is given). A folder has no document.
pub(super) fn document_for<'a>(
    env: &Env<'_>,
    target: &Target,
    location: &LocationKey,
    key: &str,
    document: Option<&'a Document>,
) -> Result<MarkerDoc<'a>, AttachError> {
    let path = env.project.git_root().join(location.path().as_str());
    match (target, document) {
        (Target::Folder { .. }, _) => Ok(MarkerDoc::Folder),
        (_, Some(document)) => {
            if document.uri.to_file_path().ok() != Some(path) {
                return Err(AttachError::WrongDocument(key.to_string()));
            }
            Ok(MarkerDoc::Open(Cow::Borrowed(document)))
        }
        (_, None) if env.edits.is_disk() => Ok(match Document::from_disk(&path) {
            Ok(document) => MarkerDoc::Open(Cow::Owned(document)),
            Err(e) => MarkerDoc::Unreadable(e.to_string()),
        }),
        (_, None) => Err(AttachError::MissingDocument(key.to_string())),
    }
}

/// Writes the ID where the node is: the driver plans a def's or a file's line and the sink applies it; the core writes a folder's `.vnoc-id`.
/// `Err` says why not, in words for the user.
pub(super) async fn write_marker(
    env: &mut Env<'_>,
    target: &Target,
    location: &LocationKey,
    id: &NodeId,
    document: &MarkerDoc<'_>,
) -> Result<MarkerWritten, String> {
    match (target, document) {
        (_, MarkerDoc::Unreadable(reason)) => Err(reason.clone()),
        (Target::Def { .. }, MarkerDoc::Open(document)) => {
            write_def_marker(env, location, id, document).await
        }
        (Target::File { .. }, MarkerDoc::Open(document)) => {
            write_file_marker(env, id, document).await
        }
        _ => write_folder_marker(env.project.git_root(), location.path(), id)
            .map(|()| MarkerWritten::FolderFile),
    }
}

/// Plans the `ID:` line with the driver and has the sink apply it. `Err` says why not, in words for the user.
async fn write_def_marker(
    env: &mut Env<'_>,
    location: &LocationKey,
    id: &NodeId,
    document: &Document,
) -> Result<MarkerWritten, String> {
    let ask = PlanIdsParams {
        text: document.text.clone(),
        version: i64::from(document.version),
        ids: vec![PlanId {
            qualname: location.qualname().to_string(),
            occurrence: location.occurrence(),
            id: id.to_string(),
            replace_existing: false,
        }],
    };
    let plan = env.host.plan_ids(&ask).await.map_err(|e| e.to_string())?;
    if let Some(skipped) = plan.skipped.first() {
        return Err(skipped.reason.clone());
    }
    if plan.edits.is_empty() {
        return Err("the function is not in the open document".into());
    }
    apply_edits(env, document, "V-NOC: add an ID", &plan.edits).await?;
    let first = plan.written.first().map(|w| w.span.start);
    Ok(MarkerWritten::Line(line_of(first, &plan.edits)))
}

/// The 1-based line of what was written: where the plan says it is (in the new text), else where the first edit begins.
fn line_of(written: Option<vnoc_driver_protocol::Position>, edits: &[TextEdit]) -> u32 {
    written
        .or_else(|| edits.first().map(|e| e.range.start))
        .map_or(1, |at| at.line + 1)
}

/// Plans the `# FileID:` comment with the driver and has the sink apply it.
async fn write_file_marker(
    env: &mut Env<'_>,
    id: &NodeId,
    document: &Document,
) -> Result<MarkerWritten, String> {
    let ask = PlanFileIdParams {
        text: document.text.clone(),
        version: i64::from(document.version),
        id: id.to_string(),
    };
    let plan = env
        .host
        .plan_file_id(&ask)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(reason) = plan.skipped {
        return Err(reason);
    }
    apply_edits(env, document, "V-NOC: add a file ID", &plan.edits).await?;
    let first = plan.written.as_ref().map(|w| w.span.start);
    Ok(MarkerWritten::Line(line_of(first, &plan.edits)))
}

/// Writes `.vnoc-id` in the folder. The core does it itself: nobody has it open, and no language is involved.
fn write_folder_marker(
    git_root: &std::path::Path,
    folder: &RepoPath,
    id: &NodeId,
) -> Result<(), String> {
    folder_marker::write(&git_root.join(folder.as_str()), id).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => format!(
            "the folder already has a {} that V-NOC cannot read",
            folder_marker::FILE_NAME
        ),
        _ => e.to_string(),
    })
}

async fn apply_edits(
    env: &mut Env<'_>,
    document: &Document,
    label: &str,
    edits: &[TextEdit],
) -> Result<(), String> {
    env.edits.apply(document, label, edits).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_body(body: &str) -> AttachOptions {
        AttachOptions {
            body: body.into(),
            ..AttachOptions::default()
        }
    }

    #[test]
    fn a_tag_given_twice_is_kept_once_in_the_order_first_given() {
        let tags = |list: &[&str]| list.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(
            once_each(&tags(&["plan", "billing", "plan", "x", "billing"])),
            tags(&["plan", "billing", "x"])
        );
        assert_eq!(once_each(&[]), Vec::<String>::new());
    }

    #[test]
    fn the_rules_about_a_note_are_asked_before_anything_is_written() {
        let ok = AttachOptions::default();
        assert!(check_note("Why we parse twice", &ok).is_ok());
        for bad in ["", "  ", "two\nlines", "tab\there"] {
            assert!(
                matches!(check_note(bad, &ok), Err(AttachError::BadTitle)),
                "{bad:?}"
            );
        }
        // the size: exactly 256 KB is a note, one byte more is not
        assert!(check_note("T", &with_body(&"x".repeat(MAX_BODY_BYTES))).is_ok());
        assert!(matches!(
            check_note("T", &with_body(&"x".repeat(MAX_BODY_BYTES + 1))),
            Err(AttachError::BodyTooLarge)
        ));
        // a body with its own frontmatter block; a lone `---` is a rule
        for body in ["---\na: b\n---\n", "\n \n---\r\na: b\r\n---\r\ntext"] {
            assert!(
                matches!(
                    check_note("T", &with_body(body)),
                    Err(AttachError::BodyHasFrontmatter)
                ),
                "{body:?}"
            );
        }
        for body in ["---\nno closing line\n", "text\n---\na: b\n---\n", ""] {
            assert!(check_note("T", &with_body(body)).is_ok(), "{body:?}");
        }
        // tags
        let tagged = |tags: &[&str]| AttachOptions {
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..AttachOptions::default()
        };
        assert!(check_note("T", &tagged(&["plan", "v2", "why-not", "2024"])).is_ok());
        for bad in ["Plan", "two words", "", "under_score", "caf\u{e9}", "a/b"] {
            assert!(
                matches!(check_note("T", &tagged(&["plan", bad])), Err(AttachError::BadTag(t)) if t == bad),
                "{bad:?}"
            );
        }
        // the author
        let by = |author: &str| AttachOptions {
            author: Some(author.into()),
            ..AttachOptions::default()
        };
        assert!(check_note("T", &by("claude-code")).is_ok());
        assert!(check_note("T", &by("Ana Perez")).is_ok());
        for bad in ["", " ", "a\nb"] {
            assert!(
                matches!(check_note("T", &by(bad)), Err(AttachError::BadAuthor)),
                "{bad:?}"
            );
        }
    }
}
