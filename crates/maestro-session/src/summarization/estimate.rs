//! Context usage and message-size estimates.
use super::CompactionSettings;
use crate::message_records::CustomAgentMessages;
use crate::session_manager::history::property;
use maestro_agent::AgentMessage;
use maestro_json::is_truthy;
use maestro_lock::read;
use maestro_models::{ESTIMATED_IMAGE_CHARS, StopReason, tokens_for_chars, utf16_units};
use serde::Deserialize;
use serde::de::Error as _;
use serde_json::Value;
use std::borrow::Borrow;
/// Whether enabled compaction exceeds the available context window.
#[must_use]
pub fn should_compact(
    context_tokens: f64,
    context_window: f64,
    settings: &CompactionSettings,
) -> bool {
    settings.enabled && context_tokens > context_window - settings.reserve_tokens
}
/// Decode one reached scalar without inspecting sibling fields.
fn scalar<'a, T: Deserialize<'a>>(value: Option<&'a Value>) -> Result<T, serde_json::Error> {
    T::deserialize(value.ok_or_else(|| serde_json::Error::custom("missing field"))?)
}
/// Use a truthy reported total, otherwise add the four usage categories in order.
///
/// # Errors
/// Returns a native JSON error for a reached invalid numeric field.
pub fn calculate_context_tokens(usage: &Value) -> Result<f64, serde_json::Error> {
    if let Some(total) = property(Some(usage), "totalTokens")?.filter(|value| is_truthy(value)) {
        return scalar(Some(total));
    }
    let input: f64 = scalar(property(Some(usage), "input")?)?;
    let output: f64 = scalar(property(Some(usage), "output")?)?;
    let read: f64 = scalar(property(Some(usage), "cacheRead")?)?;
    let write: f64 = scalar(property(Some(usage), "cacheWrite")?)?;
    Ok(input + output + read + write)
}
/// Borrow successful assistant usage that reports context tokens, ignoring unread content
/// and cost; aborted, failed and all-zero usage describe no context.
fn assistant_usage(message: Option<&Value>) -> Result<Option<&Value>, serde_json::Error> {
    if property(message, "role")?.and_then(Value::as_str) != Some("assistant") {
        return Ok(None);
    }
    let stop = property(message, "stopReason")?.and_then(Value::as_str);
    if matches!(stop, Some("aborted" | "error")) {
        return Ok(None);
    }
    let Some(usage) = property(message, "usage")?.filter(|value| is_truthy(value)) else {
        return Ok(None);
    };
    Ok((calculate_context_tokens(usage)? > 0.0).then_some(usage))
}
/// Borrow the latest successful assistant usage that reports tokens from stored entries.
///
/// # Errors
/// Returns a native JSON error for a reached null entry or message.
pub fn get_last_assistant_usage<T: Borrow<crate::SessionEntry>>(
    entries: &[T],
) -> Result<Option<&Value>, serde_json::Error> {
    for entry in entries.iter().map(Borrow::borrow).rev() {
        if property(Some(entry), "type")?.and_then(Value::as_str) == Some("message")
            && let Some(usage) = assistant_usage(entry.get("message"))?
        {
            return Ok(Some(usage));
        }
    }
    Ok(None)
}
/// Selected role policy, independent of unused message fields.
enum Role {
    /// User text and image attachments.
    User,
    /// Assistant categories.
    Assistant,
    /// Tool or custom content with images.
    WithImages,
    /// Shell command and output.
    Shell,
    /// Summary without wrappers.
    Summary,
    /// Unknown roles contribute nothing.
    Other,
}
/// Classify the reached role without decoding the stored record.
fn role(message: &Value) -> Result<Role, serde_json::Error> {
    Ok(
        match property(Some(message), "role")?.and_then(Value::as_str) {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            Some("custom" | "toolResult") => Role::WithImages,
            Some("bashExecution") => Role::Shell,
            Some("branchSummary" | "compactionSummary") => Role::Summary,
            _ => Role::Other,
        },
    )
}
/// Count UTF-16 units in a selected text field.
fn text_units(value: Option<&Value>) -> Result<f64, serde_json::Error> {
    let text: &str = scalar(value)?;
    Ok(utf16_units(text))
}
/// Count selected block fields; image payloads and unknown blocks remain unread.
fn block_units(block: &Value, role: &Role) -> Result<f64, serde_json::Error> {
    match property(Some(block), "type")?.and_then(Value::as_str) {
        Some("text") => {
            let text = block.get("text");
            if !matches!(role, Role::Assistant) && !text.is_some_and(is_truthy) {
                return Ok(0.0);
            }
            text_units(text)
        }
        Some("thinking") if matches!(role, Role::Assistant) => text_units(block.get("thinking")),
        Some("toolCall") if matches!(role, Role::Assistant) => {
            let name = text_units(block.get("name"))?;
            let arguments = block
                .get("arguments")
                .ok_or_else(|| serde_json::Error::custom("missing arguments"))?;
            Ok(name + utf16_units(&maestro_json::compact_json(arguments)?))
        }
        Some("image") if matches!(role, Role::User | Role::WithImages) => Ok(ESTIMATED_IMAGE_CHARS),
        _ => Ok(0.0),
    }
}
/// Estimate a raw application message using UTF-16 units divided by four.
///
/// # Errors
/// Returns a native JSON error for a reached invalid content or text field.
pub fn estimate_tokens(message: &Value) -> Result<f64, serde_json::Error> {
    let role = role(message)?;
    let chars = match &role {
        Role::Shell => text_units(message.get("command"))? + text_units(message.get("output"))?,
        Role::Summary => text_units(message.get("summary"))?,
        Role::Other => return Ok(0.0),
        Role::User | Role::WithImages | Role::Assistant => {
            let content = message.get("content");
            match content {
                Some(Value::String(_)) if !matches!(role, Role::Assistant) => text_units(content)?,
                Some(Value::Array(blocks)) => blocks.iter().try_fold(0.0, |sum, block| {
                    Ok::<_, serde_json::Error>(sum + block_units(block, &role)?)
                })?,
                _ if matches!(role, Role::User) => 0.0,
                _ => return Err(serde_json::Error::custom("invalid content array")),
            }
        }
    };
    Ok(tokens_for_chars(chars))
}
/// Read a message timestamp when it is a number.
fn timestamp(message: &Value) -> Option<f64> {
    message.get("timestamp").and_then(Value::as_f64)
}
/// Find the newest assistant usage that still describes the messages before it.
///
/// A message stamped after an answer, such as a compaction summary inserted ahead of the
/// retained span, replaces the context that answer measured, so that usage is stale until
/// a newer answer reports. A message without a numeric timestamp is never stale.
/// `usage_tokens` reports the context tokens of the answer at an index, when it has any.
fn latest_usage_by<E>(
    stamps: &[Option<f64>],
    mut usage_tokens: impl FnMut(usize) -> Result<Option<f64>, E>,
) -> Result<Option<(usize, f64)>, E> {
    let mut newest = f64::NEG_INFINITY;
    let preceding: Vec<f64> = stamps
        .iter()
        .map(|stamp| {
            let before = newest;
            newest = stamp.map_or(newest, |stamp| newest.max(stamp));
            before
        })
        .collect();
    for (index, stamp) in stamps.iter().enumerate().rev() {
        if stamp.is_some_and(|stamp| stamp < preceding[index]) {
            continue;
        }
        if let Some(tokens) = usage_tokens(index)? {
            return Ok(Some((index, tokens)));
        }
    }
    Ok(None)
}
/// The newest applicable usage among raw message records.
fn latest_usage(messages: &[Value]) -> Result<Option<(usize, f64)>, serde_json::Error> {
    let stamps: Vec<Option<f64>> = messages.iter().map(timestamp).collect();
    latest_usage_by(&stamps, |index| {
        assistant_usage(Some(&messages[index]))?
            .map(calculate_context_tokens)
            .transpose()
    })
}
/// Add the estimate of the messages after the selected usage to that usage.
fn estimate_after(
    latest: Option<(usize, f64)>,
    trailing: &[Value],
) -> Result<super::ContextUsageEstimate, serde_json::Error> {
    let usage_tokens = latest.map_or(0.0, |(_, tokens)| tokens);
    let trailing_tokens = trailing.iter().try_fold(0.0, |sum, message| {
        Ok::<_, serde_json::Error>(sum + estimate_tokens(message)?)
    })?;
    Ok(super::ContextUsageEstimate {
        tokens: usage_tokens + trailing_tokens,
        usage_tokens,
        trailing_tokens,
        last_usage_index: latest.map(|(index, _)| index),
    })
}
/// Estimate context from the latest applicable successful usage and the trailing messages.
///
/// # Errors
/// Returns a native JSON error for reached usage or message fields.
pub fn estimate_context_tokens(
    messages: &[Value],
) -> Result<super::ContextUsageEstimate, serde_json::Error> {
    let latest = latest_usage(messages)?;
    estimate_after(
        latest,
        &messages[latest.map_or(0, |(index, _)| index + 1)..],
    )
}
/// A typed application message.
type TypedMessage = AgentMessage<CustomAgentMessages>;
/// The numeric timestamp the message's record carries; a nonfinite one serializes as null.
fn message_timestamp(message: &TypedMessage) -> Option<f64> {
    let stamp = match message {
        AgentMessage::User(message) => read(message).timestamp,
        AgentMessage::Assistant(message) => read(message).timestamp,
        AgentMessage::ToolResult(message) => read(message).timestamp,
        AgentMessage::Custom(message) => match &*read(message) {
            CustomAgentMessages::BashExecution(message) => message.timestamp,
            CustomAgentMessages::Custom(message) => message.timestamp,
            CustomAgentMessages::BranchSummary(message) => message.timestamp,
            CustomAgentMessages::CompactionSummary(message) => message.timestamp,
            CustomAgentMessages::Other(never) => match *never {},
        },
    };
    stamp.is_finite().then_some(stamp)
}
/// The context tokens of a successful answer that reports any; aborted, failed and all-zero
/// usage describe no context.
fn message_usage_tokens(message: &TypedMessage) -> Result<Option<f64>, serde_json::Error> {
    let AgentMessage::Assistant(answer) = message else {
        return Ok(None);
    };
    let answer = read(answer);
    if matches!(answer.stop_reason, StopReason::Aborted | StopReason::Error) {
        return Ok(None);
    }
    let tokens = calculate_context_tokens(&serde_json::to_value(&answer.usage)?)?;
    Ok((tokens > 0.0).then_some(tokens))
}
/// Estimate context from typed messages like [`estimate_context_tokens`], converting to records
/// only the messages after the latest applicable usage.
///
/// # Errors
/// Returns a native JSON error for reached usage or message fields.
pub fn estimate_message_context_tokens(
    messages: &[TypedMessage],
) -> Result<super::ContextUsageEstimate, serde_json::Error> {
    let stamps: Vec<Option<f64>> = messages.iter().map(message_timestamp).collect();
    let latest = latest_usage_by(&stamps, |index| message_usage_tokens(&messages[index]))?;
    let trailing = messages[latest.map_or(0, |(index, _)| index + 1)..]
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    estimate_after(latest, &trailing)
}
