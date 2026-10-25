//! Compact wire events and reconstruction into the shared assistant message.
use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEvent as Event,
    AssistantMessageEventStream, DoneReason, ErrorReason, JsonObject, SharedAssistantMessage,
    StopReason, TextContent, ThinkingContent, ToolCall, Usage, parsed_arguments,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};

/// Compact server event without the reconstructed shared partial.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ProxyAssistantMessageEvent {
    /// Compact `start` form.
    Start,
    /// Compact `text_start` form.
    TextStart {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
    },
    /// Compact `text_delta` form.
    TextDelta {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Delta.
        delta: String,
    },
    /// Compact `text_end` form.
    TextEnd {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Content signature.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_signature: Option<String>,
    },
    /// Compact `thinking_start` form.
    ThinkingStart {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
    },
    /// Compact `thinking_delta` form.
    ThinkingDelta {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Delta.
        delta: String,
    },
    /// Compact `thinking_end` form.
    ThinkingEnd {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Content signature.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_signature: Option<String>,
    },
    /// Compact `toolcall_start` form.
    ToolcallStart {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Id.
        id: String,
        /// Tool name.
        tool_name: String,
    },
    /// Compact `toolcall_delta` form.
    ToolcallDelta {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
        /// Delta.
        delta: String,
    },
    /// Compact `toolcall_end` form.
    ToolcallEnd {
        /// Content index.
        #[serde(deserialize_with = "index")]
        content_index: usize,
    },
    /// Compact `done` form.
    Done {
        /// Reason.
        reason: DoneReason,
        /// Usage.
        usage: Usage,
    },
    /// Compact `error` form.
    Error {
        /// Reason.
        reason: ErrorReason,
        /// Error message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
        /// Usage.
        usage: Usage,
    },
}

/// Accept a finite integral non-negative JSON number that fits `usize` as a content index,
/// in any decimal or exponent spelling.
fn index<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
    struct Index;
    impl serde::de::Visitor<'_> for Index {
        type Value = usize;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a non-negative integral number")
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<usize, E> {
            usize::try_from(value).map_err(E::custom)
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<usize, E> {
            // An integral f64 prints without a fraction or exponent, so parsing is exact.
            if value >= 0.0
                && value.fract() == 0.0
                && let Ok(index) = value.abs().to_string().parse()
            {
                return Ok(index);
            }
            Err(E::invalid_value(serde::de::Unexpected::Float(value), &self))
        }
    }
    deserializer.deserialize_any(Index)
}

