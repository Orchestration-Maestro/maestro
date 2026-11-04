//! Renders source-marked component paths for nextest.

use std::{fmt::Write, io, path::PathBuf};

use super::components::{self, Fixed};

/// Build marked components and render their nextest environment entries.
///
/// `fixed` holds the components prepared without a marker that this workspace builds. Variables
/// are unique across all roots, including every fixed component export.
///
/// # Errors
/// Returns discovery, invalid or duplicate marker, source-reading or build failures.
pub fn prepare(
    folders: &[PathBuf],
    fixed: &[Fixed],
    mut build: impl FnMut(&str, &str) -> io::Result<PathBuf>,
) -> io::Result<String> {
    let mut result = String::new();
    components::visit(folders, fixed, |name, variable| {
        let path = build(name, variable)?;
        writeln!(result, "{variable}={}", path.display()).map_err(io::Error::other)
    })?;
    Ok(result)
}
