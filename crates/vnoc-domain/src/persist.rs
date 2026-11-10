//! `first_free_id`: picks the permanent [`NodeId`] of a node that has none. It hashes the location and steps past any ID that is
//! already taken. It does no I/O: writing the ID into the code (a conversation with the language driver and the editor) and
//! creating the node's files are the caller's job, in that order.

use crate::id::NodeId;
use crate::location::LocationKey;

/// A safety stop for the collision loop. Hitting it means the "is it taken?" check is broken.
pub const MAX_ATTEMPTS: u32 = 1000;

#[derive(Debug, thiserror::Error)]
#[error("no free node ID after {0} attempts: is the `is_taken` check broken?")]
pub struct NoFreeId(pub u32);

/// The ID for `location`: the first one that `is_taken` does not refuse. The same situation always gives the same answer.
///
/// `is_taken` is "a node folder with this ID exists" (in `nodes/` or in the trash). **After this the ID is opaque**: no code may
/// derive it again to check a node.
pub fn first_free_id(location: &LocationKey, is_taken: impl Fn(&NodeId) -> bool) -> Result<NodeId, NoFreeId> {
    (1..=MAX_ATTEMPTS).map(|attempt| NodeId::derive(location, attempt)).find(|id| !is_taken(id)).ok_or(NoFreeId(MAX_ATTEMPTS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::RepoPath;

    fn loc() -> LocationKey {
        LocationKey::new(RepoPath::new("src/a.py").unwrap(), "f", 1).unwrap()
    }

    #[test]
    fn it_steps_past_taken_ids_and_is_deterministic() {
        let first = first_free_id(&loc(), |_| false).unwrap();
        let second = first_free_id(&loc(), |id| *id == first).unwrap();
        let third = first_free_id(&loc(), |id| *id == first || *id == second).unwrap();
        assert!(first != second && second != third && first != third);
        // the same situation always gives the same answer
        assert_eq!(second, first_free_id(&loc(), |id| *id == first).unwrap());
    }

    #[test]
    fn a_broken_taken_check_stops() {
        assert!(matches!(first_free_id(&loc(), |_| true), Err(NoFreeId(MAX_ATTEMPTS))));
    }
}
