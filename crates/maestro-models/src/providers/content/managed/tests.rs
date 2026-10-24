//! Recorded cloud endpoint and key boundary behavior.
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    operation: String,
    input: Value,
    expected: Value,
}

fn rows(operation: &str) -> impl Iterator<Item = Row> {
    serde_json::from_str::<Vec<Row>>(include_str!(
        "../../../../tests/fixtures/cloud_content_policy.json"
    ))
    .unwrap()
    .into_iter()
    .filter(move |row| row.operation == operation)
}

#[test]
fn endpoint_defaults_match_version_boundaries() {
    for row in rows("endpoint") {
        let base = super::policy::custom_endpoint(row.input.as_str().unwrap());
        assert_eq!(base, row.expected["base"].as_str());
        assert_eq!(
            base.is_some_and(super::policy::has_version),
            row.expected["version"].as_bool().unwrap()
        );
    }
}

#[test]
fn key_resolution_keeps_precedence_at_whitespace_boundaries() {
    for row in rows("key") {
        let key =
            super::policy::select_key(row.input["apiKey"].as_str(), row.input["envKey"].as_str());
        assert!(key == row.expected.as_str(), "key selection differs");
    }
}
