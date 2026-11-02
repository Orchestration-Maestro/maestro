//! Nested credential callbacks retain their original update capability.
use super::{Call, Callback, LocalCall};
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Owned operands of one nested credential backend method.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalBackend {
    /// Authored backend identity.
    target: u32,
    /// Invocation-local native update.
    update: Resource<crate::local_resources::credential_scope::Endpoint>,
    /// Whether the author must await its update.
    asynchronous: bool,
}
/// Owned operands of one nested fallback lookup.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalFallback {
    /// Authored resolver identity.
    target: u32,
    /// Literal provider.
    provider: String,
}

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

/// Delivers a nested credential or OAuth callback's original result.
pub(super) fn settle(call: Call<Callback>, value: super::LocalResult) -> wasmtime::Result<()> {
    use super::LocalResult;
    match (call, value) {
        (Call::OAuth(call), LocalResult::OAuth(value)) => {
            let _ = call.reply.send(value);
        }
        (
            Call::Credential(crate::guest_calls::credentials::CredentialCall::Backend(call)),
            LocalResult::Unit(value),
        ) => {
            let _ = call.reply.send(value);
        }
        (
            Call::Credential(crate::guest_calls::credentials::CredentialCall::Fallback(call)),
            LocalResult::AuthFallback(value),
        ) => {
            let _ = call.reply.send(value);
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension route result kind mismatch"
            ));
        }
    }
    Ok(())
}

/// Owned input for a nested synchronous OAuth notification.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalOAuth {
    /// Original authored identity.
    target: u32,
    /// Native-selected interaction form.
    phase: crate::oauth_callbacks::Phase,
    /// Exact supplied record or progress text.
    data: String,
    /// Original native interaction for an authored provider login.
    interaction: Option<Resource<crate::oauth_interaction::Interaction>>,
}

/// Marshals the original OAuth identity and optional native interaction.
pub(super) fn oauth(
    table: &mut ResourceTable,
    call: &crate::oauth_callbacks::Invocation<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(LocalCall::OAuth(LocalOAuth {
        target: call.handler.id,
        phase: call.phase,
        data: call.data.clone(),
        interaction: call
            .interaction
            .clone()
            .map(|interaction| table.push(interaction))
            .transpose()?,
    }))
}
