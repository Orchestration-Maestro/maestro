//! Native terminal input reaches its authored listener through the original invocation frame.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{Registry, effects::EmitterEndpoint};
use maestro_extensions::{
    EventError, ExtensionAPI, InputListenerResult, Local, TerminalInputHandler,
};
use wasmtime::component::{ComponentType, Lift};

/// What the listener decided; absent members leave the input as it is.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Decision {
    /// Stops the input when true.
    consume: Option<bool>,
    /// Replaces the input passed on.
    data: Option<String>,
}

/// The native result of a decision: consumption wins over replacement, and no decision or
/// absent members pass the input on.
fn decided(decision: Option<Decision>) -> InputListenerResult {
    match decision {
        Some(Decision {
            consume: Some(true),
            ..
        }) => InputListenerResult::Consume,
        Some(Decision {
            data: Some(data), ..
        }) => InputListenerResult::Replace(data),
        _ => InputListenerResult::Pass,
    }
}

/// One chunk of terminal input for an authored listener.
pub(crate) struct Input<H = Handle> {
    /// Retains the authored closure and its admission.
    pub(crate) handler: H,
    /// The literal chunk.
    pub(crate) data: String,
    /// Original native bus and deferred deliveries.
    pub(crate) emitter: EmitterEndpoint,
    /// Returns the listener's decision.
    pub(crate) reply: Reply<Option<Decision>>,
}

impl Handle {
    /// A native terminal input listener bound to the original extension's callback frame.
    pub(crate) fn terminal_input(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
    ) -> TerminalInputHandler {
        api.ui_callback(move |data: String, emitter| {
            let emitter = registry.emitter(emitter.frame())?;
            self.decide(&registry, emitter, data)
                .map_err(EventError::from)
        })
    }

    /// Asks the authored listener about one chunk within a native callback's own frame.
    pub(crate) fn decide(
        &self,
        registry: &Registry,
        emitter: EmitterEndpoint,
        data: String,
    ) -> Result<InputListenerResult, String> {
        let decision = self.0.endpoint.synchronous(registry, |reply| {
            Ok(Call::TerminalInput(Input {
                handler: self.clone(),
                data,
                emitter,
                reply,
            }))
        });
        decision.map(decided)
    }
}
