//! Shared file tracking and conversation text for summaries.
pub use maestro_entry::compaction::FileOperations;
use maestro_models::{AssistantContent, AssistantMessage, Message, UserBlock, UserContent};
use serde_json::Value;
use std::borrow::Cow;
/// Create empty insertion-ordered file-operation sets.
#[must_use]
pub fn create_file_ops() -> FileOperations {
    FileOperations {
        read: std::iter::empty().collect(),
        written: std::iter::empty().collect(),
        edited: std::iter::empty().collect(),
    }
}
/// Record nonempty string paths from assistant read, write and edit calls.
pub fn extract_file_ops_from_message(message: &Value, file_ops: &mut FileOperations) {
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("toolCall") {
            continue;
        }
        let Some(path) = block
            .get("arguments")
            .and_then(|args| args.get("path"))
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let set = match block.get("name").and_then(Value::as_str) {
            Some("read") => &mut file_ops.read,
            Some("write") => &mut file_ops.written,
            Some("edit") => &mut file_ops.edited,
            _ => continue,
        };
        set.insert(path.to_owned());
    }
}
/// Compute disjoint read-only and modified lists in UTF-16 lexical order.
#[must_use]
pub fn compute_file_lists(file_ops: &FileOperations) -> super::CompactionDetails {
    let mut modified_files: Vec<_> = file_ops.edited.union(&file_ops.written).cloned().collect();
    let mut read_files: Vec<_> = file_ops
        .read
        .iter()
        .filter(|path| !file_ops.edited.contains(*path) && !file_ops.written.contains(*path))
        .cloned()
        .collect();
    read_files.sort_by(|a, b| maestro_text::utf16_order(a, b));
    modified_files.sort_by(|a, b| maestro_text::utf16_order(a, b));
    super::CompactionDetails {
        read_files,
        modified_files,
    }
}
/// Format supplied file lists, retaining their order and duplicates.
#[must_use]
pub fn format_file_operations(read_files: &[String], modified_files: &[String]) -> String {
    let mut sections = Vec::new();
    for (tag, files) in [
        ("read-files", read_files),
        ("modified-files", modified_files),
    ] {
        if !files.is_empty() {
            sections.push(format!("<{tag}>\n{}\n</{tag}>", files.join("\n")));
        }
    }
    if sections.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", sections.join("\n\n"))
    }
}
/// Scalar character limit for tool-result summary text.
const TOOL_RESULT_MAX_CHARS: usize = 2000;
/// Concatenate text blocks while leaving image payloads unused.
fn plain_text(blocks: &[UserBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            UserBlock::Text(text) => Some(text.text.as_str()),
            UserBlock::Image(_) => None,
        })
        .collect()
}
/// Clip only tool text on a complete scalar boundary and count omitted scalars.
fn truncate_for_summary(text: &str) -> String {
    let Some((boundary, _)) = text.char_indices().nth(TOOL_RESULT_MAX_CHARS) else {
        return text.to_owned();
    };
    format!(
        "{}\n\n[... {} more characters truncated]",
        &text[..boundary],
        text[boundary..].chars().count()
    )
}
/// Append assistant categories in thinking, text and tool order.
fn assistant_parts(
    message: &AssistantMessage,
    parts: &mut Vec<String>,
) -> Result<(), serde_json::Error> {
    let mut thinking = Vec::new();
    let mut text = Vec::new();
    let mut tools = Vec::new();
    for block in &message.content {
        match block {
            AssistantContent::Text(block) => text.push(block.text.as_str()),
            AssistantContent::Thinking(block) => thinking.push(block.thinking.as_str()),
            AssistantContent::ToolCall(block) => {
                let args = maestro_json::canonical_entries(&block.arguments)
                    .into_iter()
                    .map(|(key, value)| Ok(format!("{key}={}", maestro_json::compact_json(value)?)))
                    .collect::<Result<Vec<_>, serde_json::Error>>()?;
                tools.push(format!("{}({})", block.name, args.join(", ")));
            }
        }
    }
    if !thinking.is_empty() {
        parts.push(format!("[Assistant thinking]: {}", thinking.join("\n")));
    }
    if !text.is_empty() {
        parts.push(format!("[Assistant]: {}", text.join("\n")));
    }
    if !tools.is_empty() {
        parts.push(format!("[Assistant tool calls]: {}", tools.join("; ")));
    }
    Ok(())
}
/// Serialize already-projected model messages as labeled summary input.
///
/// Only tool results are clipped, at 2000 Unicode scalar values.
///
/// # Errors
/// Returns the canonical argument writer's JSON error.
pub fn serialize_conversation(messages: &[Message]) -> Result<String, serde_json::Error> {
    let mut parts = Vec::new();
    for message in messages {
        match message {
            Message::User(message) => {
                let content = match &message.content {
                    UserContent::Text(text) => Cow::Borrowed(text.as_str()),
                    UserContent::Blocks(blocks) => Cow::Owned(plain_text(blocks)),
                };
                if !content.is_empty() {
                    parts.push(format!("[User]: {content}"));
                }
            }
            Message::Assistant(message) => assistant_parts(message, &mut parts)?,
            Message::ToolResult(message) => {
                let content = plain_text(&message.content);
                if !content.is_empty() {
                    parts.push(format!("[Tool result]: {}", truncate_for_summary(&content)));
                }
            }
        }
    }
    Ok(parts.join("\n\n"))
}
