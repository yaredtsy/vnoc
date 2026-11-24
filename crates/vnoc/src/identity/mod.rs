//! IDs, markers and finding nodes again (`05-identity/`): attaching a doc to a function, class, file or folder, and resolving a documented node.

mod attach;
pub mod edits;
pub mod folder_marker;
mod locate;
mod relink;
mod resolve;
mod write_id;

pub use attach::{
    AttachError, AttachOptions, Attached, Done, Ensured, Env, MAX_BODY_BYTES, MarkerWritten,
    attach, check_note, ensure_node,
};
pub use edits::EditSink;
pub use locate::{LocateError, locate, locate_in};
pub use relink::{RelinkError, relink};
pub use resolve::{FoundBy, Markers, Resolved, resolve};
pub use write_id::{WriteIdError, write_id};
