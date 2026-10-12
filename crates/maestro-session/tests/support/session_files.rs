//! Strict fixture consumption and disposable native histories.
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// Entire committed corpus, with no ignored fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Every owned fixture row.
    cases: Vec<Case>,
}
/// A query and its complete expected result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Unique diagnostic label.
    pub id: String,
    /// Named owning test.
    pub test: String,
    /// Public operation input.
    pub input: Value,
    /// Complete surviving result.
    pub expected: Value,
}
/// Read strict rows, validating unique queries and real named owners.
pub fn cases(name: &str) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/session_files.json")).unwrap();
    assert_eq!(corpus.cases.len(), 94);
    let source = format!(
        "{}\n{}\n{}",
        include_str!("../session_files.rs"),
        include_str!("../session_conversions.rs"),
        include_str!("../../src/session_manager/tests.rs")
    );
    let mut ids = HashSet::new();
    let mut queries = HashSet::new();
    for case in &corpus.cases {
        assert!(ids.insert(&case.id), "duplicate label");
        assert!(
            queries.insert(serde_json::to_string(&(&case.test, &case.input)).unwrap()),
            "duplicate query"
        );
        assert!(
            source.contains(&format!("fn {}(", case.test)),
            "unowned fixture {}",
            case.id
        );
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!selected.is_empty(), "empty query group");
    selected
}
