//! Concrete native handlers preserve independent guest edits and decisions.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::{
    context_reads::{Endpoint, Owner},
    local_resources::{Registry, SignalEndpoint},
};
use maestro_extensions::{ExtensionContext, ExtensionHandler, Local};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift, Resource};

/// Canonical encoded document, independently absent or failed.
type Encoded = Result<Option<String>, String>;
/// Exact canonical decision discriminant.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Decision {
    /// A returned document, including an encoding failure.
    #[component(name = "returned")]
    Returned(Encoded),
    /// A handler failure, not a returned value.
    #[component(name = "failed")]
    Failed(String),
}
/// Independent canonical event outcomes, as the guest returns them.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Returned {
    /// Edited event data or encoding failure.
    event: Encoded,
    /// Returned decision or handler failure.
    decision: Decision,
    /// Identity of the returned authored shell operations.
    operations: Option<Resource<super::Callback>>,
}
impl Returned {
    /// Takes ownership of returned operations before the native outcome leaves the Store.
    pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
        self,
        store: &mut T,
    ) -> wasmtime::Result<Outcome> {
        let operations = self
            .operations
            .map(|operations| crate::callback_imports::transferred(store, operations))
            .transpose()?;
        Ok(Outcome {
            event: self.event,
            decision: self.decision,
            operations,
        })
    }
}
/// Independent event outcomes, with returned operations under their retained identity.
pub(crate) struct Outcome {
    /// Edited event data or encoding failure.
    pub(crate) event: Encoded,
    /// Returned decision or handler failure.
    pub(crate) decision: Decision,
    /// Retained authored shell operations.
    pub(crate) operations: Option<Handle>,
}
/// Original operands retained until one asynchronous event call settles.
pub(crate) struct EventCall<H = Handle> {
    /// Actual authored callback and original admission.
    pub(crate) handler: H,
    /// Complete tagged event document.
    pub(crate) event: String,
    /// Actual native invocation context.
    pub(crate) context: Endpoint,
    /// Independently owned original cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Exact guest outcome, not an invented event union.
    pub(crate) reply: Reply<Outcome>,
}
/// The concrete event's JSON view, excluding native resource operands.
pub(crate) trait EventData: 'static {
    /// Encodes the original event with its canonical tag.
    fn encode(&self, name: &str) -> Result<String, String>;
    /// Applies a valid edit without replacing native resources.
    fn apply(&mut self, data: &str) -> Result<(), String>;
    /// Returns the original cancellation for capability transfer.
    fn signal(&self) -> Option<maestro_extensions::Cancellation> {
        None
    }
}
/// A concrete native result can retain returned shell operations.
pub(crate) trait EventResult: Sized + 'static {
    /// Decodes the concrete verdict, or ignores a notification's unused return value.
    fn decode(data: &str) -> Result<Self, String>;
    /// Transfers operations only into the concrete shell verdict.
    fn attach(&mut self, _operations: Option<super::bash::ReturnedBash>) {}
}

impl Handle {
    /// Adapts the actual native payload and result, never a JSON-only handler.
    pub(crate) fn event<V: EventData, R: EventResult>(
        &self,
        name: String,
        registry: Registry,
    ) -> ExtensionHandler<V, R, Local> {
        let handler = self.clone();
        Rc::new(move |event, context| {
            let handler = handler.clone();
            let registry = registry.clone();
            let name = name.clone();
            Box::pin(async move {
                handler
                    .invoke_event(event, context, &name, registry)
                    .await
                    .map_err(Into::into)
            })
        })
    }

    /// Encodes before admission and applies edits before decoding the separate verdict.
    async fn invoke_event<V: EventData, R: EventResult>(
        &self,
        event: &mut V,
        context: ExtensionContext,
        name: &str,
        registry: Registry,
    ) -> Result<Option<R>, String> {
        let data = event.encode(name)?;
        let signal = event
            .signal()
            .map(|signal| registry.signal(signal))
            .transpose()?;
        let context = registry.context(Owner::Base(context))?;
        let outcome = self
            .0
            .endpoint
            .call(&registry, |reply| {
                Call::Event(EventCall {
                    handler: self.clone(),
                    event: data,
                    context,
                    signal,
                    reply,
                })
            })
            .await?;
        Self::apply_outcome(event, outcome, registry)
    }

    /// Failed edits discard decisions; valid edits survive a separately failed decision.
    pub(crate) fn apply_outcome<V: EventData, R: EventResult>(
        event: &mut V,
        outcome: Outcome,
        registry: Registry,
    ) -> Result<Option<R>, String> {
        let operations = outcome
            .operations
            .map(|owner| super::bash::ReturnedBash::new(owner, registry));
        if let Some(data) = outcome.event? {
            event.apply(&data)?;
        }
        let decision = match outcome.decision {
            Decision::Returned(encoded) => encoded?,
            Decision::Failed(error) => return Err(error),
        };
        let mut result: Option<R> = decision.map(|data| R::decode(&data)).transpose()?;
        if let Some(result) = &mut result {
            result.attach(operations);
        }
        Ok(result)
    }
}
