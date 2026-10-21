//! Checks of recorded rows, each in a process holding the environment its row records.

#![cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;
use maestro_test_process::run_per_environment;
use serde_json::Value;
use std::collections::BTreeMap;

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
    let (here, own): (Vec<_>, Vec<_>) = rows.iter().partition(|row| environment(row).is_none());
    let group = run_per_environment(test, own, |row| {
        environment(row).cloned().unwrap_or_default()
    })?;
    group.unwrap_or(here).into_iter().try_for_each(check)
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
/// The rows this process runs: none in the parent, which reruns the rows of each distinct
/// environment, including the empty one, together in a cleared child.
pub fn selected<'a, R>(
    test: &str,
    rows: &'a [R],
    environment: impl Fn(&R) -> TestResult<Option<Vec<(&str, &str)>>>,
) -> TestResult<Vec<&'a R>> {
    assert!(!rows.is_empty(), "{test} owns cases");
    let mut cases = Vec::new();
    for row in rows {
        let variables: BTreeMap<String, String> = environment(row)?
            .into_iter()
            .flatten()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        cases.push((row, variables));
    }
    let group = run_per_environment(test, cases, |(_, variables)| variables.clone())?;
    Ok(group.into_iter().flatten().map(|(row, _)| row).collect())
}
