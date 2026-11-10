//! IDs, markers and finding nodes again (`05-identity/`): attaching a doc to a function or class, and resolving a documented node.

mod attach;
mod resolve;

pub use attach::{AttachError, Env, attach};
pub use resolve::{FoundBy, Markers, Resolved, resolve};
