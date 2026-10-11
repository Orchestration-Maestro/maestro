//! Synchronous manifest expansion using the shared compiled matcher.
mod pattern;
mod processor;
mod walker;
use maestro_pattern::{Minimatch, MinimatchOptions, Platform};
use maestro_resources::skills::ResourceOperations;
use std::io;
/// Expands a positive authored pattern before kind-specific collection.
pub(super) fn glob_sync(
    pattern: &str,
    root: &str,
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<String>> {
    let platform = if cfg!(windows) {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::Darwin
    } else {
        Platform::Posix
    };
    let matcher = Minimatch::new(pattern, MinimatchOptions::Filesystem(platform))
        .map_err(io::Error::other)?;
    walker::walk(&matcher, root, operations)
}
#[cfg(test)]
mod tests;
