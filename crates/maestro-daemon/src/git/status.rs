//! Status snapshots and the fingerprints that decide when a cached one is rebuilt.

use maestro_workspace::workspace::{
    GitSpaceMetadata, GitStatusRefreshDemand, WorkspaceGitStatusSnapshot,
    git::config::ConfigEnvironment,
};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

/// One cached observation: a repository's fingerprint and snapshot, or a directory outside any.
#[derive(Debug, Clone)]
pub struct GitStatusCacheEntry {
    /// What the repository looked like, absent for a directory outside any repository.
    pub fingerprint: Option<GitStatusFingerprint>,
    /// Until when a result without a repository is kept instead of searched again.
    pub retry_after: Option<Instant>,
    /// The snapshot built from the observation.
    pub snapshot: WorkspaceGitStatusSnapshot,
}

/// The reference and commit identities whose change makes a cached snapshot stale.
#[derive(Debug, Clone)]
pub struct GitStatusFingerprint {
    /// What the checkout has out.
    pub head: GitHeadIdentity,
    /// The branch's upstream, when tracking is configured and was demanded.
    pub upstream: Option<GitUpstreamIdentity>,
}

/// What `HEAD` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHeadIdentity {
    /// A local branch.
    Branch {
        /// Full reference name.
        full_ref: String,
        /// Name without the `refs/heads/` prefix.
        short_name: String,
        /// Commit the branch points at, absent for an unborn branch.
        oid: Option<String>,
    },
    /// A commit checked out directly.
    Detached {
        /// The commit.
        oid: String,
    },
}

/// The tracking reference of a branch and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUpstreamIdentity {
    /// Remote named by the branch configuration.
    pub remote: String,
    /// Reference the branch merges from.
    pub merge_ref: String,
    /// Local reference the fetch refspecs map it to.
    pub full_ref: String,
    /// Commit that reference points at, absent when it does not exist.
    pub oid: Option<String>,
}

/// Cache identity of the checkout containing `cwd`.
#[must_use]
pub fn git_status_cache_key(_cwd: &Path) -> Option<PathBuf> {
    None
}

/// Cache identity of a checkout whose metadata is already known.
#[must_use]
pub fn git_status_cache_key_for_space(_space: &GitSpaceMetadata) -> PathBuf {
    PathBuf::new()
}

/// Observe `cwd` for the demanded fields, reusing `cached` while its fingerprint still holds.
///
/// The second value is the entry to cache; it is absent when the repository could not be read.
#[must_use]
pub fn git_status_snapshot_for_cwd_with_demand(
    _cwd: &Path,
    _cached: Option<&GitStatusCacheEntry>,
    _demand: GitStatusRefreshDemand,
    _environment: &ConfigEnvironment,
) -> (WorkspaceGitStatusSnapshot, Option<GitStatusCacheEntry>) {
    (
        WorkspaceGitStatusSnapshot {
            auto_label: String::new(),
            branch: None,
            ahead_behind: None,
            space: None,
        },
        None,
    )
}
