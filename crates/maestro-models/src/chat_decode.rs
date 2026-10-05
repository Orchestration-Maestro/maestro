//! Chat chunk assembly with validated terminal framing.
use crate::*;
use serde_json::Value;
use std::collections::VecDeque;
struct Tool {
    block: usize,
    index: Option<u64>,
    aliases: Vec<String>,
    id: String,
    name: String,
}
pub(crate) struct Decoder {
    pub queue: VecDeque<ProviderUpdate>,
    text: Option<usize>,
    thinking: Option<usize>,
    pub finish: Option<StopReason>,
    blocks: usize,
    tools: Vec<Tool>,
    requested: String,
    response_id: Option<String>,
    response_model: Option<String>,
}
impl Decoder {
    pub fn new(requested: String) -> Self {
        Self {
            queue: VecDeque::new(),
            text: None,
            thinking: None,
            finish: None,
            blocks: 0,
            tools: Vec::new(),
            requested,
            response_id: None,
            response_model: None,
        }
    }
    pub fn data(&mut self, data: &str) -> Result<bool, Failure> {
        if data == "[DONE]" {
            self.end()?;
            return Ok(true);
        }
        let value: Value = serde_json::from_str(data).map_err(|_| Failure::MalformedStream)?;
        if !value.is_object() {
            return Ok(false);
        }
        if value.get("error").is_some() {
            return Err(crate::chat_failure::classify(None, &value));
        }
        if self.response_id.is_none() {
            self.response_id = value["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(String::from);
        }
        if self.response_model.is_none() {
            self.response_model = value["model"]
                .as_str()
                .filter(|s| !s.is_empty() && *s != self.requested)
                .map(String::from);
        }
        self.queue.push_back(ProviderUpdate::ResponseIdentity {
            response_id: self.response_id.clone(),
            response_model: self.response_model.clone(),
        });
        let choice = &value["choices"][0];
        if let Some(raw) = value
            .get("usage")
            .filter(|v| !v.is_null())
            .or_else(|| choice.get("usage").filter(|v| !v.is_null()))
        {
            let counter = |v: Option<&Value>| -> Result<u64, Failure> {
                match v {
                    None => Ok(0),
                    Some(v) => v
                        .as_u64()
                        .or_else(|| {
                            v.as_f64()
                                .filter(|n| {
                                    n.is_finite()
                                        && *n >= 0.0
                                        && n.fract() == 0.0
                                        && *n < 18446744073709551616.0
                                })
                                .map(|n| n as u64)
                        })
                        .ok_or(Failure::MalformedStream),
                }
            };
            let prompt = counter(raw.get("prompt_tokens"))?;
            let output = counter(raw.get("completion_tokens"))?;
            let details = &raw["prompt_tokens_details"];
            let cached = counter(
                details
                    .get("cached_tokens")
                    .or_else(|| raw.get("prompt_cache_hit_tokens")),
            )?;
            let write = counter(details.get("cache_write_tokens"))?;
            let read = cached.saturating_sub(write);
            self.queue.push_back(ProviderUpdate::Usage {
                usage: Usage {
                    input: prompt.saturating_sub(read).saturating_sub(write),
                    output,
                    cache_read: read,
                    cache_write: write,
                    ..Usage::default()
                },
            });
        }
        if let Some(text) = choice["delta"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            let index = if let Some(i) = self.text {
                i
            } else {
                let i = self.blocks;
                self.blocks += 1;
                self.text = Some(i);
                self.queue
                    .push_back(ProviderUpdate::TextStart { content_index: i });
                i
            };
            self.queue.push_back(ProviderUpdate::TextDelta {
                content_index: index,
                delta: text.into(),
            });
        }
        for field in ["reasoning_content", "reasoning", "reasoning_text"] {
            if let Some(text) = choice["delta"][field].as_str().filter(|s| !s.is_empty()) {
                let index = if let Some(i) = self.thinking {
                    i
                } else {
                    let i = self.blocks;
                    self.blocks += 1;
                    self.thinking = Some(i);
                    self.queue.push_back(ProviderUpdate::ThinkingStart {
                        content_index: i,
                        signature: Some(field.into()),
                    });
                    i
                };
                self.queue.push_back(ProviderUpdate::ThinkingDelta {
                    content_index: index,
                    delta: text.into(),
                });
                break;
            }
        }
        if let Some(calls) = choice["delta"]["tool_calls"].as_array() {
            for call in calls {
                let index = call["index"].as_u64();
                let id = call["id"].as_str().filter(|s| !s.is_empty()).unwrap_or("");
                let name = call["function"]["name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("");
                let found = index
                    .and_then(|index| self.tools.iter().position(|t| t.index == Some(index)))
                    .or_else(|| {
                        (!id.is_empty())
                            .then(|| {
                                self.tools
                                    .iter()
                                    .position(|t| t.aliases.iter().any(|a| a == id))
                            })
                            .flatten()
                    });
                let position = found.unwrap_or_else(|| {
                    let block = self.blocks;
                    self.blocks += 1;
                    self.tools.push(Tool {
                        block,
                        index,
                        aliases: Vec::new(),
                        id: id.into(),
                        name: name.into(),
                    });
                    self.queue.push_back(ProviderUpdate::ToolCallStart {
                        content_index: block,
                        id: id.into(),
                        name: name.into(),
                        replay_metadata: None,
                    });
                    self.tools.len() - 1
                });
                let tool = &mut self.tools[position];
                if tool.index.is_none() {
                    tool.index = index;
                }
                if !id.is_empty() && !tool.aliases.iter().any(|a| a == id) {
                    tool.aliases.push(id.into());
                }
                let mut new_id = None;
                let mut new_name = None;
                if tool.id.is_empty() && !id.is_empty() {
                    tool.id = id.into();
                    new_id = Some(id.into());
                }
                if tool.name.is_empty() && !name.is_empty() {
                    tool.name = name.into();
                    new_name = Some(name.into());
                }
                if new_id.is_some() || new_name.is_some() {
                    self.queue.push_back(ProviderUpdate::ToolCallMetadata {
                        content_index: tool.block,
                        id: new_id,
                        name: new_name,
                        replay_metadata: None,
                    });
                }
                if let Some(arguments) = call["function"]["arguments"].as_str() {
                    self.queue.push_back(ProviderUpdate::ToolCallDelta {
                        content_index: tool.block,
                        delta: arguments.into(),
                    });
                }
            }
        }
        if let Some(details) = choice["delta"]["reasoning_details"].as_array() {
            for detail in details {
                if detail["type"] == "reasoning.encrypted"
                    && detail["data"].as_str().is_some_and(|s| !s.is_empty())
                    && let Some(tool) = detail["id"]
                        .as_str()
                        .and_then(|id| self.tools.iter().find(|t| t.id == id))
                {
                    self.queue.push_back(ProviderUpdate::ToolCallMetadata {
                        content_index: tool.block,
                        id: None,
                        name: None,
                        replay_metadata: Some(crate::chat_json::compact(detail)),
                    });
                }
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish = Some(match reason {
                "stop" | "end" => StopReason::Stop,
                "length" => StopReason::Length,
                "tool_calls" | "function_call" => StopReason::ToolUse,
                "content_filter" => return Err(Failure::AdapterFailed),
                "network_error" => return Err(Failure::Transport),
                "model_context_window_exceeded" => return Err(Failure::ContextOverflow),
                _ => return Err(Failure::MalformedStream),
            });
        }
        Ok(false)
    }
    pub fn end(&mut self) -> Result<(), Failure> {
        let reason = self.finish.ok_or(Failure::IncompleteStream)?;
        for index in 0..self.blocks {
            if self.text == Some(index) {
                self.queue.push_back(ProviderUpdate::TextEnd {
                    content_index: index,
                    replay_metadata: None,
                });
            } else if self.thinking == Some(index) {
                self.queue.push_back(ProviderUpdate::ThinkingEnd {
                    content_index: index,
                });
            } else {
                self.queue.push_back(ProviderUpdate::ToolCallEnd {
                    content_index: index,
                });
            }
        }
        self.queue.push_back(ProviderUpdate::Done { reason });
        Ok(())
    }
}
