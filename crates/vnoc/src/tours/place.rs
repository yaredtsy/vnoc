//! The reader's place in a tour (`docs/26-tours/03-core.md` §4, `01-on-disk.md` §9): where they are, which stops they have read, and where each jump
//! came from. **Pure**: no disk. [`go`] takes the tour, the place and a move, and gives the new place; the state file (`store.rs`) keeps it, and the
//! canvas and `vnoc tour go` both ask this function, so there is one answer to "what is next".
//!
//! The steps of a tour are its stops' steps in the order a person reads them ([`steps_in_order`], stop after stop): a stop's intro, then each block with
//! its details under it. A planned stop is one step, its intro, with no text yet. The cover comes before the first step and the end after the last.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use super::fold::{Tour, steps_in_order};
use super::record::{IdError, StepId, StopId};
use super::time;

/// How many jumps `back` remembers: the oldest is dropped (03 §4).
pub const BACK_MAX: usize = 20;

/// Where the reader is: before the first step, on one, or after the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum At {
    Cover,
    Step(StepId),
    End,
}

impl At {
    /// `cover`, `end`, or a step's id (`s3`, `s3.b2`).
    pub fn parse(text: &str) -> Result<At, IdError> {
        match text {
            "cover" => Ok(At::Cover),
            "end" => Ok(At::End),
            _ => StepId::parse(text).map(At::Step),
        }
    }
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            At::Cover => f.write_str("cover"),
            At::Step(step) => write!(f, "{step}"),
            At::End => f.write_str("end"),
        }
    }
}

impl FromStr for At {
    type Err = IdError;
    fn from_str(text: &str) -> Result<At, IdError> {
        At::parse(text)
    }
}

impl Serialize for At {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for At {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<At, D::Error> {
        let text = String::deserialize(d)?;
        At::parse(&text).map_err(de::Error::custom)
    }
}

/// Who moved the place last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum By {
    /// the person, on the canvas
    #[default]
    Reader,
    /// `vnoc tour go`
    Command,
}

/// The place, as the state file keeps it (`01` §9). Every key but `at` may be missing from a file: the reader then starts as at the cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub at: At,
    /// the stops read to their last step, in the order they were
    #[serde(default)]
    pub seen: Vec<StopId>,
    /// where the reader was before each jump, the latest last, at most [`BACK_MAX`]. An `At` and not a step: a jump from the cover or the end is
    /// taken back to it too (03 §4 says `StepId`s; a place on the cover would otherwise have nothing to return to)
    #[serde(default)]
    pub back: Vec<At>,
    /// when it last moved: `2026-11-23T18:52:10Z`; empty for a place nobody has moved
    #[serde(default)]
    pub moved: String,
    #[serde(default)]
    pub by: By,
    /// a window is showing this tour (set and cleared by the core; `go` never changes it)
    #[serde(default)]
    pub open: bool,
}

impl Place {
    /// A tour nobody has moved in: the cover, nothing read.
    pub fn cover() -> Place {
        Place {
            at: At::Cover,
            seen: Vec::new(),
            back: Vec::new(),
            moved: String::new(),
            by: By::Reader,
            open: false,
        }
    }
}

impl Default for Place {
    fn default() -> Place {
        Place::cover()
    }
}

/// A move. `To` goes there and remembers nothing; `Jump` is the same and pushes where the reader was onto `back` (a row, a card, a segment, a call in
/// the code, **Go there**, `vnoc tour go s4`). `Next`, `Prev`, `NextStop` and `PrevStop` never push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Next,
    Prev,
    NextStop,
    PrevStop,
    Back,
    To(At),
    Jump(At),
}

impl Move {
    /// What `vnoc tour go` was given: `next`, `prev`, `next-stop`, `prev-stop`, `back`, or a place to jump to: `cover`, `end`, `s4`, `s3.b2`.
    pub fn parse(text: &str) -> Result<Move, String> {
        match text {
            "next" => Ok(Move::Next),
            "prev" => Ok(Move::Prev),
            "next-stop" => Ok(Move::NextStop),
            "prev-stop" => Ok(Move::PrevStop),
            "back" => Ok(Move::Back),
            other => At::parse(other).map(Move::Jump).map_err(|_| {
                format!(
                    "`{other}` is not a move: next, prev, next-stop, prev-stop, back, cover, end, a stop (s4) or a block (s3.b2)"
                )
            }),
        }
    }
}

