//! Case-based helpers for launches still built outside [`TestProcess`].

use std::process::Output;

use crate::proof::check_output;
use crate::{TestProcess, TestProcessError, selection};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Environment variable naming the case a child was started for.
const CASE: &str = "MAESTRO_CHILD_CASE";

/// Name the case this process was started for, or `None` in the parent test process.
#[must_use]
pub fn child_case() -> Option<String> {
    std::env::var(CASE).ok()
}

/// Resolve the current libtest case without changing corpus-selection keys.
///
/// # Errors
///
/// Fails when the thread is unnamed or `test` is not the running test.
pub fn selector(test: &str) -> TestResult<String> {
    Ok(selection::current_name(test)?)
}

/// Require the start prologue, the named test's success line and one passed test.
///
/// # Errors
///
/// Fails when the output does not show `test` started and passed as the only test.
pub fn assert_executed(output: &Output, test: &str) -> Result<(), TestProcessError> {
    check_output(test, output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains(&format!("test {test} ... ok")) {
        return Err(
            TestProcessError::new(test, "the selected child test did not pass")
                .with_streams(&output.stdout, &output.stderr),
        );
    }
    if !stdout.contains("test result: ok. 1 passed;") {
        return Err(
            TestProcessError::new(test, "the child did not execute one test")
                .with_streams(&output.stdout, &output.stderr),
        );
    }
    Ok(())
}

/// Run `test` again in a child whose environment holds only `variables` and the case marker,
/// plus `LLVM_PROFILE_FILE` when this process has one, returning what the child wrote.
///
/// A `variables` entry replaces the inherited profile path, and the case marker replaces a
/// `variables` entry of the same name. An unsuccessful child is returned, not an error.
///
/// # Errors
///
/// Fails when `test` is not the running test or the child does not start it.
pub fn capture(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult<Output> {
    Ok(TestProcess::current(test)?
        .nocapture()
        .env_clear()
        .envs(variables.iter().copied())
        .env(CASE, case)
        .output()?)
}

/// Run `test` again as in [`capture`] and require that it succeeds and passes as one test.
///
/// # Errors
///
/// As [`capture`], and when the child fails or does not report the named test passing.
pub fn rerun(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult {
    let name = selector(test)?;
    let output = capture(test, case, variables)?;
    if !output.status.success() {
        return Err(TestProcessError::new(&name, format!("case {case} failed"))
            .with_streams(&output.stdout, &output.stderr)
            .into());
    }
    Ok(assert_executed(&output, &name)?)
}
