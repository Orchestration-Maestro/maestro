//! Nested registered provider setups and returned-stream operations.
use super::{Call, Callback, LocalCall};
use wasmtime::component::ResourceTable;

/// Marshals one registered provider setup or returned-stream operation.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Call<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::Provider(call) => LocalCall::Provider(crate::provider_calls::LocalProvider {
            target: call.handler.id,
            data: call.data.clone(),
            input: call.input.lower(table)?,
        }),
        Call::Assistant(call) => LocalCall::Assistant(crate::provider_calls::LocalAssistant {
            target: call.handler.id,
            operation: call.operation,
            data: call.data.clone(),
            signal: call
                .signal
                .clone()
                .map(|signal| table.push(signal))
                .transpose()?,
        }),
        _ => return Err(wasmtime::format_err!("expected a provider call")),
    })
}
