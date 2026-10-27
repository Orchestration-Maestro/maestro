//! Awaited model hooks retain their original guest identities through native producer completion.
use crate::{
    guest_calls::{Call, Callback, Handle},
    lifecycle::replies::Reply,
    local_resources::{Registry, SignalEndpoint},
};
use futures_util::TryFutureExt;
use maestro_extensions::{
    DiagnosticErrorInfo, Local, Model, PayloadHookInput, ProviderResponse, StreamOptions,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasmtime::component::{ComponentType, Lift, Lower, Resource};

/// Callback ownership for this invocation, released when its producer and supplied hooks end.
pub(crate) type Scopes = Rc<RefCell<Vec<Handle>>>;
/// Actual native phase; the protocol keeps the decision about when to call it.
#[derive(Clone, Copy, ComponentType, Lift, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Phase {
    /// Select request parameters.
    #[component(name = "payload")]
    Payload,
    /// Observe response metadata.
    #[component(name = "response")]
    Response,
}
/// Non-data controls supplied by the guest's synchronous invocation.
#[derive(Clone)]
pub(crate) struct Controls {
    /// Original arbitrary guest cancellation forwarded to native ownership.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Original payload selector.
    pub(crate) payload: Option<Handle>,
    /// Original response observer.
    pub(crate) response: Option<Handle>,
    /// Retained opaque author values and signal bindings.
    pub(crate) objects: Option<Handle>,
}
impl Controls {
    /// Keep caller-local bindings alive even when all guest observers are dropped.
    pub(crate) fn scopes(&self) -> Scopes {
        Rc::new(RefCell::new(
            [self.objects.clone()].into_iter().flatten().collect(),
        ))
    }
    /// Adapt only capability slots; ordinary data is decoded by the shared record owner.
    pub(crate) fn apply(
        &self,
        registry: &Registry,
        scopes: &Scopes,
        common: &mut StreamOptions<Local>,
    ) -> Result<(), String> {
        common.signal = self
            .signal
            .as_ref()
            .map(|signal| registry.model_signal(signal))
            .transpose()?;
        if let Some(handler) = self.payload.clone() {
            let (registry, scopes) = (registry.clone(), scopes.clone());
            common.on_payload = Some(Rc::new(move |input, model| {
                Box::pin(
                    payload(
                        handler.clone(),
                        registry.clone(),
                        scopes.clone(),
                        input,
                        model,
                    )
                    .map_err(DiagnosticErrorInfo::error),
                )
            }));
        }
        if let Some(handler) = self.response.clone() {
            common.on_response = Some(Rc::new(move |response, model| {
                Box::pin(observe_response(handler.clone(), response, model))
            }));
        }

        Ok(())
    }
}
/// Invoke the original payload hook and retain the selected cancellation binding for this producer.
async fn payload(
    handler: Handle,
    registry: Registry,
    scopes: Scopes,
    input: PayloadHookInput,
    model: Arc<Model>,
) -> Result<PayloadHookInput, String> {
    let data = serde_json::to_string(&input.payload).map_err(|e| e.to_string())?;
    let signal = input
        .signal
        .map(|signal| registry.signal(signal))
        .transpose()?;
    let selected = invoke(
        &handler,
        Phase::Payload,
        Input {
            data,
            model,
            signal,
        },
    )
    .await?;
    if let Some(owner) = selected.owner {
        scopes.borrow_mut().push(owner);
    }
    Ok(PayloadHookInput {
        payload: crate::writer_reads::decode(&selected.data)?,
        signal: selected
            .signal
            .as_ref()
            .map(|signal| registry.model_signal(signal))
            .transpose()?,
    })
}
/// Await the response observer without moving protocol body-consumption policy into the adapter.
async fn observe_response(
    handler: Handle,
    response: ProviderResponse,
    model: Arc<Model>,
) -> Result<(), DiagnosticErrorInfo> {
    let data =
        serde_json::to_string(&response).map_err(|e| DiagnosticErrorInfo::error(e.to_string()))?;
    invoke(
        &handler,
        Phase::Response,
        Input {
            data,
            model,
            signal: None,
        },
    )
    .await
    .map(|_| ())
    .map_err(DiagnosticErrorInfo::error)
}
/// One awaited authored hook invocation, using the original lifecycle reply owner.
pub(crate) struct Invocation<H = Handle> {
    /// Retained originating closure.
    pub(crate) handler: H,
    /// Selected native hook phase.
    pub(crate) phase: Phase,
    /// Ordinary input JSON, without capability identifiers.
    pub(crate) data: String,
    /// Original descriptor passed by the provider.
    pub(crate) model: String,
    /// Independently owned optional cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Original invocation's result.
    pub(crate) reply: Reply<Selection>,
}
/// Payload returned from the guest; response observers use empty data and no signal.
pub(crate) struct Selection {
    /// Exact selected payload encoding, including falsy JSON values.
    pub(crate) data: String,
    /// Selected cancellation with independent resource ownership.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Keeps an arbitrary selected guest signal subscribed until its producer ends.
    pub(crate) owner: Option<Handle>,
}
/// The canonical owned hook result.
#[derive(ComponentType, Lift, Lower)]
#[component(record)]
pub(crate) struct WireSelection {
    /// Selected ordinary JSON.
    pub(crate) data: String,
    /// Original or replaced native cancellation endpoint.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
    /// Operation-scoped callback identity retaining the guest signal binding.
    pub(crate) owner: Option<Resource<Callback>>,
}
/// Owned data and cancellation for one awaited hook.
struct Input {
    /// Ordinary JSON parameters or response metadata.
    data: String,
    /// Original supplied model descriptor.
    model: Arc<Model>,
    /// Independent optional cancellation.
    signal: Option<SignalEndpoint>,
}
/// Await an actual export on the original Store; no callback-table borrow crosses this await.
async fn invoke(handler: &Handle, phase: Phase, input: Input) -> Result<Selection, String> {
    let model = serde_json::to_string(input.model.as_ref()).map_err(|e| e.to_string())?;
    handler
        .endpoint()
        .submit(|reply| {
            Call::ModelHook(Invocation {
                handler: handler.clone(),
                phase,
                data: input.data,
                model,
                signal: input.signal,
                reply,
            })
        })
        .await
}
