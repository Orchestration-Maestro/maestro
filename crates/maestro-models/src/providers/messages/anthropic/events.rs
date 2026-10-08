//! Reduction of decoded message events into shared assistant updates.

use std::sync::{Arc, PoisonError};

use serde_json::Value;
use serde_json::value::RawValue;

use super::wire::{
    BlockDelta, ContentBlockDelta, ContentBlockStart, ContentBlockStop, Event, MessageDelta,
    MessageStart, OpenedBlock,
};
use crate::providers::http::RequestFailure;
use crate::providers::json_text::json_value;
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    JsonObject, Model, SharedAssistantMessage, StopReason, TextContent, ThinkingContent, ToolCall,
    calculate_cost, parse_streaming_json,
};

/// Text of a redacted reasoning block.
const REDACTED_TEXT: &str = "[Reasoning redacted]";

/// What an event leaves to read.
pub(super) enum Progress {
    /// More events may follow.
    Open,
    /// The message is complete; later events are not read.
    Complete,
}

/// Kind of a content block, with the state only that kind needs.
enum Kind {
    /// A text block.
    Text,
    /// A reasoning block.
    Thinking,
    /// A tool call; its argument text is `None` until a fragment arrives.
    Tool(Option<String>),
}

/// A block of the message and the wire position that addresses it while it is open.
struct Slot {
    /// Position of the block on the wire.
    wire_index: Option<f64>,
    /// Whether the block still accepts updates.
    open: bool,
    /// Kind of the block.
    kind: Kind,
}

/// Builds the update that announces a block opening at a content position.
type StartEvent = fn(usize, SharedAssistantMessage) -> AssistantMessageEvent;

/// Reduces events into the shared message, announcing each change.
pub(super) struct Reducer {
    /// Model that was asked, for cost rates.
    model: Arc<Model>,
    /// Message shared by every update and the final result.
    output: SharedAssistantMessage,
    /// Receiver of the updates.
    stream: AssistantMessageEventStream,
    /// One slot per content block, in content order.
    slots: Vec<Slot>,
}

impl Reducer {
    /// Start reducing into `output`, announcing changes on `stream`.
    pub(super) fn new(
        model: &Arc<Model>,
        output: &SharedAssistantMessage,
        stream: &AssistantMessageEventStream,
    ) -> Self {
        Self {
            model: Arc::clone(model),
            output: Arc::clone(output),
            stream: stream.clone(),
            slots: Vec::new(),
        }
    }

    /// Change the shared message without holding the lock while updates are announced.
    fn update<R>(&self, change: impl FnOnce(&mut AssistantMessage) -> R) -> R {
        change(&mut self.output.write().unwrap_or_else(PoisonError::into_inner))
    }

    /// Announce a change to the shared message.
    fn announce(&self, event: impl FnOnce(SharedAssistantMessage) -> AssistantMessageEvent) {
        self.stream.push(event(Arc::clone(&self.output)));
    }

    /// Reduce one event.
    ///
    /// # Errors
    /// Fails on a stop reason that names no known outcome.
    pub(super) fn event(&mut self, event: Event) -> Result<Progress, RequestFailure> {
        match event {
            Event::MessageStart(start) => self.start_message(&start),
            Event::ContentBlockStart(start) => self.start_block(&start),
            Event::ContentBlockDelta(delta) => self.delta(&delta),
            Event::ContentBlockStop(stop) => self.stop_block(&stop),
            Event::MessageDelta(delta) => self.message_delta(&delta)?,
            Event::MessageStop => return Ok(Progress::Complete),
        }
        Ok(Progress::Open)
    }

    /// Record the response identifier and the usage the message opens with.
    fn start_message(&self, start: &MessageStart) {
        self.update(|message| {
            message.response_id.clone_from(&start.message.id);
            let usage = &start.message.usage;
            message.usage.input = reported(usage.input);
            message.usage.output = reported(usage.output);
            message.usage.cache_read = reported(usage.cache_read);
            message.usage.cache_write = reported(usage.cache_write);
            self.total_usage(message);
        });
    }

    /// Recompute the token total and the costs of the message.
    fn total_usage(&self, message: &mut AssistantMessage) {
        let usage = &mut message.usage;
        usage.total_tokens = usage.input + usage.output + usage.cache_read + usage.cache_write;
        calculate_cost(&self.model, usage);
    }

