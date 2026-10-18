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
