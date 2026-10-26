//! Nested credential callbacks retain their original update capability.
use super::{Callback, LocalBackend, LocalCall, LocalFallback};
use wasmtime::component::ResourceTable;

/// Marshals one credential callback with its original update capability.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &crate::guest_calls::credentials::CredentialCall<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        crate::guest_calls::credentials::CredentialCall::Backend(call) => {
            LocalCall::AuthBackend(LocalBackend {
                target: call.handler.id,
                update: table.push(call.update.clone())?,
                asynchronous: call.asynchronous,
            })
        }
        crate::guest_calls::credentials::CredentialCall::Fallback(call) => {
            LocalCall::AuthFallback(LocalFallback {
                target: call.handler.id,
                provider: call.provider.clone(),
            })
        }
    })
}
