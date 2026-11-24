//! Tours: an agent explains a piece of the code, step by step (`docs/26-tours/`).
//!
//! A tour is a log: one file, one JSON record per line, and reading it is replaying its lines (`01-on-disk.md` §2, `03-core.md` §1).
//! This module starts with the part that touches nothing: [`record`] (the lines: parse one, print one) and [`fold`] (lines in, a tour out).
//! No disk, no index, no driver and no git here; the files come in later steps (`06-build-order.md`).

pub mod fold;
pub mod record;

pub use fold::{Problem, Size, State, Tour, TourBlock, TourStop, Unreadable, fold, steps_in_order};
pub use record::{
    Commit, Detail, IdError, LineError, Lines, Record, StepId, StopId, TourId, parse_line,
    print_line,
};
