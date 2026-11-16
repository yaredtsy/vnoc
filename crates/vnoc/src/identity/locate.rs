//! Locate: where a function or class is in a document's current text (`docs/17-edit-in-canvas.md` §6).
//!
//! A draft in a card saves at the range it started from, unless the document changed since: then the host asks where the node is now. The text is the editor's
//! buffer, outlined by the driver; nothing is cached. The node is found the way a documented node is found again (`resolve`): by the ID written in its code
//! first (it follows a rename and a move), then by qualname and which same-named definition it is.

use std::sync::RwLock;

use vnoc_domain::{LocationKey, LocationKeyError, NodeId, occurrences};
use vnoc_driver_protocol::{Def, Range};

use crate::driver_host::DriverHost;
use crate::index::Index;
use crate::project::Project;
use crate::protocol::{LocateParams, Located};

#[derive(Debug, thiserror::Error)]
pub enum LocateError {
    #[error(transparent)]
    BadKey(#[from] LocationKeyError),
    #[error("`{0}` is not a function or a class: only a definition has a place to look for")]
    NotADef(String),
    #[error("the document is not the file of `{0}`")]
    WrongDocument(String),
    #[error("the driver could not read the text: {0}")]
    Driver(String),
}

/// Where `key` is in `defs` (the outline of the text), or `None`. `id` is the ID the index knows in the code of the node `key` named, if it has one.
///
/// 1. By ID: a def of the text that carries it. With several (a copy-paste), the one at the key's own place, else the first.
/// 2. By qualname: the def with the key's qualname and the key's occurrence (which same-named definition it is).
/// A node with an ID that the text no longer carries is looked for by name, as a documented node is: the ID may have been deleted.
pub fn locate_in(defs: &[Def], key: &LocationKey, id: Option<&NodeId>) -> Option<Range> {
    let which = occurrences(defs.iter().map(|d| d.qualname.as_str()));
    let at_key = |(d, n): &(&Def, &u32)| d.qualname == key.qualname() && **n == key.occurrence();
    let places: Vec<(&Def, &u32)> = defs.iter().zip(which.iter()).collect();
    if let Some(id) = id {
        let carrying: Vec<&(&Def, &u32)> = places
            .iter()
            .filter(|(d, _)| {
                d.id.as_deref().and_then(|t| NodeId::parse(t).ok()).as_ref() == Some(id)
            })
            .collect();
        if let Some(found) = carrying.iter().find(|p| at_key(p)).or(carrying.first()) {
            return Some(found.0.range);
        }
    }
    places.iter().find(|p| at_key(p)).map(|(d, _)| d.range)
}

/// `vnoc/locate`: refuses a key that names no definition and a document that is not the key's file. Outlines the document's text, and looks.
pub async fn locate(
    project: &Project,
    index: &RwLock<Index>,
    host: &DriverHost,
    params: LocateParams,
) -> Result<Option<Located>, LocateError> {
    if !params.key.contains("::") {
        return Err(LocateError::NotADef(params.key));
    }
    let key: LocationKey = params.key.parse()?;
    if params.document.uri.to_file_path().ok() != Some(project.git_root().join(key.path().as_str()))
    {
        return Err(LocateError::WrongDocument(params.key));
    }
    // the ID in the code of the node as the index last read it: the saved file; the text may have lost or kept it
    let id = index
        .read()
        .unwrap()
        .def_at(&key)
        .and_then(|d| d.id.as_deref().and_then(|t| NodeId::parse(t).ok()));
    let outline = host
        .outline_text(&params.document.text)
        .await
        .map_err(|e| LocateError::Driver(e.to_string()))?;
    Ok(locate_in(&outline.defs, &key, id.as_ref()).map(|range| Located { range }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vnoc_driver_protocol::{DefKind, Position};

    const ID_A: &str = "0000000000000000000000000A";
    const ID_B: &str = "0000000000000000000000000B";

    fn range(line: u32) -> Range {
        Range {
            start: Position { line, character: 0 },
            end: Position {
                line: line + 1,
                character: 10,
            },
        }
    }

    fn def(qualname: &str, line: u32, id: Option<&str>) -> Def {
        let name = qualname.rsplit('.').next().unwrap();
        Def {
            qualname: qualname.to_string(),
            name: name.to_string(),
            kind: DefKind::Function,
            parent: String::new(),
            parent_kind: None,
            range: range(line),
            params: vec![],
            id: id.map(str::to_string),
            syntax_error: false,
        }
    }

    fn key(text: &str) -> LocationKey {
        text.parse().unwrap()
    }

    fn id(text: &str) -> NodeId {
        NodeId::parse(text).unwrap()
    }

    #[test]
    fn found_by_qualname_where_it_moved_to() {
        let defs = [def("a", 20, None), def("b", 24, None)];
        assert_eq!(locate_in(&defs, &key("m.py::b"), None), Some(range(24)));
    }

    #[test]
    fn found_by_its_id_after_a_rename() {
        let defs = [def("renamed", 3, Some(ID_A)), def("b", 8, None)];
        assert_eq!(
            locate_in(&defs, &key("m.py::old"), Some(&id(ID_A))),
            Some(range(3))
        );
    }

    #[test]
    fn an_id_beats_a_qualname_that_is_now_another_function() {
        // `a` was renamed to `c`, and another function is called `a` now
        let defs = [def("a", 3, Some(ID_B)), def("c", 9, Some(ID_A))];
        assert_eq!(
            locate_in(&defs, &key("m.py::a"), Some(&id(ID_A))),
            Some(range(9))
        );
    }

    #[test]
    fn with_several_copies_of_an_id_the_one_at_the_key_wins_else_the_first() {
        let defs = [def("x", 1, Some(ID_A)), def("a", 5, Some(ID_A))];
        assert_eq!(
            locate_in(&defs, &key("m.py::a"), Some(&id(ID_A))),
            Some(range(5))
        );
        assert_eq!(
            locate_in(&defs, &key("m.py::gone"), Some(&id(ID_A))),
            Some(range(1))
        );
    }

    #[test]
    fn an_id_the_text_no_longer_carries_falls_back_to_the_name() {
        let defs = [def("a", 7, None)];
        assert_eq!(
            locate_in(&defs, &key("m.py::a"), Some(&id(ID_A))),
            Some(range(7))
        );
    }

    #[test]
    fn a_deleted_function_is_none() {
        let defs = [def("b", 2, None)];
        assert_eq!(locate_in(&defs, &key("m.py::a"), None), None);
        assert_eq!(locate_in(&defs, &key("m.py::a"), Some(&id(ID_A))), None);
        assert_eq!(locate_in(&[], &key("m.py::a"), None), None);
    }

    #[test]
    fn the_occurrence_says_which_same_named_definition() {
        let defs = [def("f", 1, None), def("f", 10, None)];
        assert_eq!(locate_in(&defs, &key("m.py::f"), None), Some(range(1)));
        assert_eq!(locate_in(&defs, &key("m.py::f#2"), None), Some(range(10)));
        assert_eq!(
            locate_in(&defs[..1], &key("m.py::f#2"), None),
            None,
            "the second twin was deleted"
        );
    }
}