/// Every step of the tour in the order it is read, across the stops of the route.
pub fn steps(tour: &Tour) -> Vec<StepId> {
    tour.stops.iter().flat_map(steps_in_order).collect()
}

/// Whether the tour has this step (an intro of a stop on the route, or a block of one).
pub fn has_step(tour: &Tour, step: StepId) -> bool {
    tour.stop(step.stop())
        .is_some_and(|stop| steps_in_order(stop).contains(&step))
}

/// A place the tour still has: a step that is gone is moved to the nearest one that is not (03 §4): its stop's intro, else the next stop's (the
/// lowest id above it: ids are never reused, so it can never land on a different step), else the cover. What `where` and `go` apply to a place they
/// read. It does **not** touch `moved`, and it is not written back by a read. Entries of `back` and `seen` that the tour no longer has are dropped.
pub fn settle(tour: &Tour, place: &Place) -> Place {
    let mut settled = place.clone();
    if let At::Step(step) = place.at
        && !has_step(tour, step)
    {
        let next = tour
            .stops
            .iter()
            .filter(|s| s.id > step.stop())
            .min_by_key(|s| s.id);
        settled.at = if tour.stop(step.stop()).is_some() {
            At::Step(StepId::intro(step.stop()))
        } else {
            next.map_or(At::Cover, |s| At::Step(StepId::intro(s.id)))
        };
    }
    settled.back.retain(|at| match at {
        At::Step(step) => has_step(tour, *step),
        At::Cover | At::End => true,
    });
    settled.seen.retain(|stop| tour.stop(*stop).is_some());
    settled
}

/// Whether the step is the last of its stop.
fn last_of_its_stop(tour: &Tour, step: StepId) -> bool {
    tour.stop(step.stop())
        .and_then(|stop| steps_in_order(stop).last().copied())
        == Some(step)
}

/// The step before and after `at`, as places: the neighbours a `Prev` and a `Next` would go to.
pub fn around(tour: &Tour, at: At) -> (At, At) {
    let place = Place {
        at,
        ..Place::cover()
    };
    let step = |m| go(tour, &place, m).at;
    (step(Move::Prev), step(Move::Next))
}

/// The new place for a move. **Pure**: `moved` is the time now, `by` and `open` are what they were (the caller says who moved, as the canvas or `go`
/// does). A target (`To`, `Jump`) that is a step the tour does not have, and a `Back` with nothing to go back to, change nothing at all, `moved`
/// included. The place given is expected to be one the tour has: [`settle`] it first.
///
/// | Move | Does |
/// |---|---|
/// | `Next` | the cover to the first step; a step to the one after it, the last to the end; the end stays |
/// | `Prev` | the end to the last step; a step to the one before it, the first to the cover; the cover stays |
/// | `NextStop`, `PrevStop` | the next stop's intro (after the last stop, the end), the previous stop's (before the first, the cover) |
/// | `Back` | the last place on `back`, taken off it |
/// | `Jump(at)` | there, and where the reader was goes onto `back` unless it is the same place; at most [`BACK_MAX`], the oldest dropped |
/// | `To(at)` | there, nothing remembered |
///
/// A stop goes into `seen` when its last step has become the place, by any move.
pub fn go(tour: &Tour, place: &Place, m: Move) -> Place {
    let all = steps(tour);
    let position = match place.at {
        At::Step(step) => all.iter().position(|s| *s == step),
        At::Cover | At::End => None,
    };
    let first = |all: &[StepId]| all.first().map_or(At::Cover, |s| At::Step(*s));
    let stop_index = |stop: StopId| tour.stops.iter().position(|s| s.id == stop);
    let intro_of = |index: usize| At::Step(StepId::intro(tour.stops[index].id));

    let mut next = place.clone();
    let target = match m {
        Move::Next => match place.at {
            At::Cover => first(&all),
            At::End => At::End,
            At::Step(_) => match position {
                Some(i) => all.get(i + 1).map_or(At::End, |s| At::Step(*s)),
                None => place.at,
            },
        },
        Move::Prev => match place.at {
            At::Cover => At::Cover,
            At::End => all.last().map_or(At::Cover, |s| At::Step(*s)),
            At::Step(_) => match position {
                Some(0) => At::Cover,
                Some(i) => At::Step(all[i - 1]),
                None => place.at,
            },
        },
        Move::NextStop => match place.at {
            At::Cover => first(&all),
            At::End => At::End,
            At::Step(step) => match stop_index(step.stop()) {
                Some(i) if i + 1 < tour.stops.len() => intro_of(i + 1),
                Some(_) => At::End,
                None => place.at,
            },
        },
        Move::PrevStop => match place.at {
            At::Cover => At::Cover,
            At::End => tour.stops.len().checked_sub(1).map_or(At::Cover, intro_of),
            At::Step(step) => match stop_index(step.stop()) {
                Some(0) => At::Cover,
                Some(i) => intro_of(i - 1),
                None => place.at,
            },
        },
        Move::Back => match next.back.pop() {
            Some(at) => at,
            None => return place.clone(),
        },
        Move::To(at) | Move::Jump(at) => {
            if let At::Step(step) = at
                && !has_step(tour, step)
            {
                return place.clone();
            }
            if matches!(m, Move::Jump(_)) && at != place.at {
                next.back.push(place.at);
                if next.back.len() > BACK_MAX {
                    next.back.remove(0);
                }
            }
            at
        }
    };
    next.at = target;
    if let At::Step(step) = target
        && last_of_its_stop(tour, step)
        && !next.seen.contains(&step.stop())
    {
        next.seen.push(step.stop());
    }
    next.moved = time::now();
    next
}

