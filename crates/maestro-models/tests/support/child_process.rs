//! Re-run one test in a fresh process whose environment holds only chosen variables.

use std::process::Command;

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CASE: &str = "MAESTRO_CHILD_CASE";

/// Name the case this process was started for, or `None` in the parent test process.
pub fn child_case() -> Option<String> {
    std::env::var(CASE).ok()
}

/// Run `test` again in a child with exactly `variables` set and `case` as its marker.
pub fn rerun(test: &str, case: &str, variables: &[(&str, &str)]) -> TestResult {
    let mut command = Command::new(std::env::current_exe()?);
    command.env_clear();
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = command
        .args(["--exact", test, "--nocapture"])
        .envs(variables.iter().copied())
        .env(CASE, case)
        .output()?;
    assert!(
        output.status.success(),
        "{test} case {case} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
