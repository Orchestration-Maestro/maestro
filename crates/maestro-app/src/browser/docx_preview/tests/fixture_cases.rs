//! Controlled Word-format observations.
#![cfg(test)]
use serde::Deserialize;
use serde_json::Value;

/// One named operation and its complete expected observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I> {
    /// Behavior group.
    pub test: String,
    /// Additional behaviors sharing this complete observation.
    #[serde(default)]
    pub also: Vec<String>,
    /// Operation arguments.
    pub input: I,
    /// Independently recorded result.
    pub expected: Value,
}

/// Read only the cases owned by this test.
pub fn cases<I: serde::de::DeserializeOwned>(source: &str, name: &str) -> Vec<Case<I>> {
    let cases: Vec<Case<I>> = serde_json::from_str(source).unwrap();
    let selected: Vec<_> = cases
        .into_iter()
        .filter(|case| case.test == name || case.also.iter().any(|test| test == name))
        .collect();
    assert!(!selected.is_empty());
    selected
}
