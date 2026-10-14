//! Checks the workspace dependency graph and bounded source ownership.

use std::path::Path;

/// Declaration ownership and extension interface checks.
mod boundaries;
/// Rust comment content checks.
mod comments;
/// The lint exception of the generated extension bindings.
mod generated;
/// Workspace membership and dependency graph checks.
mod graph;
/// Single-target integration-test packaging checks.
mod integration_tests;
/// Source-size and protected-lint checks.
mod quality;
/// Repeated documentation and test source layout checks.
mod repetition;
/// Workspace Rust source loading and lexical helpers.
mod source;
/// Test-owned timed-wait checks.
mod time_limits;

/// Checks names, membership, dependencies and bounded source/build ownership.
///
/// Declared edges cover all features and targets. The resolved pass uses default
/// features for the compiler host so it reads only crates a normal build fetched.
/// Complete direct sets are checked over both passes; absent future owners are
/// inactive. Source/WIT checks are structural and do not replace semantic review.
///
/// Recognized test-owned timer calls receive file/line diagnostics. Timer aliases
/// apply file-wide; local shadowing is not modelled. Production deadlines remain
/// allowed. This scan does not expand macros, infer receiver types, resolve
/// arbitrary re-exports/function values or evaluate compound configurations.
///
/// Migrated integration-test roots are checked structurally for one runnable
/// target and unconditional out-of-line declarations of top-level test files.
///
/// Reads workspace manifests and Rust files, invoking Cargo and the compiler
/// for metadata. Emits no output. Legal included fragments are counted without
/// requiring a full-module parse; macros are not expanded.
///
/// The canonical workspace `serde_json` entry must name `serde_json` and explicitly
/// enable `float_roundtrip`. Normal member declarations, including aliases, targets
/// and optional entries, must inherit an entry enabling it. Dev/build declarations
/// are outside member inheritance enforcement and cannot supply the required
/// workspace feature; test feature unification can hide missing normal precision.
///
/// # Errors
///
/// Returns a human-readable diagnostic on malformed input or a violated boundary.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = graph::check(root)?;
    let members = source::load(&metadata)?;
    integration_tests::check(&members)?;
    boundaries::check(&members)?;
    comments::check(&members)?;
    repetition::check(&members)?;
    quality::check(root, &members)?;
    time_limits::check(&members)
}
