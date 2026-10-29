//! Repository identity with caller-owned Git process fallbacks.
/// Shared native discovery and daemon Git command queries.
pub mod discovery;
/// Status snapshots and the fingerprints that decide when a cached one is rebuilt.
pub mod status;
/// Worktree names, checkout paths and the Git commands that manage worktrees.
pub mod worktree;