/// Producer-local protocol state; argument text never escapes into shared records.
#[derive(Default)]
pub(super) struct Reducer {
    /// Whether a remote terminal event was received, even if later input fails.
    pub(super) terminal: bool,
    /// Incomplete argument strings indexed by content block.
    arguments: HashMap<usize, String>,
}
impl Reducer {
    /// Decode one completed line and publish its reconstructed update.
    pub(super) fn line(
        &mut self,
        line: &str,
        partial: &SharedAssistantMessage,
        stream: &AssistantMessageEventStream,
    ) -> Result<(), String> {
        let Some(payload) = line.strip_prefix("data: ") else {
            return Ok(());
        };
        let payload = payload.trim_matches(maestro_text::is_whitespace_scalar);
        if payload.is_empty() {
            return Ok(());
        }
        let value: serde_json::Value =
            serde_json::from_str(payload).map_err(|error| error.to_string())?;
        let kind = value.get("type").and_then(serde_json::Value::as_str);
        if !kind.is_some_and(|kind| KINDS.contains(&kind)) {
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!(
                "Unhandled proxy event type: {}",
                kind.unwrap_or("undefined")
            );
            return Ok(());
        }
        let event = serde_json::from_value(value).map_err(|error| error.to_string())?;
        let mut message = crate::agent::write(partial);
        let output = self.process_proxy_event(event, &mut message, Arc::clone(partial))?;
        drop(message);
        if let Some(output) = output {
            stream.push(output);
        }
        Ok(())
    }
    /// Apply one typed update to the retained message and build the stream event.
    fn process_proxy_event(
        &mut self,
        event: ProxyAssistantMessageEvent,
        message: &mut AssistantMessage,
        partial: SharedAssistantMessage,
    ) -> Result<Option<Event>, String> {
        use ProxyAssistantMessageEvent as Wire;
        Ok(Some(match event {
            Wire::Start => Event::Start { partial },
            Wire::TextStart { content_index } => text_start(message, content_index, partial)?,
            Wire::TextDelta {
                content_index,
                delta,
            } => text_delta(message, content_index, delta, partial)?,
            Wire::TextEnd {
                content_index,
                content_signature,
            } => text_end(message, content_index, content_signature, partial)?,
            Wire::ThinkingStart { content_index } => {
                thinking_start(message, content_index, partial)?
            }
            Wire::ThinkingDelta {
                content_index,
                delta,
            } => thinking_delta(message, content_index, delta, partial)?,
            Wire::ThinkingEnd {
                content_index,
                content_signature,
            } => thinking_end(message, content_index, content_signature, partial)?,
            Wire::ToolcallStart {
                content_index,
                id,
                tool_name,
            } => {
                self.arguments.insert(content_index, String::new());
                tool_start(message, content_index, id, tool_name, partial)?
            }
            Wire::ToolcallDelta {
                content_index,
                delta,
            } => {
                let text = self.arguments.entry(content_index).or_default();
                text.push_str(&delta);
                tool_delta(message, content_index, delta, text, partial)?
            }
            Wire::ToolcallEnd { content_index } => {
                self.arguments.remove(&content_index);
                return Ok(tool_end(message, content_index, partial));
            }
            Wire::Done { reason, usage } => {
                self.terminal = true;
                done(message, reason, usage, partial)
            }
            Wire::Error {
                reason,
                error_message,
                usage,
            } => {
                self.terminal = true;
                failed(message, reason, error_message, usage, partial)
            }
        }))
    }
}

