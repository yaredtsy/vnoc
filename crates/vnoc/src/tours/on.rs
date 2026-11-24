//! Which tours pass through a node (`docs/26-tours/03-core.md` §6): the list's second view, `vnoc tour list --on`.
//!
//! From what the tours **wrote**, as finding does (D6): a stop is on a node by the node's ID when the stop has one, else by its name, and a file
//! or folder by its path. No code is read and nothing is checked: a function with no ID that was renamed is not "on" its new name.

use std::collections::BTreeMap;

use vnoc_domain::{NodeId, NodeKind, RepoPath};

use super::find::place_of;
use super::fold::{Tour, TourStop};
use super::record::{StopId, TourId};
use crate::index::{Kind, Node};

/// The node asked about: what a tree node says of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// the node's permanent ID, when it has one
    pub id: Option<NodeId>,
    pub path: RepoPath,
    /// `""` for a file or a folder
    pub qualname: String,
    pub occurrence: u32,
    pub kind: NodeKind,
}

impl Query {
    /// What a tree node says of itself; `None` for the root of the tree, which has no repo path.
    pub fn of(node: &Node) -> Option<Query> {
        let (path, qualname, occurrence) = place_of(node)?;
        Some(Query {
            id: node
                .node_id
                .as_deref()
                .and_then(|id| NodeId::parse(id).ok()),
            path,
            qualname,
            occurrence,
            kind: match node.kind {
                Kind::Folder => NodeKind::Folder,
                Kind::File => NodeKind::File,
                Kind::Class => NodeKind::Class,
                Kind::Function => NodeKind::Function,
            },
        })
    }
}

/// How a tour is on the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// the tour's first stop is the node
    Starts,
    /// the node is these stops of the route (their numbers on screen, from 1)
    Stops(Vec<usize>),
    /// no stop is the node, and this many are inside it
    Inside(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct On {
    pub tour: TourId,
    /// the tour's place among the tours the map was made of
    pub at: usize,
    pub how: How,
    /// stops inside the node besides the ones that are it (`starts here · 4 stops inside`)
    pub inside: usize,
    /// the first stop that is the node, else the first inside it: where a click opens
    pub first: StopId,
    /// how many stops the route has, written or planned
    pub of: usize,
}

type Hit = (usize, usize); // (tour index, stop index)

/// The three maps of §6, built from replayed tours.
#[derive(Debug)]
pub struct Map<'t> {
    tours: Vec<&'t Tour>,
    by_id: BTreeMap<NodeId, Vec<Hit>>,
    /// `path::qualname#n`, for every stop
    by_name: BTreeMap<(String, String, u32), Vec<Hit>>,
    /// every stop by its path, sorted: a prefix walk finds "inside this file or folder"
    by_path: BTreeMap<String, Vec<Hit>>,
}

