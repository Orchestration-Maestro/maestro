use super::{Pending, builders, lock};
use crate::{
    AssistantContent, AssistantMessage, CacheRetention, Context, DiagnosticErrorInfo, Message,
    StreamOptions, UserBlock, UserContent,
};
use std::sync::Mutex;

/// Estimate tokens from Unicode scalar count in groups of four.
pub(super) fn tokens(text: &str) -> f64 {
    (text.chars().fold(0.0_f64, |count, _| count + 1.0) / 4.0).ceil()
}
/// Serialize compact JSON using native insertion order and number spelling.
pub(super) fn json<T: serde::Serialize>(value: &T) -> Result<String, DiagnosticErrorInfo> {
    serde_json::to_string(value).map_err(|error| DiagnosticErrorInfo {
        name: None,
        message: error.to_string(),
        stack: None,
        code: None,
    })
}
/// Project a user text or image block into estimated prompt text.
fn user_block(block: &UserBlock) -> String {
    match block {
        UserBlock::Text(value) => value.text.clone(),
        UserBlock::Image(value) => {
            format!("[image:{}:{}]", value.mime_type, value.data.chars().count())
        }
    }
}
/// Project assistant text, thinking and compact tool arguments in order.
pub(super) fn assistant_text(content: &[AssistantContent]) -> Result<String, DiagnosticErrorInfo> {
    content
        .iter()
        .map(|block| match block {
            AssistantContent::Text(value) => Ok(value.text.clone()),
            AssistantContent::Thinking(value) => Ok(value.thinking.clone()),
            AssistantContent::ToolCall(value) => {
                Ok(format!("{}:{}", value.name, json(&value.arguments)?))
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("\n"))
}
/// Project authored prompt parts with their role labels and separators.
fn serialize_context(context: &Context) -> Result<String, DiagnosticErrorInfo> {
    let mut parts = Vec::new();
    if let Some(system) = context
        .system_prompt
        .as_ref()
        .filter(|text| !text.is_empty())
    {
        parts.push(format!("system:{system}"));
    }
    for message in &context.messages {
        parts.push(match message {
            Message::User(value) => format!(
                "user:{}",
                match &value.content {
                    UserContent::Text(text) => text.clone(),
                    UserContent::Blocks(blocks) =>
                        blocks.iter().map(user_block).collect::<Vec<_>>().join("\n"),
                }
            ),
            Message::Assistant(value) => format!("assistant:{}", assistant_text(&value.content)?),
            Message::ToolResult(value) => format!(
                "toolResult:{}",
                std::iter::once(value.tool_name.clone())
                    .chain(value.content.iter().map(user_block))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        });
    }
    if let Some(tools) = context.tools.as_ref().filter(|tools| !tools.is_empty()) {
        parts.push(format!("tools:{}", json(tools)?));
    }
    Ok(parts.join("\n\n"))
}
/// Estimate prompt/output usage and partition enabled session caching.
pub(super) fn estimate(
    message: &mut AssistantMessage,
    context: &Context,
    options: Option<&StreamOptions>,
    pending: &Mutex<Pending>,
) -> Result<(), DiagnosticErrorInfo> {
    let prompt = serialize_context(context)?;
    let prompt_tokens = tokens(&prompt);
    let output = tokens(&assistant_text(&message.content)?);
    let mut usage = builders::zero_usage();
    usage.input = prompt_tokens;
    usage.output = output;
    usage.total_tokens = prompt_tokens + output;
    if let Some(session) = options
        .filter(|options| !matches!(options.cache_retention, Some(CacheRetention::None)))
        .and_then(|options| options.session_id.as_ref())
        .filter(|session| !session.is_empty())
    {
        let previous = lock(pending).cache.insert(session.clone(), prompt.clone());
        let prefix = previous.as_ref().map_or(0.0, |previous| {
            previous
                .chars()
                .zip(prompt.chars())
                .take_while(|(left, right)| left == right)
                .fold(0.0_f64, |count, _| count + 1.0)
        });
        usage.input = 0.0;
        usage.cache_read = (prefix / 4.0).ceil().min(prompt_tokens);
        usage.cache_write = prompt_tokens - usage.cache_read;
    }
    message.usage = usage;
    Ok(())
}
