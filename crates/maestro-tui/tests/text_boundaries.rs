//! Public text classification over the shared whitespace set.
#![cfg(test)]
use maestro_tui::is_whitespace_char;
use serde::Deserialize;

/// One recorded text composition; only the any-character answer is read here.
#[derive(Deserialize)]
struct Case {
    input: String,
    any: bool,
}

/// All recorded rules.
#[derive(Deserialize)]
struct Rules {
    classification: Vec<Case>,
}

#[test]
fn text_classification_keeps_any_semantics() {
    let rules: Rules = serde_json::from_str(include_str!(
        "../../maestro-text/tests/fixtures/text_rules.json"
    ))
    .unwrap();
    assert_eq!(rules.classification.len(), 8);
    for case in rules.classification {
        assert_eq!(
            is_whitespace_char(&case.input),
            case.any,
            "{:?}",
            case.input
        );
    }
}
