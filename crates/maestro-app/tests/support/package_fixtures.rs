//! Strict recorded package/output workflow selection.
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashSet;
/// One recorded operation with its complete input and observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I, E> {
    /// Stable case identity.
    pub id: String,
    /// Operation operands.
    pub input: I,
    /// Complete recorded result.
    pub expected: E,
}
/// Select typed cases, rejecting duplicate identities and complete inputs.
pub fn cases<I: DeserializeOwned, E: DeserializeOwned>(prefix: &str) -> Vec<Case<I, E>> {
    let raw: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/package_workflows.json")).unwrap();
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for case in &raw {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        assert!(inputs.insert(case["input"].to_string()));
    }
    raw.into_iter()
        .filter(|v| v["id"].as_str().unwrap().starts_with(prefix))
        .map(|v| serde_json::from_value(v).unwrap())
        .collect()
}
