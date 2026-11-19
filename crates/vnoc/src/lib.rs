//! The V-NOC core. A library so the pieces can be tested; the binary (`main.rs`) only wires them to stdio, or to the command line (`cli`).
//!
//! Nothing in the library prints: in `vnoc --stdio` standard output is the protocol, and a command prints through `cli/out.rs` alone (05 §4).
#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod calls;
pub mod cli;
pub mod driver_host;
pub mod grep;
pub mod identity;
pub mod index;
pub mod node_ref;
pub mod project;
pub mod protocol;
pub mod scan;
pub mod search;
pub mod server;
pub mod source;
pub mod suggest;
pub mod userdata;
pub mod watchdog;
