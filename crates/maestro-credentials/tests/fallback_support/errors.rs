//! Original diagnostic construction and lossless diagnostic observation.
use super::{DiagnosticErrorInfo, OAuthError};
use serde_json::{Value, json};

/// Original diagnostic and nested cause supplied by the callback.
pub fn failure() -> OAuthError {
    let mut error: OAuthError = serde_json::from_value::<DiagnosticErrorInfo>(json!({
        "message":"Custom credential resolver failed", "name":"CustomCredentialFailure", "code":"CUSTOM_FALLBACK"
    })).unwrap().into();
    error.cause = Some(Box::new(
        serde_json::from_value::<DiagnosticErrorInfo>(json!({
            "message":"resolver cause", "code":"CAUSE"
        }))
        .unwrap()
        .into(),
    ));
    error
}
/// Project retained diagnostics without replacing the native error.
pub fn error_value(error: &OAuthError) -> Value {
    let mut value = serde_json::to_value(&error.diagnostic).unwrap();
    if let Some(cause) = &error.cause {
        value["cause"] = error_value(cause);
    }
    value
}
