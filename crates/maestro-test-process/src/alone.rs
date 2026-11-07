//! Isolation of a live pipe transaction in a process that runs only that test.

use crate::child::repeat_stderr;
use crate::{TestProcess, TestProcessError};

/// Variable that marks the process [`run_alone`] started.
const ALONE: &str = "MAESTRO_TEST_ALONE";

/// Run the calling test alone in a fresh process, so no sibling test can fork while its pipes
/// are live.
///
/// A forked child inherits every open pipe end until it execs, so a sibling fork overlapping
/// a transaction that closes a pipe end keeps that end open. Returns `Ok(true)` in the
/// calling process once the isolated run passed; the test then returns. Returns `Ok(false)`
/// in the isolated process, where the test body runs. Call it before the body starts any
/// child whose pipes the test closes.
///
/// A passing run repeats what the body wrote to standard error on this process's standard
/// error, such as why it skipped a case, so the run shows it even while the test passes; its
/// standard output stays hidden.
///
/// # Errors
///
/// Fails when `test` is not the running test, the isolated process does not start exactly
/// this test, or the test fails there.
pub fn run_alone(test: &str) -> Result<bool, TestProcessError> {
    let process = TestProcess::current(test)?;
    if std::env::var_os(ALONE).is_some() {
        return Ok(false);
    }
    let name = process.name().to_owned();
    let output = process.env(ALONE, "1").nocapture().output()?;
    if output.status.success() {
        repeat_stderr(&name, &output.stderr)?;
        return Ok(true);
    }
    Err(TestProcessError::new(
        &name,
        format!("the isolated run failed ({})", output.status),
    )
    .with_streams(&output.stdout, &output.stderr))
}
