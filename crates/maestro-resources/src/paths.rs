//! Resource path resolution, classification and real-path fallback.

use crate::{
    current_directory, drive_directories, frontmatter::text_whitespace, skills::real_path,
};
use std::{io, path::Path};

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
/// Every group sees one reading of those directories.
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve_groups(groups: &[&[&str]]) -> io::Result<Vec<String>> {
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

/// Join the rest of a leading `~` of `path` to the home directory, which `home` reads only
/// then; any other path is returned as written.
///
/// # Errors
/// Returns the error of `home`.
pub fn expand_tilde(path: &str, home: impl FnOnce() -> io::Result<String>) -> io::Result<String> {
    match path.strip_prefix('~') {
        // A bare `~` names the home directory itself, empty spelling included.
        Some("") => home(),
        Some(rest) => Ok(maestro_path::join(&[
            &home()?,
            rest.trim_start_matches('/'),
        ])),
        None => Ok(path.to_owned()),
    }
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
