//! Creation of attributed diagnostics.
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

/// Return the current timestamp as milliseconds since the Unix epoch.
#[must_use]
pub fn timestamp_now() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or_else(
                |error| -(error.duration().as_secs_f64() * 1000.0).ceil(),
                |elapsed| (elapsed.as_secs_f64() * 1000.0).floor(),
            )
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
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
