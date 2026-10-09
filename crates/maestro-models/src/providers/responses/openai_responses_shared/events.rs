//! Reduction of response events into the shared assistant message.

use std::borrow::Cow;
use std::sync::{Arc, PoisonError};

use serde_json::value::RawValue;

use super::wire::{
    appended, arguments, field, joined, kind, last, required, signature, spelled, string,
};
use super::{OpenAIResponsesStreamOptions, failure, native};
use crate::providers::json_text::{
    compact_raw, is_truthy, member, or_zero, parsed_arguments, raw_json, raw_number,
};
use crate::{
    AssistantContent as Block, AssistantMessage, AssistantMessageEvent as Update,
    AssistantMessageEventStream, DiagnosticErrorInfo, JsonObject, Model, SharedAssistantMessage,
    StopReason, TextContent, ThinkingContent, ToolCall, Usage, calculate_cost,
};

/// The open item and the block position captured when it started.
enum Current {
    /// No item is open.
    Idle,
    /// Thinking block and the raw summary used by later summary events.
    Reasoning(usize, Option<Box<RawValue>>),
    /// Text block and the raw content used by later part events.
    Message(usize, Option<Box<RawValue>>),
    /// Call block and accumulated argument text.
    Function(usize, String),
}
impl Current {
    /// Position of the open block, independent of later appends.
    fn index(&self) -> Option<usize> {
        match self {
            Self::Idle => None,
            Self::Reasoning(index, _) | Self::Message(index, _) | Self::Function(index, _) => {
                Some(*index)
            }
        }
    }
}

