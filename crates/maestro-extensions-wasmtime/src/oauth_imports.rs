//! Canonical OAuth resources use the existing local request pump.
use crate::{
    oauth_reads::{Endpoint, Request},
    read_imports::{ReadEndpoint, add_read, resource},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Links native provider metadata, callable key conversion and credential enumeration.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "oauth-provider")?;
    add_read(
        host,
        "[method]oauth-provider.info",
        Request::Info,
        resource::<T, Endpoint>,
    )?;
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
    host.func_wrap(
        "[method]oauth-provider.modify-models",
        |mut store: StoreContextMut<'_, T>, (resource, data): (Resource<Endpoint>, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Models(data, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]oauth-provider.get-api-key",
        |mut store: StoreContextMut<'_, T>, (resource, data): (Resource<Endpoint>, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Key(data, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    crate::oauth_interaction::add_to_linker(host)?;
    login(host)?;
    credential_login(host)?;
    enumeration(host)
}

/// Transfers the native owner's ordered implementations as independently owned resources.
fn enumeration<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]auth-storage.get-oauth-providers",
        |mut store: StoreContextMut<'_, T>,
         (resource,): (Resource<crate::credential_reads::Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let result = endpoint
                .read(crate::credential_reads::Request::Providers)
                .and_then(|providers| {
                    providers
                        .into_iter()
                        .map(|provider| {
                            store
                                .data_mut()
                                .ctx()
                                .table
                                .push(provider)
                                .map_err(|error| error.to_string())
                        })
                        .collect::<Result<Vec<_>, _>>()
                });
            Ok((result,))
        },
    )
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
