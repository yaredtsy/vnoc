//! Everything under `.vnoc/`, as use cases: callers say **what** should happen (`create_doc`), never **where** to write. The
//! layout lives in `layout.rs` alone, and every write goes through `atomic.rs` (`04-storage/03-writing-safely.md` §5).
//!
//! This module does not know the index, the driver or the editor, so it is tested with nothing but a temp folder.

pub(crate) mod atomic;
mod doc_text;
pub(crate) mod layout;
mod lock;
mod read;
mod tags;
mod trash;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, value};
use vnoc_domain::{LocationKey, NodeId, NodeToml};

pub use doc_text::{DocStats, DocText, DocTexts};
pub use lock::{DEFAULT_WAIT, LockError, ToursLock, ToursLockError, WriteLock};
pub(crate) use read::after_frontmatter;
pub use read::{DocInfo, Frontmatter, LoadProblem, LoadReport, NodeRecord, frontmatter};
pub use tags::{
    AuthorCount, Change, DescribeError, Described, Descriptions, TagCount, TagError, WriteError,
    author_counts, check_description, is_tag, rewrite_tags, tag_counts,
};
pub use trash::{MovePlan, TrashEntry, TrashError};

#[derive(Debug, thiserror::Error)]
pub enum UserDataError {
    #[error("could not write in .vnoc: {0}")]
    Io(#[from] io::Error),
    #[error(".vnoc/vnoc.toml is not valid: {0}")]
    Config(String),
    #[error(
        ".vnoc/vnoc.toml has `format = {0}`, which is newer than this V-NOC: update V-NOC to change docs in this project"
    )]
    TooNew(i64),
}

/// The project's settings (`.vnoc/vnoc.toml`), with defaults for what it does not say.
#[derive(Debug, PartialEq, Eq)]
pub struct Settings {
    /// `.vnoc/vnoc.toml` exists: something was attached here before
    pub initialized: bool,
    /// `[ids] default` is `"on-attach"` (the default): attaching writes an ID into the code
    pub markers: bool,
}

/// Why a tag was not described.
#[derive(Debug, thiserror::Error)]
pub enum DescribeTagError {
    #[error(transparent)]
    Describe(#[from] DescribeError),
    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Why the tags of a doc were not changed.
#[derive(Debug, thiserror::Error)]
pub enum EditTagsError {
    #[error(transparent)]
    Tag(#[from] TagError),
    #[error("{0} is not a doc of a node")]
    NotADoc(PathBuf),
    #[error("the doc is not UTF-8 text")]
    NotUtf8,
    #[error(transparent)]
    Write(#[from] WriteError),
}

pub struct UserData {
    /// `<git root>/.vnoc`
    root: PathBuf,
}

impl UserData {
    pub fn new(git_root: &Path) -> UserData {
        UserData {
            root: git_root.join(layout::FOLDER),
        }
    }

    pub fn settings(&self) -> Result<Settings, UserDataError> {
        let file = layout::project_file(&self.root);
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Settings {
                    initialized: false,
                    markers: true,
                });
            }
            Err(e) => return Err(e.into()),
        };
        let doc: DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| UserDataError::Config(e.to_string()))?;
        match doc.get("format").and_then(|f| f.as_integer()) {
            Some(1) => {}
            Some(newer) if newer > 1 => return Err(UserDataError::TooNew(newer)),
            _ => return Err(UserDataError::Config("`format = 1` is missing".into())),
        }
        let markers = doc
            .get("ids")
            .and_then(|ids| ids.get("default"))
            .and_then(|d| d.as_str())
            != Some("none");
        Ok(Settings {
            initialized: true,
            markers,
        })
    }

    /// The lock for one attach, relink or write-ID (waiting up to `DEFAULT_WAIT` for another writer). `None` in a project with no `.vnoc/`
    /// yet. Blocks the thread: from async code, run it with `spawn_blocking`.
    pub fn write_lock(&self) -> Result<Option<WriteLock>, LockError> {
        WriteLock::take_if_initialized(&self.root, DEFAULT_WAIT)
    }

    /// The two files `initialize` writes: `.vnoc/.gitignore` and `.vnoc/vnoc.toml`.
    pub fn init_files(&self) -> [PathBuf; 2] {
        [
            layout::gitignore_file(&self.root),
            layout::project_file(&self.root),
        ]
    }

