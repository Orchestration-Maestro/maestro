//! Supplied repository identity and lexical label rules.
/// Native repository identity and supplied lexical labels.
pub mod discovery;
/// Native repository paths and metadata text.
pub mod native;
/// Branch tracking read from user, repository, included and worktree configuration.
pub mod tracking;
pub use discovery::{automatic_workspace_label, fallback_label_from_cwd};

mod status;
pub use discovery::GitSpaceMetadata;
pub use status::GitStatusRefreshDemand;
