//! Estimated token size of a conversation: the one estimate the output budget and the
//! session's compaction both count with.
use num_traits::ToPrimitive;

use serde_json::Value;

use crate::{
    AssistantContent, AssistantMessage, Context, Message, StopReason, Tool, UserBlock, UserContent,
};

/// Characters counted as one token.
const CHARS_PER_TOKEN: f64 = 4.0;
/// Characters counted for one image block.
pub const ESTIMATED_IMAGE_CHARS: f64 = 4800.0;
/// Characters counted for a value that cannot be written as JSON.
const UNSERIALIZABLE_CHARS: f64 = 16.0;

/// UTF-16 code units of `text`, the unit the characters-per-token figure is measured in.
#[must_use]
pub fn utf16_units(text: &str) -> f64 {
    text.encode_utf16().count().to_f64().unwrap_or(f64::MAX)
}

/// Tokens estimated for `chars` characters, rounded up.
#[must_use]
pub fn tokens_for_chars(chars: f64) -> f64 {
    (chars / CHARS_PER_TOKEN).ceil()
}

/// Characters of the user blocks, images counted at a fixed size.
fn block_chars(blocks: &[UserBlock]) -> f64 {
    blocks
        .iter()
        .map(|block| match block {
            UserBlock::Text(text) => utf16_units(&text.text),
            UserBlock::Image(_) => ESTIMATED_IMAGE_CHARS,
        })
        .sum()
}

/// Characters of an assistant message's text, thinking, tool names and tool arguments.
fn assistant_chars(message: &AssistantMessage) -> f64 {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Text(text) => utf16_units(&text.text),
            AssistantContent::Thinking(thinking) => utf16_units(&thinking.thinking),
            AssistantContent::ToolCall(call) => {
                utf16_units(&call.name) + json_chars(&call.arguments)
            }
        })
        .sum()
}

/// Characters of the compact JSON of `value`.
fn json_chars(value: &Value) -> f64 {
    maestro_json::compact_json(value).map_or(UNSERIALIZABLE_CHARS, |json| utf16_units(&json))
}

/// Characters of the compact JSON of the tool declarations.
fn tools_chars(tools: &[Tool]) -> f64 {
    serde_json::to_value(tools).map_or(UNSERIALIZABLE_CHARS, |value| json_chars(&value))
}

/// Tokens estimated for one message from its characters.
fn message_tokens(message: &Message) -> f64 {
    tokens_for_chars(match message {
        Message::User(user) => match &user.content {
            UserContent::Text(text) => utf16_units(text),
            UserContent::Blocks(blocks) => block_chars(blocks),
        },
        Message::ToolResult(result) => block_chars(&result.content),
        Message::Assistant(assistant) => assistant_chars(assistant),
    })
}

/// Tokens a successful response reported for the whole context it answered: its total, or
/// the sum of its categories when the total is absent or zero.
fn reported_tokens(message: &AssistantMessage) -> f64 {
    let usage = &message.usage;
    usage
        .total_tokens
        .present()
        .copied()
        .filter(|total| *total != 0.0)
        .unwrap_or(usage.input + usage.output + usage.cache_read + usage.cache_write)
}

/// Estimate the tokens of `context`.
///
/// The newest response that did not fail or abort and reported usage anchors the estimate:
/// its reported tokens plus the estimate of the messages after it. Without one, every
/// message is estimated, plus the system prompt and the tool declarations.
pub(crate) fn estimate_context_tokens(context: &Context) -> f64 {
    let anchor =
        context
            .messages
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, message)| match message {
                Message::Assistant(response)
                    if !matches!(
                        response.stop_reason,
                        StopReason::Aborted | StopReason::Error
                    ) && reported_tokens(response) > 0.0 =>
                {
                    Some((index, reported_tokens(response)))
                }
                _ => None,
            });
    if let Some((index, reported)) = anchor {
        return reported
            + context.messages[index + 1..]
                .iter()
                .map(message_tokens)
                .sum::<f64>();
    }
    let prefix = context
        .system_prompt
        .as_deref()
        .map_or(0.0, |prompt| tokens_for_chars(utf16_units(prompt)))
        + context
            .tools
            .as_deref()
            .filter(|tools| !tools.is_empty())
            .map_or(0.0, |tools| tokens_for_chars(tools_chars(tools)));
    prefix + context.messages.iter().map(message_tokens).sum::<f64>()
}
