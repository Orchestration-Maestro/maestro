//! Canonical OAuth resources use the existing local request pump.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostOauthProvider,
    context_imports::add_resource,
    lifecycle::State,
    oauth_reads::Endpoint,
    read_imports::{forwards, reads, release, routes},
};
use wasmtime::component::{LinkerInstance, Resource};

impl HostOauthProvider for State {
    reads! { Endpoint;
        info() -> String;
    }

    routes! { Endpoint;
        get_api_key(credentials: String);
        modify_models(data: String);
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Registers the provider resource with its synchronous methods forwarded to the trait.
pub(crate) fn add_provider(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "oauth-provider")?;
    forwards! { host, "oauth-provider", HostOauthProvider, Endpoint,
        "info" => info();
        "get-api-key" => get_api_key(credentials: String);
        "modify-models" => modify_models(data: String);
    }
    Ok(())
}

/// Links native provider metadata and callable key conversion.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]oauth-provider.refresh-token",
        |accessor, (resource, data): (Resource<Endpoint>, String)| {
            Box::pin(async move {
                let endpoint =
                    accessor.with(|mut access| access.get().ctx().table.get(&resource).cloned())?;
                Ok((endpoint.refresh(data).await,))
            })
        },
    )?;
    crate::oauth_interaction::add_to_linker(host)?;
    login(host)?;
    credential_login(host)
}

/// Canonical login operands retain their typed callback and cancellation capabilities.
type LoginInput = (
    Resource<Endpoint>,
    Resource<crate::guest_calls::Callback>,
    (bool, bool, bool),
    Option<Resource<crate::local_resources::SignalEndpoint>>,
);

/// Retains interaction before releasing Store access, then uses the existing local pump.
fn login<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]oauth-provider.login",
        |accessor, (provider, handler, options, signal): LoginInput| {
            Box::pin(async move {
                let (endpoint, controls) = accessor.with(|mut access| {
                    let state = access.get();
                    let endpoint = state.ctx().table.get(&provider)?.clone();
                    let controls = controls(state, &handler, options, signal)?;
                    Ok::<_, wasmtime::Error>((endpoint, controls))
                })?;
                Ok((endpoint.login(controls).await,))
            })
        },
    )
}

/// Canonical credential login operands keep provider selection separate from interaction.
type CredentialLoginInput = (
    Resource<crate::credential_reads::Endpoint>,
    String,
    Resource<crate::guest_calls::Callback>,
    (bool, bool, bool),
    Option<Resource<crate::local_resources::SignalEndpoint>>,
);

/// Retains interaction before native credentials select a provider and persist its result.
fn credential_login<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]auth-storage.login",
        |accessor, (owner, provider, handler, options, signal): CredentialLoginInput| {
            Box::pin(async move {
                let (endpoint, controls) = accessor.with(|mut access| {
                    let state = access.get();
                    let endpoint = state.ctx().table.get(&owner)?.clone();
                    let controls = controls(state, &handler, options, signal)?;
                    Ok::<_, wasmtime::Error>((endpoint, controls))
                })?;
                Ok((endpoint.login(provider, controls).await,))
            })
        },
    )
}

/// Uses the same callback identity and live signal adaptation for both login entry points.
fn controls<T: crate::callback_imports::CallbackView>(
    state: &mut T,
    handler: &Resource<crate::guest_calls::Callback>,
    options: (bool, bool, bool),
    signal: Option<Resource<crate::local_resources::SignalEndpoint>>,
) -> wasmtime::Result<crate::oauth_callbacks::Controls> {
    let handler = crate::callback_imports::retain(state, handler)?;
    let signal = signal
        .map(|signal| state.ctx().table.get(&signal).cloned())
        .transpose()?;
    Ok(crate::oauth_callbacks::Controls {
        handler,
        options,
        signal,
    })
}
