//! The trailing-comma transform's whitespace domain.
use super::strip_comments;

/// One recorded transform.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Authored text.
    input: String,
    /// Complete transformed text.
    expected: String,
}

#[test]
fn trailing_comma_transform_keeps_its_whitespace_domain() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../tests/fixtures/trailing_commas.json")).unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        assert_eq!(
            strip_comments(&case.input),
            case.expected,
            "{:?}",
            case.input
        );
    }
}
