//! Which tours pass through a node (`docs/26-tours/03-core.md` §6): the list's second view, `vnoc tour list --on`.
//!
//! From what the tours **wrote**, as finding does (D6): a stop is on a node by the node's ID when the stop has one, else by its name, and a file
//! or folder by its path. No code is read and nothing is checked: a function with no ID that was renamed is not "on" its new name.

use std::collections::BTreeMap;

use vnoc_domain::{NodeId, NodeKind, RepoPath};

use super::find::place_of;
use super::fold::Tour;
use super::record::{StopId, TourId};
use super::store::Where;
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
    /// the folder of the tour's file: the same id can be in both (the private copy is its own row)
    pub place: Where,
    pub how: How,
    /// stops inside the node besides the ones that are it (`starts here · 4 stops inside`)
    pub inside: usize,
    /// the first stop that is the node, else the first inside it: where a click opens
    pub first: StopId,
    /// how many stops the route has, written or planned
    pub of: usize,
}

/// What `list --on` says when the node is the root of the tree: every tour passes through it.
pub const ROOT_IS_EVERY_TOURS: &str =
    "the project's root folder is every tour's: name a folder in it";

/// A tour's file: its id and the folder it is in.
pub type Key = (TourId, Where);

/// What the map keeps of one stop: what the tour wrote, nothing of the code.
#[derive(Debug, Clone)]
struct Wrote {
    id: StopId,
    qualname: String,
}

/// One tour as the map has it: its stops in route order.
#[derive(Debug, Clone)]
struct Kept {
    stops: Vec<Wrote>,
}

/// (tour, stop index in the tour)
type Hit = (Key, usize);

/// The three maps of §6, built from replayed tours. Owned, so the core can keep it: [`Map::set`] puts one tour in again when its file is replayed, and no
/// other tour is touched.
#[derive(Debug, Default)]
pub struct Map {
    tours: BTreeMap<Key, Kept>,
    by_id: BTreeMap<NodeId, Vec<Hit>>,
    /// `path::qualname#n`, for every stop
    by_name: BTreeMap<(String, String, u32), Vec<Hit>>,
    /// every stop by its path, sorted: a prefix walk finds "inside this file or folder"
    by_path: BTreeMap<String, Vec<Hit>>,
    /// how many times a tour was put in: what the tests count
    built: usize,
}

