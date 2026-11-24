//! Tours: an agent explains a piece of the code, step by step (`docs/26-tours/`).
//!
//! A tour is a log: one file, one JSON record per line, and reading it is replaying its lines (`01-on-disk.md` §2, `03-core.md` §1).
//! [`record`] (the lines: parse one, print one) and [`fold`] (lines in, a tour out) touch nothing; [`store`] is the first that writes to disk: the
//! folders, the lock, start, append, read, list, the state file. No index, no driver and no git here yet (`06-build-order.md`).

pub mod body;
pub mod fold;
pub mod hash;
pub mod record;
pub mod rules;
pub mod store;
pub mod time;

pub use fold::{Problem, Size, State, Tour, TourBlock, TourStop, Unreadable, fold, steps_in_order};
pub use record::{
    Commit, Detail, IdError, LineError, Lines, Record, StepId, StopId, TourId, parse_line,
    print_line,
};
