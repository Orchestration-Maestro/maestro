//! Raw display operands for the offline viewer.
use serde_json::Value;

/// Borrow a string, keeping absent/null distinct from invalid display types.
pub(crate) fn str(value: Option<&Value>) -> Option<&str> {
    match value {
        None | Some(Value::Null) => Some(""),
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// Restrict the shared suffix registry to viewer-admitted languages.
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

/// Borrow the first matching result, reading message receivers only when reached.
/// # Errors
/// Returns the shared invalid-data failure for a reached missing/null message.
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

/// Shorten the viewer's lexical home prefixes without observing the host filesystem.
pub(crate) fn shorten_path(path: &str) -> std::borrow::Cow<'_, str> {
    for prefix in ["/Users/", "/home/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let suffix = rest.find('/').map_or("", |index| &rest[index..]);
            return std::borrow::Cow::Owned(format!("~{suffix}"));
        }
    }
    std::borrow::Cow::Borrowed(path)
}
