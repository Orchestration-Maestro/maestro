//! Reduction of response events into the shared assistant message.

use std::borrow::Cow;
use std::mem;
use std::sync::{Arc, PoisonError};

use serde_json::value::RawValue;

use super::wire::{self, Item, ItemKind, Kind, Part, PartKind, Response};
use super::{OpenAIResponsesStreamOptions, failure, native};
use crate::providers::json_text::{compact_raw, or_zero, parsed_arguments};
use crate::{
    AssistantContent as Block, AssistantMessage, AssistantMessageEvent as Update,
    AssistantMessageEventStream, DiagnosticErrorInfo, JsonObject, Model, SharedAssistantMessage,
    StopReason, TextContent, TextSignatureV1, ThinkingContent, ToolCall, Usage, calculate_cost,
};

/// The output item being reduced; the content block it fills is the message's last one.
enum Current {
    /// No item is open.
    Idle,
    /// A reasoning item, with whether its last summary part is an object; summary deltas
    /// apply only while it is.
    Reasoning(bool),
    /// A message, with the kind of its last content part once it has one; a text delta applies
    /// only to a part of its kind.
    Message(Option<PartKind>),
    /// A function call, with the argument text received so far, which the message never holds.
    Function(String),
}

/// Reduces response events into the shared message, announcing each content change.
pub(super) struct Reducer<'a> {
    /// Message shared by every update and the final result.
    output: &'a SharedAssistantMessage,
    /// Receiver of the updates.
    stream: &'a AssistantMessageEventStream,
    /// Model that was asked, for cost rates.
    model: &'a Model,
    /// Service tier settings of the caller.
    options: Option<&'a OpenAIResponsesStreamOptions<'a>>,
    /// The open item.
    current: Current,
    /// Whether a completed or incomplete response was seen.
    terminal: bool,
}

impl<'a> Reducer<'a> {
    /// Start reducing into `output`, announcing changes on `stream`.
    pub(super) fn new(
        output: &'a SharedAssistantMessage,
        stream: &'a AssistantMessageEventStream,
        model: &'a Model,
        options: Option<&'a OpenAIResponsesStreamOptions<'a>>,
    ) -> Self {
        Self {
            output,
            stream,
            model,
            options,
            current: Current::Idle,
            terminal: false,
        }
    }

    /// Accept the end of the events only after a completed or incomplete response.
    ///
    /// # Errors
    /// Fails when no such response arrived.
    pub(super) fn finish(&self) -> Result<(), DiagnosticErrorInfo> {
        if self.terminal {
            Ok(())
        } else {
            Err(failure("Response stream ended before a terminal event"))
        }
    }

    /// Change the shared message without holding the lock while updates are announced.
    fn change<R>(&self, edit: impl FnOnce(&mut AssistantMessage) -> R) -> R {
        edit(&mut self.output.write().unwrap_or_else(PoisonError::into_inner))
    }

    /// Edit the last content block, then announce the update the edit describes. The edit gets
    /// the block, its content position and the shared message.
    fn publish(
        &self,
        edit: impl FnOnce(&mut Block, usize, SharedAssistantMessage) -> Option<Update>,
    ) {
        let partial = Arc::clone(self.output);
        let update = self.change(|message| {
            let position = message.content.len().checked_sub(1)?;
            edit(message.content.get_mut(position)?, position, partial)
        });
        if let Some(update) = update {
            self.stream.push(update);
        }
    }

    /// Append a block to the message and announce that it started.
    fn open(&self, block: Block) {
        let partial = Arc::clone(self.output);
        let update = self.change(|message| {
            let content_index = message.content.len();
            let update = match block {
                Block::Thinking(_) => Update::ThinkingStart {
                    content_index,
                    partial,
                },
                Block::Text(_) => Update::TextStart {
                    content_index,
                    partial,
                },
                Block::ToolCall(_) => Update::ToolcallStart {
                    content_index,
                    partial,
                },
            };
            message.content.push(block);
            update
        });
        self.stream.push(update);
    }

    /// Add text to the last block, which is a thinking or a text block, and announce it.
    fn append(&self, text: &str) {
        self.publish(|block, content_index, partial| match block {
            Block::Thinking(block) => {
                block.thinking.push_str(text);
                Some(Update::ThinkingDelta {
                    content_index,
                    delta: text.to_owned(),
                    partial,
                })
            }
            Block::Text(block) => {
                block.text.push_str(text);
                Some(Update::TextDelta {
                    content_index,
                    delta: text.to_owned(),
                    partial,
                })
            }
            Block::ToolCall(_) => None,
        });
    }