    /// Creates `.vnoc/` with its `vnoc.toml` and `.gitignore` the first time. Does nothing when the project already has them
    /// (so `markers` only matters for the very first attach).
    pub fn initialize(&self, markers: bool) -> Result<(), UserDataError> {
        let file = layout::project_file(&self.root);
        if file.exists() {
            return Ok(());
        }
        atomic::write_atomic(&layout::gitignore_file(&self.root), b"local/\n")?;
        let mut doc = DocumentMut::new();
        doc["format"] = value(1);
        if !markers {
            doc["ids"]["default"] = value("none");
        }
        // last: its existence is what says "initialized"
        atomic::write_atomic(&file, doc.to_string().as_bytes())?;
        Ok(())
    }

    /// Whether a node folder with this ID exists, in `nodes/` or in the trash. A taken ID is never reused.
    pub fn is_taken(&self, id: &NodeId) -> bool {
        layout::node_dir(&self.root, id).exists() || !layout::trashed(&self.root, id).is_empty()
    }

    /// The ID of the node already attached at `location`, if any (found by its anchor in `node.toml`).
    pub fn find_node_at(&self, location: &LocationKey) -> Option<NodeId> {
        read::load_all(&self.root)
            .records
            .into_iter()
            .find(|(_, record)| record.toml.location().is_ok_and(|l| l == *location))
            .map(|(id, _)| id)
    }

    /// Creates the node folder and its `node.toml`. A folder that is already there is left as it is.
    pub fn create_node(&self, id: &NodeId, toml: &NodeToml) -> Result<(), UserDataError> {
        let file = layout::node_file(&self.root, id);
        if !file.exists() {
            atomic::write_atomic(&file, toml.to_string().as_bytes())?;
        }
        Ok(())
    }

    /// The `node.toml` of a node, as the file holds it. `None` when there is no readable one.
    pub fn node_toml(&self, id: &NodeId) -> Option<NodeToml> {
        NodeToml::parse(&std::fs::read_to_string(layout::node_file(&self.root, id)).ok()?).ok()
    }

    /// Replaces the `node.toml` of a node that exists (one atomic write). The node folder is never created here.
    pub fn write_node_toml(&self, id: &NodeId, toml: &NodeToml) -> Result<(), UserDataError> {
        let file = layout::node_file(&self.root, id);
        if !file.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("node {id} has no node.toml"),
            )
            .into());
        }
        atomic::write_atomic(&file, toml.to_string().as_bytes())?;
        Ok(())
    }

    /// Creates a new Markdown doc in the node's folder and returns its path.
    pub fn create_doc(&self, id: &NodeId, doc: &NewDoc) -> Result<PathBuf, UserDataError> {
        let folder = layout::node_dir(&self.root, id);
        let content = doc.render(id);
        let slug = slug(doc.title);
        for n in 1.. {
            let name = if n == 1 {
                format!("{slug}.md")
            } else {
                format!("{slug}-{n}.md")
            };
            match atomic::create_new(&folder.join(&name), content.as_bytes()) {
                Ok(()) => return Ok(folder.join(name)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue, // taken: try the next number
                Err(e) => return Err(e.into()),
            }
        }
        unreachable!("the loop ends by returning")
    }

    /// The docs of one node, with their titles (read from the files).
    pub fn docs_of(&self, id: &NodeId) -> Vec<DocInfo> {
        read::docs_of(&self.root, id)
    }

    /// Whether `path` is a file inside `nodes/` (the editor asks before it shows a doc it was told about).
    pub fn is_doc(&self, path: &Path) -> bool {
        path.starts_with(layout::nodes_dir(&self.root))
            && path.extension().is_some_and(|e| e == "md")
            && !path.components().any(|c| c.as_os_str() == "..")
    }

    /// Every node folder, read. Broken ones are reported, not fatal.
    pub fn load_all(&self) -> LoadReport {
        read::load_all(&self.root)
    }

    /// What each tag means, from `.vnoc/tags.toml` (none when the file is not there; its problems are `load_all`'s).
    pub fn tag_descriptions(&self) -> Descriptions {
        tags::read_descriptions(&self.root).0
    }

    /// Sets (`Some`) or takes away (`None`) the description of `tag` in `.vnoc/tags.toml`, creating the file when it is not there (`describe_in` says what
    /// changes in it). Nothing is written when nothing changes. The caller holds the write lock.
    pub fn describe_tag(
        &self,
        tag: &str,
        description: Option<&str>,
    ) -> Result<Described, DescribeTagError> {
        check_description(tag, description)?;
        let file = layout::tags_file(&self.root);
        let was = match fs::read_to_string(&file) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(WriteError::from(e).into()),
        };
        let (new, done) = tags::describe_in(was.as_deref().unwrap_or(""), tag, description)?;
        match &was {
            // a file that is not there is created, but only for a description: `--clear` of nothing makes no file
            None if done == Described::Set => {
                atomic::write_atomic(&file, new.as_bytes()).map_err(WriteError::from)?
            }
            Some(was) if *was != new => tags::write_if_unchanged(&file, was, &new)?,
            _ => {}
        }
        Ok(done)
    }

    /// Changes the tags of the doc at `path` (a Markdown file under `nodes/`): read, rewritten as `rewrite_tags` says, and written atomically only if
    /// the file is still what was read. Nothing is written when nothing changes. The caller holds the write lock.
    pub fn edit_doc_tags(
        &self,
        path: &Path,
        add: &[String],
        remove: &[String],
    ) -> Result<Change, EditTagsError> {
        if !self.is_doc(path) {
            return Err(EditTagsError::NotADoc(path.to_path_buf()));
        }
        let text = fs::read(path)
            .map_err(WriteError::from)
            .map_err(EditTagsError::Write)?;
        let text = String::from_utf8(text).map_err(|_| EditTagsError::NotUtf8)?;
        let (new, change) = rewrite_tags(&text, add, remove)?;
        if change.changes_text() {
            tags::write_if_unchanged(path, &text, &new)?;
        }
        Ok(change)
    }

    /// The doc texts of `records` (what `load_all` gave), reading only the files whose stamp moved since `held`. Disk work only.
    pub fn refresh_doc_texts(
        &self,
        held: &DocTexts,
        records: &[(NodeId, NodeRecord)],
    ) -> (DocTexts, DocStats) {
        held.refresh(&self.root, records)
    }
}

