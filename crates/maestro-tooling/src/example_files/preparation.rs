//! Renders source-marked component paths for nextest.

use std::{fmt::Write, io, path::PathBuf};

use super::components;

/// Build marked components and render their nextest environment entries.
///
/// Variables are unique across all roots, including the fixed author and context exports.
///
/// # Errors
/// Returns discovery, invalid or duplicate marker, source-reading or build failures.
pub fn prepare(
    folders: &[PathBuf],
    mut build: impl FnMut(&str, &str) -> io::Result<PathBuf>,
) -> io::Result<String> {
    let mut result = String::new();
    components::visit(folders, |name, variable| {
        let path = build(name, variable)?;
        writeln!(result, "{variable}={}", path.display()).map_err(io::Error::other)
    })?;
    Ok(result)
}
