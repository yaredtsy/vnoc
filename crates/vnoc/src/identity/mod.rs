//! IDs, markers and finding nodes again (`05-identity/`): attaching a doc to a function, class, file or folder, and resolving a documented node.

mod attach;
pub mod folder_marker;
mod resolve;

pub use attach::{AttachError, Env, attach};
pub use resolve::{FoundBy, Markers, Resolved, resolve};
