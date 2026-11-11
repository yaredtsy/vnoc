//! Everything under `.vnoc/`, as use cases: callers say **what** should happen (`create_doc`), never **where** to write. The
//! layout lives in `layout.rs` alone, and every write goes through `atomic.rs` (`04-storage/03-writing-safely.md` §5).
//!
//! This module does not know the index, the driver or the editor, so it is tested with nothing but a temp folder.

pub(crate) mod atomic;
mod layout;
mod read;

use std::io;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, value};
use vnoc_domain::{LocationKey, NodeId, NodeToml};

pub use read::{DocInfo, LoadProblem, LoadReport, NodeRecord};

#[derive(Debug, thiserror::Error)]
pub enum UserDataError {
    #[error("could not write in .vnoc: {0}")]
    Io(#[from] io::Error),
    #[error(".vnoc/vnoc.toml is not valid: {0}")]
    Config(String),
    #[error(".vnoc/vnoc.toml has `format = {0}`, which is newer than this V-NOC: update V-NOC to change docs in this project")]
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

pub struct UserData {
    /// `<git root>/.vnoc`
    root: PathBuf,
}

impl UserData {
    pub fn new(git_root: &Path) -> UserData {
        UserData { root: git_root.join(layout::FOLDER) }
    }

    pub fn settings(&self) -> Result<Settings, UserDataError> {
        let file = layout::project_file(&self.root);
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Settings { initialized: false, markers: true }),
            Err(e) => return Err(e.into()),
        };
        let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| UserDataError::Config(e.to_string()))?;
        match doc.get("format").and_then(|f| f.as_integer()) {
            Some(1) => {}
            Some(newer) if newer > 1 => return Err(UserDataError::TooNew(newer)),
            _ => return Err(UserDataError::Config("`format = 1` is missing".into())),
        }
        let markers = doc.get("ids").and_then(|ids| ids.get("default")).and_then(|d| d.as_str()) != Some("none");
        Ok(Settings { initialized: true, markers })
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
        read::load_all(&self.root).records.into_iter().find(|(_, record)| record.toml.location().is_ok_and(|l| l == *location)).map(|(id, _)| id)
    }

    /// Creates the node folder and its `node.toml`. A folder that is already there is left as it is.
    pub fn create_node(&self, id: &NodeId, toml: &NodeToml) -> Result<(), UserDataError> {
        let file = layout::node_file(&self.root, id);
        if !file.exists() {
            atomic::write_atomic(&file, toml.to_string().as_bytes())?;
        }
        Ok(())
    }

    /// Creates a new Markdown doc in the node's folder and returns its path. `hint` is for people reading a pull request.
    pub fn create_doc(&self, id: &NodeId, title: &str, hint: &str) -> Result<PathBuf, UserDataError> {
        let folder = layout::node_dir(&self.root, id);
        let content = format!("---\nnode: {id}\nhint: {hint}\n---\n# {}\n\n", title.trim());
        let slug = slug(title);
        for n in 1.. {
            let name = if n == 1 { format!("{slug}.md") } else { format!("{slug}-{n}.md") };
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
        path.starts_with(layout::nodes_dir(&self.root)) && path.extension().is_some_and(|e| e == "md") && !path.components().any(|c| c.as_os_str() == "..")
    }

    /// Every node folder, read. Broken ones are reported, not fatal.
    pub fn load_all(&self) -> LoadReport {
        read::load_all(&self.root)
    }
}

