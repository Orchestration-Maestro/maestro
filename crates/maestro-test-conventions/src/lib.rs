//! Checks the workspace dependency graph and bounded source ownership.

use std::path::Path;

mod boundaries;
mod comments;
mod graph;
mod quality;
mod source;

/// Checks names, membership, dependencies and bounded source/build ownership.
///
/// Declared edges cover all features and targets. The resolved pass uses default
/// features for the compiler host so it reads only crates a normal build fetched.
/// Complete direct sets are checked over both passes; absent future owners are
/// inactive. Source/WIT checks are structural and do not replace semantic review.
///
/// Reads workspace manifests and Rust files, invoking Cargo and the compiler
/// for metadata. Emits no output. Legal included fragments are counted without
/// requiring a full-module parse; macros are not expanded.
///
/// # Errors
///
/// Returns a human-readable diagnostic on malformed input or a violated boundary.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = graph::check(root)?;
    let members = source::load(&metadata)?;
    boundaries::check(&members)?;
    comments::check(&members)?;
    quality::check(root, &members)
}
