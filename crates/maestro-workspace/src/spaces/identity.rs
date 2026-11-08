//! Supplied workspace repository and worktree identity.
use super::repository::{
    GitSpaceMetadata, GitStatusRefreshDemand, automatic_workspace_label, fallback_label_from_cwd,
};
use std::path::{Path, PathBuf};
/// Explicit managed-worktree grouping provenance.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorktreeSpaceMembership {
    /// Group identity.
    pub key: String,
    /// Group label.
    pub label: String,
    /// Repository root observation.
    pub repo_root: PathBuf,
    /// Checkout path observation.
    pub checkout_path: PathBuf,
    /// Whether this is a linked worktree.
    pub is_linked_worktree: bool,
}
/// A resolved repository observation tied to a workspace and cache key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGitStatus {
    /// Workspace handle.
    pub workspace_id: String,
    /// Identity directory used for the observation.
    pub resolved_identity_cwd: PathBuf,
    /// Cache identity supplied by discovery.
    pub status_cache_key: PathBuf,
    /// Requested refresh fields.
    pub demand: GitStatusRefreshDemand,
    /// Automatic label derived from the supplied paths.
    pub auto_label: String,
    /// Observed branch.
    pub branch: Option<String>,
    /// Ahead and behind counts.
    pub ahead_behind: Option<(usize, usize)>,
    /// Repository grouping observation.
    pub space: Option<GitSpaceMetadata>,
}
/// Repository data before association with a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGitStatusSnapshot {
    /// Unassociated display label; conversion derives the current label from paths.
    pub auto_label: String,
    /// Observed branch.
    pub branch: Option<String>,
    /// Ahead and behind counts.
    pub ahead_behind: Option<(usize, usize)>,
    /// Repository grouping observation.
    pub space: Option<GitSpaceMetadata>,
}
impl WorkspaceGitStatusSnapshot {
    /// Associate this observation with a workspace, deriving its label from supplied paths.
    /// The path pair contains the identity cwd followed by the status cache key.
    #[must_use]
    pub fn into_workspace_status(
        self,
        workspace_id: String,
        paths: (PathBuf, PathBuf),
        demand: GitStatusRefreshDemand,
        home: Option<&Path>,
    ) -> WorkspaceGitStatus {
        let (resolved_identity_cwd, status_cache_key) = paths;
        let auto_label = self.space.as_ref().map_or_else(
            || fallback_label_from_cwd(&resolved_identity_cwd, home),
            |space| automatic_workspace_label(&resolved_identity_cwd, &space.repo_root, home),
        );
        WorkspaceGitStatus {
            workspace_id,
            resolved_identity_cwd,
            status_cache_key,
            demand,
            auto_label,
            branch: self.branch,
            ahead_behind: self.ahead_behind,
            space: self.space,
        }
    }
}
