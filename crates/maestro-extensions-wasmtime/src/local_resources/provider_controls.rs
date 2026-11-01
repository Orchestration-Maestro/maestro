//! Native options and hooks stay on their original local owner until final release.
use super::{Capability, Lease, Registry, pump::State};
use crate::{
    guest_calls::{Call, Handle},
    model_calls::{Phase, Selection},
    provider_calls::{Input, Invocation},
};
use maestro_extensions::{
    Context, Local, Model, ProviderExecution, ProviderSpawner, StreamOptionsInput, Threaded,
};
use std::rc::Rc;
use std::sync::{Arc, mpsc::Sender};

/// Original invocation representations, including opaque objects, fetch and admission.
pub(crate) enum Owner {
    /// Caller-local native controls and admission.
    Local(Option<StreamOptionsInput<Local>>, ProviderSpawner<Local>),
    /// Thread-safe native controls and optional provider admission.
    Threaded(
        Option<StreamOptionsInput>,
        Option<ProviderSpawner<Threaded>>,
    ),
}
/// Send setup from a threaded caller outside the original pump.
pub(crate) struct Creation {
    /// Original registered callback.
    pub(crate) handler: Handle,
    /// Supplied descriptor.
    pub(crate) model: Model,
    /// Supplied conversation.
    pub(crate) context: Context,
    /// Original raw or simple options.
    pub(crate) options: Option<StreamOptionsInput>,
    /// Original threaded admission.
    pub(crate) submit: Option<ProviderSpawner<Threaded>>,
    /// Original synchronous native completion.
    pub(crate) reply: Sender<Result<maestro_extensions::AssistantMessageEventStream, String>>,
}
impl Creation {
    /// Setup runs on the sole existing pump, not a replacement executor.
    pub(super) fn answer(self, registry: &Registry) {
        let result = registry.provider_start(
            &self.handler,
            self.model,
            &self.context,
            Owner::Threaded(self.options, self.submit),
        );
        let _ = self.reply.send(result);
    }
}
/// Retained native control lease, safe to lend to the Store.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
/// Awaited native hook request, retaining caller-supplied signal bindings.
pub(crate) struct Request {
    /// Native hook phase.
    pub(crate) phase: Phase,
    /// Ordinary payload or response data.
    pub(crate) data: String,
    /// Original descriptor.
    pub(crate) model: String,
    /// Selected input cancellation.
    pub(crate) signal: Option<super::SignalEndpoint>,
    /// Selected author signal binding.
    pub(crate) owner: Option<Handle>,
    /// Independent result, settled by the existing pending-work owner.
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<Selection, String>>,
}
impl Registry {
    /// Invoke the original registration synchronously, then front its original returned resource.
    pub(crate) fn provider_start(
        &self,
        handler: &Handle,
        model: Model,
        context: &Context,
        owner: Owner,
    ) -> Result<maestro_extensions::AssistantMessageEventStream, String> {
        let (data, signal, payload, response) = match &owner {
            Owner::Local(options, _) => encode(&model, context, options.as_ref()),
            Owner::Threaded(options, _) => encode(&model, context, options.as_ref()),
        }?;
        let input = Input {
            controls: Endpoint(
                self.insert(Capability::ProviderControls(Box::new(owner), Rc::default()))?,
            ),
            signal: signal.map(|signal| self.signal(signal)).transpose()?,
            payload,
            response,
        };
        let stream = handler.endpoint().synchronous(self, |reply| {
            Ok(Call::Provider(Invocation {
                handler: handler.clone(),
                data,
                input,
                reply,
            }))
        })?;
        Ok(crate::imported_assistant::stream(
            stream,
            model,
            self.requests()?,
        ))
    }
}
/// Keep option absence and raw/simple policy distinct while encoding only data slots.
fn encode<E: ProviderExecution>(
    model: &Model,
    context: &Context,
    options: Option<&StreamOptionsInput<E>>,
) -> Result<(String, Option<maestro_extensions::Cancellation>, bool, bool), String> {
    let common = options.map(StreamOptionsInput::common);
    let data = options.map(|options| match options {
        StreamOptionsInput::Raw(raw) => serde_json::json!((
            false,
            &raw.common,
            &raw.extra,
            Option::<u8>::None,
            Option::<u8>::None
        )),
        StreamOptionsInput::Simple(simple) => serde_json::json!((
            true,
            &simple.common,
            serde_json::json!({}),
            &simple.reasoning,
            &simple.thinking_budgets
        )),
    });
    Ok((
        serde_json::to_string(&(model, context, data)).map_err(|e| e.to_string())?,
        common.and_then(|c| c.signal.clone()),
        common.is_some_and(|c| c.on_payload.is_some()),
        common.is_some_and(|c| c.on_response.is_some()),
    ))
}
impl State {
    /// Native callbacks run outside capability storage borrows and under their original work owner.
    pub(super) fn provider_hook(&self, id: u64, request: Request, admission: super::Admission) {
        let capability = self.registry.capability(id);
        let registry = self.registry.clone();
        self.stage(Box::pin(async move {
            let result = answer(&registry, capability.as_deref(), &request).await;
            let _ = request.reply.send(result);
            drop(admission);
        }));
    }
}
/// Await the exact supplied hook family without changing the provider's phase policy.
async fn invoke<E: ProviderExecution>(
    registry: &Registry,
    options: Option<&StreamOptionsInput<E>>,
    request: &Request,
) -> Result<Selection, String> {
    let common = options
        .map(StreamOptionsInput::common)
        .ok_or("extension provider options absent")?;
    let model = Arc::new(crate::writer_reads::decode(&request.model)?);
    match request.phase {
        Phase::Payload => {
            let input = maestro_extensions::PayloadHookInput {
                payload: crate::writer_reads::decode(&request.data)?,
                signal: request
                    .signal
                    .as_ref()
                    .map(|signal| registry.model_signal(signal))
                    .transpose()?,
            };
            let selected = common
                .on_payload
                .as_ref()
                .ok_or("extension payload hook absent")?(input, model)
            .await
            .map_err(|e| e.message)?;
            Ok(Selection {
                data: serde_json::to_string(&selected.payload).map_err(|e| e.to_string())?,
                signal: selected
                    .signal
                    .map(|signal| registry.signal(signal))
                    .transpose()?,
                owner: None,
            })
        }
        Phase::Response => {
            common
                .on_response
                .as_ref()
                .ok_or("extension response hook absent")?(
                crate::writer_reads::decode(&request.data)?,
                model,
            )
            .await
            .map_err(|e| e.message)?;
            Ok(Selection {
                data: String::new(),
                signal: None,
                owner: None,
            })
        }
    }
}

/// Provider operations are routed by the existing pump, never by a separate receiver.
pub(crate) enum Command {
    /// A thread-safe caller's original synchronous setup.
    Start(Box<Creation>),
    /// One awaited native hook with independent admission.
    Hook(u64, Request, super::Admission),
}
impl State {
    /// Dispatch provider work on the original local request owner.
    pub(super) fn provider_request(&self, command: Command) {
        match command {
            Command::Start(creation) => creation.answer(&self.registry),
            Command::Hook(id, request, admission) => self.provider_hook(id, request, admission),
        }
    }
}
/// Retain selected signal storage before awaiting the exact supplied hook family.
async fn answer(
    registry: &Registry,
    capability: Option<&Capability>,
    request: &Request,
) -> Result<Selection, String> {
    let Some(Capability::ProviderControls(owner, scopes)) = capability else {
        return Err("extension provider controls ended".into());
    };
    scopes.borrow_mut().extend(request.owner.clone());
    match owner.as_ref() {
        Owner::Local(options, submit) => {
            let _ = submit;
            invoke(registry, options.as_ref(), request).await
        }
        Owner::Threaded(options, submit) => {
            let _ = submit;
            invoke(registry, options.as_ref(), request).await
        }
    }
}
