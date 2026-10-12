//! Deterministic whole-UUID collision witnesses.
#![cfg(test)]
use super::generate_id;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

/// Consumed private collision case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Unique corpus label.
    id: String,
    /// Owning witness.
    test: String,
    /// Private operation name.
    operation: String,
    /// Occupied prefixes.
    existing: Vec<String>,
    /// Complete candidate stream.
    candidates: Vec<String>,
    /// Returned identity and consumed count.
    expected: Expected,
}
/// Complete collision result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Returned short or full identity.
    id: String,
    /// Number of whole UUIDs consumed.
    calls: usize,
}
/// Consume a single collision case, checking the result and exhaustion.
fn collision(name: &str) {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/session_history.json")).unwrap();
    let cases: Vec<Case> = rows
        .into_iter()
        .filter(|row| row["operation"] == "generate_id")
        .map(|row| serde_json::from_value(row).unwrap())
        .collect();
    assert_eq!(cases.len(), 4);
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert_eq!(selected.len(), 1);
    let case = &selected[0];
    assert_eq!(case.operation, "generate_id");
    assert!(case.id.starts_with("id-"));
    let mut candidates = case
        .candidates
        .iter()
        .map(|candidate| Uuid::parse_str(candidate).unwrap());
    let mut calls = 0;
    let result = generate_id(
        |id| case.existing.iter().any(|existing| existing == id),
        || {
            calls += 1;
            candidates.next().unwrap()
        },
    );
    assert_eq!(
        result, case.expected.id,
        "case {} returned identity",
        case.id
    );
    assert_eq!(
        calls, case.expected.calls,
        "case {} candidate count",
        case.id
    );
    assert_eq!(
        candidates.next(),
        None,
        "case {} candidate exhaustion",
        case.id
    );
}

#[test]
fn entry_id_first_candidate_is_short() {
    collision("entry_id_first_candidate_is_short");
}
#[test]
fn entry_id_retries_existing_prefix() {
    collision("entry_id_retries_existing_prefix");
}
#[test]
fn entry_id_accepts_the_hundredth_candidate() {
    collision("entry_id_accepts_the_hundredth_candidate");
}
#[test]
fn entry_id_falls_back_after_one_hundred_collisions() {
    collision("entry_id_falls_back_after_one_hundred_collisions");
}
