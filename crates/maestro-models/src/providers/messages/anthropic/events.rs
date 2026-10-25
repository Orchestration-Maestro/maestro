//! Reduction of decoded message events into shared assistant updates.

use std::sync::{Arc, PoisonError};

use serde_json::Value;
use serde_json::value::RawValue;

use super::wire::{
    BlockDelta, ContentBlockDelta, ContentBlockStart, ContentBlockStop, Event, MessageDelta,
    MessageStart, OpenedBlock, Position, StartedMessage, ToolUse, Usage,
};
use crate::arguments::replay::nullish_default;
use crate::providers::http::RequestFailure;
use crate::providers::json_text::{json_value, or_zero};
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Model,
    SharedAssistantMessage, StopReason, TextContent, ThinkingContent, Tool, ToolCall,
    calculate_cost, parsed_arguments,
};
use maestro_request::presence::Presence;

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
    wire_index: Position,
    /// Whether the block still accepts updates.
    open: bool,
    /// Kind of the block.
    kind: Kind,
}

/// Builds the update that announces a block opening at a content position.
type StartEvent = fn(usize, SharedAssistantMessage) -> AssistantMessageEvent;

/// Reduces events into the shared message, announcing each change.
pub(super) struct Reducer<'a> {
    /// Model that was asked, for cost rates.
    model: Arc<Model>,
    /// Message shared by every update and the final result.
    output: SharedAssistantMessage,
    /// Receiver of the updates.
    stream: AssistantMessageEventStream,
    /// One slot per content block, in content order.
    slots: Vec<Slot>,
    /// Subscription declarations in their original order.
    tools: Option<&'a [Tool]>,
}

