//! Resource path resolution, classification and real-path fallback.

use crate::{
    current_directory, drive_directories, frontmatter::text_whitespace, skills::real_path,
};
use std::{
    convert::Infallible,
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

/// Resolve an explicit instruction path: trim it, expand a leading `~` below `home`, keep an
/// absolute result and resolve any other result against `cwd`.
///
/// # Errors
/// Returns the working-directory error of a resolution that needed it.
pub(crate) fn resolve_explicit_path(path: &str, cwd: &str, home: &str) -> io::Result<String> {
    let path = path.trim_matches(text_whitespace);
    let Ok(normalized) = maestro_path::expand_tilde(path, || Ok::<_, Infallible>(home.to_owned()));
    if maestro_path::is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[cwd, &normalized])
    }
}

/// The home directory as a native path: on Unix the `HOME` variable as set, an empty one
/// included, otherwise the account entry.
#[must_use]
pub fn native_home_directory() -> Option<PathBuf> {
    maestro_path::native_homedir()
}

/// The home directory as text.
///
/// # Errors
/// Returns a not-found error when the host knows no home directory, or an invalid-data one
/// when the home directory is not valid UTF-8.
pub fn home_directory() -> io::Result<String> {
    native_home_directory()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?
        .into_os_string()
        .into_string()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "home directory is not valid UTF-8",
            )
        })
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
