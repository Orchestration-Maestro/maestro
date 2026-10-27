//! Nested custom callbacks use the same prefix and owned continuation bodies.
use super::{Callback, LocalCall, LocalResult};
use crate::guest_calls::custom::{Invocation, WireInput};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Transferred operands of one nested synchronous prefix.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct Prefix {
    /// Original callback identity.
    target: u32,
    /// Original terminal, theme, keybindings and close operation.
    input: WireInput,
    /// Original borrowed invocation frame.
    emitter: Resource<EmitterEndpoint>,
}
/// Transfers only the original invocation's admitted operands.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Invocation<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Invocation::Prefix(call) => LocalCall::CustomFactory(Prefix {
            target: call.handler.id,
            input: call.input.transfer(table)?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Invocation::Future(handler, _) => LocalCall::CustomFuture(handler.id),
    })
}
/// Settles a nested custom prefix or continuation with the actual retained resources.
pub(super) fn settle<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Invocation<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Invocation::Prefix(call), LocalResult::CustomFactory(value)) => {
            let value = match value {
                Ok(built) => Ok(crate::guest_exports::custom::retained(store, built)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Invocation::Future(_, reply), LocalResult::Factory(value)) => {
            let value = match value {
                Ok(built) => Ok(crate::guest_exports::retained(store, built)?),
                Err(error) => Err(error),
            };
            let _ = reply.send(value);
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension nested custom result kind mismatch"
            ));
        }
    }
    Ok(())
}