impl<'t> Map<'t> {
    pub fn of(tours: &[&'t Tour]) -> Map<'t> {
        let mut map = Map {
            tours: tours.to_vec(),
            by_id: BTreeMap::new(),
            by_name: BTreeMap::new(),
            by_path: BTreeMap::new(),
        };
        for (t, tour) in tours.iter().enumerate() {
            for (s, stop) in tour.stops.iter().enumerate() {
                if let Some(id) = stop.node {
                    map.by_id.entry(id).or_default().push((t, s));
                }
                map.by_name
                    .entry((
                        stop.path.to_string(),
                        stop.qualname.clone(),
                        stop.occurrence,
                    ))
                    .or_default()
                    .push((t, s));
                map.by_path
                    .entry(stop.path.to_string())
                    .or_default()
                    .push((t, s));
            }
        }
        map
    }

    fn stop(&self, (t, s): Hit) -> &TourStop {
        &self.tours[t].stops[s]
    }

    /// Every tour that passes through the node, in the order the tours were given.
    pub fn on(&self, query: &Query) -> Vec<On> {
        let mut direct: Vec<Hit> = Vec::new();
        let mut inside: Vec<Hit> = Vec::new();
        match query.kind {
            NodeKind::Folder => {
                let path = query.path.as_str();
                for (key, hits) in &self.by_path {
                    for hit in hits {
                        let stop = self.stop(*hit);
                        if key == path && stop.qualname.is_empty() {
                            direct.push(*hit);
                        } else if key == path || key.starts_with(&format!("{path}/")) {
                            inside.push(*hit);
                        }
                    }
                }
            }
            NodeKind::File => {
                for hit in self.by_path.get(query.path.as_str()).into_iter().flatten() {
                    if self.stop(*hit).qualname.is_empty() {
                        direct.push(*hit);
                    } else {
                        inside.push(*hit);
                    }
                }
                self.also_by_id(query, &mut direct);
            }
            _ => {
                let name = (
                    query.path.to_string(),
                    query.qualname.clone(),
                    query.occurrence,
                );
                direct.extend(self.by_name.get(&name).into_iter().flatten());
                self.also_by_id(query, &mut direct);
                // a class: the names inside it
                if query.kind == NodeKind::Class {
                    let prefix = format!("{}.", query.qualname);
                    for hit in self.by_path.get(query.path.as_str()).into_iter().flatten() {
                        if self.stop(*hit).qualname.starts_with(&prefix) {
                            inside.push(*hit);
                        }
                    }
                }
            }
        }
        direct.sort_unstable();
        direct.dedup();
        inside.sort_unstable();
        inside.dedup();
        inside.retain(|hit| !direct.contains(hit));

        let mut tours: Vec<usize> = direct.iter().chain(&inside).map(|(t, _)| *t).collect();
        tours.sort_unstable();
        tours.dedup();
        tours
            .into_iter()
            .map(|t| {
                let mine: Vec<usize> = direct.iter().filter(|h| h.0 == t).map(|h| h.1).collect();
                let within: Vec<usize> = inside.iter().filter(|h| h.0 == t).map(|h| h.1).collect();
                let tour = self.tours[t];
                let how = match (mine.first(), within.is_empty()) {
                    (Some(0), _) => How::Starts,
                    (Some(_), _) => How::Stops(mine.iter().map(|s| s + 1).collect()),
                    (None, _) => How::Inside(within.len()),
                };
                let first = mine.first().or(within.first()).copied().unwrap_or(0);
                On {
                    tour: tour.id.clone(),
                    at: t,
                    how,
                    inside: if mine.is_empty() { 0 } else { within.len() },
                    first: tour.stops[first].id,
                    of: tour.stops.len(),
                }
            })
            .collect()
    }

    /// The stops that carry the node's ID, whatever name they were written with.
    fn also_by_id(&self, query: &Query, direct: &mut Vec<Hit>) {
        if let Some(id) = query.id {
            direct.extend(self.by_id.get(&id).into_iter().flatten());
        }
    }
}

/// Line 2 of a tour's row (02 §6): `starts here`, `stop 3 of 10`, `stops 2 and 4 of 10`, `4 stops inside`; joined by ` · `.
pub fn words(on: &On) -> String {
    let mut parts = Vec::new();
    match &on.how {
        How::Starts => parts.push("starts here".to_string()),
        How::Stops(numbers) => {
            let list: Vec<String> = numbers.iter().map(ToString::to_string).collect();
            parts.push(match list.as_slice() {
                [one] => format!("stop {one} of {}", on.of),
                [init @ .., last] => {
                    let init = init.join(", ");
                    format!("stops {init} and {last} of {}", on.of)
                }
                [] => String::new(),
            });
        }
        How::Inside(n) => parts.push(inside_words(*n)),
    }
    if on.inside > 0 {
        parts.push(inside_words(on.inside));
    }
    parts.join(" · ")
}

fn inside_words(n: usize) -> String {
    format!("{n} {} inside", if n == 1 { "stop" } else { "stops" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;

    const HEAD: &str =
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;
    const ID: &str = "0Q8V5W4T2F7KX9M3B6D1RZ4H7K";

    fn stop(n: u32, path: &str, qualname: &str, kind: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"stop","id":"s{n}","path":"{path}","qualname":"{qualname}","kind":"{kind}"{extra},"text":"t"}}"#
        )
    }

    fn tour(id: &str, stops: &[String]) -> Tour {
        let head = HEAD.replace("t-0000", id);
        fold(&format!("{head}\n{}\n", stops.join("\n"))).unwrap()
    }

    fn query(path: &str, qualname: &str, kind: NodeKind, id: Option<&str>) -> Query {
        Query {
            id: id.map(|i| NodeId::parse(i).unwrap()),
            path: RepoPath::new(path).unwrap(),
            qualname: qualname.to_string(),
            occurrence: 1,
            kind,
        }
    }

    fn said(map: &Map, q: &Query) -> Vec<(String, String)> {
        map.on(q)
            .iter()
            .map(|on| (on.tour.to_string(), words(on)))
            .collect()
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn a_function_is_on_by_its_name_or_its_id_and_the_tours_say_where() {
        let a = tour(
            "a-0000",
            &[
                stop(1, "x.py", "run", "function", ""),
                stop(2, "x.py", "helper", "function", ""),
                stop(3, "y.py", "other", "function", ""),
                stop(4, "x.py", "helper", "function", r#","same_as":"s2""#),
            ],
        );
        // written before the function had an ID, and under a name it no longer has
        let b = tour(
            "b-0000",
            &[
                stop(
                    1,
                    "z.py",
                    "old_name",
                    "function",
                    &format!(r#","node":"{ID}""#),
                ),
                stop(2, "x.py", "helper", "function", ""),
            ],
        );
        let map = Map::of(&[&a, &b]);
        assert_eq!(
            said(&map, &query("x.py", "run", NodeKind::Function, None)),
            pairs(&[("a-0000", "starts here")])
        );
        assert_eq!(
            said(&map, &query("x.py", "helper", NodeKind::Function, None)),
            pairs(&[("a-0000", "stops 2 and 4 of 4"), ("b-0000", "stop 2 of 2")])
        );
        // by the ID: any name, any file
        assert_eq!(
            said(
                &map,
                &query("w.py", "new_name", NodeKind::Function, Some(ID))
            ),
            pairs(&[("b-0000", "starts here")])
        );
        // the same function without its ID, under its new name: not on
        assert!(said(&map, &query("w.py", "new_name", NodeKind::Function, None)).is_empty());
        let first = &map.on(&query("x.py", "helper", NodeKind::Function, None))[0];
        assert_eq!(first.first.to_string(), "s2");
    }

    #[test]
    fn three_stops_are_said_with_commas_and_a_class_has_the_names_inside_it() {
        let a = tour(
            "a-0000",
            &[
                stop(1, "m.py", "Box.one", "function", ""),
                stop(2, "m.py", "Box.two", "function", ""),
                stop(3, "m.py", "Boxes", "function", ""),
                stop(4, "m.py", "Box.one", "function", ""),
            ],
        );
        let b = tour(
            "b-0000",
            &[
                stop(1, "m.py", "Box", "class", ""),
                stop(2, "m.py", "Box.one", "function", ""),
            ],
        );
        let map = Map::of(&[&a, &b]);
        let class = query("m.py", "Box", NodeKind::Class, None);
        assert_eq!(
            said(&map, &class),
            pairs(&[
                ("a-0000", "3 stops inside"),
                ("b-0000", "starts here · 1 stop inside")
            ]),
            "`Boxes` is not inside `Box`"
        );
        let one = query("m.py", "Box.one", NodeKind::Function, None);
        assert_eq!(
            said(&map, &one),
            pairs(&[("a-0000", "starts here"), ("b-0000", "stop 2 of 2")]),
            "stop 1 is the first stop of a, so it starts there; stop 4 is not said"
        );
        let many = tour(
            "c-0000",
            &(1..=4)
                .map(|n| stop(n, "m.py", "f", "function", ""))
                .collect::<Vec<_>>(),
        );
        let map = Map::of(&[&many]);
        assert_eq!(
            said(&map, &query("m.py", "f", NodeKind::Function, None)),
            pairs(&[("c-0000", "starts here")])
        );
        let spread = tour(
            "d-0000",
            &[
                stop(1, "m.py", "a", "function", ""),
                stop(2, "m.py", "f", "function", ""),
                stop(3, "m.py", "f", "function", ""),
                stop(4, "m.py", "f", "function", ""),
            ],
        );
        let map = Map::of(&[&spread]);
        assert_eq!(
            said(&map, &query("m.py", "f", NodeKind::Function, None)),
            pairs(&[("d-0000", "stops 2, 3 and 4 of 4")])
        );
    }

    #[test]
    fn a_file_and_a_folder_have_the_paths_at_or_under_them() {
        let a = tour(
            "a-0000",
            &[
                stop(1, "app", "", "folder", ""),
                stop(2, "app/m.py", "", "file", ""),
                stop(3, "app/m.py", "run", "function", ""),
                stop(4, "app/sub/n.py", "go", "function", ""),
                stop(5, "apple/z.py", "z", "function", ""),
            ],
        );
        let b = tour("b-0000", &[stop(1, "app/m.py", "run", "function", "")]);
        let map = Map::of(&[&a, &b]);
        assert_eq!(
            said(&map, &query("app/m.py", "", NodeKind::File, None)),
            pairs(&[
                ("a-0000", "stop 2 of 5 · 1 stop inside"),
                ("b-0000", "1 stop inside")
            ])
        );
        assert_eq!(
            said(&map, &query("app", "", NodeKind::Folder, None)),
            pairs(&[
                ("a-0000", "starts here · 3 stops inside"),
                ("b-0000", "1 stop inside")
            ]),
            "`apple/` is not under `app`"
        );
        assert_eq!(
            said(&map, &query("app/sub", "", NodeKind::Folder, None)),
            pairs(&[("a-0000", "1 stop inside")])
        );
        assert!(said(&map, &query("nowhere", "", NodeKind::Folder, None)).is_empty());
    }

    #[test]
    fn a_planned_stop_counts_and_the_second_occurrence_is_its_own_name() {
        let a = tour(
            "a-0000",
            &[
                r#"{"type":"plan","stops":[{"id":"s1","path":"x.py","qualname":"f","occurrence":2}]}"#
                    .to_string(),
            ],
        );
        let map = Map::of(&[&a]);
        let mut q = query("x.py", "f", NodeKind::Function, None);
        assert!(map.on(&q).is_empty());
        q.occurrence = 2;
        assert_eq!(said(&map, &q), pairs(&[("a-0000", "starts here")]));
    }
}
