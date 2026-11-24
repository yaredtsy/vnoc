//! What `finish` does after it has written `end` (`docs/26-tours/02-commands.md` §5, `01-on-disk.md` §12): step 1, the IDs; step 2, the pin.
//!
//! This module decides and words; `cli/tour.rs` does what it says (it owns the index, the locks and the printing). The IDs are decided by the setting
//! `[tours] ids` and the two flags of one call, and only stops whose node has **no ID** count. The pin is always tried; only the commit of files that are
//! not committed is a setting (`[tours] commit`, `--commit`, `--no-commit`).

use super::ids::Pick;
use super::pin::{Changed, Pinnable};
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

/// What `finish` does about the pin (step 2 of 02 §5), once it knows what can be done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinStep {
    /// everything is committed: pin
    Pin,
    /// the only thing in the way is files not committed, and the choice is `always`: commit exactly those files, then pin
    CommitThenPin,
    /// the same, and the choice is `ask`: say the files and the command, for the agent to ask the user
    Ask,
    /// the same, and the choice is `never`: `not pinned` and the files, with no command
    NotPinned,
    /// something else is in the way: `not pinned: <reason>` for each (the choice has nothing to say about it)
    Why,
}

/// The step: a pin when one can be made; for files that are not committed, by `commit` (the setting, or the flags of this call: see [`effective`]); for
/// any other reason, the reason.
pub fn pin_step(choice: Choice, pinnable: &Pinnable) -> PinStep {
    match pinnable {
        Pinnable::Yes { .. } => PinStep::Pin,
        Pinnable::No(_) if pinnable.only_uncommitted().is_none() => PinStep::Why,
        Pinnable::No(_) => match choice {
            Choice::Always => PinStep::CommitThenPin,
            Choice::Ask => PinStep::Ask,
            Choice::Never => PinStep::NotPinned,
        },
    }
}

/// The line for the agent to pass on as a question, not to act on, and the files under it:
///
/// ```text
/// ask the user  not pinned: 2 files have changes that are not committed       vnoc tour pin <tour> --commit
///                 M  app/inventory.py
///                 ?? app/gateway.py
/// ```
pub fn ask_commit_lines(tour: &TourId, why: &str, files: &[Changed]) -> Vec<String> {
    let mut lines = vec![format!(
        "ask the user  not pinned: {why}       vnoc tour pin {tour} --commit"
    )];
    lines.extend(
        files
            .iter()
            .map(|f| format!("                {}", f.line())),
    );
    lines
}

/// The message of a commit made to pin, when the user gave none.
pub fn default_message(title: &str) -> String {
    format!("Code for the tour \"{title}\"")
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

    #[test]
    fn the_pin_is_tried_always_and_only_the_commit_of_files_is_by_the_choice() {
        use super::super::pin::Reason;
        use super::super::record::Commit;
        use vnoc_domain::RepoPath;
        let yes = Pinnable::Yes {
            head: Commit::parse("f4d6e15c0a9be1d2f7a8a1b0c3d4e5f60718293a").unwrap(),
        };
        let file = |status: &str, path: &str| Changed {
            status: status.to_string(),
            path: RepoPath::new(path).unwrap(),
        };
        let files = Pinnable::No(vec![Reason::Uncommitted {
            files: vec![file(" M", "a.py"), file("??", "b.py")],
        }]);
        let more = Pinnable::No(vec![
            Reason::Uncommitted {
                files: vec![file(" M", "a.py")],
            },
            Reason::NotHeld {
                path: RepoPath::new("c.py").unwrap(),
                ignored: true,
            },
        ]);
        let other = Pinnable::No(vec![Reason::NotARepository]);
        for choice in [Choice::Always, Choice::Ask, Choice::Never] {
            assert_eq!(pin_step(choice, &yes), PinStep::Pin, "{choice}");
            assert_eq!(pin_step(choice, &other), PinStep::Why, "{choice}");
            // files not committed AND a file git will not hold: committing would not pin, so the choice does not matter
            assert_eq!(pin_step(choice, &more), PinStep::Why, "{choice}");
        }
        assert_eq!(pin_step(Choice::Always, &files), PinStep::CommitThenPin);
        assert_eq!(pin_step(Choice::Ask, &files), PinStep::Ask);
        assert_eq!(pin_step(Choice::Never, &files), PinStep::NotPinned);
    }

    #[test]
    fn the_ask_line_for_a_commit_names_the_files_under_it_and_the_message_is_the_plans() {
        use vnoc_domain::RepoPath;
        let tour = TourId::parse("checkout-order-7k2q").unwrap();
        let files = [
            Changed {
                status: "M ".into(),
                path: RepoPath::new("app/inventory.py").unwrap(),
            },
            Changed {
                status: "??".into(),
                path: RepoPath::new("app/gateway.py").unwrap(),
            },
        ];
        assert_eq!(
            ask_commit_lines(&tour, "2 files have changes that are not committed", &files),
            [
                "ask the user  not pinned: 2 files have changes that are not committed       vnoc tour pin checkout-order-7k2q --commit",
                "                M  app/inventory.py",
                "                ?? app/gateway.py",
            ]
        );
        assert_eq!(
            default_message("checkout_order"),
            "Code for the tour \"checkout_order\""
        );
    }
}
