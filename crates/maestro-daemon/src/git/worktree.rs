//! Worktree names, checkout paths and the Git commands that add, remove and list worktrees.
use std::{
    fmt,
    path::{Path, PathBuf},
};

/// A failed worktree operation, carrying the message Git printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeError {
    /// The text a caller shows or matches.
    message: String,
}

impl WorktreeError {
    /// Wraps a message that came from Git or from checking a checkout.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The message Git gives when a checkout holds changes and removal is not forced.
    #[must_use]
    pub fn dirty_checkout(path: &Path) -> Self {
        let _ = path;
        Self::new("")
    }

    /// The text of the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether Git refused removal because the checkout holds changes that `--force` discards.
    #[must_use]
    pub fn is_dirty_checkout(&self) -> bool {
        false
    }
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WorktreeError {}

/// One entry of `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingWorktree {
    /// The checkout directory as Git printed it.
    pub path: PathBuf,
    /// The checked-out branch without `refs/heads/`, absent when detached or bare.
    pub branch: Option<String>,
    /// Whether the entry is the bare repository itself.
    pub is_bare: bool,
    /// Whether HEAD is detached.
    pub is_detached: bool,
    /// Whether Git would prune the entry because its checkout is gone.
    pub is_prunable: bool,
}

impl ExistingWorktree {
    /// Parses the output of `git worktree list --porcelain`.
    #[must_use]
    pub fn parse_porcelain(output: &[u8]) -> Vec<Self> {
        let _ = output;
        Vec::new()
    }
}

/// A branch name for a worktree created without one.
#[must_use]
pub fn generated_branch_slug(seed: u64) -> String {
    let _ = seed;
    String::new()
}

/// A folder name for a branch: lowercase ASCII letters and digits joined by single dashes.
#[must_use]
pub fn branch_to_path_slug(branch: &str) -> String {
    let _ = branch;
    String::new()
}

/// The absolute folder that holds every repository's checkouts.
#[must_use]
pub fn worktrees_root(directory: &str, home: impl FnOnce() -> Option<PathBuf>) -> PathBuf {
    let _ = (directory, home);
    PathBuf::new()
}

/// The checkout folder for `branch` of the repository named `repo_name`.
#[must_use]
pub fn default_checkout_path(root: &Path, repo_name: &str, branch: &str) -> PathBuf {
    let _ = (root, repo_name, branch);
    PathBuf::new()
}

/// Whether `refs/heads/<branch>` exists in the repository.
///
/// # Errors
/// Returns Git's message when the lookup fails for a reason other than a missing branch.
pub fn local_branch_exists(repo_root: &Path, branch: &str) -> Result<bool, WorktreeError> {
    let _ = (repo_root, branch);
    Ok(false)
}

/// Adds a checkout of `branch` at `path`, creating the branch from `base` when it is new.
///
/// # Errors
/// Returns Git's message when the branch lookup or `git worktree add` fails.
pub fn add(repo_root: &Path, path: &Path, branch: &str, base: &str) -> Result<(), WorktreeError> {
    let _ = (repo_root, path, branch, base);
    Err(WorktreeError::new(""))
}

/// Removes the checkout at `path`; a forced removal also clears a leftover directory Git no
/// longer lists.
///
/// # Errors
/// Returns Git's message, or the cleanup failure of a leftover directory.
pub fn remove(repo_root: &Path, path: &Path, force: bool) -> Result<(), WorktreeError> {
    let _ = force;
    recover_leftover(repo_root, path, WorktreeError::new(""))
}

/// Settles a forced removal that Git answered with "not a working tree".
fn recover_leftover(
    repo_root: &Path,
    path: &Path,
    error: WorktreeError,
) -> Result<(), WorktreeError> {
    let _ = (repo_root, path);
    Err(error)
}

/// Whether the checkout at `path` has modified or untracked files.
///
/// # Errors
/// Returns Git's message when `git status` fails.
pub fn checkout_has_dirty_files(path: &Path) -> Result<bool, WorktreeError> {
    let _ = path;
    Ok(false)
}

/// Lists the worktrees of the repository.
///
/// # Errors
/// Returns Git's message when `git worktree list` fails.
pub fn list(repo_root: &Path) -> Result<Vec<ExistingWorktree>, WorktreeError> {
    let _ = repo_root;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
