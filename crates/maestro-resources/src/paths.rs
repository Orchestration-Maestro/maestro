//! Resource path resolution, classification and real-path fallback.

use crate::{
    current_directory, drive_directories, frontmatter::text_whitespace, skills::real_path,
};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// The project and user configuration directory name.
pub const CONFIG_DIR_NAME: &str = ".maestro";

/// Classify local paths, excluding the known lowercase package and URL prefixes.
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim_matches(text_whitespace);
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// Resolve each operand group from right to left, reading the process working
/// directory and the drive directories only when an operand leaves a part open.
///
/// Every group sees one reading of those directories, and the working directory
/// reaches its text through [`current_directory`].
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve_groups(groups: &[&[&str]]) -> io::Result<Vec<String>> {
    resolve_groups_with(groups, current_directory)
}

/// Resolve each operand group, reading the working directory through
/// `current_directory`, which keeps the caller's own text conversion.
///
/// # Errors
/// Returns the error `current_directory` returned, unchanged, when a group
/// needs the working directory.
pub fn resolve_groups_with(
    groups: &[&[&str]],
    current_directory: impl FnOnce() -> io::Result<String>,
) -> io::Result<Vec<String>> {
    maestro_path::resolve_with(groups, drive_directories, current_directory)
}

/// Resolve `paths` from right to left, reading the process working directory and the
/// drive directories only when the operands leave a part open.
///
/// Each resolution reads them anew.
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve(paths: &[&str]) -> io::Result<String> {
    resolve_groups(&[paths]).map(|resolved| resolved.into_iter().collect::<String>())
}

/// The home directory and the rest written after a leading `~` of `path`, reading `home` only
/// for a path that starts with `~`; `None` for any other path.
///
/// The rest has to be text, so a path whose bytes text cannot hold is not a home path.
///
/// # Errors
/// Returns the error of `home`.
pub fn tilde_parts(
    path: &Path,
    home: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<Option<(PathBuf, &str)>> {
    match path
        .as_os_str()
        .to_str()
        .and_then(|text| text.strip_prefix('~'))
    {
        Some(rest) => Ok(Some((home()?, rest))),
        None => Ok(None),
    }
}

/// Join the rest of a leading `~` of `path` to the home directory exactly as written: an exact
/// `~` gives the home directory, and a `~/` rest gives the home followed by every byte it was
/// written with, separators and parent segments included.
///
/// Any other path, a `~` followed by text included, is returned as written.
///
/// # Errors
/// Returns the error of `home`.
pub fn expand_tilde(
    path: &Path,
    home: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    match tilde_parts(path, home)? {
        Some((home, rest)) if rest.is_empty() || rest.starts_with('/') => {
            let mut expanded = OsString::from(home);
            expanded.push(rest);
            Ok(PathBuf::from(expanded))
        }
        _ => Ok(path.to_owned()),
    }
}

/// Join the rest of a leading `~` of `path` to the home directory as text: the rest drops the
/// separator that introduced it, joins below the home, and the result is normalized lexically,
/// so `.`, `..` and repeated separators resolve.
///
/// An empty home spelling keeps the written rest, its separator included; any other path is
/// returned as written.
///
/// # Errors
/// Returns the error of `home`.
pub fn expand_tilde_text(
    path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    let Some((home, rest)) = tilde_parts(Path::new(path), || home().map(PathBuf::from))? else {
        return Ok(path.to_owned());
    };
    let home = home.to_string_lossy();
    if home.is_empty() {
        return Ok(rest.to_owned());
    }
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    Ok(maestro_path::join(&[home.as_ref(), rest]))
}

/// Compare preserved target spelling against a root resolved from the process
/// directories, on a separator boundary: `root` itself or any path below it.
///
/// # Errors
/// Returns the working directory's error when resolving `root` needs it.
pub fn is_under_path(target: &str, root: &str) -> io::Result<bool> {
    let root = resolve(&[root])?;
    let sep = maestro_path::SEP;
    Ok(target == root || target.starts_with(&format!("{}{sep}", root.trim_end_matches(sep))))
}

/// Return the real path or retain the original spelling when resolving it fails.
#[must_use]
pub fn canonicalize_path(path: &str) -> String {
    real_path(Path::new(path)).map_or_else(
        |_| path.to_owned(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}