    /// Record the stop reason and the usage counts a message update reports.
    fn message_delta(&self, delta: &MessageDelta) -> Result<(), RequestFailure> {
        let stop = delta
            .delta
            .stop_reason
            .as_deref()
            .filter(|reason| !reason.is_empty())
            .map(stop_reason)
            .transpose()?;
        self.update(|message| {
            if let Some(stop) = stop {
                message.stop_reason = stop;
            }
            let reported = &delta.usage;
            let usage = &mut message.usage;
            replace_count(&mut usage.input, reported.input);
            replace_count(&mut usage.output, reported.output);
            replace_count(&mut usage.cache_read, reported.cache_read);
            replace_count(&mut usage.cache_write, reported.cache_write);
            self.total_usage(message);
        });
        Ok(())
    }

    /// Add a block to the message and return its content position.
    fn push_block(
        &mut self,
        wire_index: Option<f64>,
        kind: Kind,
        content: AssistantContent,
    ) -> usize {
        self.slots.push(Slot {
            wire_index,
            open: true,
            kind,
        });
        self.update(|message| {
            message.content.push(content);
            message.content.len() - 1
        })
    }

    /// Open the block a start event describes; kinds the message has no block for are ignored.
    fn start_block(&mut self, start: &ContentBlockStart) {
        let block = &start.content_block;
        let (kind, content, announced): (Kind, AssistantContent, StartEvent) =
            match block.r#type.as_deref() {
                Some("text") => (
                    Kind::Text,
                    AssistantContent::Text(TextContent {
                        text: String::new(),
                        text_signature: None,
                    }),
                    |content_index, partial| AssistantMessageEvent::TextStart {
                        content_index,
                        partial,
                    },
                ),
                Some("thinking") => (
                    Kind::Thinking,
                    thinking(String::new(), Some(String::new()), None),
                    thinking_start,
                ),
                Some("redacted_thinking") => (
                    Kind::Thinking,
                    thinking(REDACTED_TEXT.to_owned(), block.data.clone(), Some(true)),
                    thinking_start,
                ),
                Some("tool_use") => (
                    Kind::Tool(None),
                    tool_call(block),
                    |content_index, partial| AssistantMessageEvent::ToolcallStart {
                        content_index,
                        partial,
                    },
                ),
                _ => return,
            };
        let position = self.push_block(start.index, kind, content);
        self.announce(|partial| announced(position, partial));
    }

    /// Find the open block that carries a wire position, with its content position.
    fn open_slot(&mut self, wire_index: Option<f64>) -> Option<(usize, &mut Slot)> {
        self.slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.open && slot.wire_index == wire_index)
    }

    /// Apply a change to the open block it addresses; changes that do not suit the block are
    /// ignored.
    fn delta(&mut self, change: &ContentBlockDelta) {
        let Some((position, slot)) = self.open_slot(change.index) else {
            return;
        };
        let delta = &change.delta;
        match (delta.r#type.as_deref(), &mut slot.kind) {
            (Some("text_delta"), Kind::Text) => {
                if let Some(text) = delta.text.as_deref() {
                    self.append_text(position, text);
                }
            }
            (Some("thinking_delta"), Kind::Thinking) => {
                if let Some(thinking) = delta.thinking.as_deref() {
                    self.append_thinking(position, thinking);
                }
            }
            (Some("signature_delta"), Kind::Thinking) => self.append_signature(position, delta),
            (Some("input_json_delta"), Kind::Tool(partial)) => {
                if let Some(fragment) = delta.partial_json.as_deref() {
                    let buffer = partial.get_or_insert_with(String::new);
                    buffer.push_str(fragment);
                    let arguments = streamed_arguments(buffer);
                    self.append_arguments(position, fragment, arguments);
                }
            }
            _ => {}
        }
    }

    /// Append to a text block.
    fn append_text(&self, position: usize, delta: &str) {
        self.update(|message| {
            if let Some(AssistantContent::Text(text)) = message.content.get_mut(position) {
                text.text.push_str(delta);
            }
        });
        self.announce(|partial| AssistantMessageEvent::TextDelta {
            content_index: position,
            delta: delta.to_owned(),
            partial,
        });
    }

    /// Append to a reasoning block.
    fn append_thinking(&self, position: usize, delta: &str) {
        self.update(|message| {
            if let Some(AssistantContent::Thinking(thinking)) = message.content.get_mut(position) {
                thinking.thinking.push_str(delta);
            }
        });
        self.announce(|partial| AssistantMessageEvent::ThinkingDelta {
            content_index: position,
            delta: delta.to_owned(),
            partial,
        });
    }

    /// Append a signature fragment to a reasoning block without announcing it.
    fn append_signature(&self, position: usize, delta: &BlockDelta) {
        let Some(fragment) = delta.signature.as_deref() else {
            return;
        };
        self.update(|message| {
            if let Some(AssistantContent::Thinking(thinking)) = message.content.get_mut(position) {
                thinking
                    .thinking_signature
                    .get_or_insert_with(String::new)
                    .push_str(fragment);
            }
        });
    }

    /// Replace the arguments of a tool call with those the text received so far spells.
    fn append_arguments(&self, position: usize, fragment: &str, arguments: JsonObject) {
        self.update(|message| {
            if let Some(AssistantContent::ToolCall(call)) = message.content.get_mut(position) {
                call.arguments = arguments;
            }
        });
        self.announce(|partial| AssistantMessageEvent::ToolcallDelta {
            content_index: position,
            delta: fragment.to_owned(),
            partial,
        });
    }

    /// Close the open block a stop event addresses and announce its end.
    fn stop_block(&mut self, stop: &ContentBlockStop) {
        let Some((position, slot)) = self.open_slot(stop.index) else {
            return;
        };
        slot.open = false;
        let tool_arguments = match &slot.kind {
            Kind::Tool(Some(partial)) => Some(streamed_arguments(partial)),
            _ => None,
        };
        let ended = self.update(|message| {
            let block = message.content.get_mut(position)?;
            if let (AssistantContent::ToolCall(call), Some(arguments)) =
                (&mut *block, tool_arguments)
            {
                call.arguments = arguments;
            }
            Some(block.clone())
        });
        let Some(block) = ended else { return };
        self.announce(|partial| match block {
            AssistantContent::Text(text) => AssistantMessageEvent::TextEnd {
                content_index: position,
                content: text.text,
                partial,
            },
            AssistantContent::Thinking(thinking) => AssistantMessageEvent::ThinkingEnd {
                content_index: position,
                content: thinking.thinking,
                partial,
            },
            AssistantContent::ToolCall(tool_call) => AssistantMessageEvent::ToolcallEnd {
                content_index: position,
                tool_call,
                partial,
            },
        });
    }
}