    /// Store the arguments parsed so far on the last block, a tool call, and announce `delta`
    /// when there is one.
    fn set_arguments(&self, arguments: JsonObject, delta: Option<&str>) {
        self.publish(|block, content_index, partial| {
            let Block::ToolCall(call) = block else {
                return None;
            };
            call.arguments = arguments;
            delta.map(|delta| Update::ToolcallDelta {
                content_index,
                delta: delta.to_owned(),
                partial,
            })
        });
    }

    /// Replace the text of the last block, a thinking or a text block, with the final one,
    /// keep the signature, and announce the end. Final thinking text may be absent.
    fn end(&self, text: Option<String>, signature: String) {
        self.publish(|block, content_index, partial| match block {
            Block::Thinking(block) => {
                if let Some(text) = text {
                    block.thinking = text;
                }
                block.thinking_signature = Some(signature);
                let content = block.thinking.clone();
                Some(Update::ThinkingEnd {
                    content_index,
                    content,
                    partial,
                })
            }
            Block::Text(block) => {
                block.text = text.unwrap_or_default();
                block.text_signature = Some(signature);
                let content = block.text.clone();
                Some(Update::TextEnd {
                    content_index,
                    content,
                    partial,
                })
            }
            Block::ToolCall(_) => None,
        });
    }

    /// Reduce one event given as JSON text. Events of kinds not known, and events that are not
    /// objects, are ignored.
    ///
    /// # Errors
    /// Fails on malformed text, a failed or errored response, an unknown response status, or
    /// a signature nested beyond the conversion bound.
    pub(super) fn event(&mut self, text: &str) -> Result<(), DiagnosticErrorInfo> {
        let Some(event) = wire::event(text)? else {
            return Ok(());
        };
        let delta = event.delta.as_deref();
        match event.kind {
            Some(Kind::Created) => {
                let id = event.response.and_then(|response| response.id);
                self.change(|message| message.response_id = id);
            }
            Some(Kind::ItemAdded) => event
                .item
                .as_deref()
                .map_or(Ok(()), |raw| self.item_added(raw))?,
            Some(Kind::ItemDone) => event
                .item
                .as_deref()
                .map_or(Ok(()), |raw| self.item_done(raw))?,
            Some(Kind::SummaryPartAdded) => {
                if let Current::Reasoning(open) = &mut self.current {
                    *open = event.part.is_some();
                }
            }
            Some(Kind::SummaryTextDelta) => self.reasoning_text(delta, true),
            Some(Kind::SummaryPartDone) => self.reasoning_text(Some("\n\n"), true),
            Some(Kind::ReasoningTextDelta) => self.reasoning_text(delta, false),
            Some(Kind::ContentPartAdded) => self.content_part_added(event.part.as_ref()),
            Some(Kind::OutputTextDelta) => self.message_text(delta, PartKind::OutputText),
            Some(Kind::RefusalDelta) => self.message_text(delta, PartKind::Refusal),
            Some(Kind::ArgumentsDelta) => self.arguments_delta(delta),
            Some(Kind::ArgumentsDone) => self.arguments_done(event.arguments.as_deref()),
            Some(Kind::Completed | Kind::Incomplete) => self.completed(event.response.as_ref())?,
            Some(Kind::Error) => {
                let (code, message) = (&event.code, &event.message);
                return Err(failure(format!("Error Code {code}: {message}")));
            }
            Some(Kind::Failed) => return Err(failed_error(event.response.as_ref())),
            Some(Kind::Other) | None => {}
        }
        Ok(())
    }

    /// Open the block of a new item.
    fn item_added(&mut self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        let Some(item) = wire::item(raw)? else {
            return Ok(());
        };
        let (block, current) = match item.kind {
            Some(ItemKind::Reasoning) => {
                let thinking = Block::Thinking(ThinkingContent {
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: None,
                });
                let summary_open = item.summary.last().is_some_and(Option::is_some);
                (thinking, Current::Reasoning(summary_open))
            }
            Some(ItemKind::Message) => {
                let text = Block::Text(TextContent {
                    text: String::new(),
                    text_signature: None,
                });
                let kind = |part: &Option<Part>| part.as_ref().and_then(|part| part.kind);
                let last_part = item
                    .content
                    .last()
                    .map(|part| kind(part).unwrap_or(PartKind::Other));
                (text, Current::Message(last_part))
            }
            Some(ItemKind::FunctionCall) => {
                let call = Block::ToolCall(ToolCall {
                    id: call_id(&item),
                    name: item.name.unwrap_or_default(),
                    arguments: JsonObject::new(),
                    thought_signature: None,
                });
                (call, Current::Function(item.arguments.unwrap_or_default()))
            }
            Some(ItemKind::Other) | None => return Ok(()),
        };
        self.open(block);
        self.current = current;
        Ok(())
    }

