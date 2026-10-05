//! Pull-based cumulative assembly with independent caller snapshots.

use crate::{
    AssistantMessage, Failure, Model, ModelEvent, ProviderStream, ProviderUpdate, StopReason,
    TextContent, Usage,
};

/// One request's normalization state and owned adapter source.
/// Events are produced incrementally; no producer, channel or runtime is required.
pub struct ModelStream {
    source: Option<Box<dyn ProviderStream>>,
    message: AssistantMessage,
    pending: Option<ProviderUpdate>,
    failure: Option<Failure>,
    started: bool,
    finishing: bool,
    terminal: bool,
}

impl ModelStream {
    pub(crate) fn new(
        model: Model,
        timestamp: u64,
        source: Result<Box<dyn ProviderStream>, Failure>,
    ) -> Self {
        let (source, failure) = match source {
            Ok(source) => (Some(source), None),
            Err(failure) => (None, Some(failure)),
        };
        Self {
            source,
            message: AssistantMessage {
                provider: model.identity.provider,
                protocol: model.protocol,
                model: model.identity.model,
                timestamp,
                content: Vec::new(),
                usage: Usage::default(),
                stop_reason: None,
                failure: None,
            },
            pending: None,
            failure,
            started: false,
            finishing: false,
            terminal: false,
        }
    }

    fn error(&mut self, failure: Failure) -> ModelEvent {
        self.source = None;
        self.terminal = true;
        self.message.stop_reason = Some(StopReason::Error);
        self.message.failure = Some(failure);
        ModelEvent::Error {
            reason: StopReason::Error,
            error: self.message.clone(),
        }
    }

    fn done(&mut self) -> ModelEvent {
        self.source = None;
        self.terminal = true;
        self.message.stop_reason = Some(StopReason::Stop);
        ModelEvent::Done {
            reason: StopReason::Stop,
            message: self.message.clone(),
        }
    }

    /// Return the next independent owned event, or None forever after terminal delivery.
    /// Successful text emits start, text-start, deltas, text-end and done.
    /// Empty success emits start/done. Setup or source failures emit a terminal error.
    pub async fn next(&mut self) -> Option<ModelEvent> {
        if self.terminal {
            return None;
        }
        if let Some(failure) = self.failure.take() {
            return Some(self.error(failure));
        }
        if self.finishing {
            return Some(self.done());
        }
        loop {
            if matches!(self.pending, Some(ProviderUpdate::TextDelta { .. }))
                && self.message.content.is_empty()
            {
                self.message.content.push(TextContent {
                    text: String::new(),
                });
                return Some(ModelEvent::TextStart {
                    content_index: 0,
                    partial: self.message.clone(),
                });
            }
            if let Some(update) = self.pending.take() {
                return Some(match update {
                    ProviderUpdate::TextDelta { delta } => {
                        self.message.content[0].text.push_str(&delta);
                        ModelEvent::TextDelta {
                            content_index: 0,
                            delta,
                            partial: self.message.clone(),
                        }
                    }
                    ProviderUpdate::Done { usage } => {
                        self.message.usage = usage;
                        self.source = None;
                        if let Some(content) = self.message.content.first() {
                            self.finishing = true;
                            ModelEvent::TextEnd {
                                content_index: 0,
                                content: content.text.clone(),
                                partial: self.message.clone(),
                            }
                        } else {
                            self.done()
                        }
                    }
                    ProviderUpdate::Error { failure } => self.error(failure),
                });
            }
            let update = match self.source.as_mut() {
                Some(source) => source.next().await,
                None => None,
            };
            let Some(update) = update else {
                return Some(self.error(Failure::IncompleteStream));
            };
            if let ProviderUpdate::Error { failure } = update {
                return Some(self.error(failure));
            }
            self.pending = Some(update);
            if !self.started {
                self.started = true;
                return Some(ModelEvent::Start {
                    partial: self.message.clone(),
                });
            }
        }
    }
}
