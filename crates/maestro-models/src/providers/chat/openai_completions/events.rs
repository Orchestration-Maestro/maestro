//! Reduction of decoded response chunks into shared assistant updates.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use serde_json::Value;

use super::chunk::{Chunk, Delta, RawUsage, ToolCallDelta};
use super::usage::parse_usage;
use crate::providers::json_text::{compact_json, is_truthy};
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    JsonObject, Model, SharedAssistantMessage, StopReason, TextContent, ThinkingContent, ToolCall,
    parse_streaming_json,
};

/// Parse state of one content block that is not part of the message itself.
enum Block {
    /// A text block.
    Text,
    /// A reasoning block.
    Thinking,
    /// A tool call whose arguments are still arriving.
    Tool(ToolScratch),
}

/// Scratch for a streaming tool call.
struct ToolScratch {
    /// Argument text received so far.
    partial_args: String,
    /// Provider-assigned position of the call within the response.
    stream_index: Option<i64>,
}

/// Reduces response chunks into the shared message, announcing each change.
pub(super) struct Reducer {
    /// Model that was asked, for cost rates and the echoed-model check.
    model: Arc<Model>,
    /// Message shared by every update and the final result.
    output: SharedAssistantMessage,
    /// Receiver of the updates.
    stream: AssistantMessageEventStream,
    /// Parse state, one entry per content block.
    blocks: Vec<Block>,
    /// Content position of the one text block.
    text: Option<usize>,
    /// Content position of the one reasoning block.
    thinking: Option<usize>,
    /// Tool calls by provider-assigned position.
    tools_by_index: HashMap<i64, usize>,
    /// Tool calls by identifier.
    tools_by_id: HashMap<String, usize>,
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
            blocks: Vec::new(),
            text: None,
            thinking: None,
            tools_by_index: HashMap::new(),
            tools_by_id: HashMap::new(),
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

    /// Reduce one decoded chunk. A value that is not an object is ignored; an object updates
    /// the response identity and usage even without choices, and only its first choice is
    /// reduced further.
    pub(super) fn chunk(&mut self, value: Value) {
        if !value.is_object() {
            return;
        }
        let Ok(chunk) = serde_json::from_value::<Chunk>(value) else {
            return;
        };
        self.note_identity(&chunk);
        if let Some(usage) = &chunk.usage {
            self.set_usage(usage);
        }
        let Some(Some(choice)) = chunk.choices.first() else {
            return;
        };
        if chunk.usage.is_none()
            && let Some(usage) = &choice.usage
        {
            self.set_usage(usage);
        }
        if let Some(reason) = choice.finish_reason.as_deref().filter(|r| !r.is_empty()) {
            self.finish_reason(reason);
        }
        if let Some(delta) = &choice.delta {
            self.delta(delta);
        }
    }

    /// Keep the first non-empty response identifier and the first non-empty model name that
    /// differs from the request.
    fn note_identity(&self, chunk: &Chunk) {
        let id = chunk.id.as_deref().filter(|id| !id.is_empty());
        let model = chunk
            .model
            .as_deref()
            .filter(|name| !name.is_empty() && *name != self.model.id);
        self.update(|message| {
            if message.response_id.is_none() {
                message.response_id = id.map(str::to_owned);
            }
            if message.response_model.is_none() {
                message.response_model = model.map(str::to_owned);
            }
        });
    }

    /// Replace the usage with the chunk's report, priced at the model's rates.
    fn set_usage(&self, raw: &RawUsage) {
        let usage = parse_usage(raw, &self.model);
        self.update(|message| message.usage = usage);
    }

    /// Record the outcome a finish reason names.
    fn finish_reason(&self, reason: &str) {
        let (stop_reason, error_message) = match reason {
            "stop" | "end" => (StopReason::Stop, None),
            "length" => (StopReason::Length, None),
            "function_call" | "tool_calls" => (StopReason::ToolUse, None),
            other => (
                StopReason::Error,
                Some(format!("Provider finish_reason: {other}")),
            ),
        };
        self.update(|message| {
            message.stop_reason = stop_reason;
            message.error_message = error_message;
        });
    }

