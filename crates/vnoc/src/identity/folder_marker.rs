//! The folder marker: a small `.vnoc-id` file inside a folder (`05-identity/04-folders.md` §2). It travels with the folder, so the
//! folder's docs follow it through renames and moves. It is TOML with one key, so a later version can add keys.

use std::io;
use std::path::Path;

use vnoc_domain::NodeId;

use crate::userdata::atomic;

/// The file's name, directly inside the folder it identifies.
pub const FILE_NAME: &str = ".vnoc-id";

/// What the file says, for the one who finds it.
pub fn render(id: &NodeId) -> String {
    format!("# V-NOC folder ID. It keeps this folder's docs attached when the folder is renamed or moved.\n# Safe to commit. Do not copy this file into another folder.\nid = \"{id}\"\n")
}

/// The ID in a `.vnoc-id` file. Other keys are ignored. `Err` says what is wrong, for the problems list.
pub fn parse(text: &str) -> Result<NodeId, String> {
    let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.message().to_string())?;
    let id = doc.get("id").and_then(|id| id.as_str()).ok_or("`id` (a string) is missing")?;
    NodeId::parse(id).map_err(|e| e.to_string())
}

/// Writes the marker into `folder`. A `.vnoc-id` that is already there is never overwritten (`AlreadyExists`): it may be one V-NOC
/// cannot read, and it is not ours to replace.
pub fn write(folder: &Path, id: &NodeId) -> io::Result<()> {
    atomic::create_new(&folder.join(FILE_NAME), render(id).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    #[test]
    fn what_is_written_is_read_back() {
        let id = NodeId::parse(ID).unwrap();
        assert_eq!(parse(&render(&id)), Ok(id));
    }

    #[test]
    fn unknown_keys_are_ignored_and_bad_content_says_why() {
        assert_eq!(parse(&format!("id = \"{ID}\"\nfuture = 1\n")).unwrap().to_string(), ID);
        assert!(parse("").unwrap_err().contains("missing"));
        assert!(parse("id = [").is_err());
        assert!(parse("id = \"not an id\"").is_err());
    }

    #[test]
    fn an_existing_file_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("vnoc-folder-marker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = NodeId::parse(ID).unwrap();
        write(&dir, &id).unwrap();
        assert_eq!(write(&dir, &id).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(parse(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()), Ok(id));
    }
}
