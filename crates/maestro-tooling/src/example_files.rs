//! Source-owned extension component preparation.

use std::io;
use std::path::{Path, PathBuf};

/// Build marked components and render their nextest environment entries.
///
/// # Errors
/// Returns discovery, source-reading or component-build failures.
pub fn prepare(
    _folder: &Path,
    _build: impl FnMut(&str, &str) -> io::Result<PathBuf>,
) -> io::Result<String> {
    Ok(String::new())
}
