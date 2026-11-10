//! The V-NOC core. A library so the pieces can be tested; the binary (`main.rs`) only wires them to stdio.

pub mod driver_host;
pub mod identity;
pub mod index;
pub mod project;
pub mod protocol;
pub mod scan;
pub mod server;
pub mod userdata;
pub mod watchdog;
