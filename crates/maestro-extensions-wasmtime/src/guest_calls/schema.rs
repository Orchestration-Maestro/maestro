//! Native schema callbacks retain their originating component admission.
use super::{Call, Handle};
use crate::{
    lifecycle::replies::Reply,
    local_resources::{Registry, effects::EmitterEndpoint},
    schema_wire::{Error, Operation},
};
use maestro_extensions::{DiagnosticErrorInfo, EventBus, Local};
use serde::de::DeserializeOwned;
/// A schema callback invocation retaining its actual operand capability.
pub(crate) struct Invocation<H = Handle> {
    /// The original retained callback identity.
    pub(crate) handler: H,
    /// The single author method requested by the checker.
    pub(crate) operation: Operation,
    /// The current borrowed native operand.
    pub(crate) node: crate::schema_reads::Endpoint,
    /// The original callback-scoped emission capability.
    pub(crate) emitter: EmitterEndpoint,
    /// The original lifecycle-owned schema result.
    pub(crate) reply: Reply<Result<String, Error>>,
}
impl Handle {
    /// Invoke one author schema method against the borrowed native operand.
    pub(crate) fn schema_call<R: DeserializeOwned>(
        &self,
        registry: &Registry,
        events: &EventBus<Local>,
        operation: Operation,
        value: &mut serde_json::Value,
    ) -> Result<R, DiagnosticErrorInfo> {
        #[cfg(test)]
        eprintln!("native schema start {}", self.id());
        let result = events
            .invoke_callback(|emitter| {
                let emitter = registry.emitter(emitter.frame())?;
                self.0
                    .endpoint
                    .replies
                    .synchronous_with(registry, Some(value), |reply, node| {
                        registry.dispatch(
                            &self.0.endpoint,
                            Call::Schema(Invocation {
                                handler: self.clone(),
                                operation,
                                node,
                                emitter,
                                reply,
                            }),
                        )
                    })
            })
            .map_err(DiagnosticErrorInfo::error)?
            .map_err(DiagnosticErrorInfo::from)
            .and_then(|text| {
                maestro_extensions::schema_transport::decode_json(&text)
                    .map_err(DiagnosticErrorInfo::error)
            });
        #[cfg(test)]
        eprintln!("native schema end {}", self.id());
        result
    }
}