/// A file name for a title that is legal on macOS, Linux and Windows (`04-storage/01-vnoc-folder.md` §3, "Docs").
fn slug(title: &str) -> String {
    const RESERVED: [&str; 22] = ["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];
    let cleaned: String = title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .map(|c| if c.is_whitespace() { '-' } else { c.to_ascii_lowercase() })
        .collect();
    let cleaned: String = cleaned.trim_matches(|c| c == '.' || c == '-').chars().take(60).collect();
    let cleaned = cleaned.trim_end_matches(['.', '-']).to_string();
    if cleaned.is_empty() || RESERVED.contains(&cleaned.as_str()) { "doc".to_string() } else { cleaned }
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
        LocationKey::new(RepoPath::new("src/orders.py").unwrap(), "OrderService.parse_order", 1).unwrap()
    }

    #[test]
    fn titles_become_legal_file_names() {
        assert_eq!(slug("Why we parse twice"), "why-we-parse-twice");
        assert_eq!(slug("What? <Where>: a/b"), "what-where-ab");
        assert_eq!(slug("  .hidden. "), "hidden");
        assert_eq!(slug("CON"), "doc");
        assert_eq!(slug("???"), "doc");
        assert_eq!(slug(&"x".repeat(100)).len(), 60);
        assert_eq!(slug("Caf\u{e9} notes"), "caf\u{e9}-notes", "other letters are kept");
    }

    #[test]
    fn nothing_exists_until_the_first_attach_and_then_the_project_is_set_up_once() {
        let data = fresh("init");
        assert_eq!(data.settings().unwrap(), Settings { initialized: false, markers: true });
        assert!(!data.root.exists());

        data.initialize(false).unwrap();
        assert_eq!(data.settings().unwrap(), Settings { initialized: true, markers: false });
        assert_eq!(std::fs::read_to_string(data.root.join(".gitignore")).unwrap(), "local/\n");

        data.initialize(true).unwrap(); // the first answer stays
        assert!(!data.settings().unwrap().markers);
    }

    #[test]
    fn a_project_from_a_newer_version_is_refused_and_unknown_keys_are_fine() {
        let data = fresh("format");
        std::fs::create_dir_all(&data.root).unwrap();
        std::fs::write(data.root.join("vnoc.toml"), "format = 2\n").unwrap();
        assert!(matches!(data.settings(), Err(UserDataError::TooNew(2))));
        std::fs::write(data.root.join("vnoc.toml"), "format = 1\nfuture = true\n[ids]\nstyle = \"comment\"\n").unwrap();
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
        assert_ne!(first_free_id(&location(), |id| data.is_taken(id)).unwrap(), id, "a taken ID is stepped past");

        let first = data.create_doc(&id, "Why we parse twice", "src/orders.py::OrderService.parse_order").unwrap();
        let second = data.create_doc(&id, "Why we parse twice", "src/orders.py::OrderService.parse_order").unwrap();
        assert_eq!((first.file_name().unwrap().to_str(), second.file_name().unwrap().to_str()), (Some("why-we-parse-twice.md"), Some("why-we-parse-twice-2.md")));
        let text = std::fs::read_to_string(&first).unwrap();
        assert_eq!(text, format!("---\nnode: {id}\nhint: src/orders.py::OrderService.parse_order\n---\n# Why we parse twice\n\n"));

        assert_eq!(data.find_node_at(&location()), Some(id));
        let report = data.load_all();
        assert!(report.problems.is_empty());
        let (found, record) = &report.records[0];
        assert_eq!(found, &id);
        assert_eq!(record.docs, ["why-we-parse-twice-2.md", "why-we-parse-twice.md"]);
        assert_eq!(record.toml.id_in_code(), Some(IdInCode::No));
    }

    #[test]
    fn a_broken_node_is_a_problem_and_the_others_still_load() {
        let data = fresh("broken");
        data.initialize(true).unwrap();
        let id = first_free_id(&location(), |_| false).unwrap();
        data.create_node(&id, &NodeToml::new(&location())).unwrap();
        let bad = first_free_id(&LocationKey::new(RepoPath::new("b.py").unwrap(), "g", 1).unwrap(), |_| false).unwrap();
        std::fs::create_dir_all(layout::node_dir(&data.root, &bad)).unwrap();
        std::fs::write(layout::node_file(&data.root, &bad), "not = [valid").unwrap();
        std::fs::create_dir_all(data.root.join("nodes/zz/not-an-id")).unwrap();

        let report = data.load_all();
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.problems.len(), 2, "{:?}", report.problems);
        let temp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(report.problems.iter().all(|p| p.text.starts_with(".vnoc/nodes/") && !p.text.contains(&temp)), "paths in the text are relative to the repo: {:?}", report.problems);
    }
}