/// Replace a count with the one a report supplies, if it supplies one.
fn replace_count(count: &mut f64, supplied: Option<f64>) {
    if let Some(supplied) = supplied {
        *count = supplied;
    }
}

/// The update that announces a reasoning block.
fn thinking_start(content_index: usize, partial: SharedAssistantMessage) -> AssistantMessageEvent {
    AssistantMessageEvent::ThinkingStart {
        content_index,
        partial,
    }
}

/// A reported count where a missing, null or zero report means zero.
fn reported(count: Option<f64>) -> f64 {
    count.filter(|count| *count != 0.0).unwrap_or(0.0)
}

/// Build a reasoning block.
fn thinking(text: String, signature: Option<String>, redacted: Option<bool>) -> AssistantContent {
    AssistantContent::Thinking(ThinkingContent {
        thinking: text,
        thinking_signature: signature,
        redacted,
    })
}

/// Build the tool call a block opens: its initial arguments are those it carries when they
/// are an object that fits the conversion bound, otherwise none.
fn tool_call(block: &OpenedBlock) -> AssistantContent {
    let arguments = block
        .input
        .as_deref()
        .and_then(|raw: &RawValue| json_value(raw).ok())
        .and_then(|value| match value {
            Value::Object(arguments) => Some(arguments),
            _ => None,
        })
        .unwrap_or_default();
    AssistantContent::ToolCall(ToolCall {
        id: block.id.clone().unwrap_or_default(),
        name: block.name.clone().unwrap_or_default(),
        arguments,
        thought_signature: None,
    })
}

/// Parse streamed argument text into an object; anything else becomes empty.
fn streamed_arguments(partial: &str) -> JsonObject {
    match parse_streaming_json(Some(partial)) {
        Value::Object(arguments) => arguments,
        _ => JsonObject::new(),
    }
}

/// The outcome a stop reason names.
///
/// # Errors
/// Fails on a reason that names no known outcome.
fn stop_reason(reason: &str) -> Result<StopReason, RequestFailure> {
    match reason {
        "end_turn" | "pause_turn" | "stop_sequence" => Ok(StopReason::Stop),
        "max_tokens" => Ok(StopReason::Length),
        "tool_use" => Ok(StopReason::ToolUse),
        "refusal" | "sensitive" => Ok(StopReason::Error),
        other => Err(RequestFailure::new(format!(
            "Unhandled stop reason: {other}"
        ))),
    }
}
