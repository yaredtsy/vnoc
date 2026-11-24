//! Tours: an agent explains a piece of the code, step by step (`docs/26-tours/`).
//!
//! A tour is a log: one file, one JSON record per line, and reading it is replaying its lines (`01-on-disk.md` §2, `03-core.md` §1).
//! [`record`] (the lines: parse one, print one) and [`fold`] (lines in, a tour out) touch nothing; [`store`] is the first that writes to disk: the
//! folders, the lock, start, append, read, list, the state file. [`find`], [`add`] and [`check`] are the ones that look at the code, through the index they are
//! given: they start no driver, and a recorded check is answered before any index exists. [`rebase`] and [`ids`] are the two that write to the code's
//! side: V-NOC's own edit of a file tells the tours in it where their lines went, and `vnoc tour ids` gives a tour's nodes an ID. [`settings`] and [`finish`] say when
//! the second happens by itself. No git here yet (`06-build-order.md`).

pub mod add;
pub mod body;
pub mod check;
pub mod doc;
pub mod find;
pub mod finish;
pub mod fold;
pub mod hash;
pub mod ids;
pub mod on;
pub mod rebase;
pub mod record;
pub mod rules;
pub mod settings;
pub mod store;
pub mod time;
pub mod view;

pub use fold::{Problem, Size, State, Tour, TourBlock, TourStop, Unreadable, fold, steps_in_order};
pub use record::{
    Commit, Detail, IdError, LineError, Lines, Record, StepId, StopId, TourId, parse_line,
    print_line,
};
