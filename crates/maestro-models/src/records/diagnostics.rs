//! Supplied error fields and assistant diagnostic records.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A supplied textual or numeric error code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticCode {
    /// Textual code supplied by the adapter.
    Text(String),
    /// Numeric code supplied by the adapter.
    Number(f64),
}

/// Error fields supplied by an adapter, without cause-chain traversal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticErrorInfo {
    /// Optional supplied error name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Supplied message, or the name when the message is empty.
    pub message: String,
    /// Optional supplied stack text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// Optional supplied error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
}

impl fmt::Display for DiagnosticErrorInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&format_thrown_value(DiagnosticInput::Error(self)))
    }
}

impl std::error::Error for DiagnosticErrorInfo {}

/// A structured supplied error or already-formatted thrown text.
#[derive(Clone, Copy, Debug)]
pub enum DiagnosticInput<'a> {
    /// Supplied structured error fields.
    Error(&'a DiagnosticErrorInfo),
    /// Already-formatted thrown text.
    Text(&'a str),
}

/// Format the message, falling back to the supplied name for an empty message.
#[must_use]
pub fn format_thrown_value(value: DiagnosticInput<'_>) -> String {
    match value {
        DiagnosticInput::Error(error) if error.message.is_empty() => {
            error.name.clone().unwrap_or_default()
        }
        DiagnosticInput::Error(error) => error.message.clone(),
        DiagnosticInput::Text(text) => text.to_owned(),
    }
}

/// Copy supplied error fields, omitting an empty name and retaining text exactly.
#[must_use]
pub fn extract_diagnostic_error(value: DiagnosticInput<'_>) -> DiagnosticErrorInfo {
    match value {
        DiagnosticInput::Error(error) => DiagnosticErrorInfo {
            name: error.name.clone().filter(|name| !name.is_empty()),
            message: format_thrown_value(value),
            stack: error.stack.clone(),
            code: error.code.clone(),
        },
        DiagnosticInput::Text(text) => DiagnosticErrorInfo {
            name: Some("ThrownValue".to_owned()),
            message: text.to_owned(),
            stack: None,
            code: None,
        },
    }
}

/// An attributed assistant diagnostic retaining supplied error fields and details.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    /// Diagnostic category supplied by the adapter.
    pub r#type: String,
    /// Unix epoch milliseconds when the diagnostic was created.
    pub timestamp: f64,
    /// Optional supplied error fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticErrorInfo>,
    /// Optional open diagnostic details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<super::types::JsonObject>,
}

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
pub(crate) fn timestamp_now() -> f64 {
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
