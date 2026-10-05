//! Incremental normalization with owned snapshots and terminal source release.

use crate::{
    AssistantContent, AssistantMessage, Cancellation, Failure, Model, ModelEvent, ProviderStream,
    ProviderUpdate, StopReason, TextContent, ThinkingContent, ToolCall, Usage,
};

/// One request's normalization state and owned adapter source.
pub struct ModelStream {
    source: Option<Box<dyn ProviderStream>>,
    message: AssistantMessage,
    pending: Option<ProviderUpdate>,
    pending_event: Option<ModelEvent>,
    failure: Option<Failure>,
    started: bool,
    terminal: bool,
    cancellation: Cancellation,
    blocks: Vec<BlockState>,
}

enum BlockState {
    Text,
    Thinking,
    Tool(String),
    Closed,
}

impl ModelStream {
    pub(crate) fn new(
        model: Model,
        timestamp: u64,
        source: Result<Box<dyn ProviderStream>, Failure>,
        cancellation: Cancellation,
    ) -> Self {
        let (source, failure) = match source {
            Ok(source) => (Some(source), None),
            Err(failure) => (None, Some(failure)),
        };
        Self {
            source,
            failure,
            pending: None,
            pending_event: None,
            started: false,
            terminal: false,
            cancellation,
            blocks: Vec::new(),
            message: AssistantMessage {
                provider: model.identity.provider,
                protocol: model.protocol,
                model: model.identity.model,
                timestamp,
                content: Vec::new(),
                usage: Usage::default(),
                stop_reason: None,
                failure: None,
                response_model: None,
                response_id: None,
            },
        }
    }
    fn error(&mut self, failure: Failure) -> ModelEvent {
        self.source = None;
        self.terminal = true;
        let reason = if failure == Failure::Cancelled {
            StopReason::Aborted
        } else {
            StopReason::Error
        };
        self.message.stop_reason = Some(reason);
        self.message.failure = Some(failure);
        ModelEvent::Error {
            reason,
            error: self.message.clone(),
        }
    }
    fn start_block(
        &mut self,
        index: usize,
        content: AssistantContent,
        state: BlockState,
    ) -> Result<(), Failure> {
        if index != self.blocks.len() {
            return Err(Failure::MalformedStream);
        }
        self.blocks.push(state);
        self.message.content.push(content);
        Ok(())
    }
    fn apply(&mut self, update: ProviderUpdate) -> Result<ModelEvent, Failure> {
        use ProviderUpdate as U;
        Ok(match update {
            U::TextStart { content_index } => {
                self.start_block(
                    content_index,
                    AssistantContent::Text(TextContent {
                        text: String::new(),
                        replay_metadata: None,
                    }),
                    BlockState::Text,
                )?;
                ModelEvent::TextStart {
                    content_index,
                    partial: self.message.clone(),
                }
            }
            U::ThinkingStart {
                content_index,
                signature,
            } => {
                self.start_block(
                    content_index,
                    AssistantContent::Thinking(ThinkingContent::Readable {
                        text: String::new(),
                        signature,
                    }),
                    BlockState::Thinking,
                )?;
                ModelEvent::ThinkingStart {
                    content_index,
                    partial: self.message.clone(),
                }
            }
            U::ToolCallStart {
                content_index,
                id,
                name,
                replay_metadata,
            } => {
                self.start_block(
                    content_index,
                    AssistantContent::ToolCall(ToolCall {
                        id,
                        name,
                        replay_metadata,
                        arguments: None,
                    }),
                    BlockState::Tool(String::new()),
                )?;
                ModelEvent::ToolCallStart {
                    content_index,
                    partial: self.message.clone(),
                }
            }
            U::RedactedThinking {
                content_index,
                data,
            } => {
                self.start_block(
                    content_index,
                    AssistantContent::Thinking(ThinkingContent::Redacted { data }),
                    BlockState::Closed,
                )?;
                self.pending_event = Some(ModelEvent::ThinkingEnd {
                    content_index,
                    content: String::new(),
                    partial: self.message.clone(),
                });
                ModelEvent::ThinkingStart {
                    content_index,
                    partial: self.message.clone(),
                }
            }
            U::TextDelta {
                content_index,
                delta,
            } => {
                if !matches!(self.blocks.get(content_index), Some(BlockState::Text)) {
                    return Err(Failure::MalformedStream);
                }
                if let AssistantContent::Text(content) = &mut self.message.content[content_index] {
                    content.text.push_str(&delta);
                }
                ModelEvent::TextDelta {
                    content_index,
                    delta,
                    partial: self.message.clone(),
                }
            }
            U::ThinkingDelta {
                content_index,
                delta,
            } => {
                if !matches!(self.blocks.get(content_index), Some(BlockState::Thinking)) {
                    return Err(Failure::MalformedStream);
                }
                if let AssistantContent::Thinking(ThinkingContent::Readable { text, .. }) =
                    &mut self.message.content[content_index]
                {
                    text.push_str(&delta);
                }
                ModelEvent::ThinkingDelta {
                    content_index,
                    delta,
                    partial: self.message.clone(),
                }
            }
            U::ToolCallDelta {
                content_index,
                delta,
            } => {
                let Some(BlockState::Tool(json)) = self.blocks.get_mut(content_index) else {
                    return Err(Failure::MalformedStream);
                };
                json.push_str(&delta);
                ModelEvent::ToolCallDelta {
                    content_index,
                    delta,
                    partial: self.message.clone(),
                }
            }
            U::TextEnd {
                content_index,
                replay_metadata,
            } => {
                if !matches!(self.blocks.get(content_index), Some(BlockState::Text)) {
                    return Err(Failure::MalformedStream);
                }
                let AssistantContent::Text(text) = &mut self.message.content[content_index] else {
                    return Err(Failure::MalformedStream);
                };
                text.replay_metadata = replay_metadata;
                let content = text.text.clone();
                self.blocks[content_index] = BlockState::Closed;
                ModelEvent::TextEnd {
                    content_index,
                    content,
                    partial: self.message.clone(),
                }
            }
            U::ThinkingEnd { content_index } => {
                if !matches!(self.blocks.get(content_index), Some(BlockState::Thinking)) {
                    return Err(Failure::MalformedStream);
                }
                let AssistantContent::Thinking(ThinkingContent::Readable { text, .. }) =
                    &self.message.content[content_index]
                else {
                    return Err(Failure::MalformedStream);
                };
                let content = text.clone();
                self.blocks[content_index] = BlockState::Closed;
                ModelEvent::ThinkingEnd {
                    content_index,
                    content,
                    partial: self.message.clone(),
                }
            }
            U::ToolCallEnd { content_index } => {
                let Some(BlockState::Tool(json)) = self.blocks.get(content_index) else {
                    return Err(Failure::MalformedStream);
                };
                let object =
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json)
                        .map_err(|_| Failure::MalformedStream)?;
                let AssistantContent::ToolCall(call) = &mut self.message.content[content_index]
                else {
                    return Err(Failure::MalformedStream);
                };
                call.arguments = Some(object);
                let tool_call = call.clone();
                self.blocks[content_index] = BlockState::Closed;
                ModelEvent::ToolCallEnd {
                    content_index,
                    tool_call,
                    partial: self.message.clone(),
                }
            }
            U::Done { reason }
                if matches!(
                    reason,
                    StopReason::Stop | StopReason::Length | StopReason::ToolUse
                ) && self.blocks.iter().all(|b| matches!(b, BlockState::Closed)) =>
            {
                self.source = None;
                self.terminal = true;
                self.message.stop_reason = Some(reason);
                ModelEvent::Done {
                    reason,
                    message: self.message.clone(),
                }
            }
            U::Error { failure } => self.error(failure),
            _ => return Err(Failure::MalformedStream),
        })
    }
    /// Obtain one independent owned event, or None permanently after termination.
    /// Cancellation wins observable readiness ties, wakes blocked reads and drops
    /// the source. Failures retain valid partial content, reported usage and identity.
    /// Success requires balanced blocks and an explicit successful Done update.
    pub async fn next(&mut self) -> Option<ModelEvent> {
        if self.terminal {
            return None;
        }
        if self.cancellation.is_cancelled() {
            return Some(self.error(Failure::Cancelled));
        }
        if let Some(failure) = self.failure.take() {
            return Some(self.error(failure));
        }
        if let Some(event) = self.pending_event.take() {
            return Some(event);
        }
        if let Some(update) = self.pending.take() {
            return Some(match self.apply(update) {
                Ok(event) => event,
                Err(failure) => self.error(failure),
            });
        }
        loop {
            let cancellation = self.cancellation.clone();
            let update = match self.source.as_mut() {
                Some(source) => {
                    let mut read = source.next();
                    let mut cancelled = std::pin::pin!(cancellation.cancelled());
                    std::future::poll_fn(|cx| {
                        use std::future::Future;
                        use std::task::Poll;
                        if cancelled.as_mut().poll(cx).is_ready() {
                            return Poll::Ready(None);
                        }
                        let result = read.as_mut().poll(cx);
                        if cancellation.is_cancelled() {
                            Poll::Ready(None)
                        } else {
                            result
                        }
                    })
                    .await
                }
                None => None,
            };
            if cancellation.is_cancelled() {
                return Some(self.error(Failure::Cancelled));
            }
            let Some(update) = update else {
                return Some(self.error(Failure::IncompleteStream));
            };
            match update {
                ProviderUpdate::Usage { usage } => {
                    self.message.usage = usage;
                    continue;
                }
                ProviderUpdate::ResponseIdentity {
                    response_model,
                    response_id,
                } => {
                    self.message.response_model = response_model;
                    self.message.response_id = response_id;
                    continue;
                }
                _ => {}
            }
            if let ProviderUpdate::Error { failure } = update {
                return Some(self.error(failure));
            }
            if !self.started {
                self.started = true;
                self.pending = Some(update);
                return Some(ModelEvent::Start {
                    partial: self.message.clone(),
                });
            }
            return Some(match self.apply(update) {
                Ok(event) => event,
                Err(failure) => self.error(failure),
            });
        }
    }
}
