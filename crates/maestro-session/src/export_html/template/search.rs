//! Literal search over shared raw entry and label observations.
use super::{SessionTreeNode, extract_content, extract_content_prefix, read_message, read_text};
use maestro_json::is_truthy;
use maestro_text::is_whitespace_scalar;
use serde_json::Value;
use std::io;
/// Complete lowercase search text for the offline view.
///
/// # Errors
/// Returns invalid data for a reached malformed searchable field or receiver.
pub fn get_searchable_text(entry: &Value, label: Option<&Value>) -> io::Result<String> {
    searchable(entry, label, false).map(|text| text.to_lowercase())
}
/// Applies every nonempty lowercase literal token to the entry's searchable text.
///
/// # Errors
/// Returns invalid data for a reached malformed field; empty queries avoid reads.
pub fn matches_search(node: &SessionTreeNode, query: &str) -> io::Result<bool> {
    let lowercase = query.to_lowercase();
    let tokens: Vec<_> = lowercase
        .split(is_whitespace_scalar)
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return Ok(true);
    }
    let searchable = searchable(node.entry(), node.label().as_ref(), true)?.to_lowercase();
    Ok(tokens.iter().all(|token| searchable.contains(token)))
}
/// Collects only the entry-specific fields admitted to literal search.
fn searchable(entry: &Value, label: Option<&Value>, terminal: bool) -> io::Result<String> {
    let mut parts = Vec::new();
    if let Some(label) = label.filter(|v| is_truthy(v)) {
        parts.push(read_text(label)?.to_owned());
    }
    let field = |key: &str| {
        entry
            .get(key)
            .filter(|v| !v.is_null())
            .map_or(Ok(""), read_text)
            .map(str::to_owned)
    };
    match entry["type"].as_str() {
        Some("message") => {
            let message = read_message(entry)?;
            parts.push(message.get("role").map_or(Ok(""), read_text)?.to_owned());
            if is_truthy(&message["content"]) {
                parts.push(search_content(&message["content"], terminal)?);
            }
            if message["role"] == "bashExecution" && is_truthy(&message["command"]) {
                parts.push(read_text(&message["command"])?.to_owned());
            }
        }
        Some("custom_message") => {
            parts.push(field("customType")?);
            parts.push(if let Some(value) = entry["content"].as_str() {
                value.to_owned()
            } else {
                search_content(&entry["content"], terminal)?
            });
        }
        Some("compaction") => parts.push("compaction".to_owned()),
        Some("branch_summary") => parts.extend(["branch summary".to_owned(), field("summary")?]),
        Some("session_info") if terminal => parts.extend([
            "title".to_owned(),
            if is_truthy(&entry["name"]) {
                field("name")?
            } else {
                String::new()
            },
        ]),
        Some("model_change") => parts.extend(["model".to_owned(), field("modelId")?]),
        Some("thinking_level_change") => {
            parts.extend(["thinking".to_owned(), field("thinkingLevel")?]);
        }
        Some("custom") if terminal => parts.extend(["custom".to_owned(), field("customType")?]),
        Some("label") if terminal => parts.extend(["label".to_owned(), field("label")?]),
        _ => {}
    }
    Ok(parts.join(" "))
}

/// Apply only the content-length difference of the two real search surfaces.
fn search_content(value: &Value, terminal: bool) -> io::Result<String> {
    if terminal {
        extract_content_prefix(value, 200)
    } else {
        extract_content(value)
    }
}
