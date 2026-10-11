//! Strict recorded invocation decoding shared by the image tests.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// Fallible public test observation.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A consumed recorded invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Distinguishable case label.
    pub id: String,
    /// Owning public test.
    pub test: String,
    /// Operation selected at the public boundary.
    pub operation: String,
    /// Operation-specific typed input, decoded by its consumer.
    pub input: Value,
    /// Complete independently recorded expectation.
    pub expected: Value,
}

/// Select a nonempty group after rejecting repeated labels and queries.
pub fn cases(file: &str, name: &str) -> TestResult<Vec<Case>> {
    let rows: Vec<Case> = serde_json::from_str(include_str!("../fixtures/image_preparation.json"))?;
    let mut ids = HashSet::new();
    let mut queries = HashSet::new();
    for row in &rows {
        assert!(ids.insert(&row.id), "duplicate case label");
        assert!(
            queries.insert((&row.operation, row.input.to_string())),
            "duplicate query"
        );
    }
    let owner = format!("crates/maestro-tools/tests/{file}.rs::{name}");
    let selected: Vec<_> = rows.into_iter().filter(|row| row.test == owner).collect();
    assert!(!selected.is_empty(), "empty test group");
    Ok(selected)
}
