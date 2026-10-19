//! Raw display operands for the offline viewer.
use serde_json::Value;

/// Borrow a string, keeping absent/null distinct from invalid display types.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Prepared for offline message consumers")
)]
pub(crate) fn str(value: Option<&Value>) -> Option<&str> {
    match value {
        None | Some(Value::Null) => Some(""),
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// Restrict the shared suffix registry to viewer-admitted languages.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Prepared for offline tool consumers")
)]
pub(crate) fn get_language_from_path(file_path: &str) -> Option<&'static str> {
    let suffix = file_path.rsplit('.').next()?.to_lowercase();
    matches!(
        suffix.as_str(),
        "ts" | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "rb"
            | "rs"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "php"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
            | "html"
            | "css"
            | "scss"
            | "json"
            | "yaml"
            | "yml"
            | "xml"
            | "md"
            | "dockerfile"
    )
    .then(|| maestro_theme::get_language_from_path(&suffix))
    .flatten()
}

/// Avoid native time observation for absent or falsy operands.
/// # Errors
/// Propagates the owning locale observer's original failure.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Prepared for offline message consumers")
)]
pub(crate) fn format_timestamp(
    timestamp: Option<&Value>,
    localize: &dyn Fn(&Value) -> std::io::Result<String>,
) -> std::io::Result<String> {
    match timestamp.filter(|value| maestro_app::is_truthy(value)) {
        None => Ok(String::new()),
        Some(value) => localize(value),
    }
}

/// Borrow the first matching result, reading message receivers only when reached.
/// # Errors
/// Returns the shared invalid-data failure for a reached missing/null message.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Prepared for offline tool consumers")
)]
pub(crate) fn find_tool_result<'a>(
    document: &'a super::SessionDocument,
    tool_call_id: &str,
) -> std::io::Result<Option<&'a Value>> {
    let entries = document
        .data
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
    for entry in entries {
        if entry["type"] == "message" {
            let message = maestro_app::export_html::template::read_message(entry)?;
            if message["role"] == "toolResult"
                && message["toolCallId"].as_str() == Some(tool_call_id)
            {
                return Ok(Some(message));
            }
        }
    }
    Ok(None)
}

/// Observe native local time with the viewer's two-digit hour, minute and second options.
/// # Errors
/// Propagates native conversion and locale-operation failures.
#[cfg(target_arch = "wasm32")]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Prepared for offline message consumers")
)]
pub(crate) fn localize_timestamp(timestamp: &serde_json::Value) -> std::io::Result<String> {
    use crate::browser::json::encode_json;
    use wasm_bindgen::JsValue;
    let date = js_sys::Date::new(&encode_json(timestamp, |e| {
        std::io::Error::other(format!("{e:?}"))
    })?);
    let options = js_sys::Object::new();
    for field in ["hour", "minute", "second"] {
        js_sys::Reflect::set(&options, &field.into(), &"2-digit".into())
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    }
    let f = js_sys::Reflect::get(&date, &"toLocaleTimeString".into())
        .and_then(wasm_bindgen::JsCast::dyn_into::<js_sys::Function>)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    f.call2(&date, &JsValue::UNDEFINED, &options)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?
        .as_string()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))
}
