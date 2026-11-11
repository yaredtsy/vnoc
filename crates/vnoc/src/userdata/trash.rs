//! Remove and Restore, as plans (`docs/11-missing-relink-remove.md` §5). The core says **where** a node folder moves; VS Code moves it (an open doc
//! follows the move). Nothing here moves or deletes a node folder.
//!
//! `trashed.toml` is written before the move. A crash between the two leaves an **empty entry** (a `trashed.toml` and no node folder) and the node
//! where it was: nothing is lost, and the next listing removes the entry. `.vnoc/local/` is git-ignored but it is a plain file anyone can edit, so an
//! entry is trusted only when its folder name, the `id` in `trashed.toml` and the folder inside all agree, and where a node goes back to is worked
//! out from the ID alone.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toml_edit::{DocumentMut, value};
use vnoc_domain::NodeId;

use super::{UserData, UserDataError, atomic, layout, read};

#[derive(Debug, thiserror::Error)]
pub enum TrashError {
    #[error("that node has no docs folder to remove")]
    NothingToRemove,
    #[error("this node already has docs: restoring would overwrite them")]
    AlreadyThere,
    #[error("`{0}` is not in the trash")]
    NoSuchEntry(String),
    #[error(transparent)]
    UserData(#[from] UserDataError),
}

/// Where a node folder moves from and to (absolute paths).
#[derive(Debug, PartialEq, Eq)]
pub struct MovePlan {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// One removed node that can be restored.
#[derive(Debug, PartialEq, Eq)]
pub struct TrashEntry {
    /// the entry's folder name: `<YYYYMMDDTHHMMSS>_<id>`
    pub entry: String,
    pub id: NodeId,
    /// UTC, `2026-11-11T10:30:00Z`
    pub removed_at: String,
    pub docs: usize,
    /// the place the node was at, as its `node.toml` says: for the list the user picks from
    pub name: String,
}

impl UserData {
    /// Step 1 of Remove: writes the entry's `trashed.toml`, and says where the node folder moves. Does not move it.
    /// The caller has checked that the node is Missing.
    pub fn trash_plan(&self, id: &NodeId) -> Result<MovePlan, TrashError> {
        let from = layout::node_dir(&self.root, id);
        if !from.is_dir() {
            return Err(TrashError::NothingToRemove);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        // two removals of the same ID in one second must not share an entry: take the next free second
        let (entry, stamp) = (0..).map(|n| (stamp(now + n), iso(now + n))).map(|(s, i)| (format!("{s}_{id}"), i)).find(|(e, _)| !layout::trash_dir(&self.root).join(e).exists()).expect("a free second");
        let folder = layout::trash_dir(&self.root).join(&entry);
        let mut doc = DocumentMut::new();
        doc["id"] = value(id.to_string());
        doc["removed_at"] = value(stamp);
        atomic::write_atomic(&folder.join("trashed.toml"), doc.to_string().as_bytes()).map_err(UserDataError::from)?;
        let to = folder.join("nodes").join(id.shard()).join(id.to_string());
        // the move (`WorkspaceEdit.renameFile`) may not make parent folders: make them, empty
        fs::create_dir_all(to.parent().expect("a node folder has a parent")).map_err(UserDataError::from)?;
        Ok(MovePlan { from, to })
    }

    /// The entries that can be restored, newest first. Empty entries are removed here; an entry that disagrees with itself is skipped and left alone.
    pub fn trash_list(&self) -> Vec<TrashEntry> {
        let trash = layout::trash_dir(&self.root);
        let Ok(read_dir) = fs::read_dir(&trash) else { return vec![] };
        let mut entries = Vec::new();
        for folder in read_dir.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
            let name = folder.file_name().to_string_lossy().into_owned();
            match inspect(&folder.path(), &name) {
                Inspected::Empty => {
                    let _ = fs::remove_dir_all(folder.path()); // nothing in it: a removal that stopped halfway
                }
                Inspected::Entry(entry) => entries.push(entry),
                Inspected::Skip => tracing::warn!("trash entry {name} disagrees with itself: skipped"),
            }
        }
        entries.sort_by(|a, b| b.entry.cmp(&a.entry));
        entries
    }

    /// Step 2 of Restore: where the node folder in this entry goes back to, worked out from its ID. Refuses when that folder exists.
    pub fn restore_plan(&self, entry: &str) -> Result<MovePlan, TrashError> {
        // the name comes from the UI: only an entry the listing just vouched for is a place to move from
        let found = self.trash_list().into_iter().find(|e| e.entry == entry).ok_or_else(|| TrashError::NoSuchEntry(entry.to_string()))?;
        let from = layout::trash_dir(&self.root).join(&found.entry).join("nodes").join(found.id.shard()).join(found.id.to_string());
        let to = layout::node_dir(&self.root, &found.id);
        if to.exists() {
            return Err(TrashError::AlreadyThere);
        }
        fs::create_dir_all(to.parent().expect("a node folder has a parent")).map_err(UserDataError::from)?;
        Ok(MovePlan { from, to })
    }

