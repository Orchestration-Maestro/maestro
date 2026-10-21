//! Native repository paths shared by metadata consumers.
use maestro_path::{dirname, resolve_with};
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

/// Resolve a metadata link lexically, observing ambient directories only when needed.
fn resolve_path(base: &Path, target: &str) -> io::Result<PathBuf> {
    let base = base.to_string_lossy();
    let paths = [base.as_ref(), target];
    let drives: Vec<(char, String)> = paths
        .iter()
        .filter_map(|path| {
            let [letter @ (b'a'..=b'z' | b'A'..=b'Z'), b':', ..] = path.as_bytes() else {
                return None;
            };
            let drive = letter.to_ascii_uppercase() as char;
            Some((drive, std::env::var(format!("={drive}:")).ok()?))
        })
        .collect();
    resolve_with(
        &[&paths],
        || &drives,
        || std::env::current_dir().map(|path| path.to_string_lossy().into_owned()),
    )
    .map(|resolved| PathBuf::from(resolved.into_iter().collect::<String>()))
}

/// Caller-specific metadata acceptance within the shared repository walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryPolicy {
    /// Workspace identity accepts unspaced links, requires regular HEAD, and tolerates unreadable common metadata.
    Workspace,
    /// Footer links require a space; directory metadata ignores `commondir`.
    Footer,
}

/// Find the nearest repository, stopping at an unreadable link or missing HEAD.
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
            if !valid {
                return Ok(None);
            }
            let git_common_dir =
                if policy == DiscoveryPolicy::Footer && current.join(".git").is_dir() {
                    git_dir.clone()
                } else {
                    git_common_dir_for_git_dir(&git_dir, policy)?
                };
            return Ok(Some(GitPaths {
                repo_root: current,
                git_dir,
                git_common_dir,
            }));
        }
        let parent = PathBuf::from(dirname(&current.to_string_lossy()));
        if parent == current {
            return Ok(None);
        }
        current = parent;
    }
}

/// Locate a directory's metadata marker, including standalone bare layouts.
pub(super) fn git_dir_for_repo_root(root: &Path) -> io::Result<Option<PathBuf>> {
    git_dir_for_root(root, DiscoveryPolicy::Workspace)
}

/// Locate metadata with the caller's worktree marker syntax.
fn git_dir_for_root(root: &Path, policy: DiscoveryPolicy) -> io::Result<Option<PathBuf>> {
    let marker = root.join(".git");
    match std::fs::metadata(&marker) {
        Ok(metadata) if metadata.is_dir() => return Ok(Some(marker)),
        Ok(metadata) if metadata.is_file() => {
            let contents = read_text(&marker)?;
            let prefix = match policy {
                DiscoveryPolicy::Workspace => "gitdir:",
                DiscoveryPolicy::Footer => "gitdir: ",
            };
            return trim(&contents)
                .strip_prefix(prefix)
                .map(|target| resolve_path(root, trim(target)))
                .transpose();
        }
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok((root.join("HEAD").is_file()
        && root.join("objects").is_dir()
        && root.join("refs").is_dir()
        && git_dir_is_bare(root))
    .then(|| root.to_path_buf()))
}

/// Resolve shared metadata with caller-specific unreadable-link handling.
fn git_common_dir_for_git_dir(git_dir: &Path, policy: DiscoveryPolicy) -> io::Result<PathBuf> {
    match read_text(&git_dir.join("commondir")) {
        Ok(contents) => resolve_path(git_dir, trim(&contents)),
        Err(error)
            if error.kind() == io::ErrorKind::NotFound || policy == DiscoveryPolicy::Workspace =>
        {
            Ok(git_dir.to_path_buf())
        }
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

/// Extract a simple case-insensitive repository configuration value.
fn read_git_config_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = read_text(path).ok()?;
    let mut in_section = false;
    for line in contents.lines().map(trim) {
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.split_once(']'))
            .map(|(name, _)| trim(name))
            .filter(|name| !name.contains('"'))
        {
            in_section = name.eq_ignore_ascii_case(section);
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((name, value)) = line.split_once('=')
            && trim(name).eq_ignore_ascii_case(key)
        {
            return Some(trim(strip_git_config_comment(value)).to_owned());
        }
    }
    None
}

/// Drop a configuration comment only when separated from its value by whitespace.
fn strip_git_config_comment(value: &str) -> &str {
    let value = trim(value);
    for marker in ['#', ';'] {
        if let Some((prefix, _)) = value.split_once(marker)
            && prefix
                .chars()
                .next_back()
                .is_some_and(maestro_text::is_whitespace_scalar)
        {
            return prefix;
        }
    }
    value
}
