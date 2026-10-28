//! Isolated test execution on a fresh Windows console.

use std::process::Command;

use crate::TestProcessError;

/// Run `scenario` in a copy of the calling test attached to a fresh console.
///
/// `configure` runs in the console host and chooses the scenario's standard handles.
/// Standard error stays captured for diagnostics and completion proof. The calling
/// process checks both exit status and completion, so an early successful exit fails.
///
/// # Errors
///
/// Fails when the test cannot be selected, launched or proven to have finished, or
/// when the console host or scenario fails.
pub fn on_console(
    test: &str,
    _configure: impl FnOnce(&mut Command) -> &mut Command,
    _scenario: impl FnOnce(),
) -> Result<(), TestProcessError> {
    Err(TestProcessError::new(
        test,
        "console execution is unavailable",
    ))
}
