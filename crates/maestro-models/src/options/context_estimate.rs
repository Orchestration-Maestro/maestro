//! Estimated token size of a request, used to fit an output budget in the room the context
//! window leaves.
use num_traits::ToPrimitive;

use crate::{
    AssistantContent, AssistantMessage, Context, Message, StopReason, UserBlock, UserContent,
};

/// Characters counted as one token.
const CHARS_PER_TOKEN: f64 = 4.0;
/// Characters counted for one image block.
const IMAGE_CHARS: f64 = 4800.0;
/// Characters counted for a value that cannot be written as JSON.
const UNSERIALIZABLE_CHARS: f64 = 16.0;

/// UTF-16 code units of `text`, the unit the characters-per-token figure is measured in.
fn units(text: &str) -> f64 {
    text.encode_utf16().count().to_f64().unwrap_or(f64::MAX)
}

/// Tokens estimated for `chars` characters, rounded up.
fn tokens(chars: f64) -> f64 {
    (chars / CHARS_PER_TOKEN).ceil()
}

/// Characters of the user blocks, images counted at a fixed size.
fn block_chars(blocks: &[UserBlock]) -> f64 {
    blocks
        .iter()
        .map(|block| match block {
            UserBlock::Text(text) => units(&text.text),
            UserBlock::Image(_) => IMAGE_CHARS,
        })
        .sum()
}

/// Characters of an assistant message's text, thinking, tool names and tool arguments.
fn assistant_chars(message: &AssistantMessage) -> f64 {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Text(text) => units(&text.text),
            AssistantContent::Thinking(thinking) => units(&thinking.thinking),
            AssistantContent::ToolCall(call) => units(&call.name) + json_chars(&call.arguments),
        })
        .sum()
}

/// Characters of the compact JSON of `value`.
fn json_chars<T: serde::Serialize>(value: &T) -> f64 {
    serde_json::to_string(value).map_or(UNSERIALIZABLE_CHARS, |json| units(&json))
}

/// Tokens estimated for one message from its characters.
fn message_tokens(message: &Message) -> f64 {
    tokens(match message {
        Message::User(user) => match &user.content {
            UserContent::Text(text) => units(text),
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
        .map_or(0.0, |prompt| tokens(units(prompt)))
        + context
            .tools
            .as_deref()
            .filter(|tools| !tools.is_empty())
            .map_or(0.0, |tools| tokens(json_chars(&tools)));
    prefix + context.messages.iter().map(message_tokens).sum::<f64>()
}
