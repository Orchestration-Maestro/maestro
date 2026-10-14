//! Literal search producers for retained session entries.
use super::{
    SessionTreeNode,
    entry_text::{content, text},
};
use maestro_app::{is_truthy, is_whitespace_scalar};
use std::io;

/// Applies every nonempty lowercase literal token to the entry's searchable text.
pub(super) fn matches(node: &SessionTreeNode, query: &str) -> io::Result<bool> {
    let lowercase = query.to_lowercase();
    let tokens: Vec<_> = lowercase
        .split(is_whitespace_scalar)
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return Ok(true);
    }
    let searchable = searchable(node)?.to_lowercase();
    Ok(tokens.iter().all(|token| searchable.contains(token)))
}
/// Collects only the entry-specific fields admitted to literal search.
fn searchable(node: &SessionTreeNode) -> io::Result<String> {
    let entry = node.entry();
    let label = node.label();
    let mut parts = vec![
        label
            .as_ref()
            .filter(|v| is_truthy(v))
            .map_or(Ok(""), text)?
            .to_owned(),
    ];
    drop(label);
    let field = |key: &str| {
        entry
            .get(key)
            .filter(|v| !v.is_null())
            .map_or(Ok(""), text)
            .map(str::to_owned)
    };
    match entry["type"].as_str() {
        Some("message") => {
            let message = &entry["message"];
            parts.push(message.get("role").map_or(Ok(""), text)?.to_owned());
            if is_truthy(&message["content"]) {
                parts.push(content(&message["content"], 200)?);
            }
            if message["role"] == "bashExecution" && is_truthy(&message["command"]) {
                parts.push(text(&message["command"])?.to_owned());
            }
        }
        Some("custom_message") => {
            parts.push(field("customType")?);
            parts.push(if let Some(value) = entry["content"].as_str() {
                value.to_owned()
            } else {
                content(&entry["content"], 200)?
            });
        }
        Some("compaction") => parts.push("compaction".to_owned()),
        Some("branch_summary") => parts.extend(["branch summary".to_owned(), field("summary")?]),
        Some("session_info") => parts.extend([
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
        Some("custom") => parts.extend(["custom".to_owned(), field("customType")?]),
        Some("label") => parts.extend(["label".to_owned(), field("label")?]),
        _ => {}
    }
    Ok(parts.join(" "))
}
