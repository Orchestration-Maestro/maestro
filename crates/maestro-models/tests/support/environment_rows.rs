//! Checks of recorded rows, each in a process holding the environment its row records.

use crate::chat::TestResult;
use std::collections::{BTreeMap, BTreeSet};

/// Check each row in the process holding its environment.
///
/// A row without an environment is checked here. In the parent, each distinct environment
/// reruns `test` in a child, which checks only the rows of its environment.
///
/// # Errors
/// Returns the first row or child failure.
pub fn check_by_environment<R>(
    test: &str,
    rows: &[R],
    environment: impl Fn(&R) -> Option<&BTreeMap<String, String>>,
    check: impl Fn(&R) -> TestResult,
) -> TestResult {
    if let Ok(case) = std::env::var("MAESTRO_CHILD_CASE") {
        let wanted: BTreeMap<String, String> = serde_json::from_str(&case)?;
        for row in rows.iter().filter(|row| environment(row) == Some(&wanted)) {
            check(row)?;
        }
        return Ok(());
    }
    let mut cases = BTreeSet::new();
    for row in rows {
        match environment(row) {
            None => check(row)?,
            Some(variables) => {
                cases.insert(serde_json::to_string(variables)?);
            }
        }
    }
    for case in &cases {
        let variables: BTreeMap<String, String> = serde_json::from_str(case)?;
        let pairs: Vec<(&str, &str)> = variables
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .envs(pairs.iter().copied())
                .env("MAESTRO_CHILD_CASE", case)
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}
