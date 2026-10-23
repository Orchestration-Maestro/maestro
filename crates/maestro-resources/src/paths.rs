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

/// Join the rest of a leading `~` of `path` to the home directory, which `home` reads only
/// then, and answer with its text spelling; any other path is returned as written.
///
/// The spelling is normalized lexically, and text cannot hold a home directory's native
/// bytes, so it replaces those bytes. A caller that needs the written segments or the
/// native directory reads [`expand_tilde`] itself.
///
/// # Errors
/// Returns the error of `home`.
pub fn expand_tilde_text(
    path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    if !path.starts_with('~') {
        return Ok(path.to_owned());
    }
    let home = || home().map(PathBuf::from);
    let expanded = expand_tilde(Path::new(path), home)?;
    let spelling = expanded.to_string_lossy();
    Ok(if spelling.is_empty() {
        String::new()
    } else {
        maestro_path::normalize(&spelling)
    })
}

/// Join the rest of a leading `~` of `path` to the home directory, which `home` reads only
/// then; any other path is returned as written.
///
/// The rest keeps every segment it was written with, so a written parent segment still
/// crosses the link the file system holds there.
///
/// # Errors
/// Returns the error of `home`.
pub fn expand_tilde(
    path: &Path,
    home: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    match path
        .as_os_str()
        .to_str()
        .and_then(|text| text.strip_prefix('~'))
    {
        Some(rest) => Ok(append(home()?, rest)),
        None => Ok(path.to_owned()),
    }
}

/// Append `rest` to `home` behind one separator, keeping every segment it was written with.
fn append(home: PathBuf, rest: &str) -> PathBuf {
    let rest = rest.trim_start_matches(maestro_path::is_separator);
    if rest.is_empty() {
        return home;
    }
    let mut expanded = OsString::from(home);
    expanded.push(std::path::MAIN_SEPARATOR_STR);
    expanded.push(rest);
    PathBuf::from(expanded)
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
