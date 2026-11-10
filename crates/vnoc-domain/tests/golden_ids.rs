//! The public way of making an ID (`first_free_id`) against the values an independent oracle computed.

#[allow(dead_code)] // this binary uses only part of the table
mod golden_data;

use std::collections::HashSet;

use golden_data::ID_CASES;
use vnoc_domain::{LocationKey, NodeId, RepoPath, first_free_id};

#[test]
fn first_free_id_gives_the_oracles_ids_including_collision_counters() {
    for (path, qualname, occurrence, counter, expected) in ID_CASES {
        let loc = LocationKey::new(RepoPath::new(*path).unwrap(), *qualname, *occurrence).unwrap();
        // "counter c" means: attempts 1..c are taken. Their ids are the earlier rows of the same location in the table.
        let taken: HashSet<String> = ID_CASES
            .iter()
            .filter(|(p, q, o, c, _)| p == path && q == qualname && o == occurrence && c < counter)
            .map(|(.., id)| id.to_string())
            .collect();
        let id = first_free_id(&loc, |id| taken.contains(&id.to_string())).unwrap();
        assert_eq!(id.to_string(), *expected, "{path}::{qualname}#{occurrence} counter {counter}");
    }
}

#[test]
fn the_nfc_and_nfd_spellings_of_one_path_are_two_different_ids() {
    let nfc = ID_CASES.iter().find(|c| c.0 == "caf\u{e9}/\u{e9}.py").unwrap().4;
    let nfd = ID_CASES.iter().find(|c| c.0 == "cafe\u{301}/e\u{301}.py").unwrap().4;
    assert_ne!(nfc, nfd, "the id hashes the exact bytes of the path as git records it");
}

#[test]
fn every_oracle_id_reads_back_and_has_a_shard() {
    for (.., expected) in ID_CASES {
        let id = NodeId::parse(expected).unwrap();
        assert_eq!(id.to_string(), *expected);
        assert_eq!(id.shard().len(), 2);
        assert_eq!(id.shard(), expected[24..]);
    }
}
