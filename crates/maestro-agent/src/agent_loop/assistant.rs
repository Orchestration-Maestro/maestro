//! Request conversion and streamed assistant reduction.
use super::{Driver, read, write};
use crate::{AgentEvent, AgentMessage, CustomAgentMessages};
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, DiagnosticErrorInfo,
    ProviderStreamOptions, SharedAssistantMessage, stream_simple,
};
use std::sync::{Arc, RwLock};

impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Invoke the selected model after transformation and conversion.
    pub(super) async fn assistant(&self) -> Result<SharedAssistantMessage, DiagnosticErrorInfo> {
        let history = match &self.config.transform_context {
            Some(transform) => {
                transform(
                    Arc::clone(&self.context.messages),
                    self.options.signal.clone(),
                )
                .await?
            }
            None => Arc::clone(&self.context.messages),
        };
        let messages = (self.config.convert_to_llm)(history).await?;
        let tools = self.context.tools.as_ref().map(|tools| {
            read(tools)
                .iter()
                .map(|tool| read(tool).definition.clone())
                .collect()
        });
        let context = Context {
            system_prompt: Some(self.context.system_prompt.clone()),
            messages,
            tools,
        };
        let mut options = self.config.options.clone();
        if let Some(resolve) = &self.config.get_api_key
            && let Some(key) = resolve(self.config.model.provider.clone())
                .await?
                .filter(|key| !key.is_empty())
        {
            options.common.api_key = Some(key);
        }
        options.common.signal = self.options.signal.clone();
        let stream = match &self.options.stream_fn {
            Some(stream) => {
                let open = ProviderStreamOptions {
                    common: options.common.clone(),
                    extra: self.config.extra.clone(),
                    objects: self.config.objects.clone(),
                };
                stream(self.config.model.clone(), context, options, open).await?
            }
            None => stream_simple(self.config.model.clone(), context, Some(options))?,
        };
        self.consume(stream).await
    }
    /// Reduce model events, then read the stream-owned final result.
    async fn consume(
        &self,
        stream: AssistantMessageEventStream,
    ) -> Result<SharedAssistantMessage, DiagnosticErrorInfo> {
        let mut started = false;
        while let Some(event) = stream.next().await {
            match &event {
                AssistantMessageEvent::Start { partial } => {
                    self.store_partial(partial, false);
                    started = true;
                    (self.emit)(AgentEvent::MessageStart {
                        message: AgentMessage::Assistant(snapshot(partial)),
                    })
                    .await?;
                }
                AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. } => break,
                _ if started => self.emit_update(event).await?,
                _ => (),
            }
        }
        let result = stream.result().await;
        self.store_partial(&result, started);
        if !started {
            (self.emit)(AgentEvent::MessageStart {
                message: AgentMessage::Assistant(snapshot(&result)),
            })
            .await?;
        }
        (self.emit)(AgentEvent::MessageEnd {
            message: AgentMessage::Assistant(Arc::clone(&result)),
        })
        .await?;
        Ok(result)
    }
    /// Publish an update snapshot while retaining the original model event.
    async fn emit_update(&self, event: AssistantMessageEvent) -> Result<(), DiagnosticErrorInfo> {
        if let Some(partial) = update_partial(&event) {
            self.store_partial(partial, true);
            (self.emit)(AgentEvent::MessageUpdate {
                message: snapshot(partial),
                assistant_message_event: event,
            })
            .await?;
        }
        Ok(())
    }
    /// Replace the active partial or append a previously unstarted response.
    fn store_partial(&self, message: &SharedAssistantMessage, replace: bool) {
        let replacement = AgentMessage::Assistant(Arc::clone(message));
        let old = {
            let mut messages = write(&self.context.messages);
            if replace {
                messages
                    .last_mut()
                    .map(|last| std::mem::replace(last, replacement))
            } else {
                messages.push(replacement);
                None
            }
        };
        drop(old);
    }
}
/// Capture the source's explicit assistant snapshot without changing its live handle.
fn snapshot(message: &SharedAssistantMessage) -> SharedAssistantMessage {
    Arc::new(RwLock::new(read(message).clone()))
}
/// Select the shared partial carried by any update variant.
fn update_partial(event: &AssistantMessageEvent) -> Option<&SharedAssistantMessage> {
    match event {
        AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => Some(partial),
        _ => None,
    }
}
