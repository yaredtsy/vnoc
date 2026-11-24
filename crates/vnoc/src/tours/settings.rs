//! `[tours]` in `.vnoc/vnoc.toml` (`docs/26-tours/01-on-disk.md` §12): what `finish` does by itself.
//!
//! ```toml
//! [tours]
//! ids    = "ask"      # write IDs for a tour's stops:               "always" | "ask" | "never"
//! commit = "ask"      # commit a tour's files so it can be pinned:  "always" | "ask" | "never"
//! ```
//!
//! Absent is `"ask"`. A value this version does not know is read as `"ask"` and reported once (in `problems`). The file is the project's, committed:
//! [`set`] changes one key with `toml_edit` and leaves every comment, every other table and every other key as they were. `commit` is read and set here
//! and says whether `finish` commits a tour's files so that it can be pinned (`finish.rs`, `pin.rs`).

use std::fmt;
use std::io;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::userdata::atomic::write_atomic;
use crate::userdata::layout;

/// What happens by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    /// it is done, and the output says what was written
    Always,
    /// it is not done; the output says what could be, and the agent asks the user
    Ask,
    /// it is not done and not offered
    Never,
}

impl Choice {
    pub fn parse(text: &str) -> Option<Choice> {
        match text {
            "always" => Some(Choice::Always),
            "ask" => Some(Choice::Ask),
            "never" => Some(Choice::Never),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Choice::Always => "always",
            Choice::Ask => "ask",
            Choice::Never => "never",
        }
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The two keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Ids,
    Commit,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Ids => "ids",
            Key::Commit => "commit",
        }
    }
}

/// Where a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// `[tours]` in `.vnoc/vnoc.toml` says it
    File,
    /// nothing says it, or what is said is not one of the three words
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    pub value: Choice,
    pub from: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub ids: Setting,
    pub commit: Setting,
    /// what was read as `"ask"` because it is not a value, one sentence each, in the order of the keys
    pub problems: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read .vnoc/vnoc.toml: {0}")]
    Io(#[from] io::Error),
    #[error(".vnoc/vnoc.toml is not TOML: {0}")]
    NotToml(String),
    #[error("this project has no .vnoc/vnoc.toml yet")]
    NoFile,
    #[error("`[tours]` in .vnoc/vnoc.toml is not a table")]
    NotATable,
}

const ASK: Setting = Setting {
    value: Choice::Ask,
    from: Origin::Default,
};

/// The one sentence for `ids = "always"` (the file's, or `--ids` of one call) in a project that writes no ID lines (`[ids] default = "none"`): the
/// setting asks for what the project forbids (01 §12). `None` for anything else.
pub fn does_nothing_here(ids: Choice, project_writes_ids: bool) -> Option<String> {
    (ids == Choice::Always && !project_writes_ids).then(|| {
        "ids = \"always\" does nothing here: this project writes no ID lines ([ids] default = \"none\")"
            .to_string()
    })
}

/// The settings of the project whose `.vnoc/` is `vnoc_root`. No file is the defaults; a file that is not TOML is an error.
pub fn read(vnoc_root: &Path) -> Result<Settings, SettingsError> {
    let text = match std::fs::read_to_string(layout::project_file(vnoc_root)) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(Settings {
                ids: ASK,
                commit: ASK,
                problems: Vec::new(),
            });
        }
        Err(e) => return Err(e.into()),
    };
    parse(&text)
}

fn parse(text: &str) -> Result<Settings, SettingsError> {
    let doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| SettingsError::NotToml(e.message().to_string()))?;
    let mut problems = Vec::new();
    let mut one = |key: Key| -> Setting {
        let Some(item) = doc.get("tours").and_then(|t| t.get(key.as_str())) else {
            return ASK;
        };
        match item.as_str().and_then(Choice::parse) {
            Some(value) => Setting {
                value,
                from: Origin::File,
            },
            None => {
                let said = match item.as_str() {
                    Some(text) => format!("\"{text}\""),
                    None => item.to_string().trim().to_string(),
                };
                problems.push(format!(
                    "[tours] {} = {said} in .vnoc/vnoc.toml is not \"always\", \"ask\" or \"never\": read as \"ask\"",
                    key.as_str()
                ));
                ASK
            }
        }
    };
    let ids = one(Key::Ids);
    let commit = one(Key::Commit);
    Ok(Settings {
        ids,
        commit,
        problems,
    })
}

