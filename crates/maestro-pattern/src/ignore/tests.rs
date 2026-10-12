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

#[test]
fn terminal_directory_globstars_select_base_and_readmission() {
    for pattern in ["a/**/", "/a/**/", "a/**/**/"] {
        let mut rules = Ignore::new();
        rules.add(pattern);
        assert!(rules.test("a", true) == TestResult::Ignored, "{pattern}");
        assert!(
            rules.test("a/sub", true) == TestResult::Ignored,
            "{pattern}"
        );
        assert!(rules.test("a", false) == TestResult::Unmatched, "{pattern}");
        rules.add(&format!("!{pattern}"));
        assert!(rules.test("a", true) == TestResult::Unignored, "!{pattern}");
    }
    let mut rules = Ignore::new();
    rules.add("a/**");
    assert_eq!(rules.test("a", true), TestResult::Unmatched);
    assert_eq!(rules.test("a/sub", true), TestResult::Ignored);
}

/// One recorded rule-text and path decision over whitespace framing.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Framing {
    /// The authored rules, one per line.
    pattern: String,
    /// The entry path.
    path: String,
    /// Whether the entry is excluded.
    expected: bool,
}

#[test]
fn recorded_ignore_rules_keep_whitespace_framing() {
    let rows: Vec<Framing> =
        serde_json::from_str(include_str!("../../tests/fixtures/ignore_characters.json")).unwrap();
    assert_eq!(rows.len(), 140);
    for (index, row) in rows.iter().enumerate() {
        let mut rules = Ignore::new();
        for line in row.pattern.replace("\r\n", "\n").split('\n') {
            rules.add(line);
        }
        assert_eq!(
            rules.test(&row.path, false) == TestResult::Ignored,
            row.expected,
            "case {index}"
        );
    }
}
