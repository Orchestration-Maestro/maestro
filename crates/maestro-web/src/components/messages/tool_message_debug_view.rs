//! Literal debug listings with strict JSON-string projection.
#[cfg(any(target_arch = "wasm32", test))]
use crate::tools::types::ToolUiError;
use maestro_app::ToolResultMessage;
use std::rc::Rc;

/// Native debug listings, independent of result-payload presence.
#[derive(Default)]
pub struct ToolMessageDebugView {
    /// Original optional call arguments; absence displays empty literal code.
    pub call_args: Option<serde_json::Value>,
    /// Optional original result envelope, independent of the visibility flag.
    pub result: Option<Rc<ToolResultMessage>>,
    /// Whether output and details listings are visible.
    pub has_result: bool,
}
/// Preserve missing values, strict parsed strings and raw literal fallbacks.
#[cfg(any(target_arch = "wasm32", test))]
fn pretty(value: Option<&serde_json::Value>) -> Result<(String, &'static str), ToolUiError> {
    let Some(value) = value else {
        return Ok((String::new(), "json"));
    };
    let parsed;
    let value = if let Some(text) = value.as_str() {
        match maestro_app::browser::utils::parse_json(text) {
            Ok(value) => {
                parsed = value;
                &parsed
            }
            Err(_) => return Ok((text.into(), "text")),
        }
    } else {
        value
    };
    maestro_app::pretty_json(value)
        .map(|text| (text, "json"))
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(test)]
mod tests {
    use super::pretty;
    use serde_json::{Value, json};

    #[test]
    fn debug_preparation_keeps_missing_strict_and_literal_values() {
        assert_eq!(pretty(None).unwrap(), (String::new(), "json"));
        for (value, content, language) in [
            (Value::Null, "null", "json"),
            (json!("YWJj"), "YWJj", "text"),
            (json!("1e309"), "null", "json"),
            (json!("{\"n\":1e309}"), "{\n  \"n\": null\n}", "json"),
            (json!("\u{feff}null"), "\u{feff}null", "text"),
            (json!("\"raw\ncontrol\""), "\"raw\ncontrol\"", "text"),
        ] {
            assert_eq!(pretty(Some(&value)).unwrap(), (content.into(), language));
        }
    }
}
