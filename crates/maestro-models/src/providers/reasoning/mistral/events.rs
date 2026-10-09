//! Cumulative content reduction with shared message identity.
use super::{
    chunk::{Arguments, Chunk, Tool},
    content::Content,
};
use crate::providers::{
    http::RequestFailure,
    json_text::{compact_raw, parsed_arguments},
};
use crate::{
    AssistantContent, AssistantMessageEvent as Event, AssistantMessageEventStream,
    SharedAssistantMessage, TextContent, ThinkingContent, ToolCall,
};
use num_traits::ToPrimitive;
use std::sync::{Arc, PoisonError};

/// Active content and tool associations for one shared output.
#[derive(Default)]
pub(super) struct Events {
    /// Active content index.
    current: Option<(usize, Kind)>,
    /// Tool associations in creation order.
    tools: indexmap::IndexMap<(String, i64), ToolState>,
}
impl Events {
    /// Apply a complete admitted event.
    pub fn apply(
        &mut self,
        chunk: Chunk,
        model: &crate::Model,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) -> Result<(), RequestFailure> {
        {
            let mut message = output.write().unwrap_or_else(PoisonError::into_inner);
            if let Some(usage) = chunk.usage {
                message.usage.input = usage.0.prompt_tokens.0.as_f64().unwrap_or_default() + 0.0;
                message.usage.output =
                    usage.0.completion_tokens.0.as_f64().unwrap_or_default() + 0.0;
                message.usage.total_tokens = usage
                    .0
                    .total_tokens
                    .0
                    .as_f64()
                    .filter(|total| *total != 0.0)
                    .unwrap_or(message.usage.input + message.usage.output);
                message.usage.cache_read = 0.0;
                message.usage.cache_write = 0.0;
                crate::calculate_cost(model, &mut message.usage);
            }
            if message.response_id.as_deref().is_none_or(str::is_empty) {
                message.response_id = Some(chunk.id);
            }
        }
        let Some(choice) = chunk.choices.into_iter().next() else {
            return Ok(());
        };
        let choice = choice.0;
        if let Some(reason) = choice.finish_reason.filter(|reason| !reason.is_empty()) {
            output
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .stop_reason = match reason.as_str() {
                "length" | "model_length" => crate::StopReason::Length,
                "tool_calls" => crate::StopReason::ToolUse,
                "error" => crate::StopReason::Error,
                _ => crate::StopReason::Stop,
            };
        }
        for content in choice.delta.content.0 {
            self.text(content, stream, output);
        }
        let tools = choice.delta.tool_calls.unwrap_or_default();
        if !tools.is_empty() {
            self.finish(stream, output);
        }
        for tool in tools {
            self.tool(tool.0, stream, output)?;
        }
        Ok(())
    }
    /// Append a selected block contribution and emit its ordered update.
    fn text(
        &mut self,
        content: Content,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) {
        let (text, thinking) = match content {
            Content::Text(text) => (text, Kind::Text),
            Content::Thinking(text) => (text, Kind::Thinking),
        };
        if self.current.is_some_and(|(_, active)| active != thinking) {
            self.finish(stream, output);
        }
        let index = if let Some((index, _)) = self.current {
            index
        } else {
            self.start_block(thinking, stream, output)
        };
        match output
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .content
            .get_mut(index)
        {
            Some(AssistantContent::Text(block)) => block.text.push_str(&text),
            Some(AssistantContent::Thinking(block)) => block.thinking.push_str(&text),
            _ => {}
        }
        let partial = Arc::clone(output);
        stream.push(if thinking == Kind::Thinking {
            Event::ThinkingDelta {
                content_index: index,
                delta: text,
                partial,
            }
        } else {
            Event::TextDelta {
                content_index: index,
                delta: text,
                partial,
            }
        });
    }
    /// Insert a new block before publishing its start event.
    fn start_block(
        &mut self,
        thinking: Kind,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) -> usize {
        let content = if thinking == Kind::Thinking {
            AssistantContent::Thinking(ThinkingContent {
                thinking: String::new(),
                thinking_signature: None,
                redacted: None,
            })
        } else {
            AssistantContent::Text(TextContent {
                text: String::new(),
                text_signature: None,
            })
        };
        let index = {
            let mut message = output.write().unwrap_or_else(PoisonError::into_inner);
            let index = message.content.len();
            message.content.push(content);
            index
        };
        self.current = Some((index, thinking));
        let partial = Arc::clone(output);
        stream.push(if thinking == Kind::Thinking {
            Event::ThinkingStart {
                content_index: index,
                partial,
            }
        } else {
            Event::TextStart {
                content_index: index,
                partial,
            }
        });
        index
    }
    /// End the current text or thinking block on a transition or successful exhaustion.
    pub fn finish(
        &mut self,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) {
        if let Some((index, thinking)) = self.current.take() {
            let content = match &output
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .content[index]
            {
                AssistantContent::Text(block) => block.text.clone(),
                AssistantContent::Thinking(block) => block.thinking.clone(),
                AssistantContent::ToolCall(_) => String::new(),
            };
            let partial = Arc::clone(output);
            stream.push(if thinking == Kind::Thinking {
                Event::ThinkingEnd {
                    content_index: index,
                    content,
                    partial,
                }
            } else {
                Event::TextEnd {
                    content_index: index,
                    content,
                    partial,
                }
            });
        }
    }
    /// Associate a tool by literal identity and index, then append arguments.
    fn tool(
        &mut self,
        tool: Tool,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) -> Result<(), RequestFailure> {
        let index = tool
            .index
            .0
            .as_f64()
            .and_then(|number| number.to_i64())
            .unwrap_or_default();
        let id = if tool.id.is_empty() || tool.id == "null" {
            super::request::tool_ids::derive(&format!("toolcall:{index}"), 0)
        } else {
            tool.id
        };
        let key = (id.clone(), index);
        let state = match self.tools.entry(key) {
            indexmap::map::Entry::Occupied(entry) => entry.into_mut(),
            indexmap::map::Entry::Vacant(entry) => {
                let content_index = {
                    let mut message = output.write().unwrap_or_else(PoisonError::into_inner);
                    let content_index = message.content.len();
                    message.content.push(AssistantContent::ToolCall(ToolCall {
                        id,
                        name: tool.function.0.name,
                        arguments: crate::JsonObject::new(),
                        thought_signature: None,
                    }));
                    content_index
                };
                stream.push(Event::ToolcallStart {
                    content_index,
                    partial: Arc::clone(output),
                });
                entry.insert(ToolState {
                    content_index,
                    arguments: String::new(),
                })
            }
        };
        let delta = match tool.function.0.arguments {
            Arguments::Text(text) => text,
            Arguments::Object(raw) => {
                compact_raw(&raw).map_err(|error| RequestFailure::new(error.to_string()))?
            }
        };
        state.arguments.push_str(&delta);
        let arguments = parsed_arguments(&state.arguments);
        if let AssistantContent::ToolCall(tool) = &mut output
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .content[state.content_index]
        {
            tool.arguments = arguments;
        }
        stream.push(Event::ToolcallDelta {
            content_index: state.content_index,
            delta,
            partial: Arc::clone(output),
        });
        Ok(())
    }
    /// Finalize tools in their first-creation order after the current block.
    pub fn complete(
        &mut self,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) {
        self.finish(stream, output);
        for state in self.tools.values() {
            let arguments = parsed_arguments(&state.arguments);
            let tool_call = match &mut output
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .content[state.content_index]
            {
                AssistantContent::ToolCall(tool) => {
                    tool.arguments = arguments;
                    tool.clone()
                }
                _ => continue,
            };
            stream.push(Event::ToolcallEnd {
                content_index: state.content_index,
                tool_call,
                partial: Arc::clone(output),
            });
        }
    }
}

/// Private accumulated arguments and output location of one tool.
struct ToolState {
    /// Stable output block index.
    content_index: usize,
    /// Unpublished argument fragments.
    arguments: String,
}

/// Text and thinking have distinct block event families.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Visible assistant text.
    Text,
    /// Assistant reasoning text.
    Thinking,
}
