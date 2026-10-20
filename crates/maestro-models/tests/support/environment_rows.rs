//! Checks of recorded rows, each in a process holding the environment its row records.

use crate::chat::TestResult;
use serde_json::Value;
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

/// Environment variables named by the scenario.
pub fn json_environment(row: &Value) -> TestResult<Option<Vec<(&str, &str)>>> {
    row["case"]
        .get("env")
        .map(|env| {
            env.as_object()
                .into_iter()
                .flatten()
                .map(|(k, v)| Ok((k.as_str(), v.as_str().ok_or("environment value")?)))
                .collect()
        })
        .transpose()
}
/// The rows this process runs: none in the parent, which reruns the shared-environment rows
/// together and each row with its own environment in an isolated child.
pub fn selected<'a, R>(
    test: &str,
    rows: &'a [R],
    identity: impl Fn(usize, &R) -> TestResult<String>,
    environment: impl Fn(&R) -> TestResult<Option<Vec<(&str, &str)>>>,
) -> TestResult<Vec<&'a R>> {
    assert!(!rows.is_empty(), "{test} owns cases");
    let Ok(selected) = std::env::var("MAESTRO_CHILD_CASE") else {
        {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .env("MAESTRO_CHILD_CASE", "*")
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
        for (index, row) in rows.iter().enumerate() {
            if let Some(variables) = environment(row)? {
                let output = maestro_test_process::TestProcess::current(test)?
                    .nocapture()
                    .env_clear()
                    .envs(variables.iter().copied())
                    .env("MAESTRO_CHILD_CASE", identity(index, row)?)
                    .output()?;
                assert!(output.status.success(), "{output:?}");
            }
        }
        return Ok(Vec::new());
    };
    let mut owned = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if (selected == "*" && environment(row)?.is_none()) || selected == identity(index, row)? {
            owned.push(row);
        }
    }
    Ok(owned)
}
