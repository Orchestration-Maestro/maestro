//! Reduction of framed content-generation responses into the shared assistant message.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};

use futures_util::StreamExt;
use serde_json::value::RawValue;

use super::wire::{self, nullable};
use super::{is_thinking_part, map_stop_reason, retain_thought_signature};
use crate::providers::json_text::{compact_json, is_truthy, member};
use crate::providers::selected_fields::native;
use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantContent as Block, AssistantMessage, AssistantMessageEvent as Update,
    AssistantMessageEventStream, DiagnosticErrorInfo, Model, SharedAssistantMessage, StopReason,
    TextContent, ThinkingContent, ToolCall, calculate_cost,
};

/// The text or thinking block that later parts extend, by content position.
#[derive(Clone, Copy)]
enum Active {
    /// No block is open.
    Idle,
    /// A text block.
    Text(usize),
    /// A thinking block.
    Thinking(usize),
}

/// Reduce framed response items, given as JSON text, into `output`, publishing content events
/// on `events`.
///
/// Reduction continues until `chunks` ends; a clean end closes the open text or thinking
/// block. The caller owns the initial message, the lifecycle events, cancellation and the
/// stream's end; `counter` numbers the identifiers generated for calls without a usable
/// one. A declared finish reason that maps to `error` is recorded in `output`, not returned.
///
/// # Errors
/// Returns a failure the source yields unchanged, malformed JSON, a typed read of a selected
/// member that fails, call arguments nested beyond the shared JSON conversion's bound, or
/// `Unhandled stop reason: <reason>` for a finish reason the service does not declare. Content
/// reduced before the failure stays in `output`, and the open block is not closed. A call
/// closes the open block and takes its identifier before its arguments are converted.
pub async fn process_google_stream<S>(
    mut chunks: S,
    output: &SharedAssistantMessage,
    events: &AssistantMessageEventStream,
    model: &Model,
    counter: &AtomicU64,
) -> Result<(), DiagnosticErrorInfo>
where
    S: futures_core::Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin,
{
    let mut reducer = Reducer::new(output, events, model, counter);
    while let Some(chunk) = chunks.next().await {
        reducer.chunk(&chunk?)?;
    }
    reducer.close();
    Ok(())
}

/// Reduces response items into the shared message and publishes content events.
struct Reducer<'a> {
    /// Message shared by every update.
    output: &'a SharedAssistantMessage,
    /// Receiver of the updates.
    events: &'a AssistantMessageEventStream,
    /// Model that was asked, for cost rates.
    model: &'a Model,
    /// Source of the numbers in generated call identifiers.
    counter: &'a AtomicU64,
    /// The block later parts extend.
    active: Active,
    /// Identifiers of the calls in the message.
    call_ids: HashSet<String>,
}

impl<'a> Reducer<'a> {
    /// Start reducing into `output`, which may already hold calls.
    fn new(
        output: &'a SharedAssistantMessage,
        events: &'a AssistantMessageEventStream,
        model: &'a Model,
        counter: &'a AtomicU64,
    ) -> Self {
        let call_ids = output
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .content
            .iter()
            .filter_map(|block| match block {
                Block::ToolCall(call) => Some(call.id.clone()),
                _ => None,
            })
            .collect();
        Self {
            output,
            events,
            model,
            counter,
            active: Active::Idle,
            call_ids,
        }
    }

    /// Change the shared message without holding the lock while updates are announced.
    fn change<R>(&self, edit: impl FnOnce(&mut AssistantMessage) -> R) -> R {
        edit(&mut self.output.write().unwrap_or_else(PoisonError::into_inner))
    }

    /// Edit the content blocks, then announce the update the edit describes.
    fn publish(
        &self,
        edit: impl FnOnce(&mut Vec<Block>, SharedAssistantMessage) -> Option<Update>,
    ) {
        let partial = Arc::clone(self.output);
        if let Some(update) = self.change(|message| edit(&mut message.content, partial)) {
            self.events.push(update);
        }
    }

    /// Apply one response item.
    fn chunk(&mut self, input: &str) -> Result<(), DiagnosticErrorInfo> {
        let item = wire::item(input)?;
        if self.change(|message| message.response_id.as_deref().is_none_or(str::is_empty)) {
            let id = nullable::<String>(item, "responseId")?;
            self.change(|message| message.response_id = id);
        }
        let candidate = wire::candidate(item)?;
        if let Some(candidate) = candidate {
            for part in wire::parts(candidate)? {
                self.part(part)?;
            }
            if let Some(reason) = wire::finish_reason(candidate)? {
                self.finish(&reason)?;
            }
        }
        if let Some(report) = member(item, "usageMetadata").filter(|raw| is_truthy(raw)) {
            let mut usage = wire::usage(report)?;
            calculate_cost(self.model, &mut usage);
            self.change(|message| message.usage = usage);
        }
        Ok(())
    }