impl<'a> Reducer<'a> {
    /// Start reducing into `output`, announcing changes on `stream`.
    pub(super) fn new(
        model: &Arc<Model>,
        output: &SharedAssistantMessage,
        stream: &AssistantMessageEventStream,
        tools: Option<&'a [Tool]>,
    ) -> Self {
        Self {
            model: Arc::clone(model),
            output: Arc::clone(output),
            stream: stream.clone(),
            slots: Vec::new(),
            tools,
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
    /// Fails on a stop reason that names no known outcome and on usage that a reduction needs
    /// and the event lacks or does not write as an object, or on an invalid subscription tool name
    /// when declarations are nonempty.
    pub(super) fn event(&mut self, event: Event) -> Result<Progress, RequestFailure> {
        match event {
            Event::MessageStart(start) => self.start_message(start)?,
            Event::ContentBlockStart(start) => self.start_block(start)?,
            Event::ContentBlockDelta(delta) => self.delta(delta),
            Event::ContentBlockStop(stop) => self.stop_block(&stop),
            Event::MessageDelta(delta) => self.message_delta(&delta)?,
            Event::MessageStop => return Ok(Progress::Complete),
        }
        Ok(Progress::Open)
    }

    /// Record the response identifier, then the usage the message opens with.
    fn start_message(&self, start: MessageStart) -> Result<(), RequestFailure> {
        let StartedMessage { id, usage } = start.message;
        self.update(|message| message.response_id = id);
        let usage = reported_usage(usage.as_deref())?;
        self.update(|message| {
            message.usage.input = or_zero(usage.input);
            message.usage.output = or_zero(usage.output);
            message.usage.cache_read = or_zero(usage.cache_read);
            message.usage.cache_write = or_zero(usage.cache_write);
            self.total_usage(message);
        });
        Ok(())
    }

    /// Recompute the token total and the costs of the message.
    fn total_usage(&self, message: &mut AssistantMessage) {
        let usage = &mut message.usage;
        usage.total_tokens =
            Presence::Present(usage.input + usage.output + usage.cache_read + usage.cache_write);
        calculate_cost(&self.model, usage);
    }

    /// Record the stop reason, then the usage counts a message update reports.
    fn message_delta(&self, delta: &MessageDelta) -> Result<(), RequestFailure> {
        let stop = delta
            .delta
            .stop_reason
            .as_deref()
            .filter(|reason| !reason.is_empty())
            .map(stop_reason)
            .transpose()?;
        let reported = reported_usage(delta.usage.as_deref())?;
        self.update(|message| {
            if let Some(stop) = stop {
                message.stop_reason = stop;
            }
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
    fn push_block(&mut self, wire_index: Position, kind: Kind, content: AssistantContent) -> usize {
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

    /// Open a recognized block, rejecting invalid subscription names when tools are declared.
    fn start_block(&mut self, start: ContentBlockStart) -> Result<(), RequestFailure> {
        let (kind, content, announced): (Kind, AssistantContent, StartEvent) =
            match start.content_block {
                OpenedBlock::Text => (
                    Kind::Text,
                    AssistantContent::Text(TextContent::new(String::new())),
                    |content_index, partial| AssistantMessageEvent::TextStart {
                        content_index,
                        partial,
                    },
                ),
                OpenedBlock::Thinking => (
                    Kind::Thinking,
                    thinking(String::new(), Some(String::new()), None),
                    thinking_start,
                ),
                OpenedBlock::Redacted(data) => (
                    Kind::Thinking,
                    thinking(REDACTED_TEXT.to_owned(), data, Some(true)),
                    thinking_start,
                ),
                OpenedBlock::ToolUse(call) => {
                    let decoded = call
                        .name
                        .as_deref()
                        .filter(|raw| raw.get().starts_with('"'))
                        .map(|raw| serde_json::from_str::<String>(raw.get()))
                        .transpose();
                    let name = if let Some(tools) = self.tools.filter(|tools| !tools.is_empty()) {
                        let name = decoded
                            .map_err(|error| RequestFailure::new(error.to_string()))?
                            .ok_or_else(|| RequestFailure::new("Tool name must be a string"))?;
                        super::tool_names::inbound(name, tools)
                    } else {
                        decoded.ok().flatten().unwrap_or_default()
                    };
                    (
                        Kind::Tool(None),
                        tool_call(call, name),
                        |content_index, partial| AssistantMessageEvent::ToolcallStart {
                            content_index,
                            partial,
                        },
                    )
                }
                OpenedBlock::Other => return Ok(()),
            };
        let position = self.push_block(start.index, kind, content);
        self.announce(|partial| announced(position, partial));
        Ok(())
    }

    /// Find the open block that carries a wire position, with its content position.
    fn open_slot(&mut self, wire_index: &Position) -> Option<(usize, &mut Slot)> {
        self.slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.open && slot.wire_index.names(wire_index))
    }

    /// Apply a change to the open block it addresses; changes that do not suit the block are
    /// ignored.
    fn delta(&mut self, change: ContentBlockDelta) {
        let Some((position, slot)) = self.open_slot(&change.index) else {
            return;
        };
        match (change.delta, &mut slot.kind) {
            (BlockDelta::Text(text), Kind::Text) => self.append_text(position, text),
            (BlockDelta::Thinking(thinking), Kind::Thinking) => {
                self.append_thinking(position, thinking);
            }
            (BlockDelta::Signature(fragment), Kind::Thinking) => {
                self.append_signature(position, &fragment);
            }
            (BlockDelta::ArgumentFragment(fragment), Kind::Tool(partial)) => {
                let buffer = partial.get_or_insert_with(String::new);
                buffer.push_str(&fragment);
                let arguments = parsed_arguments(buffer);
                self.append_arguments(position, fragment, arguments);
            }
            _ => {}
        }
    }

    /// Append to a text block.
    fn append_text(&self, position: usize, delta: String) {
        self.update(|message| {
            if let Some(AssistantContent::Text(text)) = message.content.get_mut(position) {
                text.text.push_str(&delta);
            }
        });
        self.announce(|partial| AssistantMessageEvent::TextDelta {
            content_index: position,
            delta,
            partial,
        });
    }

    /// Append to a reasoning block.
    fn append_thinking(&self, position: usize, delta: String) {
        self.update(|message| {
            if let Some(AssistantContent::Thinking(thinking)) = message.content.get_mut(position) {
                thinking.thinking.push_str(&delta);
            }
        });
        self.announce(|partial| AssistantMessageEvent::ThinkingDelta {
            content_index: position,
            delta,
            partial,
        });
    }

    /// Append a signature fragment to a reasoning block without announcing it.
    fn append_signature(&self, position: usize, fragment: &str) {
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
    fn append_arguments(&self, position: usize, fragment: String, arguments: Value) {
        self.update(|message| {
            if let Some(AssistantContent::ToolCall(call)) = message.content.get_mut(position) {
                call.arguments = arguments;
            }
        });
        self.announce(|partial| AssistantMessageEvent::ToolcallDelta {
            content_index: position,
            delta: fragment,
            partial,
        });
    }

    /// Close the open block a stop event addresses and announce its end.
    fn stop_block(&mut self, stop: &ContentBlockStop) {
        let Some((position, slot)) = self.open_slot(&stop.index) else {
            return;
        };
        slot.open = false;
        let tool_arguments = match &slot.kind {
            Kind::Tool(Some(partial)) => Some(parsed_arguments(partial)),
            _ => None,
        };
        let ended = self.update(|message| {
            let block = message.content.get_mut(position)?;
            if let (AssistantContent::ToolCall(call), Some(arguments)) =
                (&mut *block, tool_arguments)
            {
                call.arguments = arguments;
            }
            let partial = Arc::clone(&self.output);
            Some(match block {
                AssistantContent::Text(text) => AssistantMessageEvent::TextEnd {
                    content_index: position,
                    content: text.text.clone(),
                    partial,
                },
                AssistantContent::Thinking(thinking) => AssistantMessageEvent::ThinkingEnd {
                    content_index: position,
                    content: thinking.thinking.clone(),
                    partial,
                },
                AssistantContent::ToolCall(call) => AssistantMessageEvent::ToolcallEnd {
                    content_index: position,
                    tool_call: call.clone(),
                    partial,
                },
            })
        });
        if let Some(event) = ended {
            self.stream.push(event);
        }
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

/// Build a reasoning block.
fn thinking(text: String, signature: Option<String>, redacted: Option<bool>) -> AssistantContent {
    AssistantContent::Thinking(ThinkingContent {
        thinking: text,
        thinking_signature: signature,
        redacted,
    })
}

/// Build the tool call a block opens: its initial arguments are those it carries, or an empty
/// object when it carries none or carries null.
fn tool_call(call: ToolUse, name: String) -> AssistantContent {
    let arguments = call
        .input
        .as_deref()
        .and_then(|raw: &RawValue| json_value(raw).ok())
        .map_or_else(|| nullish_default(Value::Null), nullish_default);
    AssistantContent::ToolCall(ToolCall {
        id: call.id.unwrap_or_default(),
        name,
        arguments,
        thought_signature: None,
    })
}

/// Decode the usage an event reports where a reduction reads it.
fn reported_usage(raw: Option<&RawValue>) -> Result<Usage, RequestFailure> {
    Usage::read(raw).map_err(|cause| RequestFailure::new(cause.to_string()))
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