    /// Reduce one delta: text, reasoning, tool calls and encrypted reasoning details.
    fn delta(&mut self, delta: &Delta) {
        if let Some(text) = delta.text() {
            self.append_text(text);
        }
        if let Some((field, text)) = delta.reasoning_text() {
            self.append_thinking(field, text);
        }
        for call in delta.tool_calls.iter().flatten() {
            self.tool_call(call);
        }
        for detail in delta.reasoning_details.iter().flatten() {
            self.attach_signature(detail);
        }
    }

    /// Add a block to the message and return its content position.
    fn push_block(&mut self, content: AssistantContent, block: Block) -> usize {
        self.blocks.push(block);
        self.update(|message| {
            message.content.push(content);
            message.content.len() - 1
        })
    }

    /// Open the one text block.
    fn open_text(&mut self) -> usize {
        let content = AssistantContent::Text(TextContent {
            text: String::new(),
            text_signature: None,
        });
        let position = self.push_block(content, Block::Text);
        self.text = Some(position);
        self.announce(|partial| AssistantMessageEvent::TextStart {
            content_index: position,
            partial,
        });
        position
    }

    /// Open the one reasoning block, named after the field that carries it.
    fn open_thinking(&mut self, field: &str) -> usize {
        let content = AssistantContent::Thinking(ThinkingContent {
            thinking: String::new(),
            thinking_signature: Some(field.to_owned()),
            redacted: None,
        });
        let position = self.push_block(content, Block::Thinking);
        self.thinking = Some(position);
        self.announce(|partial| AssistantMessageEvent::ThinkingStart {
            content_index: position,
            partial,
        });
        position
    }

    /// Open a tool call for a delta that matches none.
    fn open_tool(&mut self, index: Option<i64>, id: Option<&str>, name: Option<&str>) -> usize {
        let content = AssistantContent::ToolCall(ToolCall {
            id: id.unwrap_or_default().to_owned(),
            name: name.unwrap_or_default().to_owned(),
            arguments: JsonObject::new(),
            thought_signature: None,
        });
        let scratch = ToolScratch {
            partial_args: String::new(),
            stream_index: index,
        };
        let position = self.push_block(content, Block::Tool(scratch));
        self.announce(|partial| AssistantMessageEvent::ToolcallStart {
            content_index: position,
            partial,
        });
        position
    }

