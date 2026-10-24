//! Original diagnostic construction and lossless diagnostic observation.
use super::{DiagnosticErrorInfo, OAuthError};
use serde_json::Value;

/// Original diagnostic and nested cause supplied by the callback.
#[must_use]
pub fn failure() -> OAuthError {
    let mut diagnostic = DiagnosticErrorInfo::new(
        Some("CustomCredentialFailure".into()),
        "Custom credential resolver failed",
    );
    diagnostic.code = Some(maestro_models::DiagnosticCode::Text(
        "CUSTOM_FALLBACK".into(),
    ));
    let mut error: OAuthError = diagnostic.into();
    let mut cause = DiagnosticErrorInfo::new(None, "resolver cause");
    cause.code = Some(maestro_models::DiagnosticCode::Text("CAUSE".into()));
    error.cause = Some(Box::new(cause.into()));
    error
}
/// Project retained diagnostics without replacing the native error.
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
pub fn error_value(error: &OAuthError) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(&error.diagnostic)?;
    if let Some(cause) = &error.cause {
        value["cause"] = error_value(cause)?;
    }
    Ok(value)
}
