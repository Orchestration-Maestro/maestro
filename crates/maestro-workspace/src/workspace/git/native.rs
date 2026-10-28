//! Native repository paths shared by metadata consumers.
use std::{
    io,
    path::{Path, PathBuf},
};

/// Checkout and metadata paths before canonical identity projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPaths {
    /// Authored checkout directory, or the bare repository itself.
    pub repo_root: PathBuf,
    /// Checkout-specific metadata directory.
    pub git_dir: PathBuf,
    /// Shared repository metadata directory.
    pub git_common_dir: PathBuf,
}

/// Trim the shared Unicode whitespace set at repository text boundaries.
#[must_use]
pub fn trim(text: &str) -> &str {
    text.trim_matches(maestro_text::is_whitespace_scalar)
}

/// Read native metadata text, replacing malformed UTF-8.
///
/// # Errors
/// Returns the native read failure.
pub fn read_text(path: &Path) -> io::Result<String> {
    std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Trim valid Unicode boundary text without replacing malformed interior bytes.
fn trim_link(bytes: &[u8]) -> &[u8] {
    let first = bytes.utf8_chunks().next().map_or("", |chunk| chunk.valid());
    let bytes = &bytes[first.len()
        - first
            .trim_start_matches(maestro_text::is_whitespace_scalar)
            .len()..];
    let last = bytes.utf8_chunks().last().map_or("", |chunk| chunk.valid());
    &bytes[..bytes.len()
        - (last.len()
            - last
                .trim_end_matches(maestro_text::is_whitespace_scalar)
                .len())]
}

/// Resolve metadata links without decoding native Unix path components.
fn resolve_link(base: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    #[cfg(unix)]
    let target = {
        use std::os::unix::ffi::OsStrExt;
        Path::new(std::ffi::OsStr::from_bytes(trim_link(bytes)))
    };
    #[cfg(not(unix))]
    let target = Path::new(
        std::str::from_utf8(trim_link(bytes))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    std::path::absolute(base.join(target)).map(|path| maestro_path::normalize_native(&path))
}

/// Caller-specific metadata acceptance within the shared repository walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryPolicy {
    /// Workspace identity accepts unspaced links and requires a regular HEAD.
    /// Unusable markers are skipped; unreadable common metadata falls back.
    Workspace,
    /// Footer links require a space; directory metadata ignores `commondir`.
    Footer,
}

/// Find the nearest workspace repository with a regular HEAD file.
#[must_use]
pub fn discover_git_paths(start: &Path) -> Option<GitPaths> {
    discover_git_paths_with(start, DiscoveryPolicy::Workspace)
}

/// Discover native paths with the caller's metadata acceptance rules.
#[must_use]
pub fn discover_git_paths_with(start: &Path, policy: DiscoveryPolicy) -> Option<GitPaths> {
    discover(start, policy).ok().flatten()
}

/// One native walk, retaining authored checkout paths.
fn discover(start: &Path, policy: DiscoveryPolicy) -> io::Result<Option<GitPaths>> {
    let mut current = if policy == DiscoveryPolicy::Workspace && start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    }
    .to_path_buf();
    loop {
        if let Some(git_dir) = git_dir_for_root(&current, policy)? {
            let head = git_dir.join("HEAD");
            let valid = match policy {
                DiscoveryPolicy::Workspace => head.is_file(),
                DiscoveryPolicy::Footer => head.exists(),
            };
            if !valid && policy == DiscoveryPolicy::Footer {
                return Ok(None);
            }
            if valid {
                return paths_for_root(current, git_dir, policy).map(Some);
            }
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}

/// Assemble accepted metadata paths with caller-specific shared-directory policy.
fn paths_for_root(
    root: PathBuf,
    git_dir: PathBuf,
    policy: DiscoveryPolicy,
) -> io::Result<GitPaths> {
    let git_common_dir = if policy == DiscoveryPolicy::Footer && root.join(".git").is_dir() {
        git_dir.clone()
    } else {
        git_common_dir_for_git_dir(&git_dir, policy)?
    };
    Ok(GitPaths {
        repo_root: root,
        git_dir,
        git_common_dir,
    })
}

/// Locate a directory's metadata marker, including standalone bare layouts.
///
/// A `.git` link file resolves to its target without checking that the target exists.
///
/// # Errors
/// Returns the native failure of resolving a link.
pub fn git_dir_for_repo_root(root: &Path) -> io::Result<Option<PathBuf>> {
    git_dir_for_root(root, DiscoveryPolicy::Workspace)
}

/// Locate metadata with the caller's worktree marker syntax.
fn git_dir_for_root(root: &Path, policy: DiscoveryPolicy) -> io::Result<Option<PathBuf>> {
    let marker = root.join(".git");
    match std::fs::metadata(&marker) {
        Ok(metadata) if metadata.is_dir() => return Ok(Some(marker)),
        Ok(metadata) if metadata.is_file() => {
            let contents = match std::fs::read(&marker) {
                Ok(contents) => contents,
                Err(_) if policy == DiscoveryPolicy::Workspace => return Ok(None),
                Err(error) => return Err(error),
            };
            let prefix = match policy {
                DiscoveryPolicy::Workspace => b"gitdir:".as_slice(),
                DiscoveryPolicy::Footer => b"gitdir: ".as_slice(),
            };
            return trim_link(&contents)
                .strip_prefix(prefix)
                .map(|target| resolve_link(root, target))
                .transpose();
        }
        Ok(_) => return Ok(None),
        Err(_) => {}
    }
    Ok((root.join("HEAD").is_file()
        && root.join("objects").is_dir()
        && root.join("refs").is_dir()
        && git_dir_is_bare(root))
    .then(|| root.to_path_buf()))
}

/// Resolve shared metadata with caller-specific unreadable-link handling.
fn git_common_dir_for_git_dir(git_dir: &Path, policy: DiscoveryPolicy) -> io::Result<PathBuf> {
    let common = git_dir.join("commondir");
    if policy == DiscoveryPolicy::Footer && !common.exists() {
        return Ok(git_dir.to_path_buf());
    }
    match std::fs::read(common) {
        Ok(contents) => resolve_link(git_dir, &contents),
        Err(_) if policy == DiscoveryPolicy::Workspace => Ok(git_dir.to_path_buf()),
        Err(error) => Err(error),
    }
}

/// Whether repository configuration selects reftable storage.
#[must_use]
pub fn git_ref_storage_is_reftable(common_dir: &Path) -> bool {
    read_git_config_value(&common_dir.join("config"), "extensions", "refstorage")
        .is_some_and(|value| value.eq_ignore_ascii_case("reftable"))
}

/// Whether repository configuration declares a bare layout.
pub(super) fn git_dir_is_bare(git_dir: &Path) -> bool {
    read_git_config_value(&git_dir.join("config"), "core", "bare")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

/// Extract a repository configuration value; the last assignment in the plain section wins.
fn read_git_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let items = super::config::syntax::parse(&read_text(path).ok()?);
    super::config::syntax::plain_entries(&items, section)
        .filter(|(name, _)| name.eq_ignore_ascii_case(key))
        .last()
        .and_then(|(_, value)| value.map(str::to_owned))
}
