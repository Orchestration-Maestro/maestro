//! Strict corpus records and application construction.
#![cfg(test)]
use serde::Deserialize;
use serde_json::Value;

/// Recorded source observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Complete rows.
    pub rows: Vec<Row>,
}
/// One source query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Selecting operation.
    pub suite: String,
    /// Complete supplied operands.
    pub input: Value,
    /// Complete expected result.
    pub expected: Value,
}
/// Load all source rows.
pub fn corpus() -> Vec<Row> {
    serde_json::from_str::<Corpus>(include_str!("../fixtures/attachments.json"))
        .unwrap()
        .rows
}
/// Compare JSON with the model boundary's numeric spelling.
pub fn equal(actual: &Value, expected: &Value) {
    assert_eq!(
        serde_json::from_str::<Value>(&maestro_models::compact_json(actual).unwrap()).unwrap(),
        serde_json::from_str::<Value>(&maestro_models::compact_json(expected).unwrap()).unwrap()
    );
}
