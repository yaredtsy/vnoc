//! `persist`: the only way to get a [`NodeId`] for a node that has none. It hashes the location, steps past any ID
//! that is already taken, **writes the ID into the code**, and only then hands it back. If the write fails there is no ID.

use crate::id::NodeId;
use crate::location::LocationKey;

/// Writes an ID into the source of a node (the driver decides where; in the product this is a `WorkspaceEdit`).
pub trait IdWriter {
    fn write_id(&mut self, location: &LocationKey, id: &NodeId) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// A safety stop for the collision loop. Hitting it means the "is it taken?" check is broken.
pub const MAX_ATTEMPTS: u32 = 1000;

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("no free node ID after {0} attempts: is the `is_taken` check broken?")]
    NoFreeId(u32),
    #[error("could not write the ID into the code: {0}")]
    Write(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Makes the permanent ID of `location` and writes it into the code.
///
/// `is_taken` is VS-D71's check (the SQLite registry first, then the `nodes/<shard>/<id>/` folders on disk). The first
/// ID that is free is the one; the others were taken by other nodes. **The write comes before the return**, so a
/// `NodeId` that came out of here is already in the code.
pub fn persist(location: &LocationKey, is_taken: &dyn Fn(&NodeId) -> bool, writer: &mut dyn IdWriter) -> Result<NodeId, PersistError> {
    for attempt in 1..=MAX_ATTEMPTS {
        let id = NodeId::derive(location, attempt);
        if is_taken(&id) {
            continue;
        }
        writer.write_id(location, &id).map_err(PersistError::Write)?;
        return Ok(id);
    }
    Err(PersistError::NoFreeId(MAX_ATTEMPTS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::RepoPath;

    struct Recorder(Vec<(String, NodeId)>);
    impl IdWriter for Recorder {
        fn write_id(&mut self, l: &LocationKey, id: &NodeId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.0.push((l.to_string(), *id));
            Ok(())
        }
    }
    struct Failing;
    impl IdWriter for Failing {
        fn write_id(&mut self, _: &LocationKey, _: &NodeId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("the file does not parse".into())
        }
    }
    fn loc() -> LocationKey {
        LocationKey::new(RepoPath::new("src/a.py").unwrap(), "f", 1).unwrap()
    }

    #[test]
    fn the_id_is_written_before_it_is_returned() {
        let mut w = Recorder(vec![]);
        let id = persist(&loc(), &|_| false, &mut w).unwrap();
        assert_eq!(w.0, vec![("src/a.py::f".to_string(), id)]);
    }

    #[test]
    fn a_failed_write_gives_no_id() {
        assert!(matches!(persist(&loc(), &|_| false, &mut Failing), Err(PersistError::Write(_))));
    }

    #[test]
    fn it_steps_past_taken_ids_and_is_deterministic() {
        let first = persist(&loc(), &|_| false, &mut Recorder(vec![])).unwrap();
        let second = persist(&loc(), &|id| *id == first, &mut Recorder(vec![])).unwrap();
        let third = persist(&loc(), &|id| *id == first || *id == second, &mut Recorder(vec![])).unwrap();
        assert!(first != second && second != third && first != third);
        // the same situation always gives the same answer
        assert_eq!(second, persist(&loc(), &|id| *id == first, &mut Recorder(vec![])).unwrap());
    }

    #[test]
    fn a_broken_taken_check_stops() {
        assert!(matches!(persist(&loc(), &|_| true, &mut Recorder(vec![])), Err(PersistError::NoFreeId(MAX_ATTEMPTS))));
    }
}