    /// Apply the text and then the function call of one part.
    fn part(&mut self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        let part = wire::part(raw)?;
        let signature = part.metadata.thought_signature.as_deref();
        if let Some(text) = &part.text {
            self.text(text, is_thinking_part(&part.metadata), signature);
        }
        if let Some(call) = part.call {
            let signature = signature.filter(|signature| !signature.is_empty());
            self.tool_call(call, signature)?;
        }
        Ok(())
    }

    /// Append text to the open block of the same kind, opening a new block when needed.
    fn text(&mut self, text: &str, thinking: bool, signature: Option<&str>) {
        let index = match (self.active, thinking) {
            (Active::Thinking(index), true) | (Active::Text(index), false) => index,
            _ => {
                self.close();
                self.open(thinking)
            }
        };
        self.publish(|content, partial| match content.get_mut(index)? {
            Block::Thinking(block) => {
                block.thinking.push_str(text);
                if let Some(signature) = retain_thought_signature(None, signature) {
                    block.thinking_signature = Some(signature.to_owned());
                }
                Some(Update::ThinkingDelta {
                    content_index: index,
                    delta: text.to_owned(),
                    partial,
                })
            }
            Block::Text(block) => {
                block.text.push_str(text);
                if let Some(signature) = retain_thought_signature(None, signature) {
                    block.text_signature = Some(signature.to_owned());
                }
                Some(Update::TextDelta {
                    content_index: index,
                    delta: text.to_owned(),
                    partial,
                })
            }
            Block::ToolCall(_) => None,
        });
    }

    /// Append an empty block, announce it and make it the open block.
    fn open(&mut self, thinking: bool) -> usize {
        let index = self.change(|message| message.content.len());
        self.active = if thinking {
            Active::Thinking(index)
        } else {
            Active::Text(index)
        };
        self.publish(|content, partial| {
            Some(if thinking {
                content.push(Block::Thinking(ThinkingContent {
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: None,
                }));
                Update::ThinkingStart {
                    content_index: index,
                    partial,
                }
            } else {
                content.push(Block::Text(TextContent::new(String::new())));
                Update::TextStart {
                    content_index: index,
                    partial,
                }
            })
        });
        index
    }

    /// Announce the end of the open block, if any.
    fn close(&mut self) {
        let active = std::mem::replace(&mut self.active, Active::Idle);
        let (Active::Text(index) | Active::Thinking(index)) = active else {
            return;
        };
        self.publish(|content, partial| match content.get(index)? {
            Block::Text(block) => Some(Update::TextEnd {
                content_index: index,
                content: block.text.clone(),
                partial,
            }),
            Block::Thinking(block) => Some(Update::ThinkingEnd {
                content_index: index,
                content: block.thinking.clone(),
                partial,
            }),
            Block::ToolCall(_) => None,
        });
    }

    /// Close the open block, then append and announce a complete call.
    fn tool_call(
        &mut self,
        raw: &RawValue,
        signature: Option<&str>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let call = wire::call(raw)?;
        self.close();
        let id = match call
            .id
            .filter(|id| !id.is_empty() && !self.call_ids.contains(id))
        {
            Some(id) => id,
            None => format!(
                "{}_{}_{}",
                call.name.as_deref().unwrap_or("undefined"),
                ryu_js::Buffer::new().format_finite(timestamp_now()),
                self.counter.fetch_add(1, Ordering::Relaxed) + 1
            ),
        };
        let arguments = wire::arguments(call.arguments)?;
        let delta = compact_json(&arguments).map_err(|error| native(&error))?;
        self.call_ids.insert(id.clone());
        let call = ToolCall {
            id,
            name: call.name.unwrap_or_default(),
            arguments,
            thought_signature: signature.map(str::to_owned),
        };
        let content_index = self.change(|message| {
            message.content.push(Block::ToolCall(call.clone()));
            message.content.len() - 1
        });
        let partial = || Arc::clone(self.output);
        self.events.push(Update::ToolcallStart {
            content_index,
            partial: partial(),
        });
        self.events.push(Update::ToolcallDelta {
            content_index,
            delta,
            partial: partial(),
        });
        self.events.push(Update::ToolcallEnd {
            content_index,
            tool_call: call,
            partial: partial(),
        });
        Ok(())
    }

    /// Record the outcome a declared finish reason stands for; a call in the message turns a
    /// normal stop into tool use but never hides a length or error stop.
    fn finish(&self, reason: &str) -> Result<(), DiagnosticErrorInfo> {
        let mut stop_reason = map_stop_reason(reason)?;
        if !self.call_ids.is_empty() && stop_reason == StopReason::Stop {
            stop_reason = StopReason::ToolUse;
        }
        self.change(|message| message.stop_reason = stop_reason);
        Ok(())
    }
}
