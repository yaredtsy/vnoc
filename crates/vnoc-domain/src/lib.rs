//! V-NOC's domain types. No I/O, no driver, no VS Code: just the things every other crate must agree on.
//!
//! * [`NodeId`]: a node's permanent identity, and the one rule for making it. **The on-disk format starts here and is
//!   final from the MVP on.**
//! * [`LocationKey`] / [`RepoPath`]: where a node is *right now* (it changes on every rename; it is never stored as an identity).
//! * [`persist`]: the only way to turn a location into a `NodeId`.
//!
//! Anything that saves a reference to a node (a doc link, an MCP answer, a saved canvas tab) must hold a `NodeId`, so it
//! must call [`persist`] first.

mod id;
mod location;
mod node_toml;
mod persist;

pub use id::{NodeId, NodeIdError, ProvisionalId};
pub use location::{LocationKey, LocationKeyError, RepoPath, RepoPathError, occurrences};
pub use node_toml::{IdInCode, NodeToml, NodeTomlError, Status};
pub use persist::{IdWriter, MAX_ATTEMPTS, PersistError, persist};

#[cfg(test)]
#[allow(dead_code)] // each test binary uses only part of the table
pub(crate) mod golden {
    //! The expected values written once by the independent Python oracle (tests/golden/gen_golden.py).
    include!("../tests/golden_data.rs");
}
