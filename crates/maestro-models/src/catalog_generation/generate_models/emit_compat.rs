//! Emission of complete compatibility-option objects.
use crate::ModelCompat;
use serde_json::Value;
/// Emit the complete ordered object without a runtime JSON decoder.
pub(super) fn compat(compat: Option<&ModelCompat>) -> String {
    compat.map_or_else(
        || "None".into(),
        |compat| format!("Some(crate::ModelCompat({}))", object(&compat.0)),
    )
}
/// Emit ordered object members as native value constructors.
fn object(value: &serde_json::Map<String, Value>) -> String {
    let fields = value
        .iter()
        .map(|(key, value)| format!("({key:?}.into(), {})", expression(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{fields}].into_iter().collect()")
}
/// Emit each native JSON value, retaining list order and repeated elements.
fn expression(value: &Value) -> String {
    match value {
        Value::Null => "serde_json::Value::Null".into(),
        Value::Bool(value) => format!("serde_json::Value::Bool({value})"),
        Value::String(value) => format!("serde_json::Value::String({value:?}.into())"),
        Value::Number(value) => format!("serde_json::Value::from({})", number(value)),
        Value::Object(value) => format!("serde_json::Value::Object({})", object(value)),
        Value::Array(value) => format!(
            "serde_json::Value::Array(vec![{}])",
            value.iter().map(expression).collect::<Vec<_>>().join(",")
        ),
    }
}

/// Emit native integer or floating-point constructors without losing zero signs.
fn number(value: &serde_json::Number) -> String {
    if let Some(value) = value.as_i64() {
        return format!(
            "{}_i64",
            super::emit::separate_digits(&value.to_string(), true)
        );
    }
    if let Some(value) = value.as_u64() {
        return format!(
            "{}_u64",
            super::emit::separate_digits(&value.to_string(), true)
        );
    }
    super::emit::number_text(&value.to_string())
}
