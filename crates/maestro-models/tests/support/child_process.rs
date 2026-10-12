//! Re-run one test in a fresh process whose environment holds only chosen variables.

use std::process::{Command, Output};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CASE: &str = "MAESTRO_CHILD_CASE";

/// Name the case this process was started for, or `None` in the parent test process.
pub fn child_case() -> Option<String> {
    std::env::var(CASE).ok()
}

/// Run `test` again in a child whose environment is `variables`, the case marker and the
/// coverage profile path, returning what the child wrote.
pub fn capture(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult<Output> {
    let mut command = Command::new(std::env::current_exe()?);
    command.env_clear();
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command
        .args(["--exact", test, "--nocapture"])
        .envs(variables.iter().copied())
        .env(CASE, case)
        .output()?)
}

/// Run `test` again as in [`capture`] and require
/// that it succeeds.
pub fn rerun(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult {
    let output = capture(test, case, variables)?;
    assert!(
        output.status.success(),
        "{test} case {case} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
