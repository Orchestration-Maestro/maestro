//! Checks the workspace dependency graph and bounded source ownership.

use std::path::Path;

/// Declaration ownership and extension interface checks.
mod boundaries;
/// Case observation helper ownership checks.
mod case_helpers;
/// Exact test-selection ownership.
mod child_tests;
/// Rust comment content checks.
mod comments;
/// Exact production function-body ownership.
mod duplicates;
/// Binding manifest lint exceptions and generated extension source checks.
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
/// Test-owned timed-wait and optional polling checks.
mod time_limits;

/// Checks names, membership, dependencies and bounded source/build ownership.
///
/// Declared edges cover all features and targets. The resolved pass uses default
/// features for the compiler host so it reads only crates a normal build fetched.
/// Complete direct sets are checked over both passes; absent future owners are
/// inactive. Source/WIT checks are structural and do not replace semantic review.
///
/// Recognized test-owned timer calls and optional `futures_util::poll!` macros
/// receive file/line diagnostics. Use pinned futures with standard `Future::poll`
/// and `Context::from_waker(Waker::noop())` instead of the optional macro.
/// Qualified/imported macro paths and literal nested invocations are checked,
/// without expanding macros; ordinary function calls remain allowed. Known aliases
/// apply file-wide; local shadowing is not modelled. Production deadlines remain
/// allowed. This scan does not expand macros, infer receiver types, resolve
/// arbitrary re-exports/function values or evaluate compound configurations.
///
/// Members whose manifest sets `autotests = false` or declares an `integration`
/// test target have their integration-test roots checked structurally for one
/// runnable target and unconditional out-of-line declarations of top-level test
/// files.
///
/// The crate holding `tests/support/session_harness.rs` keeps its recorded-case
/// corpus lookup, JSON substitution walk, extension input-source selection and
/// session run-end observer in that harness; a copy of one beside it is rejected.
/// Fragment matches read the whole-file token stream, so macro arguments, closures
/// and impl methods count.
///
/// Reads workspace manifests and Rust files, invoking Cargo and the compiler
/// for metadata. Emits no output. Legal included fragments are counted without
/// requiring a full-module parse; macros are not expanded.
///
/// The feature unifier `maestro-dependencies` is dedicated, has no workspace dependencies and may be
/// declared by other members only as a non-optional normal dependency under
/// `cfg(not(target_arch = "wasm32"))`; it never satisfies a required direct dependency.
///
/// The canonical workspace `serde_json` entry must name `serde_json` and explicitly
/// enable `float_roundtrip`. Normal member declarations, including aliases, targets
/// and optional entries, must inherit an entry enabling it. Dev/build declarations
/// are outside member inheritance enforcement and cannot supply the required
/// workspace feature; test feature unification can hide missing normal precision.
/// The unifier's own direct `serde_json` entries are exempt from member inheritance.
///
/// Token-identical production function bodies with at least 30 interior tokens
/// are rejected, with every file/signature-line site reported. Only whitespace
/// and comments are ignored. Parsed test scopes, test layouts and the existing
/// generated catalog/binding sources are excluded. Macros are not expanded and
/// unparsed fragments add no candidates; no suppression or baseline is supported.
///
/// # Errors
///
/// Returns a human-readable diagnostic on malformed input or a violated boundary.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = graph::check(root)?;
    let members = source::load(&metadata)?;
    child_tests::check(&members)?;
    integration_tests::check(&members)?;
    boundaries::check(&members)?;
    case_helpers::check(&members)?;
    comments::check(&members)?;
    repetition::check(&members)?;
    quality::check(root, &members)?;
    time_limits::check(&members)?;
    duplicates::check(&members)
}
