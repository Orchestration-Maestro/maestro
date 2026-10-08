//! The raw entry reads exactly the nesting the parser's recursion limit reads.

use serde_json::Value;

use super::raw_json;

/// Whether the parser reads `text` into the JSON data model.
fn parses(text: &str) -> bool {
    serde_json::from_str::<Value>(text).is_ok()
}

#[test]
fn nesting_bound_is_the_parser_recursion_limit() {
    for containers in 120..=130 {
        let arrays = format!("{}null{}", "[".repeat(containers), "]".repeat(containers));
        let objects = format!(
            "{}null{}",
            r#"{"k":"#.repeat(containers),
            "}".repeat(containers)
        );
        for text in [arrays, objects] {
            assert_eq!(
                raw_json(&text).is_ok(),
                parses(&text),
                "{containers} containers"
            );
        }
    }
}

#[test]
fn brackets_inside_strings_are_not_containers() {
    let escaped_quote = format!(r#"{{"k":"\"{}"}}"#, "[".repeat(300));
    assert!(raw_json(&escaped_quote).is_ok());

    let after_escaped_backslash =
        format!(r#"{{"k":"\\","j":{}}}"#, "[".repeat(200) + &"]".repeat(200));
    let failure = raw_json(&after_escaped_backslash)
        .err()
        .map(|error| error.to_string());
    assert_eq!(failure.as_deref(), Some("recursion limit exceeded"));
}