impl Map {
    /// The tours put in the order given.
    pub fn of<'a>(tours: impl IntoIterator<Item = (Key, &'a Tour)>) -> Map {
        let mut map = Map::default();
        for (key, tour) in tours {
            map.set(key, Some(tour));
        }
        map
    }

    /// How many tours were put in since the map was made (`set` with a tour).
    pub fn built(&self) -> usize {
        self.built
    }

    /// Puts this tour in, or takes it out (`None`: it went, or cannot be read). What the tour had before is gone from all three maps.
    pub fn set(&mut self, key: Key, tour: Option<&Tour>) {
        if self.tours.remove(&key).is_some() {
            for hits in self
                .by_id
                .values_mut()
                .chain(self.by_name.values_mut())
                .chain(self.by_path.values_mut())
            {
                hits.retain(|(k, _)| *k != key);
            }
            self.by_id.retain(|_, hits| !hits.is_empty());
            self.by_name.retain(|_, hits| !hits.is_empty());
            self.by_path.retain(|_, hits| !hits.is_empty());
        }
        let Some(tour) = tour else { return };
        self.built += 1;
        for (s, stop) in tour.stops.iter().enumerate() {
            let hit = (key.clone(), s);
            if let Some(id) = stop.node {
                self.by_id.entry(id).or_default().push(hit.clone());
            }
            self.by_name
                .entry((
                    stop.path.to_string(),
                    stop.qualname.clone(),
                    stop.occurrence,
                ))
                .or_default()
                .push(hit.clone());
            self.by_path
                .entry(stop.path.to_string())
                .or_default()
                .push(hit);
        }
        self.tours.insert(
            key,
            Kept {
                stops: tour
                    .stops
                    .iter()
                    .map(|s| Wrote {
                        id: s.id,
                        qualname: s.qualname.clone(),
                    })
                    .collect(),
            },
        );
    }

    fn stop(&self, (key, s): &Hit) -> &Wrote {
        &self.tours[key].stops[*s]
    }

    /// Every tour that passes through the node, in the order of the tours' keys (the caller puts them in the order of its list).
    pub fn on(&self, query: &Query) -> Vec<On> {
        let mut direct: Vec<Hit> = Vec::new();
        let mut inside: Vec<Hit> = Vec::new();
        match query.kind {
            NodeKind::Folder => {
                let path = query.path.as_str();
                for (key, hits) in &self.by_path {
                    for hit in hits {
                        let stop = self.stop(hit);
                        if key == path && stop.qualname.is_empty() {
                            direct.push(hit.clone());
                        } else if key == path || key.starts_with(&format!("{path}/")) {
                            inside.push(hit.clone());
                        }
                    }
                }
            }
            NodeKind::File => {
                for hit in self.by_path.get(query.path.as_str()).into_iter().flatten() {
                    if self.stop(hit).qualname.is_empty() {
                        direct.push(hit.clone());
                    } else {
                        inside.push(hit.clone());
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
                direct.extend(self.by_name.get(&name).into_iter().flatten().cloned());
                self.also_by_id(query, &mut direct);
                // a class: the names inside it
                if query.kind == NodeKind::Class {
                    let prefix = format!("{}.", query.qualname);
                    for hit in self.by_path.get(query.path.as_str()).into_iter().flatten() {
                        if self.stop(hit).qualname.starts_with(&prefix) {
                            inside.push(hit.clone());
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

        let mut keys: Vec<&Key> = direct.iter().chain(&inside).map(|(k, _)| k).collect();
        keys.sort_unstable();
        keys.dedup();
        keys.into_iter()
            .map(|key| {
                let mine: Vec<usize> = direct.iter().filter(|h| h.0 == *key).map(|h| h.1).collect();
                let within: Vec<usize> =
                    inside.iter().filter(|h| h.0 == *key).map(|h| h.1).collect();
                let kept = &self.tours[key];
                let how = match mine.first() {
                    Some(0) => How::Starts,
                    Some(_) => How::Stops(mine.iter().map(|s| s + 1).collect()),
                    None => How::Inside(within.len()),
                };
                let first = mine.first().or(within.first()).copied().unwrap_or(0);
                On {
                    tour: key.0.clone(),
                    place: key.1,
                    how,
                    inside: if mine.is_empty() { 0 } else { within.len() },
                    first: kept.stops[first].id,
                    of: kept.stops.len(),
                }
            })
            .collect()
    }

    /// The stops that carry the node's ID, whatever name they were written with.
    fn also_by_id(&self, query: &Query, direct: &mut Vec<Hit>) {
        if let Some(id) = query.id {
            direct.extend(self.by_id.get(&id).into_iter().flatten().cloned());
        }
    }

    /// How many tours pass through each node of the tree that any does (`vnoc/tourCounts`): the key is the tree's own, `Node::id`, the one `vnoc/structure`
    /// gives a node. A node is counted as `list --on` counts it, so a class, a file or a folder with stops inside counts too; a tour in both folders counts
    /// once. The root of the tree is not counted.
    pub fn counts(&self, tree: &Node) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        if !self.tours.is_empty() {
            self.count_below(tree, &mut counts);
        }
        counts
    }

    fn count_below(&self, node: &Node, counts: &mut BTreeMap<String, usize>) {
        if let Some(query) = Query::of(node) {
            let mut tours: Vec<TourId> = self.on(&query).into_iter().map(|on| on.tour).collect();
            tours.dedup(); // the keys are sorted by id
            if !tours.is_empty() {
                counts.insert(node.id.clone(), tours.len());
            }
        }
        for child in &node.children {
            self.count_below(child, counts);
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

    fn map_of(tours: &[&Tour]) -> Map {
        Map::of(tours.iter().map(|t| ((t.id.clone(), Where::Private), *t)))
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
        let map = map_of(&[&a, &b]);
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
        let map = map_of(&[&a, &b]);
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
        let map = map_of(&[&many]);
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
        let map = map_of(&[&spread]);
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
        let map = map_of(&[&a, &b]);
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
        let map = map_of(&[&a]);
        let mut q = query("x.py", "f", NodeKind::Function, None);
        assert!(map.on(&q).is_empty());
        q.occurrence = 2;
        assert_eq!(said(&map, &q), pairs(&[("a-0000", "starts here")]));
    }

    #[test]
    fn putting_one_tour_in_again_replaces_that_tour_and_touches_no_other() {
        let a = tour("a-0000", &[stop(1, "x.py", "run", "function", "")]);
        let b = tour("b-0000", &[stop(1, "x.py", "run", "function", "")]);
        let mut map = map_of(&[&a, &b]);
        assert_eq!(map.built(), 2);
        let run = query("x.py", "run", NodeKind::Function, None);
        assert_eq!(map.on(&run).len(), 2);
        // `b` is replayed and has another function now; `a` is not looked at again
        let b2 = tour("b-0000", &[stop(1, "x.py", "other", "function", "")]);
        map.set((b2.id.clone(), Where::Private), Some(&b2));
        assert_eq!(map.built(), 3);
        assert_eq!(said(&map, &run), pairs(&[("a-0000", "starts here")]));
        let other = query("x.py", "other", NodeKind::Function, None);
        assert_eq!(said(&map, &other), pairs(&[("b-0000", "starts here")]));
        // it went: nothing of it is left in any of the three
        map.set((b2.id.clone(), Where::Private), None);
        assert!(map.on(&other).is_empty());
        assert!(map.by_path.values().flatten().all(|(k, _)| k.0 != b2.id));
        assert_eq!(map.built(), 3);
    }
}
