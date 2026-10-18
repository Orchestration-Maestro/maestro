//! Workspace records and owner-routed identity allocation.
mod public_numbers;
mod state;
mod topology;
pub use public_numbers::{
    decode_public_number, encode_public_number, public_pane_id_for_number,
    public_tab_id_for_number, public_workspace_number,
};
pub use state::{TakenPane, Workspace};
pub use topology::WorkspaceTopology;

mod operations;

/// Supplied repository records and labels.
pub mod git;

mod identity;
pub use git::{GitSpaceMetadata, GitStatusRefreshDemand};
pub use identity::{WorkspaceGitStatus, WorkspaceGitStatusSnapshot, WorktreeSpaceMembership};
