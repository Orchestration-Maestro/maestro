//! Reached shared admission failures without browser display operands.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisclosureCase {
    name: String,
    input: DisclosureInput,
    #[serde(rename = "errorKind")]
    error_kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisclosureInput {
    data: Value,
}

#[test]
fn header_disclosure_admission_reports_reached_errors_in_order() {
    let cases: Vec<DisclosureCase> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/session_header_disclosure_errors.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 14);
    for row in cases {
        let doc = document(&row.input.data);
        let error = prepare(&doc, &|_| unreachable!()).unwrap_err();
        assert_eq!(row.error_kind, "invalid_data", "{}", row.name);
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{}", row.name);
        let expected = match row.name.as_str() {
            "prompt-error-wins-over-tools-error" => Some("nonstring system prompt"),
            "required-error-wins-over-null-property" => Some("invalid required list"),
            "tool-name-validated-after-parameters" => Some("null property receiver"),
            _ => None,
        };
        if let Some(expected) = expected {
            assert_eq!(error.to_string(), expected);
        }
    }
}
