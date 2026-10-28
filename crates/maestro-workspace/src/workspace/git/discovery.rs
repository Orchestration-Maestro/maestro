//! Labels derived only from supplied native paths.
use std::path::{Path, PathBuf};
/// Label a directory using supplied HOME equality, basename or native display fallback.
#[must_use]
pub fn fallback_label_from_cwd(cwd: &Path, home: Option<&Path>) -> String {
    if home == Some(cwd) {
        return "~".to_owned();
    }
    cwd.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map_or_else(|| cwd.display().to_string(), str::to_owned)
}
/// Prefer the checkout basename, falling back to the supplied directory.
#[must_use]
pub fn automatic_workspace_label(cwd: &Path, repo_root: &Path, home: Option<&Path>) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| fallback_label_from_cwd(cwd, home), str::to_owned)
}

/// Repository grouping and checkout identity supplied by discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSpaceMetadata {
    /// Repository grouping key.
    pub key: String,
    /// Checkout-specific identity key.
    pub checkout_key: String,
    /// Repository grouping display name.
    pub repo_name: String,
    /// Effective checkout root.
    pub repo_root: PathBuf,
    /// Whether this is a linked worktree.
    pub is_linked_worktree: bool,
}

/// Checkout identity and canonical metadata directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktreeInfo {
    /// Effective checkout root, preserving its authored spelling.
    pub repo_root: PathBuf,
    /// Canonical checkout-specific metadata directory when available.
    pub git_dir: PathBuf,
    /// Canonical shared repository metadata directory when available.
    pub git_common_dir: PathBuf,
    /// Whether metadata declares a bare repository.
    pub is_bare: bool,
    /// Whether this checkout uses separate metadata from its repository.
    pub is_linked_worktree: bool,
}

/// Locate the checkout root through the shared native walk.
#[must_use]
pub fn git_repo_root(cwd: &Path) -> Option<PathBuf> {
    super::native::discover_git_paths(cwd).map(|paths| paths.repo_root)
}

/// Derive a label from repository discovery or supplied directory observations.
#[must_use]
pub fn derive_label_from_cwd(cwd: &Path, home: Option<&Path>) -> String {
    git_repo_root(cwd).map_or_else(
        || fallback_label_from_cwd(cwd, home),
        |root| automatic_workspace_label(cwd, &root, home),
    )
}

/// Discover a checkout's canonical repository identity.
#[must_use]
pub fn git_worktree_info(cwd: &Path) -> Option<GitWorktreeInfo> {
    let paths = super::native::discover_git_paths(cwd)?;
    let git_dir = canonicalize_best_effort_path(&paths.git_dir);
    let git_common_dir = canonicalize_best_effort_path(&paths.git_common_dir);
    Some(GitWorktreeInfo {
        is_linked_worktree: git_dir != git_common_dir,
        is_bare: super::native::git_dir_is_bare(&git_dir),
        repo_root: paths.repo_root,
        git_dir,
        git_common_dir,
    })
}

/// Discover repository grouping and checkout identity for a native directory.
#[must_use]
pub fn git_space_metadata(cwd: &Path) -> Option<GitSpaceMetadata> {
    git_worktree_info(cwd).map(|info| git_space_metadata_from_info(&info))
}

/// Project repository grouping independently of the checkout's automatic label.
#[must_use]
fn git_space_metadata_from_info(info: &GitWorktreeInfo) -> GitSpaceMetadata {
    let common_name = info
        .git_common_dir
        .file_name()
        .and_then(|name| name.to_str());
    let label_path = match common_name {
        Some(".git") => info.git_common_dir.parent().unwrap_or(&info.repo_root),
        Some(".bare") => embedded_bare_repo_container(info).unwrap_or(&info.git_common_dir),
        _ => &info.git_common_dir,
    };
    GitSpaceMetadata {
        key: canonicalize_best_effort_path(&info.git_common_dir)
            .display()
            .to_string(),
        checkout_key: canonicalize_best_effort_path(&info.repo_root)
            .display()
            .to_string(),
        repo_name: label_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo")
            .to_owned(),
        repo_root: info.repo_root.clone(),
        is_linked_worktree: info.is_linked_worktree,
    }
}

/// A container supplies the repository name only when its marker points at this bare store.
fn embedded_bare_repo_container(info: &GitWorktreeInfo) -> Option<&Path> {
    let parent = info.git_common_dir.parent()?;
    let parent_git_dir = super::native::git_dir_for_repo_root(parent).ok()??;
    (canonicalize_best_effort_path(&parent_git_dir) == info.git_common_dir).then_some(parent)
}

/// Canonicalize an identity path without rejecting an unavailable native path.
#[must_use]
pub fn canonicalize_best_effort_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
