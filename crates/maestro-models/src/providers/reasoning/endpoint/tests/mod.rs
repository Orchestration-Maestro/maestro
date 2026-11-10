//! Request encoding behavior.

mod wire_shapes;

mod wire_domains;

use super::wire::encode_payload;
use crate::providers::json_text::compact_json;
use serde::Deserialize;
use serde_json::Value;

/// One consumed oracle query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning behavior test.
    test: String,
    /// Complete encoder input.
    input: Value,
    /// Complete expected output or rejection.
    expected: Expected,
}

/// Exactly one outcome.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// Successful transport text.
    Json {
        /// Expected full compact output.
        json: String,
    },
    /// Native rejection.
    Error {
        /// Whether validation fails.
        error: bool,
    },
}

/// Check every byte of an encoded request or its native failure context.
fn check(input: &Value, expected: &Expected) {
    let result = encode_payload(input);
    match expected {
        Expected::Json { json } => assert_eq!(
            compact_json(&result.unwrap()).unwrap(),
            *json,
            "input: {input}"
        ),
        Expected::Error { error } => {
            assert!(*error);
            assert!(
                result.is_err(),
                "expected rejection for {input}, got {result:?}"
            );
            let message = result.unwrap_err().message;
            let cause = message.strip_prefix("Input validation failed: ").unwrap();
            assert!(!cause.is_empty());
        }
    }
}

/// Run all unique queries assigned to one test.
fn corpus(name: &str, count: usize) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/reasoning-wire.json")).unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut selected = 0;
    for case in cases {
        assert!(seen.insert(serde_json::to_string(&case.input).unwrap()));
        if case.test == name {
            selected += 1;
            let before = case.input.clone();
            check(&case.input, &case.expected);
            assert_eq!(case.input, before);
        }
    }
    assert_eq!(selected, count);
}

mod request_payload;

mod request_corpus;

mod request_options;

mod request_transport;

mod request_sending;

mod request_diagnostics;

mod request_lifetime;

mod request_fixture;
