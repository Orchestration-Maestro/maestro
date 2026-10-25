//! Reduction of decoded stream events into shared assistant updates.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, PoisonError};

use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStart, ContentBlockStartEvent,
    ContentBlockStopEvent, ConversationRole, ConverseStreamMetadataEvent, ConverseStreamOutput,
    ReasoningContentBlockDelta, StopReason as Stop,
};

use crate::providers::http::RequestFailure;
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Model,
    SharedAssistantMessage, StopReason, TextContent, ThinkingContent, ToolCall, calculate_cost,
    parsed_arguments,
};
use maestro_request::presence::Presence;

/// Failure of a message that opens with a turn other than the assistant's.
const USER_START_TEXT: &str =
    "Unexpected assistant message start but got user message start instead";

/// Private state of an open content block.
enum Block {
    /// Text position in the assistant message.
    Text(usize),
    /// Thinking position in the assistant message.
    Thinking(usize),
    /// Tool position and its accumulated JSON fragments.
    Tool {
        /// Position in the public content sequence.
        position: usize,
        /// Accumulated argument fragments, never published.
        json: String,
    },
}

impl Block {
    /// Position of this block in the public content sequence.
    fn position(&self) -> usize {
        match self {
            Self::Text(position) | Self::Thinking(position) | Self::Tool { position, .. } => {
                *position
            }
        }
    }
}

/// Reduces events into the shared message, announcing each change.
pub(super) struct Reducer {
    /// Model that was asked, for cost rates.
    model: Arc<Model>,
    /// Message shared by every update and the final result.
    output: SharedAssistantMessage,
    /// Receiver of the updates.
    stream: AssistantMessageEventStream,
    /// Open content blocks by the service's block number.
    open: HashMap<i32, VecDeque<Block>>,
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
            open: HashMap::new(),
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

    /// Reduce one event; unrecognized events are skipped.
    ///
    /// # Errors
    /// Fails when the message opens with a turn other than the assistant's.
    pub(super) fn event(&mut self, event: &ConverseStreamOutput) -> Result<(), RequestFailure> {
        match event {
            ConverseStreamOutput::MessageStart(start) => {
                if *start.role() != ConversationRole::Assistant {
                    return Err(RequestFailure::new(USER_START_TEXT));
                }
                self.announce(|partial| AssistantMessageEvent::Start { partial });
            }
            ConverseStreamOutput::ContentBlockStart(start) => self.start(start),
            ConverseStreamOutput::ContentBlockDelta(delta) => self.delta(delta),
            ConverseStreamOutput::ContentBlockStop(stop) => self.stop(stop),
            ConverseStreamOutput::MessageStop(stop) => {
                let reason = stop_reason(stop.stop_reason());
                self.update(|message| message.stop_reason = reason);
            }
            ConverseStreamOutput::Metadata(metadata) => self.metadata(metadata),
            _ => {}
        }
        Ok(())
    }

    /// The content position of the open block of a service block number, opening a text block
    /// first when none is open.
    fn open_text(&mut self, number: i32) -> usize {
        if let Some(position) = self.open.get(&number).and_then(VecDeque::front) {
            return position.position();
        }
        let position = self.update(|message| {
            message
                .content
                .push(AssistantContent::Text(TextContent::new(String::new())));
            message.content.len() - 1
        });
        self.open
            .entry(number)
            .or_default()
            .push_back(Block::Text(position));
        self.announce(|partial| AssistantMessageEvent::TextStart {
            content_index: position,
            partial,
        });
        position
    }

    /// Append text to the open block of the event's number, opening one first when none is.
    fn delta(&mut self, event: &ContentBlockDeltaEvent) {
        if let Some(ContentBlockDelta::ToolUse(delta)) = event.delta() {
            self.tool_delta(event.content_block_index(), delta.input());
            return;
        }
        if let Some(ContentBlockDelta::ReasoningContent(delta)) = event.delta() {
            self.thinking(event.content_block_index(), delta);
            return;
        }
        let Some(ContentBlockDelta::Text(delta)) = event.delta() else {
            return;
        };
        let position = self.open_text(event.content_block_index());
        let changed = self.update(|message| {
            if let Some(AssistantContent::Text(text)) = message.content.get_mut(position) {
                text.text.push_str(delta);
                true
            } else {
                false
            }
        });
        if !changed {
            return;
        }
        self.announce(|partial| AssistantMessageEvent::TextDelta {
            content_index: position,
            delta: delta.clone(),
            partial,
        });
    }

    /// Append a new tool block when the service announces a tool start.
    fn start(&mut self, event: &ContentBlockStartEvent) {
        let Some(ContentBlockStart::ToolUse(start)) = event.start() else {
            return;
        };
        let position = self.update(|message| {
            message.content.push(AssistantContent::ToolCall(ToolCall {
                id: start.tool_use_id().to_owned(),
                name: start.name().to_owned(),
                arguments: parsed_arguments(""),
                thought_signature: None,
            }));
            message.content.len() - 1
        });
        self.open
            .entry(event.content_block_index())
            .or_default()
            .push_back(Block::Tool {
                position,
                json: String::new(),
            });
        self.announce(|partial| AssistantMessageEvent::ToolcallStart {
            content_index: position,
            partial,
        });
    }

