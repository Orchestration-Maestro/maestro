//! Session selection, ordered custom output and document publication.
mod colors;
mod document;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod types;
use maestro_session::session_manager::ReadonlySessionManager;
#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeExportOperations;
use serde_json::{Map, Value, json};
use std::io;
pub use types::*;

/// Export retained session entries and optional metadata to a supplied bundle.
///
/// # Errors
/// Rejects in-memory/unpublished sessions before rendering; propagates reached
/// renderer, asset, theme, encoding and write failures.
pub fn export_session_to_html(
    session: &dyn ReadonlySessionManager,
    state: Option<ExportState<'_>>,
    options: Option<ExportOptions<'_>>,
    operations: &dyn ExportOperations,
) -> Result<String, ExportError> {
    let file = session
        .get_session_file()
        .filter(|file| !file.is_empty())
        .ok_or(ExportError::InMemorySession)?;
    if !operations.exists(file) {
        return Err(ExportError::UnpublishedSession);
    }
    let options = options.unwrap_or_default();
    let entries = session.get_entries();
    let rendered = options
        .tool_renderer
        .map(|renderer| render_tools(&entries, renderer))
        .transpose()?;
    let mut data = json!({"header": session.get_header(), "entries": entries, "leafId": session.get_leaf_id()});
    if let Some(state) = state {
        if let Some(prompt) = state.system_prompt {
            data["systemPrompt"] = prompt.into();
        }
        if let Some(tools) = state.tools {
            data["tools"] = tools.iter().map(|tool| json!({"name":tool.name,"description":tool.description,"parameters":tool.parameters})).collect();
        }
    }
    if let Some(rendered) = rendered.filter(|map| !map.is_empty()) {
        data["renderedTools"] = rendered.into();
    }
    publish(file, &data, &options, operations)
}
/// Open a native session with its owner, then export without state/custom rendering.
///
/// # Errors
/// Rejects missing input before opening; propagates session-open, document and write failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_from_file(
    input_path: &str,
    options: Option<ExportOptions<'_>>,
    operations: &dyn ExportOperations,
) -> Result<String, ExportError> {
    if !operations.exists(input_path) {
        return Err(ExportError::MissingFile(input_path.to_owned()));
    }
    let session = maestro_session::SessionManager::open(input_path, None, None)?;
    let data = json!({"header":session.get_header(),"entries":session.get_entries(),"leafId":session.get_leaf_id()});
    publish(input_path, &data, &options.unwrap_or_default(), operations)
}
/// Generate a document and return the authored output path only after writing.
fn publish(
    file: &str,
    data: &Value,
    options: &ExportOptions<'_>,
    operations: &dyn ExportOperations,
) -> Result<String, ExportError> {
    let html = document::generate(data, options.theme_name, operations)?;
    let path = options
        .output_path
        .filter(|path| !path.is_empty())
        .map_or_else(
            || {
                format!(
                    "{}-session-{}.html",
                    operations.app_name(),
                    maestro_path::basename(file, Some(".jsonl"))
                )
            },
            str::to_owned,
        );
    operations.write(&path, &html)?;
    Ok(path)
}
/// Whether a tool is rendered by the supplied viewer rather than custom callbacks.
fn template_tool(name: &str) -> bool {
    matches!(name, "bash" | "read" | "write" | "edit" | "ls")
}
/// Report invalid reached session data without prevalidating later records.
fn invalid(field: &str) -> ExportError {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Invalid session field: {field}"),
    )
    .into()
}
/// Borrow a selected string identity.
fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, ExportError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(field))
}
/// Walk stored messages in order, retaining only own call-ID keys.
fn render_tools(
    entries: &[&Value],
    renderer: &dyn ToolHtmlRenderer,
) -> Result<Map<String, Value>, ExportError> {
    let mut records = Map::new();
    for entry in entries {
        if entry["type"] != "message" {
            continue;
        }
        let message = entry
            .get("message")
            .filter(|value| !value.is_null())
            .ok_or_else(|| invalid("message"))?;
        if message["role"] == "assistant"
            && let Some(blocks) = message["content"].as_array()
        {
            for block in blocks {
                render_call(block, renderer, &mut records)?;
            }
        }
        if message["role"] == "toolResult" {
            render_result(message, renderer, &mut records)?;
        }
    }
    Ok(records)
}
/// Replace a call record when custom output is nonempty.
fn render_call(
    block: &Value,
    renderer: &dyn ToolHtmlRenderer,
    records: &mut Map<String, Value>,
) -> Result<(), ExportError> {
    if block.is_null() {
        return Err(invalid("block"));
    }
    if block["type"] != "toolCall" {
        return Ok(());
    }
    let name = string(block, "name")?;
    if template_tool(name) {
        return Ok(());
    }
    let id = string(block, "id")?;
    let html = renderer
        .render_call(id, name, block.get("arguments"))
        .map_err(ExportError::Renderer)?;
    if let Some(html) = html.filter(|html| !html.is_empty()) {
        records.insert(id.to_owned(), json!({"callHtml":html}));
    }
    Ok(())
}
/// Merge present results while clearing newly absent result fields.
fn render_result(
    message: &Value,
    renderer: &dyn ToolHtmlRenderer,
    records: &mut Map<String, Value>,
) -> Result<(), ExportError> {
    let Some(id) = message
        .get("toolCallId")
        .filter(|value| maestro_json::is_truthy(value))
    else {
        return Ok(());
    };
    let id = id.as_str().ok_or_else(|| invalid("toolCallId"))?;
    let name = match message
        .get("toolName")
        .filter(|value| maestro_json::is_truthy(value))
    {
        Some(value) => value.as_str().ok_or_else(|| invalid("toolName"))?,
        None => "",
    };
    if !records.contains_key(id) && template_tool(name) {
        return Ok(());
    }
    let input = ToolHtmlResultInput {
        content: message.get("content"),
        details: message.get("details"),
        is_error: message.get("isError").is_some_and(maestro_json::is_truthy),
    };
    let Some(result) = renderer
        .render_result(id, name, input)
        .map_err(ExportError::Renderer)?
    else {
        return Ok(());
    };
    let record = records.entry(id.to_owned()).or_insert_with(|| json!({}));
    if let Value::Object(record) = record {
        for (key, value) in [
            ("resultHtmlCollapsed", result.collapsed),
            ("resultHtmlExpanded", result.expanded),
        ] {
            if let Some(value) = value {
                record.insert(key.to_owned(), value.into());
            } else {
                record.shift_remove(key);
            }
        }
    }
    Ok(())
}
