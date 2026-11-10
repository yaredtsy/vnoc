//! IDs, markers and finding nodes again (`05-identity/`). Step 2 has the first flow: attaching a doc to a function or class.

mod attach;

pub use attach::{AttachError, Env, attach};