    /// For Empty Trash: the trash folder, how many entries are in it and how many bytes. The listing runs first (it removes the empty entries, so they are
    /// not counted); then every folder counts, the skipped ones too, because Empty Trash deletes the whole folder. Links are not followed.
    pub fn trash_info(&self) -> (PathBuf, usize, u64) {
        let folder = layout::trash_dir(&self.root);
        self.trash_list();
        let entries = fs::read_dir(&folder).map_or(0, |dir| dir.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).count());
        (folder.clone(), entries, bytes_under(&folder))
    }
}

/// The sizes of the files under `dir`, added up. `symlink_metadata`: a link is its own few bytes, not what it points to.
fn bytes_under(dir: &Path) -> u64 {
    let Ok(read_dir) = fs::read_dir(dir) else { return 0 };
    read_dir
        .flatten()
        .map(|e| match e.path().symlink_metadata() {
            Ok(m) if m.is_dir() => bytes_under(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

enum Inspected {
    Empty,
    Entry(TrashEntry),
    Skip,
}

fn inspect(folder: &Path, name: &str) -> Inspected {
    // the name is `<stamp>_<id>`
    let Some((stamp, id_text)) = name.split_once('_') else { return Inspected::Skip };
    let Some(id) = NodeId::parse(id_text).ok().filter(|id| id.to_string() == id_text) else { return Inspected::Skip };
    if stamp.len() != 15 || !stamp.chars().all(|c| c.is_ascii_digit() || c == 'T') {
        return Inspected::Skip;
    }
    let Ok(text) = fs::read_to_string(folder.join("trashed.toml")) else { return Inspected::Skip };
    let Ok(doc) = text.parse::<DocumentMut>() else { return Inspected::Skip };
    if doc.get("id").and_then(|v| v.as_str()) != Some(id_text) {
        return Inspected::Skip;
    }
    let node = folder.join("nodes").join(id.shard()).join(id_text);
    if !node.is_dir() {
        // nothing but `trashed.toml` (and the shard folder made for the move)
        let only_scaffolding = fs::read_dir(folder).map(|d| d.flatten().all(|e| e.file_name() == "trashed.toml" || e.file_name() == "nodes")).unwrap_or(false);
        return if only_scaffolding { Inspected::Empty } else { Inspected::Skip };
    }
    let removed_at = doc.get("removed_at").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let docs = read::markdown_files(&node).len();
    let name = fs::read_to_string(node.join("node.toml")).ok().and_then(|t| vnoc_domain::NodeToml::parse(&t).ok()).and_then(|t| t.location().ok()).map(|l| if l.qualname().is_empty() { l.path().to_string() } else { l.qualname().to_string() }).unwrap_or_else(|| id_text.to_string());
    Inspected::Entry(TrashEntry { entry: name_of(folder), id, removed_at, docs, name })
}

fn name_of(folder: &Path) -> String {
    folder.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

/// `20261111T103000`
fn stamp(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}")
}

/// `2026-11-11T10:30:00Z`
fn iso(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// UTC date and time of a Unix time (days since 1970 → civil date, after Howard Hinnant's `civil_from_days`).
fn civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d, rem / 3_600, rem % 3_600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_domain::{LocationKey, NodeToml, RepoPath};

    fn fresh(label: &str) -> UserData {
        let d = std::env::temp_dir().join(format!("vnoc-trash-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        UserData::new(&d)
    }

    fn node(data: &UserData, n: u8) -> NodeId {
        let id = NodeId::parse(&format!("0000000000000000000000000{n}")).unwrap();
        let at = LocationKey::new(RepoPath::new("a.py").unwrap(), format!("f{n}"), 1).unwrap();
        data.create_node(&id, &NodeToml::new(&at)).unwrap();
        data.create_doc(&id, "Notes", "a.py").unwrap();
        id
    }

    /// What VS Code does with a plan.
    fn mv(plan: &MovePlan) {
        fs::rename(&plan.from, &plan.to).unwrap();
    }

    #[test]
    fn trash_info_counts_the_entries_and_their_bytes_and_drops_the_empty_ones() {
        let data = fresh("info");
        let (folder, entries, bytes) = data.trash_info();
        assert_eq!((entries, bytes), (0, 0), "no trash folder at all");
        assert!(folder.ends_with("local/trash"), "{folder:?}");

        let id = node(&data, 1);
        data.trash_plan(&id).unwrap();
        let (_, entries, _) = data.trash_info();
        assert_eq!(entries, 0, "an entry with no node folder in it is an empty one: it is removed, not counted");

        let id = node(&data, 2);
        let plan = data.trash_plan(&id).unwrap();
        mv(&plan);
        let (_, entries, bytes) = data.trash_info();
        assert_eq!(entries, 1);
        assert!(bytes > 0, "the node.toml, the doc and trashed.toml");
    }

    #[test]
    fn dates_are_right() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(stamp(1_782_000_000), "20260621T000000", "a date after a leap year");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z", "a leap day");
    }

    #[test]
    fn remove_writes_the_entry_first_and_plans_the_move_and_restore_plans_it_back_from_the_id() {
        let data = fresh("plan");
        let id = node(&data, 1);
        let before = fs::read(layout::node_dir(&data.root, &id).join("notes.md")).unwrap();

        let plan = data.trash_plan(&id).unwrap();
        assert_eq!(plan.from, layout::node_dir(&data.root, &id));
        assert!(plan.from.is_dir(), "the core moves nothing");
        let entry = plan.to.ancestors().nth(3).unwrap().file_name().unwrap().to_string_lossy().into_owned();
        assert!(entry.ends_with(&format!("_{id}")), "{entry}");
        assert_eq!(plan.to, layout::trash_dir(&data.root).join(&entry).join("nodes").join(id.shard()).join(id.to_string()));
        let trashed = fs::read_to_string(layout::trash_dir(&data.root).join(&entry).join("trashed.toml")).unwrap();
        assert!(trashed.contains(&format!("id = \"{id}\"")) && trashed.contains("removed_at = \""), "{trashed}");

        mv(&plan);
        assert!(!layout::node_dir(&data.root, &id).exists());
        assert!(data.is_taken(&id), "a removed ID is never reused");
        let list = data.trash_list();
        assert_eq!((list.len(), list[0].id, list[0].docs, list[0].name.as_str()), (1, id, 1, "f1"));

        let back = data.restore_plan(&list[0].entry).unwrap();
        assert_eq!(back.to, layout::node_dir(&data.root, &id), "worked out from the ID");
        mv(&back);
        assert_eq!(fs::read(layout::node_dir(&data.root, &id).join("notes.md")).unwrap(), before, "the same bytes");
        assert!(data.trash_list().is_empty(), "the emptied entry goes at the next listing");
        assert!(!layout::trash_dir(&data.root).join(&entry).exists());
    }

    #[test]
    fn a_removal_that_stopped_before_the_move_leaves_an_empty_entry_and_the_node_untouched() {
        let data = fresh("empty");
        let id = node(&data, 1);
        let plan = data.trash_plan(&id).unwrap(); // the rename never happens
        assert!(plan.from.is_dir());
        assert!(data.trash_list().is_empty(), "an empty entry is not offered");
        assert!(fs::read_dir(layout::trash_dir(&data.root)).unwrap().next().is_none(), "and it is removed by the listing");
        assert!(plan.from.join("notes.md").is_file(), "the node is where it was");
    }

    #[test]
    fn restore_never_overwrites_and_the_same_id_can_have_two_entries() {
        let data = fresh("twice");
        let id = node(&data, 1);
        let first = data.trash_plan(&id).unwrap();
        mv(&first);
        let entry1 = data.trash_list()[0].entry.clone();
        mv(&data.restore_plan(&entry1).unwrap());
        let second = data.trash_plan(&id).unwrap();
        mv(&second);
        // remove, restore, remove again: and a node with the same ID is back in `nodes/` meanwhile
        let entries = data.trash_list();
        assert_eq!(entries.len(), 1);
        fs::create_dir_all(layout::node_dir(&data.root, &id)).unwrap();
        assert!(matches!(data.restore_plan(&entries[0].entry), Err(TrashError::AlreadyThere)));
        assert_eq!(data.trash_list().len(), 1, "the entry is left alone");
    }

    #[test]
    fn two_removals_in_one_second_get_two_entries() {
        let data = fresh("same-second");
        let id = node(&data, 1);
        let first = data.trash_plan(&id).unwrap();
        let second = data.trash_plan(&id).unwrap();
        assert_ne!(first.to, second.to);
    }

    #[test]
    fn an_entry_that_disagrees_with_itself_is_skipped_and_left_alone_and_a_name_that_is_not_listed_is_refused() {
        let data = fresh("liar");
        let (a, b) = (node(&data, 1), node(&data, 2));
        let plan = data.trash_plan(&a).unwrap();
        mv(&plan);
        let entry = data.trash_list()[0].entry.clone();
        // the `id` in trashed.toml says another node
        let file = layout::trash_dir(&data.root).join(&entry).join("trashed.toml");
        fs::write(&file, format!("id = \"{b}\"\nremoved_at = \"x\"\n")).unwrap();
        assert!(data.trash_list().is_empty());
        assert!(layout::trash_dir(&data.root).join(&entry).exists(), "skipped, not deleted");
        assert!(matches!(data.restore_plan(&entry), Err(TrashError::NoSuchEntry(_))));
        // a name that climbs out of the trash, and one that was never there
        assert!(matches!(data.restore_plan("../../nodes"), Err(TrashError::NoSuchEntry(_))));
        assert!(matches!(data.restore_plan("nope"), Err(TrashError::NoSuchEntry(_))));
        // a folder name that is not `<stamp>_<id>`
        fs::create_dir_all(layout::trash_dir(&data.root).join("whatever")).unwrap();
        assert!(data.trash_list().is_empty());
        assert!(layout::trash_dir(&data.root).join("whatever").exists());
    }

    #[test]
    fn removing_what_is_not_there_is_refused() {
        let data = fresh("nothing");
        let id = NodeId::parse("0000000000000000000000000Z").unwrap();
        assert!(matches!(data.trash_plan(&id), Err(TrashError::NothingToRemove)));
        assert!(!layout::trash_dir(&data.root).exists(), "and no entry was made");
    }
}
