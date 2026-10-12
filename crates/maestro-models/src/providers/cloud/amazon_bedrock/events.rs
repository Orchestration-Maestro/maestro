//! Reduction of decoded stream events into shared assistant updates.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStopEvent, ConversationRole,
    ConverseStreamMetadataEvent, ConverseStreamOutput, StopReason as Stop,
};

use crate::providers::http::RequestFailure;
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Model,
    SharedAssistantMessage, StopReason, TextContent, calculate_cost,
};

/// Failure of a message that opens with a turn other than the assistant's.
const USER_START_TEXT: &str =
    "Unexpected assistant message start but got user message start instead";

/// Reduces events into the shared message, announcing each change.
pub(super) struct Reducer {
    /// Model that was asked, for cost rates.
    model: Arc<Model>,
    /// Message shared by every update and the final result.
    output: SharedAssistantMessage,
    /// Receiver of the updates.
    stream: AssistantMessageEventStream,
    /// Content position of each text block that is still open, by the service's block number.
    open: HashMap<i32, usize>,
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

    /// Reduce one event; events this delivery does not reduce are skipped.
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
        if let Some(position) = self.open.get(&number) {
            return *position;
        }
        let position = self.update(|message| {
            message.content.push(AssistantContent::Text(TextContent {
                text: String::new(),
                text_signature: None,
            }));
            message.content.len() - 1
        });
        self.open.insert(number, position);
        self.announce(|partial| AssistantMessageEvent::TextStart {
            content_index: position,
            partial,
        });
        position
    }

    /// Append text to the open block of the event's number, opening one first when none is.
    fn delta(&mut self, event: &ContentBlockDeltaEvent) {
        let Some(ContentBlockDelta::Text(delta)) = event.delta() else {
            return;
        };
        let position = self.open_text(event.content_block_index());
        self.update(|message| {
            if let Some(AssistantContent::Text(text)) = message.content.get_mut(position) {
                text.text.push_str(delta);
            }
        });
        self.announce(|partial| AssistantMessageEvent::TextDelta {
            content_index: position,
            delta: delta.clone(),
            partial,
        });
    }

    /// Close the open block of the event's number; an unknown number does nothing.
    fn stop(&mut self, event: &ContentBlockStopEvent) {
        let Some(position) = self.open.remove(&event.content_block_index()) else {
            return;
        };
        let content = self.update(|message| match message.content.get(position) {
            Some(AssistantContent::Text(text)) => Some(text.text.clone()),
            _ => None,
        });
        if let Some(content) = content {
            self.announce(|partial| AssistantMessageEvent::TextEnd {
                content_index: position,
                content,
                partial,
            });
        }
    }

    /// Record the reported token counts, each absent count as zero, and their cost. A reported
    /// total of zero is replaced by input plus output.
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
            counts.total_tokens = match usage.total_tokens() {
                0 => counts.input + counts.output,
                total => f64::from(total),
            };
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
