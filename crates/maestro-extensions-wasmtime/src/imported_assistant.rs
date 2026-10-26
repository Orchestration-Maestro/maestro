//! Native consumers front the original guest stream and cache only its final message.
use crate::{
    guest_calls::{Call, Handle},
    local_resources::{Message, Registry},
    provider_calls::{Assistant, Operation},
};
use maestro_extensions::{
    AssistantMessageEvent, AssistantMessageEventStream, AssistantMessageEventStreamPort, BoxFuture,
    DoneReason, ErrorReason, Model, SharedAssistantMessage, StopReason,
};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use tokio::sync::mpsc::UnboundedSender;

/// Canonical decoded final ownership and whether iteration has reached its terminal boundary.
#[derive(Default)]
struct Final {
    /// First final message to cross the component boundary.
    message: Option<SharedAssistantMessage>,
    /// A terminal event or EOF has already been returned.
    terminal: bool,
}
/// Direct port over the original retained resource, with no queue or reducer.
#[derive(Clone)]
struct Port {
    /// Original authored stream and its quiescent final-release owner.
    handler: Handle,
    /// Original pump for synchronous operations and reentry.
    requests: UnboundedSender<Message>,
    /// Supplied descriptor used only when transport failure needs a native terminal message.
    model: Model,
    /// One shared final-message cache, not a second settlement owner.
    final_value: Arc<Mutex<Final>>,
}
/// Front the returned resource without starting any forwarding producer.
pub(crate) fn stream(
    handler: Handle,
    model: Model,
    requests: UnboundedSender<Message>,
) -> AssistantMessageEventStream {
    AssistantMessageEventStream::from_port(Port {
        handler,
        requests,
        model,
        final_value: Arc::default(),
    })
}
impl Port {
    /// Recover the final cache without running callbacks or author destructors under its guard.
    fn state(&self) -> MutexGuard<'_, Final> {
        self.final_value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Preserve the first decoded terminal even when a later Store operation fails.
    fn cache(&self, message: SharedAssistantMessage) -> SharedAssistantMessage {
        self.state().message.get_or_insert(message).clone()
    }
    /// Project only an actual transport or codec failure, never an unread guest terminal.
    fn failure(&self, error: String) -> SharedAssistantMessage {
        let mut message = maestro_extensions::faux_assistant_message(
            "",
            maestro_extensions::FauxAssistantMessageOptions::default(),
        );
        message.api.clone_from(&self.model.api);
        message.provider.clone_from(&self.model.provider);
        message.model.clone_from(&self.model.id);
        message.stop_reason = StopReason::Error;
        message.error_message = Some(error);
        self.cache(Arc::new(RwLock::new(message)))
    }
    /// Build one terminal event from the cached message after a failed resource operation.
    fn terminal(&self, error: String) -> Option<AssistantMessageEvent> {
        let message = self.failure(error);
        let mut state = self.state();
        if state.terminal {
            return None;
        }
        state.terminal = true;
        drop(state);
        let reason = message
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop_reason
            .clone();
        Some(match reason {
            StopReason::Error => AssistantMessageEvent::Error {
                reason: ErrorReason::Error,
                error: message,
            },
            StopReason::Aborted => AssistantMessageEvent::Error {
                reason: ErrorReason::Aborted,
                error: message,
            },
            StopReason::Stop => AssistantMessageEvent::Done {
                reason: DoneReason::Stop,
                message,
            },
            StopReason::Length => AssistantMessageEvent::Done {
                reason: DoneReason::Length,
                message,
            },
            StopReason::ToolUse => AssistantMessageEvent::Done {
                reason: DoneReason::ToolUse,
                message,
            },
        })
    }
    /// Await one ordinary operation through the same lifecycle and Store pump.
    async fn observe(&self, operation: Operation) -> Result<Option<String>, String> {
        let cancellation = maestro_extensions::Cancellation::new();
        let signal = if let Ok(signal) = Registry::on_pump(&self.requests, |registry| {
            registry.signal(cancellation.clone())
        }) {
            signal?
        } else {
            let (reply, result) = std::sync::mpsc::channel();
            self.requests
                .send(Message::NewSignal(Some(cancellation.clone()), reply))
                .map_err(|_| "extension resource owner ended")?;
            result
                .recv()
                .map_err(|_| "extension resource reply ended")??
        };
        let _observation = Observation(cancellation);
        self.handler
            .endpoint()
            .submit(|reply| {
                Call::Assistant(Assistant {
                    handler: self.handler.clone(),
                    operation,
                    data: None,
                    signal: Some(signal),
                    reply,
                })
            })
            .await
    }
    /// Reentry uses the original scoped route; callers on another thread wait on its original reply.
    fn mutate(&self, operation: Operation, data: Option<String>) -> Result<(), String> {
        let mut data = Some(data);
        let on_pump = Registry::on_pump(&self.requests, |registry| {
            self.handler.endpoint().synchronous(registry, |reply| {
                Ok(Call::Assistant(Assistant {
                    handler: self.handler.clone(),
                    operation,
                    data: data.take().flatten(),
                    signal: None,
                    reply,
                }))
            })
        });
        if let Ok(result) = on_pump {
            return result.map(|_| ());
        }
        let (sender, result) = std::sync::mpsc::channel();
        let reply = self.handler.endpoint().replies.register(move |value| {
            let _ = sender.send(value);
        })?;
        self.handler.endpoint().send(Call::Assistant(Assistant {
            handler: self.handler.clone(),
            operation,
            data: data.flatten(),
            signal: None,
            reply,
        }))?;
        result
            .recv()
            .map_err(|_| "extension guest reply ended")?
            .map(|_| ())
    }
    /// Canonicalize terminal/result ownership without retaining previous events.
    fn decoded(&self, data: &str) -> Result<AssistantMessageEvent, String> {
        let mut event: AssistantMessageEvent = crate::writer_reads::decode(data)?;
        match &mut event {
            AssistantMessageEvent::Done { message, .. }
            | AssistantMessageEvent::Error { error: message, .. } => {
                *message = self.cache(message.clone());
                self.state().terminal = true;
            }
            _ => {}
        }
        Ok(event)
    }
}
impl AssistantMessageEventStreamPort for Port {
    fn push(&self, event: AssistantMessageEvent) {
        if let Err(error) = serde_json::to_string(&event)
            .map_err(|e| e.to_string())
            .and_then(|data| self.mutate(Operation::Push, Some(data)))
        {
            self.failure(error);
        }
    }
    fn end(&self, result: Option<SharedAssistantMessage>) {
        if let Err(error) = result
            .map(|result| serde_json::to_string(&result).map_err(|e| e.to_string()))
            .transpose()
            .and_then(|data| self.mutate(Operation::End, data))
        {
            self.failure(error);
        }
    }
    fn next(&self) -> BoxFuture<Option<AssistantMessageEvent>> {
        let port = self.clone();
        Box::pin(async move {
            if port.state().terminal {
                return None;
            }
            match port
                .observe(Operation::Next)
                .await
                .and_then(|data| data.map(|data| port.decoded(&data)).transpose())
            {
                Ok(value) => {
                    port.state().terminal |= value.is_none();
                    value
                }
                Err(error) => port.terminal(error),
            }
        })
    }
    fn result(&self) -> BoxFuture<SharedAssistantMessage> {
        let port = self.clone();
        Box::pin(async move {
            if let Some(message) = port.state().message.clone() {
                return message;
            }
            match port.observe(Operation::Result).await.and_then(|data| {
                crate::writer_reads::decode(&data.ok_or("missing assistant result")?)
            }) {
                Ok(message) => port.cache(message),
                Err(error) => port.failure(error),
            }
        })
    }
}

/// Dropping a pending read removes only its waiter through the existing cancellation owner.
struct Observation(maestro_extensions::Cancellation);
impl Drop for Observation {
    fn drop(&mut self) {
        self.0.abort();
    }
}
