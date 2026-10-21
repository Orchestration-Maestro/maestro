//! Repository discovery delegation and footer branch selection.

use super::operations::FooterOperations;
#[cfg(not(target_arch = "wasm32"))]
use maestro_workspace::workspace::git::native::trim;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// Footer watch paths, retaining authored rather than canonical identity.
pub(super) struct GitPaths {
    /// The checkout directory.
    pub(super) repo_dir: String,
    /// The HEAD file that names the checked-out branch.
    pub(super) head_path: String,
    /// Shared repository metadata directory.
    pub(super) common_git_dir: String,
}

/// Native metadata uses the shared workspace walk with footer acceptance rules.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn find_git_paths(cwd: &str) -> Option<GitPaths> {
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    let paths = discover_git_paths_with(Path::new(cwd), DiscoveryPolicy::Footer)?;
    Some(GitPaths {
        repo_dir: paths.repo_root.to_string_lossy().into_owned(),
        head_path: paths.git_dir.join("HEAD").to_string_lossy().into_owned(),
        common_git_dir: paths.git_common_dir.to_string_lossy().into_owned(),
    })
}

/// Browser callers have no native repository metadata.
#[cfg(target_arch = "wasm32")]
pub(super) fn find_git_paths(_cwd: &str) -> Option<GitPaths> {
    None
}

/// Read lossy native HEAD text without a second filesystem adapter.
#[cfg(not(target_arch = "wasm32"))]
fn read_head(path: &str) -> std::io::Result<String> {
    maestro_workspace::workspace::git::native::read_text(Path::new(path))
}

/// Browser builds do not read native files.
#[cfg(target_arch = "wasm32")]
fn read_head(_path: &str) -> std::io::Result<String> {
    Err(std::io::Error::other(
        "Native repository metadata is unavailable",
    ))
}

/// Browser branch output uses the shared Unicode whitespace set.
#[cfg(target_arch = "wasm32")]
fn trim(text: &str) -> &str {
    text.trim_matches(maestro_text::is_whitespace_scalar)
}

/// The branch HEAD names: `detached` for any other readable HEAD, and the branch Git
/// reports when HEAD holds the placeholder a reftable repository writes.
///
/// A HEAD that cannot be read gives `None`.
pub(super) fn resolve_branch(
    operations: &dyn FooterOperations,
    paths: &GitPaths,
) -> Option<String> {
    match read_branch(paths) {
        BranchRead::Ready(branch) => branch,
        BranchRead::Query(repo) => Some(git_answer(
            operations.symbolic_ref_sync(repo).ok().flatten(),
        )),
    }
}
/// HEAD selection or the directory requiring a symbolic-reference query.
pub(super) enum BranchRead<'a> {
    /// A completed HEAD read, including unreadable or absent metadata.
    Ready(Option<String>),
    /// A placeholder requiring Git in this repository directory.
    Query(&'a str),
}
/// Select a readable HEAD before deciding whether Git is needed.
pub(super) fn read_branch(paths: &GitPaths) -> BranchRead<'_> {
    let Ok(head) = read_head(&paths.head_path) else {
        return BranchRead::Ready(None);
    };
    let Some(branch) = trim(&head).strip_prefix("ref: refs/heads/") else {
        return BranchRead::Ready(Some("detached".to_owned()));
    };
    if branch == ".invalid" {
        BranchRead::Query(&paths.repo_dir)
    } else {
        BranchRead::Ready(Some(branch.to_owned()))
    }
}
/// Trim successful stdout, falling back to detached on failure or empty text.
pub(super) fn git_answer(output: Option<String>) -> String {
    output
        .map(|stdout| trim(&stdout).to_owned())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "detached".to_owned())
}
