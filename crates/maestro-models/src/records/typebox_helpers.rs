//! Plain string-enum schemas without alternative or constant branches.
/// Retain enum order and duplicates, omitting only absent or empty string options.
pub fn string_enum(
    values: &[String],
    description: Option<&str>,
    default: Option<&str>,
) -> serde_json::Value {
    let mut result = serde_json::json!({"type":"string", "enum":values});
    if let Some(v) = description.filter(|v| !v.is_empty()) {
        result["description"] = v.into();
    }
    if let Some(v) = default.filter(|v| !v.is_empty()) {
        result["default"] = v.into();
    }
    result
}