/// Where a step is in the tour, for saying it: the stop's number on the route and how many stops there are, the step's number in its stop and how many
/// steps the stop has. `None` on the cover and the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// the stop's place on the route, from 1
    pub stop: usize,
    pub stops: usize,
    /// the step's place in its stop, from 1 (the intro is 1)
    pub step: usize,
    pub steps: usize,
}

pub fn position(tour: &Tour, at: At) -> Option<Position> {
    let At::Step(step) = at else { return None };
    let index = tour.stops.iter().position(|s| s.id == step.stop())?;
    let in_stop = steps_in_order(&tour.stops[index]);
    Some(Position {
        stop: index + 1,
        stops: tour.stops.len(),
        step: in_stop.iter().position(|s| *s == step)? + 1,
        steps: in_stop.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tours::fold::fold;

    const HEAD: &str =
        r#"{"type":"tour","schema":1,"id":"t-0000","title":"T","at":"x","detail":"normal"}"#;
    const H: &str = "0000000000000001";

    fn stop(id: u32, extra: &str) -> String {
        format!(
            r#"{{"type":"stop","id":"s{id}","path":"a.py","qualname":"f{id}","kind":"function","lines":[1,30],"hash":"{H}"{extra},"text":"intro"}}"#
        )
    }

    fn block(stop: u32, n: u32, parent: Option<u32>) -> String {
        let parent = parent.map_or(String::new(), |p| format!(r#","parent":"s{stop}.b{p}""#));
        format!(
            r#"{{"type":"step","id":"s{stop}.b{n}","stop":"s{stop}","lines":[2,5],"hash":"{H}","title":"T{n}","text":"x"{parent}}}"#
        )
    }

    fn tour(lines: &[String]) -> Tour {
        let mut text = format!("{HEAD}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        fold(&text).unwrap()
    }

    /// s1: a block and two details of it (4 steps, one of them a detail read after b1); s2: a short stop (the intro only); s3 repeats s2 (`same_as`)
    /// with a block of its own; s4 is planned, not written.
    fn the_tour() -> Tour {
        tour(&[
            stop(1, ""),
            block(1, 1, None),
            block(1, 2, Some(1)),
            block(1, 3, Some(1)),
            stop(2, r#","from":"s1","line":3"#),
            stop(3, r#","from":"s1","line":4,"same_as":"s2""#),
            block(3, 1, None),
            r#"{"type":"plan","stops":[{"id":"s4","path":"a.py","qualname":"later"}]}"#.to_string(),
        ])
    }

    fn step(text: &str) -> At {
        At::parse(text).unwrap()
    }

    fn place_at(text: &str) -> Place {
        Place {
            at: step(text),
            ..Place::cover()
        }
    }

    fn at_after(t: &Tour, from: &str, m: Move) -> String {
        go(t, &place_at(from), m).at.to_string()
    }

    #[test]
    fn the_steps_are_read_stop_after_stop_with_the_details_under_their_block_and_a_planned_stop_is_its_intro()
     {
        let ids: Vec<String> = steps(&the_tour()).iter().map(ToString::to_string).collect();
        assert_eq!(
            ids,
            ["s1", "s1.b1", "s1.b2", "s1.b3", "s2", "s3", "s3.b1", "s4"]
        );
    }

    #[test]
    fn next_and_prev_walk_the_steps_across_stops_and_stop_at_the_ends() {
        let t = the_tour();
        let table = [
            // (from, move, to)
            ("cover", Move::Next, "s1"),
            ("s1", Move::Next, "s1.b1"),
            ("s1.b1", Move::Next, "s1.b2"),
            ("s1.b2", Move::Next, "s1.b3"),
            // the last step of s1 to the first of the next stop: the short stop
            ("s1.b3", Move::Next, "s2"),
            ("s2", Move::Next, "s3"),
            ("s3", Move::Next, "s3.b1"),
            ("s3.b1", Move::Next, "s4"),
            // a planned stop is a step: its intro
            ("s4", Move::Next, "end"),
            ("end", Move::Next, "end"),
            ("end", Move::Prev, "s4"),
            ("s4", Move::Prev, "s3.b1"),
            ("s3.b1", Move::Prev, "s3"),
            ("s2", Move::Prev, "s1.b3"),
            ("s1.b2", Move::Prev, "s1.b1"),
            // the first step's prev is the cover; the cover's stays
            ("s1", Move::Prev, "cover"),
            ("cover", Move::Prev, "cover"),
        ];
        for (from, m, to) in table {
            assert_eq!(at_after(&t, from, m), to, "{from} {m:?}");
        }
    }

    #[test]
    fn next_stop_and_prev_stop_go_to_an_intro_whatever_step_the_reader_is_on() {
        let t = the_tour();
        let table = [
            ("cover", Move::NextStop, "s1"),
            ("s1", Move::NextStop, "s2"),
            ("s1.b2", Move::NextStop, "s2"),
            ("s2", Move::NextStop, "s3"),
            ("s3.b1", Move::NextStop, "s4"),
            ("s4", Move::NextStop, "end"),
            ("end", Move::NextStop, "end"),
            ("end", Move::PrevStop, "s4"),
            ("s4", Move::PrevStop, "s3"),
            ("s3.b1", Move::PrevStop, "s2"),
            ("s2", Move::PrevStop, "s1"),
            ("s1.b3", Move::PrevStop, "cover"),
            ("s1", Move::PrevStop, "cover"),
            ("cover", Move::PrevStop, "cover"),
        ];
        for (from, m, to) in table {
            assert_eq!(at_after(&t, from, m), to, "{from} {m:?}");
        }
    }

    #[test]
    fn a_jump_pushes_where_the_reader_was_and_next_and_prev_and_the_stop_moves_never_do() {
        let t = the_tour();
        for m in [Move::Next, Move::Prev, Move::NextStop, Move::PrevStop] {
            let moved = go(&t, &place_at("s1.b1"), m);
            assert!(moved.back.is_empty(), "{m:?}");
        }
        // a jump from a step, from the cover and from the end all push where they were
        let from_step = go(&t, &place_at("s1.b1"), Move::Jump(step("s3")));
        assert_eq!(
            (from_step.at, from_step.back),
            (step("s3"), vec![step("s1.b1")])
        );
        let from_cover = go(&t, &Place::cover(), Move::Jump(step("s2")));
        assert_eq!(from_cover.back, [At::Cover]);
        let from_end = go(&t, &place_at("end"), Move::Jump(At::Cover));
        assert_eq!((from_end.at, from_end.back), (At::Cover, vec![At::End]));
        // `To` goes there and remembers nothing
        let to = go(&t, &place_at("s1"), Move::To(step("s3")));
        assert_eq!((to.at, to.back.is_empty()), (step("s3"), true));
        // a jump to where the reader is already pushes nothing
        let same = go(&t, &place_at("s2"), Move::Jump(step("s2")));
        assert_eq!((same.at, same.back.is_empty()), (step("s2"), true));
        // a jump that keeps what was pushed before it
        let mut p = go(&t, &place_at("s1"), Move::Jump(step("s2")));
        p = go(&t, &p, Move::Next);
        p = go(&t, &p, Move::Jump(step("s4")));
        assert_eq!(p.back, [step("s1"), step("s3")]);
    }

    #[test]
    fn back_holds_twenty_and_drops_the_oldest() {
        let t = the_tour();
        let mut p = place_at("s1");
        // 25 jumps, alternating between s1.b1 and s2: each pushes the place it left
        let targets = [step("s1.b1"), step("s2")];
        for i in 0..25 {
            p = go(&t, &p, Move::Jump(targets[i % 2]));
        }
        assert_eq!(p.back.len(), BACK_MAX);
        // 25 pushes, the first of them `s1` (where it started): the last 20 are kept, so the first kept is the 6th push
        assert_eq!(p.back[0], targets[(5 + 1) % 2]);
        assert_eq!(*p.back.last().unwrap(), targets[(24 + 1) % 2]);
    }

    #[test]
    fn back_pops_and_pushes_nothing_and_with_nothing_to_pop_changes_nothing() {
        let t = the_tour();
        let mut p = go(&t, &place_at("s1.b1"), Move::Jump(step("s3")));
        p = go(&t, &p, Move::Jump(At::End));
        assert_eq!(p.back, [step("s1.b1"), step("s3")]);
        p = go(&t, &p, Move::Back);
        assert_eq!((p.at, p.back.clone()), (step("s3"), vec![step("s1.b1")]));
        p = go(&t, &p, Move::Back);
        assert_eq!((p.at, p.back.is_empty()), (step("s1.b1"), true));
        // nothing to pop: the same place, `moved` too
        let mut stay = place_at("s2");
        stay.moved = "2026-11-23T18:52:10Z".to_string();
        assert_eq!(go(&t, &stay, Move::Back), stay);
    }

    #[test]
    fn a_stop_is_seen_when_its_last_step_becomes_the_place_by_any_move() {
        let t = the_tour();
        // s1 has four steps: only the last one counts
        let p = go(&t, &place_at("s1.b2"), Move::Next);
        assert_eq!(
            (p.at, p.seen.clone()),
            (step("s1.b3"), vec![StopId::new(1).unwrap()])
        );
        let p = go(&t, &place_at("s1"), Move::Next);
        assert!(p.seen.is_empty(), "{:?}", p.seen);
        // a short stop and a planned one are one step: arriving is reading it
        let p = go(&t, &place_at("s1.b2"), Move::Next);
        let p = go(&t, &p, Move::Next);
        assert_eq!(
            (p.at, p.seen),
            (
                step("s2"),
                vec![StopId::new(1).unwrap(), StopId::new(2).unwrap()]
            )
        );
        // a jump and a stop move count too, and nothing is seen twice
        let p = go(&t, &Place::cover(), Move::Jump(step("s3.b1")));
        assert_eq!(p.seen, [StopId::new(3).unwrap()]);
        let p = go(&t, &p, Move::To(step("s3.b1")));
        assert_eq!(p.seen, [StopId::new(3).unwrap()]);
        let p = go(&t, &place_at("s1"), Move::NextStop);
        assert_eq!(p.seen, [StopId::new(2).unwrap()]);
        // going back to a last step sees it, going past it and not on it does not
        let p = go(&t, &place_at("s3"), Move::Next);
        assert!(p.seen.contains(&StopId::new(3).unwrap()));
        let p = go(&t, &place_at("s3.b1"), Move::Prev);
        assert!(p.seen.is_empty());
    }

    #[test]
    fn cover_and_end_are_places_a_jump_can_go_to_and_next_from_the_cover_is_the_first_step() {
        let t = the_tour();
        assert_eq!(at_after(&t, "s2", Move::Jump(At::Cover)), "cover");
        assert_eq!(at_after(&t, "s2", Move::Jump(At::End)), "end");
        assert_eq!(at_after(&t, "cover", Move::Next), "s1");
        // a tour with no step: next and prev have nowhere to go
        let empty = tour(&[]);
        assert_eq!(at_after(&empty, "cover", Move::Next), "cover");
        assert_eq!(at_after(&empty, "end", Move::Prev), "cover");
    }

    #[test]
    fn a_target_the_tour_does_not_have_changes_nothing() {
        let t = the_tour();
        let mut p = place_at("s2");
        p.moved = "2026-11-23T18:52:10Z".to_string();
        for m in [Move::Jump(step("s9")), Move::To(step("s1.b9"))] {
            assert_eq!(go(&t, &p, m), p, "{m:?}");
        }
    }

    #[test]
    fn go_sets_the_time_and_keeps_who_and_whether_a_window_is_open() {
        let t = the_tour();
        let mut p = place_at("s1");
        p.by = By::Command;
        p.open = true;
        p.moved = "2020-01-01T00:00:00Z".to_string();
        let moved = go(&t, &p, Move::Next);
        assert_eq!((moved.by, moved.open), (By::Command, true));
        assert!(
            moved.moved.as_str() > "2026-11-24T00:00:00Z",
            "{}",
            moved.moved
        );
    }

    #[test]
    fn a_place_whose_step_is_gone_moves_to_the_nearest_that_exists() {
        let t = the_tour();
        // the block is gone (the stop was replaced with fewer): the stop's intro; a step that is there stays
        let table = [("s1.b9", "s1"), ("s3.b2", "s3"), ("s3.b1", "s3.b1")];
        for (at, want) in table {
            assert_eq!(settle(&t, &place_at(at)).at.to_string(), want, "{at}");
        }
        // a dropped stop between two: the next one on
        let dropped = tour(&[
            stop(1, ""),
            stop(3, ""),
            r#"{"type":"drop","id":"s2"}"#.to_string(),
        ]);
        assert_eq!(settle(&dropped, &place_at("s2.b1")).at, step("s3"));
        assert_eq!(settle(&dropped, &place_at("s2")).at, step("s3"));
        // the last stop gone: the cover
        assert_eq!(settle(&dropped, &place_at("s7")).at, At::Cover);
        // a cover and an end are always there
        assert_eq!(settle(&dropped, &place_at("end")).at, At::End);
        // what `back` and `seen` name that is gone is dropped, the rest stays
        let mut p = place_at("s1");
        p.back = vec![step("s2.b1"), step("s1"), At::Cover, At::End];
        p.seen = vec![StopId::new(2).unwrap(), StopId::new(3).unwrap()];
        p.moved = "2026-11-23T18:52:10Z".to_string();
        let settled = settle(&dropped, &p);
        assert_eq!(settled.back, [step("s1"), At::Cover, At::End]);
        assert_eq!(settled.seen, [StopId::new(3).unwrap()]);
        assert_eq!(settled.moved, p.moved, "settling is not a move");
    }

    #[test]
    fn where_the_step_is_in_its_stop_and_on_the_route() {
        let t = the_tour();
        let table = [
            ("s1", Some((1, 4, 1, 4))),
            ("s1.b2", Some((1, 4, 3, 4))),
            ("s2", Some((2, 4, 1, 1))),
            ("s3.b1", Some((3, 4, 2, 2))),
            ("s4", Some((4, 4, 1, 1))),
            ("cover", None),
            ("end", None),
        ];
        for (at, want) in table {
            let got = position(&t, step(at)).map(|p| (p.stop, p.stops, p.step, p.steps));
            assert_eq!(got, want, "{at}");
        }
        assert_eq!(around(&t, step("s1")), (At::Cover, step("s1.b1")));
        assert_eq!(around(&t, At::Cover), (At::Cover, step("s1")));
        assert_eq!(around(&t, At::End), (step("s4"), At::End));
    }

    #[test]
    fn a_move_is_what_the_command_line_says() {
        assert_eq!(Move::parse("next"), Ok(Move::Next));
        assert_eq!(Move::parse("prev-stop"), Ok(Move::PrevStop));
        assert_eq!(Move::parse("back"), Ok(Move::Back));
        assert_eq!(Move::parse("cover"), Ok(Move::Jump(At::Cover)));
        assert_eq!(Move::parse("end"), Ok(Move::Jump(At::End)));
        assert_eq!(Move::parse("s4"), Ok(Move::Jump(step("s4"))));
        assert_eq!(Move::parse("s3.b2"), Ok(Move::Jump(step("s3.b2"))));
        for bad in ["", "Next", "s0", "s3.b0", "s3.2", "stop3", "forward"] {
            assert!(Move::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_place_is_the_json_of_the_state_file_and_a_short_one_reads_with_defaults() {
        let mut p = place_at("s3.b2");
        p.seen = vec![StopId::new(1).unwrap(), StopId::new(2).unwrap()];
        p.back = vec![step("s1.b1"), At::Cover];
        p.moved = "2026-11-23T18:52:10Z".to_string();
        p.by = By::Command;
        p.open = true;
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"at": "s3.b2", "seen": ["s1", "s2"], "back": ["s1.b1", "cover"],
                "moved": "2026-11-23T18:52:10Z", "by": "command", "open": true})
        );
        assert_eq!(serde_json::from_value::<Place>(json).unwrap(), p);
        let short: Place = serde_json::from_str(r#"{"at":"end"}"#).unwrap();
        assert_eq!(
            short,
            Place {
                at: At::End,
                ..Place::cover()
            }
        );
        assert!(serde_json::from_str::<Place>(r#"{"at":"s0"}"#).is_err());
        assert!(serde_json::from_str::<Place>(r#"{}"#).is_err());
    }
}
