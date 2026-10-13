//! Newline conversion against recorded results.
#![cfg(test)]
use maestro_text::normalize_to_lf;
use serde::Deserialize;

/// One recorded conversion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: String,
    expected: String,
}

#[test]
fn newline_normalization_preserves_other_scalars() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/newline-cases.json")).expect("fixture");
    assert_eq!(cases.len(), 9);
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(normalize_to_lf(&case.input), case.expected, "case {index}");
    }
}
