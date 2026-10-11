//! Synchronous manifest expansion using the shared compiled matcher.
//!
//! Source attribution and [BlueOak-1.0.0](https://blueoakcouncil.org/license/1.0.0)
//! license details are in [third-party notices](https://github.com/Orchestration-Maestro/maestro/blob/main/THIRD-PARTY-NOTICES.md).
mod path_scurry;
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
    #[cfg(test)]
    let platform = TEST_PLATFORM.with(|selected| selected.get().unwrap_or(platform));
    let matcher = Minimatch::new(pattern, MinimatchOptions::Filesystem(platform))
        .map_err(io::Error::other)?;
    walker::walk(&matcher, root, platform, operations)
}
#[cfg(test)]
mod tests;

#[cfg(test)]
thread_local! {
    /// Private host-policy substitution for public-operation witnesses.
    static TEST_PLATFORM: std::cell::Cell<Option<Platform>> = const { std::cell::Cell::new(None) };
}