/// What a new doc holds. The frontmatter is V-NOC's: `node:`, `hint:`, then `author:` and `tags:` only when there are some.
pub struct NewDoc<'a> {
    pub title: &'a str,
    /// for people reading a pull request: where the node is
    pub hint: &'a str,
    /// the Markdown after the heading; empty for a doc of the heading alone
    pub body: &'a str,
    pub author: Option<&'a str>,
    pub tags: &'a [String],
}

impl<'a> NewDoc<'a> {
    /// A doc of its heading alone, for the tests of what sits around the writer.
    #[cfg(test)]
    pub(crate) fn titled(title: &'a str, hint: &'a str) -> NewDoc<'a> {
        NewDoc {
            title,
            hint,
            body: "",
            author: None,
            tags: &[],
        }
    }

    fn render(&self, id: &NodeId) -> String {
        let mut text = format!("---\nnode: {id}\nhint: {}\n", self.hint);
        if let Some(author) = self.author {
            text.push_str(&format!("author: {author}\n"));
        }
        if !self.tags.is_empty() {
            text.push_str(&format!("tags: [{}]\n", self.tags.join(", ")));
        }
        text.push_str(&format!("---\n# {}\n\n", self.title.trim()));
        text.push_str(self.body);
        if !self.body.is_empty() && !self.body.ends_with('\n') {
            text.push('\n');
        }
        text
    }
}

/// `body` without its first line when that is a `# ` heading equal to `title` (trimmed, case and runs of spaces ignored), and without the blank lines
/// right after it: V-NOC writes the title, so a body that starts with it would have it twice. A different heading, a `## ` one, or text stays as it is.
/// The flag says whether a line was dropped.
pub fn without_own_title<'b>(title: &str, body: &'b str) -> (&'b str, bool) {
    let words =
        |text: &str| -> Vec<String> { text.split_whitespace().map(str::to_lowercase).collect() };
    let mut rest = body;
    // blank lines before the heading do not hide it
    while let Some(line) = rest.split_inclusive('\n').next() {
        if !line.trim().is_empty() {
            break;
        }
        rest = &rest[line.len()..];
    }
    let Some(line) = rest.split_inclusive('\n').next() else {
        return (body, false);
    };
    match line.strip_prefix("# ") {
        Some(heading) if words(heading) == words(title) => {
            rest = &rest[line.len()..];
            while let Some(line) = rest.split_inclusive('\n').next() {
                if !line.trim().is_empty() {
                    break;
                }
                rest = &rest[line.len()..];
            }
            (rest, true)
        }
        _ => (body, false),
    }
}

