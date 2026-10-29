//! Workspace component selection shared by both native test hosts.

use std::path::{Path, PathBuf};

#[path = "cargo.rs"]
pub(crate) mod cargo;

/// Builds one registered example and finds the component artifact it reports.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn example_component(package: &str, name: &str) -> Result<PathBuf, String> {
    cargo::component(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &["--package", package, "--example", name],
        name,
    )
}
