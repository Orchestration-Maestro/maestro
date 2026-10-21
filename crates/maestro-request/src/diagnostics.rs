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

/// Supplied error fields used both as the error value a fallible operation returns and as the snapshot a diagnostic records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticErrorInfo {
    /// Optional supplied error name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Supplied message; formatting falls back to the name when this is empty.
    pub message: String,
    /// Optional supplied stack text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// Optional supplied error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
}

impl DiagnosticErrorInfo {
    /// Supplied name and message, unchanged, without a stack or code.
    #[must_use]
    pub fn new(name: Option<String>, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
            stack: None,
            code: None,
        }
    }

    /// An error named `Error` with the given message and no stack or code.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Some("Error".to_owned()), message)
    }
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
        DiagnosticInput::Text(text) => {
            DiagnosticErrorInfo::new(Some("ThrownValue".to_owned()), text)
        }
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
