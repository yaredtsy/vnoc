//! `vnoc tour ids`: give the nodes a tour passes through an ID, so it follows them through renames and moves (`docs/26-tours/01-on-disk.md` §6).
//!
//! It is the attach flow without its last step: [`ensure_node`] writes the ID, the marker in the code and `node.toml`, and no doc. The tour's own
//! `rebase` record is written by the edit itself (`identity::attach::apply_edits`, [`super::rebase`]); this module adds the `ids` record, which is
//! what gives each stop its `node`.

use vnoc_domain::NodeId;

use super::check::Code;
use super::find::find;
use super::fold::Tour;
use super::record::{IdsRecord, Ordered, StopId};
use crate::identity::{AttachError, Env, MarkerWritten, ensure_node};
use crate::index::Kind;

/// What the tour has for one stop, before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// the node has an ID already (its docs, or its marker): it is recorded and nothing is written
    Known(NodeId),
    /// the node has none: `key` is the tree's id of the node, as `ensure_node` takes it
    Needs {
        key: String,
        /// relative to the opened folder, as `vnoc` prints paths
        path: String,
        kind: Kind,
    },
    /// the stop's node is not found now: nothing to give an ID to
    Gone,
}

/// The stops of `tour` that are to be looked at: the written ones, or those named (a planned stop is skipped, even named). Each with what is
/// there to do.
pub fn pick(code: &Code, tour: &Tour, named: &[StopId]) -> Vec<(StopId, Pick)> {
    let mut picks = Vec::new();
    for stop in tour.stops.iter().filter(|s| s.written) {
        if !named.is_empty() && !named.contains(&stop.id) {
            continue;
        }
        let pick = match find(code.index, code.project, code.tree, stop) {
            None => Pick::Gone,
            Some(node) => match node
                .node_id
                .as_deref()
                .and_then(|id| NodeId::parse(id).ok())
            {
                Some(id) => Pick::Known(id),
                None => Pick::Needs {
                    key: node.id.clone(),
                    path: node.path.clone(),
                    kind: node.kind,
                },
            },
        };
        picks.push((stop.id, pick));
    }
    picks
}

/// What happened to one stop.
#[derive(Debug)]
pub enum Outcome {
    /// it had an ID already
    Had(NodeId),
    /// the ID is written: a line in the file at `path`, or a `.vnoc-id` file in the folder
    Written {
        id: NodeId,
        path: String,
        kind: Kind,
        marker: MarkerWritten,
    },
    /// it has an ID now, but not in the code: the project or the file would not take a marker (`notice` says why, when it does)
    ByName {
        id: NodeId,
        notice: Option<String>,
    },
    Gone,
}

impl Outcome {
    /// The stop's ID, when it has one.
    pub fn id(&self) -> Option<NodeId> {
        match self {
            Outcome::Had(id) | Outcome::Written { id, .. } | Outcome::ByName { id, .. } => {
                Some(*id)
            }
            Outcome::Gone => None,
        }
    }
}

/// Writes the IDs the picks ask for, one node at a time, in the order of the tour, with the sink of `env`. The caller holds the project's write lock
/// and **not** the tours lock: each marker's edit takes it to rebase the tours. A stop whose node was given an ID by an earlier pick of the same
/// call (two stops on one node) is `Had`. Stops after a failure are not tried.
pub async fn write(
    env: &mut Env<'_>,
    picks: &[(StopId, Pick)],
) -> Result<Vec<(StopId, Outcome)>, AttachError> {
    let mut done: Vec<(StopId, Outcome)> = Vec::new();
    let mut seen: Vec<(&str, NodeId)> = Vec::new();
    for (stop, pick) in picks {
        let outcome = match pick {
            Pick::Gone => Outcome::Gone,
            Pick::Known(id) => Outcome::Had(*id),
            // the same node twice in one tour (a `same_as` stop): the index of this process does not know the first's ID yet, so the second is told
            Pick::Needs { key, .. } if seen.iter().any(|(k, _)| k == key) => {
                Outcome::Had(seen.iter().find(|(k, _)| k == key).expect("just found").1)
            }
            Pick::Needs { key, path, kind } => {
                let ensured = ensure_node(env, key, None, None, false, |_, _| Ok(())).await?;
                seen.push((key, ensured.id));
                match ensured.marker {
                    Some(marker) => Outcome::Written {
                        id: ensured.id,
                        path: match marker {
                            MarkerWritten::Line(_) => path.clone(),
                            MarkerWritten::FolderFile if path.is_empty() => {
                                crate::identity::folder_marker::FILE_NAME.to_string()
                            }
                            MarkerWritten::FolderFile => {
                                format!("{path}/{}", crate::identity::folder_marker::FILE_NAME)
                            }
                        },
                        kind: *kind,
                        marker,
                    },
                    None if ensured.id_in_code => Outcome::Had(ensured.id),
                    None => Outcome::ByName {
                        id: ensured.id,
                        notice: ensured.notice,
                    },
                }
            }
        };
        done.push((*stop, outcome));
    }
    Ok(done)
}

/// The `ids` record for what was done: every stop that has an ID (also those that had one), written **only when** some stop of the tour does not
/// have that ID as its `node` yet. So a second `ids` writes nothing.
pub fn record(tour: &Tour, done: &[(StopId, Outcome)]) -> Option<IdsRecord> {
    let nodes: Vec<(StopId, NodeId)> = done
        .iter()
        .filter_map(|(stop, outcome)| outcome.id().map(|id| (*stop, id)))
        .collect();
    let news = nodes
        .iter()
        .any(|(stop, id)| tour.stop(*stop).and_then(|s| s.node) != Some(*id));
    news.then_some(IdsRecord {
        nodes: Ordered(nodes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;

    const A: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";
    const B: &str = "3M1X9C7B5N2V8K4J6H0G1F3D5S";

    fn tour() -> Tour {
        let head =
            r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;
        let stop = |n: u32, node: &str| {
            format!(
                r#"{{"type":"stop","id":"s{n}","path":"a.py","qualname":"f{n}","kind":"function"{node},"lines":[1,2],"hash":"{n:016x}","text":"t"}}"#
            )
        };
        let text = format!(
            "{head}\n{}\n{}\n",
            stop(1, &format!(r#","node":"{A}""#)),
            stop(2, "")
        );
        fold(&text).unwrap()
    }

    #[test]
    fn a_record_is_written_when_some_stop_does_not_have_its_id_yet_and_has_every_stop() {
        let t = tour();
        let (a, b) = (NodeId::parse(A).unwrap(), NodeId::parse(B).unwrap());
        let s = |n| StopId::new(n).unwrap();
        // s1 had A already, s2 got B: one record with both
        let done = vec![(s(1), Outcome::Had(a)), (s(2), Outcome::Had(b))];
        let record = record(&t, &done).unwrap();
        let got: Vec<String> = record
            .nodes
            .0
            .iter()
            .map(|(stop, id)| format!("{stop}={id}"))
            .collect();
        assert_eq!(got, [format!("s1={A}"), format!("s2={B}")]);
        // nothing new: only s1, which has it
        assert!(super::record(&t, &[(s(1), Outcome::Had(a))]).is_none());
        // a stop that is gone has no entry, and a tour with nothing gives none
        assert!(super::record(&t, &[(s(2), Outcome::Gone)]).is_none());
        // a stop whose ID is another one than its node is news
        assert!(super::record(&t, &[(s(1), Outcome::Had(b))]).is_some());
    }
}
