//! What `finish` does after it has written `end` (`docs/26-tours/02-commands.md` §5, `01-on-disk.md` §12). Step 1 here: the IDs. (Step 2, the pin, is TR9.)
//!
//! This module decides and words; `cli/tour.rs` does what it says (it owns the index, the locks and the printing). The decision is by the setting
//! `[tours] ids` and the two flags of one call, and only stops whose node has **no ID** count.

use super::ids::Pick;
use super::record::{StopId, TourId};
use super::settings::Choice;

/// What `finish` does about IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// the flag or the setting says no, or the project writes no ID lines, or there is nothing to give an ID to: nothing is done and nothing is said
    Nothing,
    /// write them, as `vnoc tour ids` does
    Write,
    /// say how many stops have none and the command, for the agent to ask the user
    Ask { missing: usize },
}

/// The choice of this call: `--ids` is `"always"` and `--no-ids` is `"never"`, whatever the file says; neither is the file's.
pub fn effective(setting: Choice, ids: bool, no_ids: bool) -> Choice {
    match (ids, no_ids) {
        (true, _) => Choice::Always,
        (_, true) => Choice::Never,
        _ => setting,
    }
}

/// How many stops have no ID: the node of a stop that is found and has none. A stop that is gone or whose node has an ID is not counted.
pub fn missing(picks: &[(StopId, Pick)]) -> usize {
    picks
        .iter()
        .filter(|(_, pick)| matches!(pick, Pick::Needs { .. }))
        .count()
}

/// The step: nothing when the project writes no ID lines (`[ids] default = "none"`), when the choice is `"never"`, or when no stop is missing one.
pub fn step(choice: Choice, project_writes_ids: bool, missing: usize) -> Step {
    if !project_writes_ids || missing == 0 {
        return Step::Nothing;
    }
    match choice {
        Choice::Never => Step::Nothing,
        Choice::Always => Step::Write,
        Choice::Ask => Step::Ask { missing },
    }
}

/// The line for the agent to pass on as a question, not to act on (04 §3): `ask the user  4 stops have no ID: …        vnoc tour ids <tour>`.
pub fn ask_line(tour: &TourId, missing: usize) -> String {
    let what = if missing == 1 {
        "1 stop has no ID".to_string()
    } else {
        format!("{missing} stops have no ID")
    };
    format!("ask the user  {what}: the tour will not follow a rename.        vnoc tour ids {tour}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn needs() -> (StopId, Pick) {
        (
            StopId::new(1).unwrap(),
            Pick::Needs {
                key: "a.py::f".into(),
                path: "a.py".into(),
                kind: crate::index::Kind::Function,
            },
        )
    }

    #[test]
    fn the_flags_win_over_the_setting_and_the_setting_over_nothing() {
        let table = [
            (Choice::Ask, false, false, Choice::Ask),
            (Choice::Ask, true, false, Choice::Always),
            (Choice::Always, false, true, Choice::Never),
            (Choice::Never, true, false, Choice::Always),
            (Choice::Never, false, false, Choice::Never),
            (Choice::Always, false, false, Choice::Always),
        ];
        for (setting, ids, no_ids, want) in table {
            assert_eq!(
                effective(setting, ids, no_ids),
                want,
                "{setting} {ids} {no_ids}"
            );
        }
    }

    #[test]
    fn what_finish_does_by_the_choice_and_only_for_stops_with_no_id() {
        let table = [
            (Choice::Always, true, 4, Step::Write),
            (Choice::Ask, true, 4, Step::Ask { missing: 4 }),
            (Choice::Never, true, 4, Step::Nothing),
            // `[ids] default = "none"`: nothing, whatever the choice
            (Choice::Always, false, 4, Step::Nothing),
            (Choice::Ask, false, 4, Step::Nothing),
            // every stop has its ID: nothing to write, nothing to ask
            (Choice::Always, true, 0, Step::Nothing),
            (Choice::Ask, true, 0, Step::Nothing),
        ];
        for (choice, writes, missing, want) in table {
            assert_eq!(
                step(choice, writes, missing),
                want,
                "{choice} {writes} {missing}"
            );
        }
    }

    #[test]
    fn only_a_node_with_no_id_is_missing_one() {
        let known = (
            StopId::new(2).unwrap(),
            Pick::Known(vnoc_domain::NodeId::parse("0Q8V5W4T2F7KX9M3B6D1RZ4H7K").unwrap()),
        );
        let gone = (StopId::new(3).unwrap(), Pick::Gone);
        assert_eq!(missing(&[needs(), known, gone, needs()]), 2);
    }

    #[test]
    fn the_ask_line_is_the_one_of_the_plan() {
        let tour = TourId::parse("checkout-order-7k2q").unwrap();
        assert_eq!(
            ask_line(&tour, 4),
            "ask the user  4 stops have no ID: the tour will not follow a rename.        vnoc tour ids checkout-order-7k2q"
        );
        assert_eq!(
            ask_line(&tour, 1),
            "ask the user  1 stop has no ID: the tour will not follow a rename.        vnoc tour ids checkout-order-7k2q"
        );
    }
}
