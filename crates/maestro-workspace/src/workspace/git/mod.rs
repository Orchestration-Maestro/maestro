//! Supplied repository identity and lexical label rules.
mod discovery;
pub use discovery::{automatic_workspace_label, fallback_label_from_cwd};

mod status;
pub use discovery::GitSpaceMetadata;
pub use status::GitStatusRefreshDemand;
