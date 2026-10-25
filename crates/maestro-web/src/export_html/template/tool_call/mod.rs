//! Offline tool-call card preparation over raw persisted records.
use super::{SessionDocument, format_expandable_output, values};
use maestro_app::{
    is_truthy,
    markup::{EscapeMode, escape},
};
use maestro_theme::SyntaxHighlighter;
use serde_json::Value;
use std::io;

mod builtins;
mod custom;
mod result;

/// Prepare one complete offline card, preserving trusted exporter-supplied renderer HTML.
/// No tool is executed and no document borrow is retained.
/// # Errors
/// Returns invalid data for a reached malformed call, result content or edit diff.
/// Propagates canonical JSON serialization failures.
pub fn render_tool_call(
    document: &SessionDocument,
    call: &Value,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let id = call["id"].as_str().ok_or_else(invalid)?;
    let name = call["name"].as_str().ok_or_else(invalid)?;
    let result = values::find_tool_result(document, id)?;
    let status = result.map_or("pending", |r| {
        if is_truthy(&r["isError"]) {
            "error"
        } else {
            "success"
        }
    });
    let empty = Value::Object(serde_json::Map::new());
    let args = call
        .get("arguments")
        .filter(|v| is_truthy(v))
        .unwrap_or(&empty);
    let body = match name {
        "bash" | "read" | "write" | "edit" | "ls" => {
            builtins::render_builtin(name, args, result, highlighter)?
        }
        _ => custom::render_custom(
            call,
            args,
            result,
            document.data.get("renderedTools").and_then(|r| r.get(id)),
            highlighter,
        )?,
    };
    Ok(format!(
        "<div class=\"tool-execution {status}\">{body}</div>"
    ))
}

/// Literal notice for a present nonstring argument.
const INVALID_ARG: &str = "<span class=\"tool-error\">[invalid arg]</span>";
/// Escape a display operand for either text or quoted attribute content.
fn html(text: &str) -> String {
    escape(text, EscapeMode::HtmlDecimal)
}
/// Construct the existing reached invalid-data category.
fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
