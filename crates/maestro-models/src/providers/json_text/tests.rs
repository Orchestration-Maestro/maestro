//! Reading raw has no nesting limit; decoding in full reads exactly the nesting the parser's
//! recursion limit reads.

use serde_json::Value;

use super::{json_value, raw_json};

/// Whether the parser reads `text` into the JSON data model.
fn parses(text: &str) -> bool {
    serde_json::from_str::<Value>(text).is_ok()
}

#[test]
fn nesting_bound_of_decoding_is_the_parser_recursion_limit() {
    for containers in 120..=130 {
        let arrays = format!("{}null{}", "[".repeat(containers), "]".repeat(containers));
        let objects = format!(
            "{}null{}",
            r#"{"k":"#.repeat(containers),
            "}".repeat(containers)
        );
        for text in [arrays, objects] {
            let raw = raw_json(&text).expect("well-formed text reads raw at any depth");
            assert_eq!(
                json_value(raw).is_ok(),
                parses(&text),
                "{containers} containers"
            );
        }
    }
}

#[test]
fn raw_reading_keeps_text_nested_far_beyond_the_decoding_bound() {
    let containers = 100_000;
    let text = format!("{}null{}", "[".repeat(containers), "]".repeat(containers));
    let raw = raw_json(&text).expect("well-formed text reads raw at any depth");
    assert_eq!(raw.get(), text);
    assert!(json_value(raw).is_err());
}

/// The typed fixture keeps acquisition metadata out of the test data.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberCorpus {
    /// Complete parser cases consumed by the public parser test.
    #[serde(rename = "strict")]
    _strict: Vec<NumberRow>,
    /// Streaming parser cases consumed by the public parser test.
    #[serde(rename = "streaming")]
    _streaming: Vec<NumberRow>,
    /// Exact writer results.
    compact: Vec<NumberRow>,
}

/// An input and its recorded result at one boundary.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberRow {
    /// Original text.
    input: String,
    /// Recorded value or exact compact text.
    expected: Value,
    /// Sign assertion consumed by the public parser test.
    #[serde(rename = "negative_zero")]
    _negative_zero: Option<bool>,
}

#[test]
fn numeric_projection_preserves_compact_output() {
    let corpus: NumberCorpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/streamed_argument_numbers.json"
    ))
    .unwrap();
    for row in corpus.compact {
        let value = json_value(raw_json(&row.input).unwrap()).unwrap();
        assert_eq!(
            super::compact_json(&value).unwrap(),
            row.expected.as_str().unwrap()
        );
    }
}