/// The twelve compact discriminants; any other discriminant is warned about and skipped.
const KINDS: [&str; 12] = [
    "start",
    "text_start",
    "text_delta",
    "text_end",
    "thinking_start",
    "thinking_delta",
    "thinking_end",
    "toolcall_start",
    "toolcall_delta",
    "toolcall_end",
    "done",
    "error",
];
/// Borrow a text block or return its authored mismatch diagnostic.
fn text<'a>(
    message: &'a mut AssistantMessage,
    index: usize,
    operation: &str,
) -> Result<&'a mut TextContent, String> {
    match message.content.get_mut(index) {
        Some(AssistantContent::Text(content)) => Ok(content),
        _ => Err(format!("Received {operation} for non-text content")),
    }
}
/// Borrow a thinking block or return its authored mismatch diagnostic.
fn thinking<'a>(
    message: &'a mut AssistantMessage,
    index: usize,
    operation: &str,
) -> Result<&'a mut ThinkingContent, String> {
    match message.content.get_mut(index) {
        Some(AssistantContent::Thinking(content)) => Ok(content),
        _ => Err(format!("Received {operation} for non-thinking content")),
    }
}
/// Replace the block at a dense index or append the next one.
fn replace(
    message: &mut AssistantMessage,
    index: usize,
    content: AssistantContent,
) -> Result<(), String> {
    let length = message.content.len();
    match index.cmp(&length) {
        Ordering::Less => message.content[index] = content,
        Ordering::Equal => message.content.push(content),
        Ordering::Greater => return Err(format!("Proxy content index {index} leaves a gap")),
    }
    Ok(())
}
/// Start an empty text block.
fn text_start(
    message: &mut AssistantMessage,
    content_index: usize,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let block = AssistantContent::Text(TextContent::new(String::new()));
    replace(message, content_index, block)?;
    Ok(Event::TextStart {
        content_index,
        partial,
    })
}
/// Append to a text block.
fn text_delta(
    message: &mut AssistantMessage,
    content_index: usize,
    delta: String,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    text(message, content_index, "text_delta")?
        .text
        .push_str(&delta);
    Ok(Event::TextDelta {
        content_index,
        delta,
        partial,
    })
}
/// Finish a text block, setting or clearing its signature.
fn text_end(
    message: &mut AssistantMessage,
    content_index: usize,
    signature: Option<String>,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let block = text(message, content_index, "text_end")?;
    block.text_signature = signature;
    Ok(Event::TextEnd {
        content_index,
        content: block.text.clone(),
        partial,
    })
}
/// Start an empty thinking block.
fn thinking_start(
    message: &mut AssistantMessage,
    content_index: usize,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let block = AssistantContent::Thinking(ThinkingContent {
        thinking: String::new(),
        thinking_signature: None,
        redacted: None,
    });
    replace(message, content_index, block)?;
    Ok(Event::ThinkingStart {
        content_index,
        partial,
    })
}
/// Append to a thinking block.
fn thinking_delta(
    message: &mut AssistantMessage,
    content_index: usize,
    delta: String,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    thinking(message, content_index, "thinking_delta")?
        .thinking
        .push_str(&delta);
    Ok(Event::ThinkingDelta {
        content_index,
        delta,
        partial,
    })
}
/// Finish a thinking block, setting or clearing its signature.
fn thinking_end(
    message: &mut AssistantMessage,
    content_index: usize,
    signature: Option<String>,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let block = thinking(message, content_index, "thinking_end")?;
    block.thinking_signature = signature;
    Ok(Event::ThinkingEnd {
        content_index,
        content: block.thinking.clone(),
        partial,
    })
}
/// Start a tool call with empty arguments.
fn tool_start(
    message: &mut AssistantMessage,
    content_index: usize,
    id: String,
    name: String,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let block = AssistantContent::ToolCall(ToolCall {
        id,
        name,
        arguments: JsonObject::new().into(),
        thought_signature: None,
    });
    replace(message, content_index, block)?;
    Ok(Event::ToolcallStart {
        content_index,
        partial,
    })
}
/// Reparse the accumulated argument text of a tool call.
fn tool_delta(
    message: &mut AssistantMessage,
    content_index: usize,
    delta: String,
    arguments: &str,
    partial: SharedAssistantMessage,
) -> Result<Event, String> {
    let Some(AssistantContent::ToolCall(block)) = message.content.get_mut(content_index) else {
        return Err("Received toolcall_delta for non-toolCall content".into());
    };
    // A falsy reconstructed root falls back to an empty object.
    let parsed = parsed_arguments(arguments);
    block.arguments = if maestro_json::is_truthy(&parsed) {
        parsed
    } else {
        JsonObject::new().into()
    };
    Ok(Event::ToolcallDelta {
        content_index,
        delta,
        partial,
    })
}
/// Finish a tool call; a block of another kind yields no event.
fn tool_end(
    message: &AssistantMessage,
    content_index: usize,
    partial: SharedAssistantMessage,
) -> Option<Event> {
    match message.content.get(content_index) {
        Some(AssistantContent::ToolCall(block)) => Some(Event::ToolcallEnd {
            content_index,
            tool_call: block.clone(),
            partial,
        }),
        _ => None,
    }
}
/// Record a remote success with its supplied usage.
fn done(
    message: &mut AssistantMessage,
    reason: DoneReason,
    usage: Usage,
    partial: SharedAssistantMessage,
) -> Event {
    message.stop_reason = match reason {
        DoneReason::Stop => StopReason::Stop,
        DoneReason::Length => StopReason::Length,
        DoneReason::ToolUse => StopReason::ToolUse,
    };
    message.usage = usage;
    Event::Done {
        reason,
        message: partial,
    }
}
/// Record a remote failure with its supplied usage and optional text.
fn failed(
    message: &mut AssistantMessage,
    reason: ErrorReason,
    error_message: Option<String>,
    usage: Usage,
    partial: SharedAssistantMessage,
) -> Event {
    message.stop_reason = match reason {
        ErrorReason::Aborted => StopReason::Aborted,
        ErrorReason::Error => StopReason::Error,
    };
    message.error_message = error_message;
    message.usage = usage;
    Event::Error {
        reason,
        error: partial,
    }
}