    /// Add text to the open thinking block. Summary text applies only while the last summary
    /// part is an object.
    fn reasoning_text(&self, text: Option<&str>, in_summary: bool) {
        if let (&Current::Reasoning(summary_open), Some(text)) = (&self.current, text)
            && (summary_open || !in_summary)
        {
            self.append(text);
        }
    }

    /// Remember the kind of the last content part of the open message; only answer text and
    /// refusals count.
    fn content_part_added(&mut self, part: Option<&Part>) {
        if let (
            Current::Message(last_part),
            Some(kind @ (PartKind::OutputText | PartKind::Refusal)),
        ) = (&mut self.current, part.and_then(|part| part.kind))
        {
            *last_part = Some(kind);
        }
    }

    /// Add text to the open text block when the last content part is of the kind the delta
    /// belongs to.
    fn message_text(&self, delta: Option<&str>, kind: PartKind) {
        if let (&Current::Message(Some(last)), Some(delta)) = (&self.current, delta)
            && last == kind
        {
            self.append(delta);
        }
    }

    /// Add argument text to the open function call and parse what arrived so far.
    fn arguments_delta(&mut self, delta: Option<&str>) {
        if let (Current::Function(scratch), Some(delta)) = (&mut self.current, delta) {
            scratch.push_str(delta);
            let arguments = parsed_arguments(scratch);
            self.set_arguments(arguments, Some(delta));
        }
    }

    /// Replace the argument text of the open function call with the complete text, announcing
    /// the part that extends what arrived.
    fn arguments_done(&mut self, complete: Option<&str>) {
        if let (Current::Function(scratch), Some(complete)) = (&mut self.current, complete) {
            let previous = mem::replace(scratch, complete.to_owned());
            let suffix = complete
                .strip_prefix(previous.as_str())
                .filter(|suffix| !suffix.is_empty());
            self.set_arguments(parsed_arguments(complete), suffix);
        }
    }

    /// Finish the open item with the complete item the service reports. A reasoning item is
    /// kept whole as the signature.
    fn item_done(&mut self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        let Some(item) = wire::item(raw)? else {
            return Ok(());
        };
        match (item.kind, &self.current) {
            (Some(ItemKind::Reasoning), Current::Reasoning(_)) => {
                let signature = compact_raw(raw).map_err(|error| native(&error))?;
                let summary = joined(&item.summary, "\n\n", |part| part.text.as_deref());
                let content = joined(&item.content, "\n\n", |part| part.text.as_deref());
                self.end(
                    [summary, content].into_iter().find(|text| !text.is_empty()),
                    signature,
                );
            }
            (Some(ItemKind::Message), Current::Message(_)) => {
                let id = item.id.unwrap_or_default();
                let signature = TextSignatureV1 {
                    v: 1,
                    id,
                    phase: item.phase,
                };
                let signature =
                    serde_json::to_string(&signature).map_err(|error| native(&error))?;
                let text = joined(&item.content, "", |part| match part.kind {
                    Some(PartKind::OutputText) => part.text.as_deref(),
                    _ => part.refusal.as_deref(),
                });
                self.end(Some(text), signature);
            }
            (Some(ItemKind::FunctionCall), _) => {
                self.function_done(item);
                return Ok(());
            }
            _ => return Ok(()),
        }
        self.current = Current::Idle;
        Ok(())
    }

    /// Finish a function call. The streamed argument text wins over the final item's; a call
    /// the stream never opened is added to the message with its end.
    fn function_done(&mut self, item: Item) {
        let streamed = match mem::replace(&mut self.current, Current::Idle) {
            Current::Function(scratch) => Some(scratch),
            _ => None,
        };
        let text = streamed.as_deref().filter(|text| !text.is_empty());
        let arguments = parsed_arguments(text.or(item.arguments.as_deref()).unwrap_or_default());
        if streamed.is_some() {
            self.end_call(arguments);
            return;
        }
        let tool_call = ToolCall {
            id: call_id(&item),
            name: item.name.unwrap_or_default(),
            arguments,
            thought_signature: None,
        };
        let partial = Arc::clone(self.output);
        let update = self.change(|message| {
            message.content.push(Block::ToolCall(tool_call.clone()));
            Update::ToolcallEnd {
                content_index: message.content.len() - 1,
                tool_call,
                partial,
            }
        });
        self.stream.push(update);
    }

