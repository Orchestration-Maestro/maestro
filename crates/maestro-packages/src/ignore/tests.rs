//! Qualified character decisions through the ordered rule owner.
#![cfg(test)]
use super::{Ignore, TestResult};
use std::collections::HashSet;

/// One distinct rule/path decision from the qualification corpus.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Ordered authored rules.
    rules: Vec<String>,
    /// Slash-relative entry, with a trailing slash for directories.
    input: String,
    /// Whether traversal excludes this entry.
    expected: bool,
}

#[test]
fn recorded_ignore_rules_keep_character_decisions() {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../../tests/fixtures/discovery_ignores.json")).unwrap();
    assert_eq!(rows.len(), 7_585);
    let mut queries = HashSet::new();
    for (index, row) in rows.into_iter().enumerate() {
        assert!(
            queries.insert((row.rules.clone(), row.input.clone())),
            "duplicate case {index}"
        );
        let mut rules = Ignore::new();
        for rule in &row.rules {
            rules.add(rule);
        }
        let path = row.input.trim_end_matches('/');
        let pruned = path
            .match_indices('/')
            .any(|(end, _)| rules.test(&path[..end], true) == TestResult::Ignored);
        assert_eq!(
            pruned || rules.test(path, row.input.ends_with('/')) == TestResult::Ignored,
            row.expected,
            "case {index}"
        );
    }
}