/// Reduces response events into the shared message and publishes content events.
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
    /// Start reducing into `output`, publishing content events on `stream`.
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

    /// Edit the captured content block, then announce the update the edit describes. The edit gets
    /// the block, its content position and the shared message.
    fn publish(
        &self,
        edit: impl FnOnce(&mut Block, usize, SharedAssistantMessage) -> Option<Update>,
    ) {
        let partial = Arc::clone(self.output);
        let update = self.change(|message| {
            let position = self.current.index()?;
            edit(message.content.get_mut(position)?, position, partial)
        });
        if let Some(update) = update {
            self.stream.push(update);
        }
    }

    /// Append a block to the message and announce that it started.
    fn open(&self, block: Block) -> usize {
        let partial = Arc::clone(self.output);
        let (index, update) = self.change(|message| {
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
            (content_index, update)
        });
        self.stream.push(update);
        index
    }

    /// Append text to the captured thinking or text block and announce it.
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

    /// Store the arguments parsed so far on the captured block, a tool call, and announce `delta`
    /// when there is one.
    fn set_arguments(&self, arguments: JsonObject, delta: Option<String>) {
        self.publish(|block, content_index, partial| {
            let Block::ToolCall(call) = block else {
                return None;
            };
            call.arguments = arguments;
            delta.map(|delta| Update::ToolcallDelta {
                content_index,
                delta,
                partial,
            })
        });
    }

    /// Publish final text before signature conversion; absent reasoning text keeps the draft.
    fn final_text(&self, text: Option<String>) {
        self.publish(|block, _, _| {
            match (block, text) {
                (Block::Thinking(block), Some(text)) => block.thinking = text,
                (Block::Text(block), Some(text)) => block.text = text,
                _ => {}
            }
            None
        });
    }

    /// Store a converted signature and announce the finalized content.
    fn end(&self, signature: String) {
        self.publish(|block, content_index, partial| match block {
            Block::Thinking(block) => {
                block.thinking_signature = Some(signature);
                let content = block.thinking.clone();
                Some(Update::ThinkingEnd {
                    content_index,
                    content,
                    partial,
                })
            }
            Block::Text(block) => {
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

    /// Select one event before reading the members its branch consumes.
    pub(super) fn event(&mut self, input: &str) -> Result<(), DiagnosticErrorInfo> {
        let raw = required(Some(raw_json(input).map_err(|error| native(&error))?))?;
        match kind(raw, "type")?.as_str() {
            "response.created" => {
                let id = field(required(member(raw, "response"))?, "id")?;
                self.change(|message| message.response_id = id);
            }
            "response.output_item.added" => {
                self.item_added(required(member(raw, "item"))?)?;
            }
            "response.output_item.done" => {
                self.item_done(required(member(raw, "item"))?)?;
            }
            "response.completed" | "response.incomplete" => {
                self.completed(member(raw, "response"))?;
            }
            "error" => {
                return Err(failure(format!(
                    "Error Code {}: {}",
                    spelled(member(raw, "code"))?,
                    spelled(member(raw, "message"))?
                )));
            }
            "response.failed" => return Err(failed_error(member(raw, "response"))?),
            kind => self.delta(raw, kind)?,
        }
        Ok(())
    }

    /// Open a canonical block, retaining only later selection state.
    fn item_added(&mut self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        self.current = match kind(raw, "type")?.as_str() {
            "reasoning" => {
                let summary = member(raw, "summary").map(RawValue::to_owned);
                Current::Reasoning(
                    self.open(Block::Thinking(ThinkingContent {
                        thinking: String::new(),
                        thinking_signature: None,
                        redacted: None,
                    })),
                    summary,
                )
            }
            "message" => {
                let content = member(raw, "content").map(RawValue::to_owned);
                Current::Message(
                    self.open(Block::Text(TextContent {
                        text: String::new(),
                        text_signature: None,
                    })),
                    content,
                )
            }
            "function_call" => {
                let call = call(raw, JsonObject::new())?;
                let scratch = arguments(raw)?;
                Current::Function(self.open(Block::ToolCall(call)), scratch)
            }
            _ => return Ok(()),
        };
        Ok(())
    }

    /// Apply guarded part and delta events without reading rejected inputs.
    fn delta(&mut self, raw: &RawValue, event_kind: &str) -> Result<(), DiagnosticErrorInfo> {
        match (&mut self.current, event_kind) {
            (Current::Reasoning(_, summary), "response.reasoning_summary_part.added") => {
                last(summary.as_deref())?;
                *summary = appended(member(raw, "part"))?;
            }
            (Current::Reasoning(_, summary), "response.reasoning_summary_part.done") => {
                if last(summary.as_deref())?.is_some_and(is_truthy) {
                    self.append("\n\n");
                }
            }
            (Current::Reasoning(_, summary), "response.reasoning_summary_text.delta") => {
                if last(summary.as_deref())?.is_some_and(is_truthy) {
                    self.append(&spelled(member(raw, "delta"))?);
                }
            }
            (Current::Reasoning(_, _), "response.reasoning_text.delta") => {
                self.append(&spelled(member(raw, "delta"))?);
            }
            (Current::Message(_, content), "response.content_part.added") => {
                let part = required(member(raw, "part"))?;
                if matches!(kind(part, "type")?.as_str(), "output_text" | "refusal") {
                    last(content.as_deref())?;
                    *content = appended(Some(part))?;
                }
            }
            (
                Current::Message(_, content),
                "response.output_text.delta" | "response.refusal.delta",
            ) => {
                let expected = if event_kind == "response.output_text.delta" {
                    "output_text"
                } else {
                    "refusal"
                };
                if let Some(part) = last(content.as_deref())?
                    && kind(part, "type")? == expected
                {
                    self.append(&spelled(member(raw, "delta"))?);
                }
            }
            (
                Current::Function(_, scratch),
                "response.function_call_arguments.delta" | "response.function_call_arguments.done",
            ) => {
                let (arguments, delta) = argument_update(scratch, raw, event_kind)?;
                self.set_arguments(arguments, delta);
            }
            _ => {}
        }
        Ok(())
    }

    /// Replace final content on the captured block and clear the open state.
    fn item_done(&mut self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        match (kind(raw, "type")?.as_str(), &self.current) {
            ("reasoning", Current::Reasoning(_, _)) => {
                let summary = joined(raw, "summary", false)?;
                let content = joined(raw, "content", false)?;
                self.final_text([summary, content].into_iter().find(|text| !text.is_empty()));
                let signature = compact_raw(raw).map_err(|error| native(&error))?;
                self.end(signature);
            }
            ("message", Current::Message(_, _)) => {
                let content = joined(raw, "content", true)?;
                self.final_text(Some(content));
                self.end(signature(raw)?);
            }
            ("function_call", _) => self.function_done(raw)?,
            _ => return Ok(()),
        }
        self.current = Current::Idle;
        Ok(())
    }

    /// Finalize existing calls without reading unused identity, or insert a final-only call.
    fn function_done(&self, raw: &RawValue) -> Result<(), DiagnosticErrorInfo> {
        let arguments = match &self.current {
            Current::Function(_, scratch) if !scratch.is_empty() => parsed_arguments(scratch),
            _ => parsed_arguments(&arguments(raw)?),
        };
        if matches!(self.current, Current::Function(_, _)) {
            self.end_call(arguments);
        } else {
            let tool_call = call(raw, arguments)?;
            let block = Block::ToolCall(tool_call.clone());
            let partial = Arc::clone(self.output);
            let index = self.change(|message| {
                let index = message.content.len();
                message.content.push(block);
                index
            });
            self.stream.push(Update::ToolcallEnd {
                content_index: index,
                tool_call,
                partial,
            });
        }
        Ok(())
    }

    /// Set the final arguments of the captured block, a tool call, and announce its end.
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

    /// Publish completion effects before callbacks and status selection.
    fn completed(&mut self, response: Option<&RawValue>) -> Result<(), DiagnosticErrorInfo> {
        if let Some(response) = response {
            let id = member(response, "id")
                .filter(|raw| is_truthy(raw))
                .map(|raw| string(Some(raw)))
                .transpose()?;
            if let Some(id) = id {
                self.change(|message| message.response_id = Some(id));
            }
        }
        let mut usage = self.change(|message| message.usage.clone());
        if let Some(reported) = response
            .and_then(|response| member(response, "usage"))
            .filter(|raw| is_truthy(raw))
        {
            let count = |raw, key| or_zero(member(raw, key).and_then(raw_number));
            let cached = member(reported, "input_tokens_details")
                .map_or(0.0, |details| count(details, "cached_tokens"));
            usage.input = count(reported, "input_tokens") - cached;
            usage.output = count(reported, "output_tokens");
            usage.cache_read = cached;
            usage.cache_write = 0.0;
            usage.total_tokens = count(reported, "total_tokens");
        }
        calculate_cost(self.model, &mut usage);
        self.change(|message| message.usage = usage.clone());
        if self
            .options
            .is_some_and(|options| options.apply_service_tier_pricing.is_some())
        {
            let echoed = response
                .map(|raw| field::<Option<String>>(raw, "service_tier"))
                .transpose()?
                .flatten();
            self.price(&mut usage, echoed.flatten().as_deref());
            self.change(|message| message.usage = usage);
        }
        let status = response.and_then(|raw| member(raw, "status"));
        let reason = stop_reason(status)?;
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

/// Advance argument scratch and select only a nonempty completion suffix for publication.
fn argument_update(
    scratch: &mut String,
    raw: &RawValue,
    event_kind: &str,
) -> Result<(JsonObject, Option<String>), DiagnosticErrorInfo> {
    let delta = if event_kind == "response.function_call_arguments.delta" {
        let delta = spelled(member(raw, "delta"))?;
        scratch.push_str(&delta);
        Some(delta)
    } else {
        let complete = string(member(raw, "arguments"))?;
        let suffix = complete
            .strip_prefix(scratch.as_str())
            .filter(|suffix| !suffix.is_empty())
            .map(str::to_owned);
        *scratch = complete;
        suffix
    };
    Ok((parsed_arguments(scratch), delta))
}

/// Build a final call from the selected identity and already-parsed arguments.
fn call(raw: &RawValue, arguments: JsonObject) -> Result<ToolCall, DiagnosticErrorInfo> {
    Ok(ToolCall {
        id: format!(
            "{}|{}",
            spelled(member(raw, "call_id"))?,
            spelled(member(raw, "id"))?
        ),
        name: string(member(raw, "name"))?,
        arguments,
        thought_signature: None,
    })
}

/// The outcome a response status stands for.
fn stop_reason(status: Option<&RawValue>) -> Result<StopReason, DiagnosticErrorInfo> {
    let Some(status) = status.filter(|raw| is_truthy(raw)) else {
        return Ok(StopReason::Stop);
    };
    let recognized = if status.get().starts_with('"') {
        spelled(Some(status))?
    } else {
        String::new()
    };
    match recognized.as_str() {
        "completed" | "in_progress" | "queued" => Ok(StopReason::Stop),
        "incomplete" => Ok(StopReason::Length),
        "failed" | "cancelled" => Ok(StopReason::Error),
        _ => Err(failure(format!(
            "Unhandled stop reason: {}",
            spelled(Some(status))?
        ))),
    }
}

/// Render the selected failed-response detail, preserving truthy raw fallbacks.
fn failed_error(response: Option<&RawValue>) -> Result<DiagnosticErrorInfo, DiagnosticErrorInfo> {
    let error = response
        .and_then(|raw| member(raw, "error"))
        .filter(|raw| is_truthy(raw));
    let message = if let Some(error) = error {
        let code = member(error, "code").filter(|raw| is_truthy(raw));
        let message = member(error, "message").filter(|raw| is_truthy(raw));
        format!(
            "{}: {}",
            code.map_or_else(|| Ok("unknown".to_owned()), |raw| spelled(Some(raw)))?,
            message.map_or_else(|| Ok("no message".to_owned()), |raw| spelled(Some(raw)))?
        )
    } else if let Some(reason) = response
        .and_then(|raw| member(raw, "incomplete_details"))
        .and_then(|raw| member(raw, "reason"))
        .filter(|raw| is_truthy(raw))
    {
        format!("incomplete: {}", spelled(Some(reason))?)
    } else {
        "Unknown error (no error details in response)".to_owned()
    };
    Ok(failure(message))
}