    /// Append to the text block, opening it first when it is not open.
    fn append_text(&mut self, delta: &str) {
        let position = self.text.unwrap_or_else(|| self.open_text());
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

    /// Append to the reasoning block, opening it first when it is not open and naming the field
    /// that carried the opening delta.
    fn append_thinking(&mut self, field: &str, delta: &str) {
        let position = self.thinking.unwrap_or_else(|| self.open_thinking(field));
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

    /// Find the call a delta belongs to by position, then by identifier, or open a new one.
    fn tool_block(&mut self, index: Option<i64>, id: Option<&str>, name: Option<&str>) -> usize {
        let known = index
            .and_then(|index| self.tools_by_index.get(&index))
            .or_else(|| id.and_then(|id| self.tools_by_id.get(id)))
            .copied();
        let position = known.unwrap_or_else(|| self.open_tool(index, id, name));
        if let Some(index) = index
            && let Some(Block::Tool(scratch)) = self.blocks.get_mut(position)
            && (known.is_none() || scratch.stream_index.is_none())
        {
            scratch.stream_index = Some(index);
            self.tools_by_index.insert(index, position);
        }
        if let Some(id) = id {
            self.tools_by_id.insert(id.to_owned(), position);
        }
        position
    }

    /// Reduce one tool-call delta.
    fn tool_call(&mut self, call: &ToolCallDelta) {
        let (index, id, name, arguments) = (call.index, call.id(), call.name(), call.arguments());
        let position = self.tool_block(index, id, name);
        let parsed = match self.blocks.get_mut(position) {
            Some(Block::Tool(scratch)) => {
                scratch.partial_args.push_str(arguments.unwrap_or_default());
                arguments.map(|_| parsed_arguments(&scratch.partial_args))
            }
            _ => return,
        };
        self.update(|message| {
            if let Some(AssistantContent::ToolCall(tool)) = message.content.get_mut(position) {
                complete_tool(tool, id, name, parsed);
            }
        });
        self.announce(|partial| AssistantMessageEvent::ToolcallDelta {
            content_index: position,
            delta: arguments.unwrap_or_default().to_owned(),
            partial,
        });
    }

    /// Attach an encrypted reasoning detail to the tool call it names.
    fn attach_signature(&self, detail: &Value) {
        let encrypted = detail.get("type").and_then(Value::as_str) == Some("reasoning.encrypted");
        let id = detail
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty());
        let data = detail.get("data").is_some_and(is_truthy);
        let (true, Some(id), true) = (encrypted, id, data) else {
            return;
        };
        let Ok(signature) = compact_json(detail) else {
            return;
        };
        self.update(|message| {
            let call = message.content.iter_mut().find_map(|block| match block {
                AssistantContent::ToolCall(call) if call.id == id => Some(call),
                _ => None,
            });
            if let Some(call) = call {
                call.thought_signature = Some(signature);
            }
        });
    }

    /// Close every block in content order, finalizing tool arguments.
    pub(super) fn finish(&mut self) {
        for position in 0..self.blocks.len() {
            let ended = self.update(|message| {
                close(
                    message.content.get_mut(position)?,
                    self.blocks.get(position)?,
                )
            });
            if let Some(ended) = ended {
                self.announce(|partial| ended.into_event(position, partial));
            }
        }
    }
}

/// Finish one block, returning its final content.
fn close(content: &mut AssistantContent, block: &Block) -> Option<Ended> {
    match (content, block) {
        (AssistantContent::Text(text), Block::Text) => Some(Ended::Text(text.text.clone())),
        (AssistantContent::Thinking(thinking), Block::Thinking) => {
            Some(Ended::Thinking(thinking.thinking.clone()))
        }
        (AssistantContent::ToolCall(call), Block::Tool(scratch)) => {
            call.arguments = parsed_arguments(&scratch.partial_args);
            Some(Ended::Tool(call.clone()))
        }
        _ => None,
    }
}

/// A block's final content, copied out so the lock is released before announcing it.
enum Ended {
    /// Final text.
    Text(String),
    /// Final reasoning.
    Thinking(String),
    /// Final tool call.
    Tool(ToolCall),
}

impl Ended {
    /// The update that announces the block's end.
    fn into_event(
        self,
        content_index: usize,
        partial: SharedAssistantMessage,
    ) -> AssistantMessageEvent {
        match self {
            Self::Text(content) => AssistantMessageEvent::TextEnd {
                content_index,
                content,
                partial,
            },
            Self::Thinking(content) => AssistantMessageEvent::ThinkingEnd {
                content_index,
                content,
                partial,
            },
            Self::Tool(tool_call) => AssistantMessageEvent::ToolcallEnd {
                content_index,
                tool_call,
                partial,
            },
        }
    }
}

/// Fill in an identifier and name that arrived late and replace the arguments when a delta
/// carried some.
fn complete_tool(
    tool: &mut ToolCall,
    id: Option<&str>,
    name: Option<&str>,
    arguments: Option<JsonObject>,
) {
    if let Some(id) = id.filter(|_| tool.id.is_empty()) {
        tool.id = id.to_owned();
    }
    if let Some(name) = name.filter(|_| tool.name.is_empty()) {
        tool.name = name.to_owned();
    }
    if let Some(arguments) = arguments {
        tool.arguments = arguments;
    }
}

/// Parse streamed argument text into an object; anything else becomes empty.
pub(super) fn parsed_arguments(partial: &str) -> JsonObject {
    match parse_streaming_json(Some(partial)) {
        Value::Object(arguments) => arguments,
        _ => JsonObject::new(),
    }
}
