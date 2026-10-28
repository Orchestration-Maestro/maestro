//! Ordered provider events reduced into one shared assistant message.
use super::request;
use maestro_extensions_wasm::{
    AssistantContent, AssistantMessage, AssistantMessageEvent as Event,
    AssistantMessageEventStream, Context, DoneReason, ErrorReason, ExtensionResult, Model,
    Presence, SharedAssistantMessage, StopReason, StreamOptionsInput, TextContent, ThinkingContent,
    ToolCall, Usage, calculate_cost,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// The native content position and unfinished JSON for one open wire block.
enum OpenBlock {
    /// An ordinary text block.
    Text(usize),
    /// A signed thinking block.
    Thinking(usize),
    /// A tool call whose argument fragments are retained until its end.
    Tool {
        /// Position in the assistant's content, not the wire index.
        index: usize,
        /// Concatenated argument fragments.
        json: String,
    },
}
impl OpenBlock {
    /// The shared message's content position.
    fn content_index(&self) -> usize {
        match self {
            Self::Text(index) | Self::Thinking(index) | Self::Tool { index, .. } => *index,
        }
    }
}

/// Mutations release message guards before publishing to the host's event buffer.
pub struct ResponseState {
    /// The single partial and final message owner.
    output: SharedAssistantMessage,
    /// The host's original buffer, never a second local queue.
    stream: AssistantMessageEventStream,
    /// Live wire-index mapping, retired at each block end.
    blocks: HashMap<u64, OpenBlock>,
    /// Authored tool spellings for subscription responses.
    tools: Vec<String>,
    /// Whether response tool names need their authored spelling restored.
    subscribed: bool,
    /// Supplied descriptor used for the shared cost estimate.
    model: Model,
    /// Whether the provider completed the message before EOF.
    pub stopped: bool,
}

impl ResponseState {
    /// Creates an empty response beside the supplied host buffer.
    pub fn new(
        model: &Model,
        context: &Context,
        options: Option<&StreamOptionsInput>,
        stream: AssistantMessageEventStream,
        now: f64,
    ) -> Self {
        Self {
            output: Arc::new(RwLock::new(AssistantMessage {
                content: Vec::new(),
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                response_model: None,
                response_id: None,
                diagnostics: None,
                usage: Usage::default(),
                stop_reason: StopReason::Stop,
                error_message: None,
                timestamp: now,
            })),
            stream,
            blocks: HashMap::new(),
            tools: context
                .tools
                .iter()
                .flatten()
                .map(|tool| tool.name.clone())
                .collect(),
            subscribed: request::subscription(
                options
                    .and_then(|options| options.common().api_key.as_deref())
                    .unwrap_or_default(),
            ),
            model: model.clone(),
            stopped: false,
        }
    }

    /// Publishes the original message before consuming the provider body.
    pub fn begin(&self) -> ExtensionResult<()> {
        self.stream.push(Event::Start {
            partial: Arc::clone(&self.output),
        })
    }

    /// Applies one provider event, ignoring unrelated event kinds.
    pub fn apply(&mut self, event: &Value) -> ExtensionResult<()> {
        match event["type"].as_str().unwrap_or_default() {
            "message_start" => self.usage(&event["message"]["usage"]),
            "content_block_start" => self.start(event),
            "content_block_delta" => self.delta(event),
            "content_block_stop" => self.end(event),
            "message_delta" => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    self.output
                        .write()
                        .map_err(|_| "Assistant message lock poisoned")?
                        .stop_reason = match reason {
                        "end_turn" | "pause_turn" | "stop_sequence" => StopReason::Stop,
                        "max_tokens" => StopReason::Length,
                        "tool_use" => StopReason::ToolUse,
                        _ => return Err(format!("Unknown stop reason: {reason}")),
                    };
                }
                self.usage(&event["usage"])
            }
            "message_stop" => {
                self.stopped = true;
                Ok(())
            }
            "error" => Err(event["error"]["message"]
                .as_str()
                .unwrap_or("Provider returned error")
                .to_owned()),
            _ => Ok(()),
        }
    }

    /// Updates only usage fields the event actually supplies.
    fn usage(&self, incoming: &Value) -> ExtensionResult<()> {
        let mut output = self
            .output
            .write()
            .map_err(|_| "Assistant message lock poisoned")?;
        let usage = &mut output.usage;
        for (name, count) in [
            ("input_tokens", &mut usage.input),
            ("output_tokens", &mut usage.output),
            ("cache_read_input_tokens", &mut usage.cache_read),
            ("cache_creation_input_tokens", &mut usage.cache_write),
        ] {
            if let Some(value) = incoming[name].as_f64() {
                *count = value;
            }
        }
        usage.total_tokens =
            Presence::Present(usage.input + usage.output + usage.cache_read + usage.cache_write);
        calculate_cost(&self.model, usage);
        Ok(())
    }

    /// Opens a recognized block using an independent native content position.
    fn start(&mut self, event: &Value) -> ExtensionResult<()> {
        let wire = wire_index(event)?;
        let block = &event["content_block"];
        let mut output = self
            .output
            .write()
            .map_err(|_| "Assistant message lock poisoned")?;
        let index = output.content.len();
        let partial = Arc::clone(&self.output);
        let start = match block["type"].as_str().unwrap_or_default() {
            "text" => {
                output
                    .content
                    .push(AssistantContent::Text(TextContent::new(String::new())));
                self.blocks.insert(wire, OpenBlock::Text(index));
                Event::TextStart {
                    content_index: index,
                    partial,
                }
            }
            "thinking" => {
                output
                    .content
                    .push(AssistantContent::Thinking(ThinkingContent {
                        thinking: String::new(),
                        thinking_signature: Some(String::new()),
                        redacted: None,
                    }));
                self.blocks.insert(wire, OpenBlock::Thinking(index));
                Event::ThinkingStart {
                    content_index: index,
                    partial,
                }
            }
            "tool_use" => {
                output
                    .content
                    .push(AssistantContent::ToolCall(self.tool(block)?));
                self.blocks.insert(
                    wire,
                    OpenBlock::Tool {
                        index,
                        json: String::new(),
                    },
                );
                Event::ToolcallStart {
                    content_index: index,
                    partial,
                }
            }
            _ => return Ok(()),
        };
        drop(output);
        self.stream.push(start)
    }

    /// Restores the authored spelling of a subscription tool call.
    fn tool(&self, block: &Value) -> ExtensionResult<ToolCall> {
        let mut name = field(block, "name")?;
        if self.subscribed {
            let lower = name.to_lowercase();
            if let Some(authored) = self.tools.iter().find(|tool| tool.to_lowercase() == lower) {
                name.clone_from(authored);
            }
        }
        Ok(ToolCall {
            id: field(block, "id")?,
            name,
            arguments: serde_json::json!({}),
            thought_signature: None,
        })
    }

    /// Dispatches deltas while retaining unknown block kinds unchanged.
    fn delta(&mut self, event: &Value) -> ExtensionResult<()> {
        let wire = wire_index(event)?;
        let Some(block) = self.blocks.get(&wire) else {
            return Ok(());
        };
        let index = block.content_index();
        let delta = &event["delta"];
        match delta["type"].as_str().unwrap_or_default() {
            "text_delta" | "thinking_delta" => self.append(index, delta),
            "signature_delta" => {
                let mut output = self
                    .output
                    .write()
                    .map_err(|_| "Assistant message lock poisoned")?;
                if let Some(AssistantContent::Thinking(thinking)) = output.content.get_mut(index) {
                    thinking
                        .thinking_signature
                        .get_or_insert_with(String::new)
                        .push_str(&field(delta, "signature")?);
                }
                Ok(())
            }
            "input_json_delta" => self.tool_delta(wire, delta),
            _ => Ok(()),
        }
    }

    /// Appends text before publishing the corresponding delta.
    fn append(&self, index: usize, incoming: &Value) -> ExtensionResult<()> {
        let update = {
            let mut output = self
                .output
                .write()
                .map_err(|_| "Assistant message lock poisoned")?;
            let partial = Arc::clone(&self.output);
            match (incoming["type"].as_str(), output.content.get_mut(index)) {
                (Some("text_delta"), Some(AssistantContent::Text(text))) => {
                    let delta = field(incoming, "text")?;
                    text.text.push_str(&delta);
                    Event::TextDelta {
                        content_index: index,
                        delta,
                        partial,
                    }
                }
                (Some("thinking_delta"), Some(AssistantContent::Thinking(thinking))) => {
                    let delta = field(incoming, "thinking")?;
                    thinking.thinking.push_str(&delta);
                    Event::ThinkingDelta {
                        content_index: index,
                        delta,
                        partial,
                    }
                }
                _ => return Ok(()),
            }
        };
        self.stream.push(update)
    }

    /// Publishes each raw argument fragment, retaining the most recent complete JSON value.
    fn tool_delta(&mut self, wire: u64, incoming: &Value) -> ExtensionResult<()> {
        let Some(OpenBlock::Tool { index, json }) = self.blocks.get_mut(&wire) else {
            return Ok(());
        };
        let delta = field(incoming, "partial_json")?;
        json.push_str(&delta);
        let mut output = self
            .output
            .write()
            .map_err(|_| "Assistant message lock poisoned")?;
        if let Some(AssistantContent::ToolCall(tool)) = output.content.get_mut(*index)
            && let Ok(arguments) = serde_json::from_str(json)
        {
            tool.arguments = arguments;
        }
        let index = *index;
        drop(output);
        self.stream.push(Event::ToolcallDelta {
            content_index: index,
            delta,
            partial: Arc::clone(&self.output),
        })
    }

    /// Closes the wire block and rejects malformed final tool arguments.
    fn end(&mut self, event: &Value) -> ExtensionResult<()> {
        let Some(block) = self.blocks.remove(&wire_index(event)?) else {
            return Ok(());
        };
        let index = block.content_index();
        let mut output = self
            .output
            .write()
            .map_err(|_| "Assistant message lock poisoned")?;
        if let OpenBlock::Tool { json, .. } = &block
            && !json.is_empty()
            && let Some(AssistantContent::ToolCall(tool)) = output.content.get_mut(index)
        {
            tool.arguments = serde_json::from_str(json)
                .map_err(|error| format!("Invalid tool arguments: {error}"))?;
        }
        let partial = Arc::clone(&self.output);
        let update = match output.content.get(index) {
            Some(AssistantContent::Text(text)) => Event::TextEnd {
                content_index: index,
                content: text.text.clone(),
                partial,
            },
            Some(AssistantContent::Thinking(thinking)) => Event::ThinkingEnd {
                content_index: index,
                content: thinking.thinking.clone(),
                partial,
            },
            Some(AssistantContent::ToolCall(tool)) => Event::ToolcallEnd {
                content_index: index,
                tool_call: tool.clone(),
                partial,
            },
            None => return Err("Missing assistant content block".into()),
        };
        drop(output);
        self.stream.push(update)
    }

    /// Publishes one terminal event and releases iteration without replacing its result.
    pub fn finish(&self, outcome: ExtensionResult<()>, aborted: bool) -> ExtensionResult<()> {
        if aborted {
            return self.fail("Request was aborted".into(), ErrorReason::Aborted);
        }
        if let Err(error) = outcome {
            return self.fail(error, ErrorReason::Error);
        }
        let stop_reason = self
            .output
            .read()
            .map_err(|_| "Assistant message lock poisoned")?
            .stop_reason
            .clone();
        let reason = match stop_reason {
            StopReason::Stop => DoneReason::Stop,
            StopReason::Length => DoneReason::Length,
            StopReason::ToolUse => DoneReason::ToolUse,
            _ => return self.fail("Invalid terminal stop reason".into(), ErrorReason::Error),
        };
        self.stream.push(Event::Done {
            reason,
            message: Arc::clone(&self.output),
        })?;
        self.stream.end(None)
    }

    /// Retains partial content when the request fails.
    fn fail(&self, error: String, reason: ErrorReason) -> ExtensionResult<()> {
        {
            let mut output = self
                .output
                .write()
                .map_err(|_| "Assistant message lock poisoned")?;
            output.stop_reason = match &reason {
                ErrorReason::Error => StopReason::Error,
                ErrorReason::Aborted => StopReason::Aborted,
            };
            output.error_message = Some(error);
        }
        self.stream.push(Event::Error {
            reason,
            error: Arc::clone(&self.output),
        })?;
        self.stream.end(None)
    }
}

/// Requires a nonnegative integral wire index before indexing retained blocks.
fn wire_index(event: &Value) -> ExtensionResult<u64> {
    event["index"]
        .as_u64()
        .ok_or_else(|| "Invalid content block index".into())
}

/// Requires string fields before accepting provider-controlled text.
fn field(event: &Value, name: &str) -> ExtensionResult<String> {
    event[name]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("Invalid {name} field"))
}