    /// Reparse accumulated tool input after each fragment of an open tool.
    fn tool_delta(&mut self, number: i32, delta: &str) {
        let Some(Block::Tool { position, json }) =
            self.open.get_mut(&number).and_then(VecDeque::front_mut)
        else {
            return;
        };
        json.push_str(delta);
        let arguments = parsed_arguments(json);
        let position = *position;
        self.update(|message| {
            if let Some(AssistantContent::ToolCall(tool)) = message.content.get_mut(position) {
                tool.arguments = arguments;
            }
        });
        self.announce(|partial| AssistantMessageEvent::ToolcallDelta {
            content_index: position,
            delta: delta.to_owned(),
            partial,
        });
    }

    /// Append reasoning text or silently accumulate a signature, opening a block if needed.
    fn thinking(&mut self, number: i32, delta: &ReasoningContentBlockDelta) {
        let position = if let Some(position) = self.open.get(&number).and_then(VecDeque::front) {
            position.position()
        } else {
            let position = self.update(|message| {
                message
                    .content
                    .push(AssistantContent::Thinking(ThinkingContent {
                        thinking: String::new(),
                        thinking_signature: Some(String::new()),
                        redacted: None,
                    }));
                message.content.len() - 1
            });
            self.open
                .entry(number)
                .or_default()
                .push_back(Block::Thinking(position));
            self.announce(|partial| AssistantMessageEvent::ThinkingStart {
                content_index: position,
                partial,
            });
            position
        };
        let text = self.update(|message| {
            let Some(AssistantContent::Thinking(thinking)) = message.content.get_mut(position)
            else {
                return None;
            };
            match delta {
                ReasoningContentBlockDelta::Text(text) if !text.is_empty() => {
                    thinking.thinking.push_str(text);
                    Some(text.clone())
                }
                ReasoningContentBlockDelta::Signature(signature) => {
                    thinking
                        .thinking_signature
                        .get_or_insert_with(String::new)
                        .push_str(signature);
                    None
                }
                _ => None,
            }
        });
        if let Some(delta) = text {
            self.announce(|partial| AssistantMessageEvent::ThinkingDelta {
                content_index: position,
                delta,
                partial,
            });
        }
    }

    /// Close the open block of the event's number; an unknown number does nothing.
    fn stop(&mut self, event: &ContentBlockStopEvent) {
        let number = event.content_block_index();
        let Some(queue) = self.open.get_mut(&number) else {
            return;
        };
        let Some(block) = queue.pop_front() else {
            return;
        };
        if queue.is_empty() {
            self.open.remove(&number);
        }
        let position = block.position();
        let content = self.update(|message| {
            if let (Block::Tool { json, .. }, Some(AssistantContent::ToolCall(tool))) =
                (&block, message.content.get_mut(position))
            {
                tool.arguments = parsed_arguments(json);
            }
            message.content.get(position).cloned()
        });
        match content {
            Some(AssistantContent::Text(text)) => {
                self.announce(|partial| AssistantMessageEvent::TextEnd {
                    content_index: position,
                    content: text.text,
                    partial,
                });
            }
            Some(AssistantContent::Thinking(thinking)) => {
                self.announce(|partial| AssistantMessageEvent::ThinkingEnd {
                    content_index: position,
                    content: thinking.thinking,
                    partial,
                });
            }
            Some(AssistantContent::ToolCall(tool_call)) => {
                self.announce(|partial| AssistantMessageEvent::ToolcallEnd {
                    content_index: position,
                    tool_call,
                    partial,
                });
            }
            _ => {}
        }
    }

    /// Record reported counts and their cost, defaulting absent cache counts to zero.
    /// A reported total of zero is replaced by input plus output.
    fn metadata(&self, event: &ConverseStreamMetadataEvent) {
        let Some(usage) = event.usage() else {
            return;
        };
        self.update(|message| {
            let counts = &mut message.usage;
            counts.input = f64::from(usage.input_tokens());
            counts.output = f64::from(usage.output_tokens());
            counts.cache_read = f64::from(usage.cache_read_input_tokens().unwrap_or(0));
            counts.cache_write = f64::from(usage.cache_write_input_tokens().unwrap_or(0));
            counts.total_tokens = Presence::Present(match usage.total_tokens() {
                0 => counts.input + counts.output,
                total => f64::from(total),
            });
            calculate_cost(&self.model, counts);
        });
    }
}

/// The stop reason a service stop names; any reason that is not a normal end is an error.
fn stop_reason(reason: &Stop) -> StopReason {
    match reason {
        Stop::EndTurn | Stop::StopSequence => StopReason::Stop,
        Stop::MaxTokens | Stop::ModelContextWindowExceeded => StopReason::Length,
        Stop::ToolUse => StopReason::ToolUse,
        _ => StopReason::Error,
    }
}