    /// Set the final arguments of the last block, a tool call, and announce its end.
    fn end_call(&self, arguments: JsonObject) {
        self.publish(|block, content_index, partial| {
            let Block::ToolCall(call) = block else {
                return None;
            };
            call.arguments = arguments;
            Some(Update::ToolcallEnd {
                content_index,
                tool_call: call.clone(),
                partial,
            })
        });
    }

    /// Take the final identity, usage and outcome of a completed or incomplete response.
    fn completed(&mut self, response: Option<&Response>) -> Result<(), DiagnosticErrorInfo> {
        let mut usage = self.change(|message| message.usage.clone());
        if let Some(reported) = response.and_then(|response| response.usage.as_ref()) {
            let cached = or_zero(
                reported
                    .input_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens),
            );
            usage.input = or_zero(reported.input_tokens) - cached;
            usage.output = or_zero(reported.output_tokens);
            usage.cache_read = cached;
            usage.cache_write = 0.0;
            usage.total_tokens = or_zero(reported.total_tokens);
        }
        calculate_cost(self.model, &mut usage);
        self.price(
            &mut usage,
            response.and_then(|response| response.service_tier.as_deref()),
        );
        let id = response
            .and_then(|response| response.id.as_deref())
            .filter(|id| !id.is_empty());
        self.change(|message| {
            if let Some(id) = id {
                message.response_id = Some(id.to_owned());
            }
            message.usage = usage;
        });
        let reason = stop_reason(response.and_then(|response| response.status.as_deref()))?;
        self.change(|message| {
            let calls_tool = message
                .content
                .iter()
                .any(|block| matches!(block, Block::ToolCall(_)));
            message.stop_reason = if calls_tool && reason == StopReason::Stop {
                StopReason::ToolUse
            } else {
                reason
            };
        });
        self.terminal = true;
        Ok(())
    }

    /// Let the caller adjust the cost for the tier the response was served at.
    fn price(&self, usage: &mut Usage, echoed: Option<&str>) {
        let Some(options) = self.options else {
            return;
        };
        let Some(apply) = options.apply_service_tier_pricing else {
            return;
        };
        let tier = match options.resolve_service_tier {
            Some(resolve) => resolve(echoed, options.service_tier).map(Cow::Owned),
            None => echoed.or(options.service_tier).map(Cow::Borrowed),
        };
        apply(usage, tier.as_deref());
    }
}

/// The identifier of a function call: its call identifier, then its item identifier.
fn call_id(item: &Item) -> String {
    let (call, id) = (item.call_id.as_deref(), item.id.as_deref());
    format!("{}|{}", call.unwrap_or_default(), id.unwrap_or_default())
}

/// Join the text of parts, a part without text contributing an empty one.
fn joined<'a>(
    parts: &'a [Option<Part>],
    separator: &str,
    text: impl Fn(&'a Part) -> Option<&'a str>,
) -> String {
    let texts: Vec<&str> = parts
        .iter()
        .map(|part| part.as_ref().and_then(&text).unwrap_or_default())
        .collect();
    texts.join(separator)
}

/// The outcome a response status stands for.
fn stop_reason(status: Option<&str>) -> Result<StopReason, DiagnosticErrorInfo> {
    match status.unwrap_or_default() {
        "" | "completed" | "in_progress" | "queued" => Ok(StopReason::Stop),
        "incomplete" => Ok(StopReason::Length),
        "failed" | "cancelled" => Ok(StopReason::Error),
        other => Err(failure(format!("Unhandled stop reason: {other}"))),
    }
}

/// The text of an optional member when it is nonempty.
fn nonempty(text: Option<&String>) -> Option<&str> {
    text.map(String::as_str).filter(|text| !text.is_empty())
}

/// The failure of a failed response: its error, else why it stopped, else a notice.
fn failed_error(response: Option<&Response>) -> DiagnosticErrorInfo {
    let reason = response
        .and_then(|response| response.incomplete_details.as_ref())
        .and_then(|details| nonempty(details.reason.as_ref()));
    let message = match (
        response.and_then(|response| response.error.as_ref()),
        reason,
    ) {
        (Some(error), _) => format!(
            "{}: {}",
            nonempty(error.code.as_ref()).unwrap_or("unknown"),
            nonempty(error.message.as_ref()).unwrap_or("no message")
        ),
        (None, Some(reason)) => format!("incomplete: {reason}"),
        (None, None) => "Unknown error (no error details in response)".to_owned(),
    };
    failure(message)
}
