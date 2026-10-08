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