/// Sets one key of `[tours]` and writes the file again (`write_atomic`). Everything else in the file stays as it was: comments, other tables, the order
/// of the keys, the white space. The caller holds the project's write lock.
pub fn set(vnoc_root: &Path, key: Key, choice: Choice) -> Result<(), SettingsError> {
    let file = layout::project_file(vnoc_root);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SettingsError::NoFile),
        Err(e) => return Err(e.into()),
    };
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| SettingsError::NotToml(e.message().to_string()))?;
    let table = match doc
        .entry("tours")
        .or_insert_with(|| Item::Table(Table::new()))
    {
        Item::Table(table) => table,
        _ => return Err(SettingsError::NotATable),
    };
    // the value is replaced and what is around it stays: the space before it and the comment after it
    let mut new = value(choice.as_str());
    if let (Some(old), Some(new_value)) = (
        table.get(key.as_str()).and_then(Item::as_value),
        new.as_value_mut(),
    ) {
        *new_value.decor_mut() = old.decor().clone();
    }
    table[key.as_str()] = new;
    write_atomic(&file, doc.to_string().as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_text(text: &str) -> Settings {
        parse(text).unwrap()
    }

    #[test]
    fn absent_is_ask_from_the_default_and_a_value_is_from_the_file() {
        for text in ["format = 1\n", "format = 1\n\n[tours]\n", ""] {
            let s = read_text(text);
            assert_eq!((s.ids, s.commit), (ASK, ASK), "{text:?}");
            assert!(s.problems.is_empty());
        }
        let s = read_text("format = 1\n[tours]\nids = \"always\"\ncommit = \"never\"\n");
        assert_eq!(
            (s.ids.value, s.ids.from, s.commit.value, s.commit.from),
            (Choice::Always, Origin::File, Choice::Never, Origin::File)
        );
        // one key given, the other not
        let s = read_text("[tours]\nids = \"ask\"\n");
        assert_eq!((s.ids.from, s.commit.from), (Origin::File, Origin::Default));
    }

    #[test]
    fn a_value_that_is_not_one_is_read_as_ask_and_said_once_per_key() {
        let s = read_text("[tours]\nids = \"sometimes\"\ncommit = 3\n");
        assert_eq!((s.ids, s.commit), (ASK, ASK));
        assert_eq!(
            s.problems,
            [
                "[tours] ids = \"sometimes\" in .vnoc/vnoc.toml is not \"always\", \"ask\" or \"never\": read as \"ask\"",
                "[tours] commit = 3 in .vnoc/vnoc.toml is not \"always\", \"ask\" or \"never\": read as \"ask\"",
            ]
        );
    }

    #[test]
    fn always_in_a_project_that_writes_no_ids_has_its_sentence() {
        assert_eq!(
            does_nothing_here(Choice::Always, false).as_deref(),
            Some(
                "ids = \"always\" does nothing here: this project writes no ID lines ([ids] default = \"none\")"
            )
        );
        for (choice, writes) in [
            (Choice::Always, true),
            (Choice::Ask, false),
            (Choice::Never, false),
        ] {
            assert_eq!(does_nothing_here(choice, writes), None);
        }
    }

    #[test]
    fn a_file_that_is_not_toml_is_an_error_and_no_file_is_the_defaults() {
        assert!(matches!(
            parse("this is = = not toml"),
            Err(SettingsError::NotToml(_))
        ));
        let dir = std::env::temp_dir().join(format!("vnoc-tours-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read(&dir).unwrap().ids, ASK);
        assert!(matches!(
            set(&dir, Key::Ids, Choice::Always),
            Err(SettingsError::NoFile)
        ));
    }

    #[test]
    fn set_changes_one_key_and_keeps_the_rest_of_the_file_byte_for_byte() {
        let dir = std::env::temp_dir().join(format!("vnoc-tours-set-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = layout::project_file(&dir);
        let cases: [(&str, &str); 4] = [
            // no [tours]: the table is added at the end, what was there is a prefix
            (
                "# the project\nformat = 1\n\n[ids]\ndefault = \"on-attach\"   # as ever\n\n[future]\nkey = [1, 2]\n",
                "# the project\nformat = 1\n\n[ids]\ndefault = \"on-attach\"   # as ever\n\n[future]\nkey = [1, 2]\n\n[tours]\nids = \"always\"\n",
            ),
            // the key is there: only its value changes, with its comment
            (
                "format = 1\n\n[tours]\nids    = \"ask\"   # for now\ncommit = \"never\"\n\n[other]\nx = 1\n",
                "format = 1\n\n[tours]\nids    = \"always\"   # for now\ncommit = \"never\"\n\n[other]\nx = 1\n",
            ),
            // the table is there without the key
            (
                "format = 1\n[tours]\ncommit = \"never\"\n",
                "format = 1\n[tours]\ncommit = \"never\"\nids = \"always\"\n",
            ),
            // no trailing newline
            ("format = 1", "format = 1\n\n[tours]\nids = \"always\"\n"),
        ];
        for (before, after) in cases {
            std::fs::write(&file, before).unwrap();
            set(&dir, Key::Ids, Choice::Always).unwrap();
            let written = std::fs::read_to_string(&file).unwrap();
            assert_eq!(written, after, "from {before:?}");
            assert_eq!(read(&dir).unwrap().ids.value, Choice::Always);
        }
        // a [tours] that is not a table
        std::fs::write(&file, "tours = 3\n").unwrap();
        assert!(matches!(
            set(&dir, Key::Ids, Choice::Always),
            Err(SettingsError::NotATable)
        ));
    }
}
