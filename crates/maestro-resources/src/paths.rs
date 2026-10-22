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

/// Resolve `paths` from right to left, reading the process working directory and the
/// drive directories only when the operands leave a part open.
///
/// Each resolution reads them anew.
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve(paths: &[&str]) -> io::Result<String> {
    maestro_path::resolve_with(&[paths], drive_directories, current_directory)
        .map(|resolved| resolved.into_iter().collect::<String>())
}

/// Return the real path or retain the original spelling when resolving it fails.
#[must_use]
pub fn canonicalize_path(path: &str) -> String {
    real_path(Path::new(path)).map_or_else(
        |_| path.to_owned(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}
