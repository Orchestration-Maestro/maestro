//! Rejection of nonfinite edits inside the single-owned records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::component_adapter::outcome;
use maestro_request::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo,
};
use maestro_request::types::{MaxPrice, RoutingPrice, RoutingThreshold};
use serde::Serialize;

/// Check the real encoder rather than a predicate isolated from event return.
fn rejected(value: impl Serialize) {
    let result = outcome(Some(value), Ok(None::<String>));
    assert_eq!(
        result.event,
        Err("extension wrote a non-finite number (Infinity or NaN)".to_owned())
    );
}

#[test]
fn maestro_nonfinite_optional_numbers_cannot_silently_become_none() {
    for number in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        rejected(RoutingThreshold::Percentiles {
            p50: Some(number),
            p75: None,
            p90: None,
            p99: None,
        });
        rejected(MaxPrice {
            prompt: Some(RoutingPrice::Number(number)),
            ..MaxPrice::default()
        });
        rejected(AssistantMessageDiagnostic {
            r#type: "error".into(),
            timestamp: 0.0,
            error: Some(DiagnosticErrorInfo {
                name: None,
                message: "error".into(),
                stack: None,
                code: Some(DiagnosticCode::Number(number)),
            }),
            details: None,
        });
    }
}

#[test]
fn maestro_plain_json_null_and_number_like_text_remain_valid() {
    for value in [
        None,
        Some(RoutingPrice::Text("Infinity".into())),
        Some(RoutingPrice::Text("f64:7ff0000000000000".into())),
    ] {
        let result = outcome(Some(value), Ok(None::<String>));
        assert!(result.event.is_ok());
    }
}