/// A file name for a title that is legal on macOS, Linux and Windows (`04-storage/01-vnoc-folder.md` §3, "Docs").
fn slug(title: &str) -> String {
    const RESERVED: [&str; 22] = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let cleaned: String = title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .map(|c| {
            if c.is_whitespace() {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    let cleaned: String = cleaned
        .trim_matches(|c| c == '.' || c == '-')
        .chars()
        .take(60)
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', '-']).to_string();
    if cleaned.is_empty() || RESERVED.contains(&cleaned.as_str()) {
        "doc".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_domain::{IdInCode, RepoPath, first_free_id};

    fn fresh(label: &str) -> UserData {
        let d = std::env::temp_dir().join(format!("vnoc-userdata-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        UserData::new(&d)
    }

    fn location() -> LocationKey {
        LocationKey::new(
            RepoPath::new("src/orders.py").unwrap(),
            "OrderService.parse_order",
            1,
        )
        .unwrap()
    }

    #[test]
    fn a_body_that_starts_with_the_title_loses_that_line_and_the_blank_lines_after_it() {
        let own = |title, body| without_own_title(title, body);
        let same = ("Body.\n", true);
        assert_eq!(own("Same title", "# Same title\n\nBody.\n"), same);
        assert_eq!(own("Same title", "# Same title\n\n\n  \nBody.\n"), same);
        // case and runs of spaces do not matter; nor do spaces around the title
        assert_eq!(own("Same title", "#   SAME   Title  \n\nBody.\n"), same);
        assert_eq!(own("  Same   title ", "# same title\nBody.\n"), same);
        // blank lines before the heading do not hide it
        assert_eq!(own("Same title", "\n  \n# Same title\n\nBody.\n"), same);
        // nothing after it
        assert_eq!(own("Same title", "# Same title"), ("", true));
        // the text after it keeps its own indentation
        assert_eq!(own("T", "# T\n\n    code\n"), ("    code\n", true));
    }

    #[test]
    fn any_other_body_is_left_as_it_is() {
        for body in [
            "",
            "\n\n",
            "Body.\n",
            "# Another title\n\nBody.\n",
            "## Same title\n\nBody.\n",
            "#Same title\n\nBody.\n",
            "Intro.\n\n# Same title\n\nBody.\n",
            "# Same title, and more\n",
        ] {
            assert_eq!(
                without_own_title("Same title", body),
                (body, false),
                "{body:?}"
            );
        }
    }

    #[test]
    fn titles_become_legal_file_names() {
        assert_eq!(slug("Why we parse twice"), "why-we-parse-twice");
        assert_eq!(slug("What? <Where>: a/b"), "what-where-ab");
        assert_eq!(slug("  .hidden. "), "hidden");
        assert_eq!(slug("CON"), "doc");
        assert_eq!(slug("???"), "doc");
        assert_eq!(slug(&"x".repeat(100)).len(), 60);
        assert_eq!(
            slug("Caf\u{e9} notes"),
            "caf\u{e9}-notes",
            "other letters are kept"
        );
    }

    #[test]
    fn nothing_exists_until_the_first_attach_and_then_the_project_is_set_up_once() {
        let data = fresh("init");
        assert_eq!(
            data.settings().unwrap(),
            Settings {
                initialized: false,
                markers: true
            }
        );
        assert!(!data.root.exists());

        data.initialize(false).unwrap();
        assert_eq!(
            data.settings().unwrap(),
            Settings {
                initialized: true,
                markers: false
            }
        );
        assert_eq!(
            std::fs::read_to_string(data.root.join(".gitignore")).unwrap(),
            "local/\n"
        );

        data.initialize(true).unwrap(); // the first answer stays
        assert!(!data.settings().unwrap().markers);
    }

    #[test]
    fn a_project_from_a_newer_version_is_refused_and_unknown_keys_are_fine() {
        let data = fresh("format");
        std::fs::create_dir_all(&data.root).unwrap();
        std::fs::write(data.root.join("vnoc.toml"), "format = 2\n").unwrap();
        assert!(matches!(data.settings(), Err(UserDataError::TooNew(2))));
        std::fs::write(
            data.root.join("vnoc.toml"),
            "format = 1\nfuture = true\n[ids]\nstyle = \"comment\"\n",
        )
        .unwrap();
        assert!(data.settings().unwrap().markers);
    }

    #[test]
    fn a_node_and_its_docs_are_written_found_and_read_back() {
        let data = fresh("node");
        data.initialize(true).unwrap();
        let id = first_free_id(&location(), |id| data.is_taken(id)).unwrap();
        let mut toml = NodeToml::new(&location());
        toml.set_id_in_code(IdInCode::No);
        data.create_node(&id, &toml).unwrap();
        assert!(data.is_taken(&id));
        assert_ne!(
            first_free_id(&location(), |id| data.is_taken(id)).unwrap(),
            id,
            "a taken ID is stepped past"
        );

        let new = NewDoc {
            title: "Why we parse twice",
            hint: "src/orders.py::OrderService.parse_order",
            body: "",
            author: None,
            tags: &[],
        };
        let first = data.create_doc(&id, &new).unwrap();
        let second = data.create_doc(&id, &new).unwrap();
        assert_eq!(
            (
                first.file_name().unwrap().to_str(),
                second.file_name().unwrap().to_str()
            ),
            (
                Some("why-we-parse-twice.md"),
                Some("why-we-parse-twice-2.md")
            )
        );
        let text = std::fs::read_to_string(&first).unwrap();
        assert_eq!(
            text,
            format!(
                "---\nnode: {id}\nhint: src/orders.py::OrderService.parse_order\n---\n# Why we parse twice\n\n"
            )
        );

        assert_eq!(data.find_node_at(&location()), Some(id));
        let report = data.load_all();
        assert!(report.problems.is_empty());
        let (found, record) = &report.records[0];
        assert_eq!(found, &id);
        assert_eq!(
            record.docs,
            ["why-we-parse-twice-2.md", "why-we-parse-twice.md"]
        );
        assert_eq!(record.toml.id_in_code(), Some(IdInCode::No));
    }

    #[test]
    fn a_new_doc_has_author_and_tags_only_when_given_and_the_body_after_the_heading() {
        let id = first_free_id(&location(), |_| false).unwrap();
        let tags = ["plan".to_string(), "billing".to_string()];
        let full = NewDoc {
            title: " Split it ",
            hint: "src/orders.py::OrderService.parse_order",
            body: "First line\n\nSecond",
            author: Some("claude-code"),
            tags: &tags,
        };
        assert_eq!(
            full.render(&id),
            format!(
                "---\nnode: {id}\nhint: src/orders.py::OrderService.parse_order\nauthor: claude-code\ntags: [plan, billing]\n---\n# Split it\n\nFirst line\n\nSecond\n"
            ),
            "a body without a last line break gets one"
        );
        let bare = NewDoc {
            author: None,
            tags: &[],
            body: "",
            ..full
        };
        assert_eq!(
            bare.render(&id),
            format!(
                "---\nnode: {id}\nhint: src/orders.py::OrderService.parse_order\n---\n# Split it\n\n"
            )
        );
        // and it reads back
        let read = frontmatter(&NewDoc { body: "x", ..full }.render(&id));
        assert_eq!(
            (read.title.as_deref(), read.author.as_deref(), read.tags),
            (Some("Split it"), Some("claude-code"), tags.to_vec())
        );
    }

    #[test]
    fn a_bad_key_in_tags_toml_is_a_problem_of_the_load_and_the_rest_is_read() {
        let data = fresh("tags-toml");
        data.initialize(true).unwrap();
        assert!(data.load_all().problems.is_empty());
        std::fs::write(data.root.join("tags.toml"), "plan = \"ok\"\nBad = \"x\"\n").unwrap();
        let report = data.load_all();
        assert_eq!(report.problems.len(), 1, "{:?}", report.problems);
        assert!(
            report.problems[0]
                .text
                .starts_with(".vnoc/tags.toml: `Bad` is not a tag")
        );
        assert_eq!(report.problems[0].file, data.root.join("tags.toml"));
        assert_eq!(data.tag_descriptions().get("plan"), Some("ok"));
    }

    #[test]
    fn a_broken_node_is_a_problem_and_the_others_still_load() {
        let data = fresh("broken");
        data.initialize(true).unwrap();
        let id = first_free_id(&location(), |_| false).unwrap();
        data.create_node(&id, &NodeToml::new(&location())).unwrap();
        let bad = first_free_id(
            &LocationKey::new(RepoPath::new("b.py").unwrap(), "g", 1).unwrap(),
            |_| false,
        )
        .unwrap();
        std::fs::create_dir_all(layout::node_dir(&data.root, &bad)).unwrap();
        std::fs::write(layout::node_file(&data.root, &bad), "not = [valid").unwrap();
        std::fs::create_dir_all(data.root.join("nodes/zz/not-an-id")).unwrap();

        let report = data.load_all();
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.problems.len(), 2, "{:?}", report.problems);
        let temp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(
            report
                .problems
                .iter()
                .all(|p| p.text.starts_with(".vnoc/nodes/") && !p.text.contains(&temp)),
            "paths in the text are relative to the repo: {:?}",
            report.problems
        );
    }
}
