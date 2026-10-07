//! Supplied thrown values and diagnostic records.
use std::sync::Arc;

/// Error-instance metadata, distinct from ordinary thrown values.
#[derive(Clone, Debug)]
pub struct Error {
    /// Supplied error name.
    pub name: String,
    /// Supplied error message.
    pub message: String,
    /// Supplied stack, including an empty stack.
    pub stack: Option<String>,
    /// Supplied arbitrary code value.
    pub code: Option<ThrownValue>,
    /// Supplied arbitrary operating-system error number.
    pub errno: Option<ThrownValue>,
    /// Supplied arbitrary cause.
    pub cause: Option<ThrownValue>,
}
/// An arbitrary thrown value or supplied string-conversion callback.
#[derive(Clone)]
pub enum ThrownValue {
    /// Error-instance metadata.
    Error(Box<Error>),
    /// The undefined primitive.
    Undefined,
    /// JSON-compatible value.
    Json(serde_json::Value),
    /// Binary64 value, including nonfinite values.
    Number(f64),
    /// Caller-supplied string conversion.
    #[cfg(not(target_arch = "wasm32"))]
    StringCoercion(Arc<dyn Fn() -> Result<String, ThrownValue> + Send + Sync>),
    /// Caller-supplied string conversion.
    #[cfg(target_arch = "wasm32")]
    StringCoercion(Arc<dyn Fn() -> Result<String, ThrownValue>>),
}
impl std::fmt::Debug for ThrownValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error(v) => f.debug_tuple("Error").field(v).finish(),
            Self::Undefined => f.write_str("Undefined"),
            Self::Json(v) => f.debug_tuple("Json").field(v).finish(),
            Self::Number(v) => f.debug_tuple("Number").field(v).finish(),
            Self::StringCoercion(_) => f.write_str("StringCoercion"),
        }
    }
}
/// Diagnostic code retains only string or numeric codes.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum DiagnosticCode {
    /// Supplied string code.
    String(String),
    /// Supplied numeric code.
    Number(f64),
}
/// Extracted error-instance metadata or ordinary thrown-value description.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticErrorInfo {
    /// Nonempty error name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Converted message.
    pub message: String,
    /// Supplied stack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// String or numeric error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
}
/// Timestamped supplied diagnostic data.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AssistantMessageDiagnostic {
    /// Supplied diagnostic kind.
    pub r#type: String,
    /// Unix milliseconds sampled before conversion.
    pub timestamp: f64,
    /// Extracted error metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticErrorInfo>,
    /// Unmodified supplied details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "super::types::serialize_json")]
    pub details: Option<serde_json::Map<String, serde_json::Value>>,
}
pub(crate) fn error(message: String) -> ThrownValue {
    ThrownValue::Error(Box::new(Error {
        name: "Error".into(),
        message,
        stack: None,
        code: None,
        errno: None,
        cause: None,
    }))
}

/// Convert a thrown value using string coercion, propagating supplied coercion errors.
pub fn format_thrown_value(value: &ThrownValue) -> Result<String, ThrownValue> {
    Ok(match value {
        ThrownValue::Error(e) => {
            if e.message.is_empty() {
                e.name.clone()
            } else {
                e.message.clone()
            }
        }
        ThrownValue::Undefined => "undefined".into(),
        ThrownValue::Json(v) => json_string(v)?,
        ThrownValue::Number(v) => ryu_js::Buffer::new().format(*v).into(),
        ThrownValue::StringCoercion(f) => return f(),
    })
}
fn json_string(value: &serde_json::Value) -> Result<String, ThrownValue> {
    Ok(match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(v) => ryu_js::Buffer::new().format(v.as_f64().unwrap()).into(),
        serde_json::Value::String(v) => v.clone(),
        serde_json::Value::Object(object) => {
            if object.contains_key("toString") {
                return Err(ThrownValue::Error(Box::new(Error {
                    name: "TypeError".into(),
                    message: "Cannot convert object to primitive value".into(),
                    stack: None,
                    code: None,
                    errno: None,
                    cause: None,
                })));
            }
            "[object Object]".into()
        }
        serde_json::Value::Array(v) => v
            .iter()
            .map(|v| {
                if v.is_null() {
                    Ok(String::new())
                } else {
                    json_string(v)
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
    })
}

/// Extract error metadata, retaining only string and binary64 code values.
pub fn extract_diagnostic_error(value: &ThrownValue) -> Result<DiagnosticErrorInfo, ThrownValue> {
    if let ThrownValue::Error(e) = value {
        let code = match &e.code {
            Some(ThrownValue::Json(serde_json::Value::String(v))) => {
                Some(DiagnosticCode::String(v.clone()))
            }
            Some(ThrownValue::Json(serde_json::Value::Number(v))) => {
                v.as_f64().map(DiagnosticCode::Number)
            }
            Some(ThrownValue::Number(v)) => Some(DiagnosticCode::Number(*v)),
            _ => None,
        };
        Ok(DiagnosticErrorInfo {
            name: (!e.name.is_empty()).then(|| e.name.clone()),
            message: format_thrown_value(value)?,
            stack: e.stack.clone(),
            code,
        })
    } else {
        Ok(DiagnosticErrorInfo {
            name: Some("ThrownValue".into()),
            message: format_thrown_value(value)?,
            stack: None,
            code: None,
        })
    }
}

/// Sample Unix milliseconds before converting the supplied error and retaining details.
pub fn create_assistant_message_diagnostic(
    r#type: String,
    error: &ThrownValue,
    details: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<AssistantMessageDiagnostic, ThrownValue> {
    #[cfg(not(target_arch = "wasm32"))]
    let timestamp = unix_milliseconds(std::time::SystemTime::now());
    #[cfg(target_arch = "wasm32")]
    let timestamp = js_sys::Date::now();
    at_time(r#type, error, details, timestamp)
}
#[cfg(not(target_arch = "wasm32"))]
fn unix_milliseconds(time: std::time::SystemTime) -> f64 {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(error) => -(error.duration().as_millis() as f64),
    }
}
fn at_time(
    r#type: String,
    error: &ThrownValue,
    details: Option<serde_json::Map<String, serde_json::Value>>,
    timestamp: f64,
) -> Result<AssistantMessageDiagnostic, ThrownValue> {
    Ok(AssistantMessageDiagnostic {
        r#type,
        timestamp,
        error: Some(extract_diagnostic_error(error)?),
        details,
    })
}
/// Append in order without rewriting existing diagnostics or emitting log output.
pub fn append_assistant_message_diagnostic(
    diagnostics: &mut Option<Vec<AssistantMessageDiagnostic>>,
    diagnostic: AssistantMessageDiagnostic,
) {
    let mut values = diagnostics.take().unwrap_or_default();
    values.push(diagnostic);
    *diagnostics = Some(values);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn diagnostic_timestamp_accepts_pre_epoch_clock() {
        let clock = || std::time::UNIX_EPOCH - std::time::Duration::from_millis(1234);
        let diagnostic = at_time(
            "kind".into(),
            &ThrownValue::Undefined,
            None,
            unix_milliseconds(clock()),
        )
        .unwrap();
        assert_eq!(diagnostic.timestamp, -1234.0);
    }
    #[test]
    fn diagnostics_timestamp_uses_supplied_clock() {
        for timestamp in [0.0, 1700000000123.0] {
            assert_eq!(
                at_time("kind".into(), &ThrownValue::Undefined, None, timestamp)
                    .unwrap()
                    .timestamp,
                timestamp
            );
        }
    }
}
