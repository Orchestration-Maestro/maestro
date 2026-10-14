//! Re-run one test in a fresh process whose environment holds only chosen variables.

use std::process::{Command, Output};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CASE: &str = "MAESTRO_CHILD_CASE";

/// Name the case this process was started for, or `None` in the parent test process.
pub fn child_case() -> Option<String> {
    std::env::var(CASE).ok()
}

/// Resolve the current libtest case without changing corpus-selection keys.
pub fn selector(test: &str) -> TestResult<String> {
    let thread = std::thread::current();
    let name = thread.name().ok_or("unnamed test thread")?;
    if name != test && !name.ends_with(&format!("::{test}")) {
        return Err("child selector does not match the running test".into());
    }
    Ok(name.to_owned())
}

/// Require the selected case's success, not a zero-match harness exit.
pub fn assert_executed(output: &Output, test: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("test {test} ... ok")),
        "selected child test did not pass"
    );
    assert!(
        stdout.contains("test result: ok. 1 passed;"),
        "child did not execute one test"
    );
}

/// Run `test` again in a child whose environment holds only `variables` and the case marker,
/// plus `LLVM_PROFILE_FILE` when this process has one, returning what the child wrote.
///
/// A `variables` entry replaces the inherited profile path, and the case marker replaces a
/// `variables` entry of the same name.
pub fn capture(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult<Output> {
    let test = selector(test)?;
    let mut command = Command::new(std::env::current_exe()?);
    command.env_clear();
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command
        .args(["--exact", &test, "--nocapture"])
        .envs(variables.iter().copied())
        .env(CASE, case)
        .output()?;
    if output.status.success() {
        assert_executed(&output, &test);
    }
    Ok(output)
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

#[cfg(test)]
mod tests {
    #[test]
    fn mismatched_child_selector_is_rejected() {
        assert!(super::capture("not_this_test", "mismatch", &[]).is_err());
    }
}
