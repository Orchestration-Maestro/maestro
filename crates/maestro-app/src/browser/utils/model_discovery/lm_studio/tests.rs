//! Recorded schema-domain admission.
use super::*;
use serde::Deserialize;
use serde_json::Value;

/// One downloaded-record observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Server record.
    input: Value,
    /// Recorded admission result.
    success: bool,
    /// Recorded projected descriptors.
    models: Option<Vec<maestro_models::Model>>,
}
#[test]
fn downloaded_model_schema_matches_recorded_domains() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lm_studio_discovery.json"
    ))
    .unwrap();
    let cases: Vec<Case> = serde_json::from_value(fixture["schema"].clone()).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        let result = schema::listing(&format!("[{}]", case.input));
        assert_eq!(result.is_ok(), case.success, "record {index}");
        if let Some(models) = case.models {
            let actual: Vec<_> = result
                .unwrap()
                .into_iter()
                .map(|record| record.model("http://fixture.invalid/base"))
                .collect();
            assert_eq!(actual, models, "record {index}");
        }
    }
}

#[test]
fn listing_schema_rejects_malformed_filtered_records() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lm_studio_discovery.json"
    ))
    .unwrap();
    let cases: Vec<Case> = serde_json::from_value(fixture["lists"].clone()).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        let result = schema::listing(&case.input.to_string());
        assert_eq!(result.is_ok(), case.success, "list {index}");
        if let Some(models) = case.models {
            let actual: Vec<_> = result
                .unwrap()
                .into_iter()
                .map(|record| record.model("http://fixture.invalid/base"))
                .collect();
            assert_eq!(actual, models, "list {index}");
        }
    }
}

/// Recorded authentication union.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthCase {
    /// Authentication reply.
    input: Value,
    /// Schema admission.
    success: bool,
    /// Accepted or refused discriminant after admission.
    accepted: Option<bool>,
}
#[test]
fn authentication_schema_keeps_success_and_refusal_domains() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lm_studio_discovery.json"
    ))
    .unwrap();
    let cases: Vec<AuthCase> = serde_json::from_value(fixture["auth"].clone()).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        let result = wire::authentication(&case.input.to_string());
        assert_eq!(result.is_ok(), case.success, "authentication {index}");
        if let Some(accepted) = case.accepted {
            assert_eq!(result.unwrap().is_ok(), accepted);
        }
    }
}

/// Recorded opaque server envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeCase {
    /// Complete incoming JSON.
    input: Value,
    /// Expected admission.
    success: bool,
}
#[test]
fn rpc_envelope_schema_tracks_control_and_error_domains() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lm_studio_discovery.json"
    ))
    .unwrap();
    let cases: Vec<EnvelopeCase> = serde_json::from_value(fixture["envelopes"].clone()).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(
            wire::envelope(&case.input.to_string()).is_ok(),
            case.success,
            "envelope {index}"
        );
    }
}
