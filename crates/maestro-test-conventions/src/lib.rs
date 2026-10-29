//! Checks the workspace dependency graph and bounded source ownership.

use std::path::Path;

/// Declaration ownership and extension interface checks.
mod boundaries;
/// Exact test-selection ownership.
mod child_tests;
/// Rust comment content checks.
mod comments;
/// Exact production function-body ownership.
mod duplicates;
/// Shared-helper ownership in the extension example tests.
mod example_helpers;
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
/// Test-owned temporary directory checks.
mod temp_directories;
/// Nextest's default budget and documented slow-test exceptions.
mod test_budget;
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
/// Every member with top-level integration-test sources needs `autotests = false`
/// and one runnable `integration` target at `tests/integration.rs`, whose root
/// declares each other top-level test file as an unconditional out-of-line module.
/// Each failing member is reported.
///
/// The default nextest profile must flag a test as slow every 30 seconds and
/// terminate it after four periods, a 120-second hang limit that no profile may
/// raise and no profile may flag at another period. A per-test override is
/// accepted only for a test that needs more than that limit: it keeps the 30-second period and requires adjacent comments naming
/// measured seconds over 120, the cause and a removal issue, and its
/// `terminate-after` must be the fewest whole periods that cover those seconds.
/// Issue references are checked offline; reviewers verify that each issue is open.
/// Periods use positive whole seconds (`"Ns"`).
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
/// The shipped extension example tests decode a document member and release every
/// registration through the example support module, whose release also asserts the
/// names the caller expects and that nothing stays lent. A per-example test that
/// writes the decode or the release walk inline is reported with its file and
/// line, wherever the copy sits.
///
/// Test code in the crates the temporary-directory migration has reached may not
/// name the platform temporary directory itself: not as a call, a value or a macro
/// argument inside test-owned code, and not as an import anywhere in the crate,
/// whatever the alias. Diagnostics name `TEST_DIRECTORY_CALLERS`; the owner's
/// `temporary_root` accessor, the owner parent and an uncreated socket path remain
/// accepted. The scope grows with each migration batch and disappears when every
/// crate uses the owner.
///
/// # Errors
///
/// Returns a human-readable diagnostic on malformed input or a violated boundary.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    test_budget::check(root)?;
    let (metadata, packages) = graph::check(root)?;
    let members = source::load(&metadata)?;
    graph::packages::check(&metadata, &members, &packages)?;
    child_tests::check(&members)?;
    integration_tests::check(&members)?;
    boundaries::check(&members)?;
    comments::check(&members)?;
    repetition::check(&members)?;
    quality::check(root, &members)?;
    time_limits::check(&members)?;
    duplicates::check(&members)?;
    example_helpers::check(&members)?;
    temp_directories::check(&members)
}
