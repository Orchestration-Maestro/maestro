//! Immediate native model invocation on the existing local producer owner.
use super::{Capability, Lease, Message, Registry, held::AssistantStream};
use crate::model_calls::Controls;
use maestro_extensions::{
    Context, Local, Model, ProviderSpawner, ProviderStreamOptions, SimpleStreamOptions,
    native_io::IoFuture,
};
use std::{
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};
/// Owned setup operands, selected before returning to the guest.
pub(crate) struct Creation {
    /// Original entry point.
    pub(crate) simple: bool,
    /// Ordinary model, conversation and option data.
    pub(crate) data: String,
    /// Independently retained capabilities, never encoded as JSON.
    pub(crate) controls: Controls,
    /// Synchronous setup boundary.
    pub(crate) reply: Sender<Result<Endpoint, String>>,
}
/// Store-safe lease of one native assistant stream.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);
impl Endpoint {
    /// Forward synchronous admission without adding an event queue.
    pub(crate) fn push(&self, event: String) -> Result<(), String> {
        self.0.run::<AssistantStream, _>(move |stream, _| {
            crate::writer_reads::decode(&event).map(|event| stream.push(event))
        })
    }
    /// Forward explicit ending to the same final-result owner.
    pub(crate) fn end(&self, value: Option<String>) -> Result<(), String> {
        self.0.run::<AssistantStream, _>(move |stream, _| {
            value
                .map(|value| crate::writer_reads::decode(&value))
                .transpose()
                .map(|value| stream.end(value))
        })
    }
    /// Await one native competing reader; canceled observers do not cancel its producer.
    pub(crate) async fn next(&self) -> Result<Option<String>, String> {
        self.0
            .observe::<AssistantStream, _>(|stream, _| {
                let next = stream.next();
                Ok(Box::pin(async move {
                    next.await
                        .map(|value| serde_json::to_string(&value).map_err(|e| e.to_string()))
                        .transpose()
                }) as IoFuture<'static, _>)
            })
            .await
    }
    /// Await the independent native final result.
    pub(crate) async fn result(&self) -> Result<String, String> {
        self.0
            .observe::<AssistantStream, _>(|stream, _| {
                let result = stream.result();
                Ok(Box::pin(async move {
                    serde_json::to_string(&result.await).map_err(|error| error.to_string())
                }) as IoFuture<'static, _>)
            })
            .await
    }
}
impl crate::api_reads::Endpoint {
    /// Invoke immediately on the admitted local owner, without requiring catalog membership.
    pub(crate) fn model_stream(
        &self,
        simple: bool,
        data: String,
        controls: Controls,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::ModelStart(Creation {
                simple,
                data,
                controls,
                reply,
            })
        })
    }
}
impl Creation {
    /// Resolve and invoke through the native model owner, then retain its returned buffer.
    pub(super) fn answer(self, registry: &Registry) {
        let result = self.start(registry);
        let _ = self.reply.send(result);
    }
    /// Preserve outer option absence and select only the supplied option variant.
    fn start(&self, registry: &Registry) -> Result<Endpoint, String> {
        let (model, context, options, api): (
            Model,
            Context,
            Option<serde_json::Value>,
            Option<String>,
        ) = crate::writer_reads::decode(&self.data)?;
        let scopes = self.controls.scopes();
        let submit = registry.model_submit(scopes.clone())?;
        let stream = if self.simple {
            let options = options
                .map(|value| {
                    let (mut common, reasoning, thinking_budgets) =
                        serde_json::from_value(value).map_err(|e| e.to_string())?;
                    self.controls.apply(registry, &scopes, &mut common)?;
                    Ok::<_, String>(SimpleStreamOptions {
                        common,
                        reasoning,
                        thinking_budgets,
                        tool_choice: None,
                    })
                })
                .transpose()?;
            if let Some(api) = api {
                maestro_extensions::stream_simple_via_for::<Local>(
                    &api, model, context, options, submit,
                )
            } else {
                maestro_extensions::stream_simple_for::<Local>(model, context, options, submit)
            }
        } else {
            let options = options
                .map(|value| {
                    let (mut common, extra) =
                        serde_json::from_value(value).map_err(|e| e.to_string())?;
                    self.controls.apply(registry, &scopes, &mut common)?;
                    Ok::<_, String>(ProviderStreamOptions {
                        common,
                        extra,
                        ..ProviderStreamOptions::default()
                    })
                })
                .transpose()?;
            maestro_extensions::stream_for::<Local>(model, context, options, submit)
        }
        .map_err(|error| error.message)?;
        registry
            .insert(Capability::AssistantStream(stream))
            .map(Endpoint)
    }
}
impl Registry {
    /// Give a model producer the same pending-work owner and independent admission as other work.
    fn model_submit(
        &self,
        scopes: crate::model_calls::Scopes,
    ) -> Result<ProviderSpawner<Local>, String> {
        let pump = self.pump.clone();
        let admission = self.admission()?;
        Ok(Rc::new(move |producer| {
            let Some(pump) = pump.upgrade() else {
                return false;
            };
            let (scopes, admission) = (scopes.clone(), admission.clone());
            pump.stage(Box::pin(async move {
                producer.await;
                drop(scopes);
                drop(admission);
            }));
            true
        }))
    }
    /// Clone the original cancellation without holding the capability table while calling hooks.
    pub(crate) fn model_signal(
        &self,
        endpoint: &super::SignalEndpoint,
    ) -> Result<maestro_extensions::Cancellation, String> {
        let capability = self.capability(endpoint.0.id);
        match capability.as_deref() {
            Some(Capability::Signal(signal)) => Ok(signal.clone()),
            _ => Err("extension signal resource ended".into()),
        }
    }
}
