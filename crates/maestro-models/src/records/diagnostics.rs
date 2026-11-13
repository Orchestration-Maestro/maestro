//! Creation of attributed diagnostics.
pub use maestro_http::timestamp_now;
pub use maestro_request::diagnostics::*;
/// Create a diagnostic using current Unix epoch milliseconds and supplied fields.
#[must_use]
pub fn create_assistant_message_diagnostic(
    kind: &str,
    error: DiagnosticInput<'_>,
    details: Option<super::types::JsonObject>,
) -> AssistantMessageDiagnostic {
    let timestamp = timestamp_now();
    AssistantMessageDiagnostic {
        r#type: kind.to_owned(),
        timestamp,
        error: Some(extract_diagnostic_error(error)),
        details,
    }
}

/// Append a diagnostic without removing prior entries.
pub fn append_assistant_message_diagnostic(
    message: &mut super::types::AssistantMessage,
    diagnostic: AssistantMessageDiagnostic,
) {
    message
        .diagnostics
        .get_or_insert_with(Vec::new)
        .push(diagnostic);
}
