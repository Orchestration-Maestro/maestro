//! Shared file tracking and conversation text for summaries.
pub use maestro_entry::compaction::FileOperations;
use maestro_json::is_truthy;
use maestro_models::{AssistantContent, Message, UserBlock, UserContent};
use serde::Deserialize;
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
/// Borrowed content needed by the conversation writer, without execution metadata.
pub(crate) enum SummaryBlock<'a> {
    /// Visible text.
    Text(&'a str),
    /// Named tool and its complete argument value.
    Tool(&'a str, &'a Value),
}
/// A selected model message for the shared writer.
pub(crate) enum SummaryMessage<'a> {
    /// Projected user text.
    User(Cow<'a, str>),
    /// Assistant categories.
    Assistant(Vec<SummaryBlock<'a>>),
    /// Tool-result text.
    Tool(String),
}
/// Pair the entries a summary writes by name. Object members keep their canonical order,
/// array and string roots take their positions, and every other root names none.
fn argument_entries(arguments: &Value) -> Vec<(String, Cow<'_, Value>)> {
    match arguments {
        Value::Object(members) => maestro_json::canonical_entries(members.iter())
            .into_iter()
            .map(|(key, value)| (key.clone(), Cow::Borrowed(value)))
            .collect(),
        Value::Array(elements) => elements
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), Cow::Borrowed(value)))
            .collect(),
        Value::String(text) => text
            .chars()
            .enumerate()
            .map(|(index, character)| {
                (
                    index.to_string(),
                    Cow::Owned(Value::String(character.to_string())),
                )
            })
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) => Vec::new(),
    }
}
/// Append assistant categories in text and tool order; thinking is never written.
fn assistant_parts(
    blocks: &[SummaryBlock<'_>],
    parts: &mut Vec<String>,
) -> Result<(), serde_json::Error> {
    let mut text = Vec::new();
    let mut tools = Vec::new();
    for block in blocks {
        match block {
            SummaryBlock::Text(block) => text.push(*block),
            SummaryBlock::Tool(name, arguments) => {
                let args = argument_entries(arguments)
                    .into_iter()
                    .map(|(key, value)| {
                        Ok(format!("{key}={}", maestro_json::compact_json(&value)?))
                    })
                    .collect::<Result<Vec<_>, serde_json::Error>>()?;
                tools.push(format!("{name}({})", args.join(", ")));
            }
        }
    }
    for (label, blocks, separator) in [
        ("Assistant", text, "\n"),
        (
            "Assistant tool calls",
            tools.iter().map(String::as_str).collect(),
            "; ",
        ),
    ] {
        if !blocks.is_empty() {
            parts.push(format!("[{label}]: {}", blocks.join(separator)));
        }
    }
    Ok(())
}
/// Write both selected raw history and typed model messages through one formatter.
fn serialize_selected<'a>(
    messages: impl IntoIterator<Item = SummaryMessage<'a>>,
) -> Result<String, serde_json::Error> {
    let mut parts = Vec::new();
    for message in messages {
        match message {
            SummaryMessage::User(content) if !content.is_empty() => {
                parts.push(format!("[User]: {content}"));
            }
            SummaryMessage::Tool(content) if !content.is_empty() => {
                parts.push(format!("[Tool result]: {}", truncate_for_summary(&content)));
            }
            SummaryMessage::Assistant(blocks) => assistant_parts(&blocks, &mut parts)?,
            SummaryMessage::User(_) | SummaryMessage::Tool(_) => (),
        }
    }
    Ok(parts.join("\n\n"))
}
/// Serialize already-projected model messages as labeled summary input.
/// Only tool results are clipped, at 2000 Unicode scalar values.
///
/// # Errors
/// Returns the canonical argument writer's JSON error.
pub fn serialize_conversation(messages: &[Message]) -> Result<String, serde_json::Error> {
    serialize_selected(messages.iter().map(|message| {
        match message {
            Message::User(message) => SummaryMessage::User(match &message.content {
                UserContent::Text(text) => Cow::Borrowed(text),
                UserContent::Blocks(blocks) => Cow::Owned(plain_text(blocks)),
            }),
            Message::Assistant(message) => SummaryMessage::Assistant(
                message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContent::Text(block) => Some(SummaryBlock::Text(&block.text)),
                        AssistantContent::ToolCall(block) => {
                            Some(SummaryBlock::Tool(&block.name, &block.arguments))
                        }
                        AssistantContent::Thinking(_) => None,
                    })
                    .collect(),
            ),
            Message::ToolResult(message) => SummaryMessage::Tool(plain_text(&message.content)),
        }
    }))
}
/// Serialize consumed raw fields without decoding unrelated historical metadata.
pub(super) fn serialize_raw_conversation(messages: &[Value]) -> Result<String, serde_json::Error> {
    serialize_selected(
        messages
            .iter()
            .filter_map(|message| crate::messages::project_summary(message).transpose())
            .collect::<Result<Vec<_>, _>>()?,
    )
}
/// System instruction shared by history and prefix requests.
pub const SUMMARIZATION_SYSTEM_PROMPT: &str = r"You are a context summarization assistant. Your task is to read a conversation between a user and an AI coding assistant, then produce a structured summary following the exact format specified.

Do NOT continue the conversation. Do NOT respond to any questions in the conversation. ONLY output the structured summary.";

/// Carry built-in summary file arrays independently, ignoring hook-generated details.
pub(super) fn carry_file_ops(
    previous: &Value,
    ops: &mut FileOperations,
) -> Result<(), serde_json::Error> {
    if previous.get("fromHook").is_some_and(is_truthy) {
        return Ok(());
    }
    let Some(details) = previous.get("details").filter(|value| is_truthy(value)) else {
        return Ok(());
    };
    for (field, set) in [
        ("readFiles", &mut ops.read),
        ("modifiedFiles", &mut ops.edited),
    ] {
        if let Some(Value::Array(paths)) = details.get(field) {
            for path in paths {
                set.insert(String::deserialize(path)?);
            }
        }
    }
    Ok(())
}
