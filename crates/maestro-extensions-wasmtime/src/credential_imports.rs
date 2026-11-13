//! Canonical credential imports delegate through the existing local resource pump.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::{HostAuthLockUpdate, HostAuthStorage},
    callback_imports::retain,
    context_imports::add_resource,
    credential_reads::Endpoint,
    guest_calls::Callback,
    import_route::{Route, push},
    lifecycle::State,
    read_imports::{endpoint, forwards, reads, release, routes},
};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

impl HostAuthStorage for State {
    reads! { Endpoint;
        set_runtime_api_key(provider: String, key: String) -> ();
        remove_runtime_api_key(provider: String) -> ();
        drain_errors() -> String;
    }

    routes! { Endpoint;
        reload();
        get(provider: String);
        set(provider: String, credential: String);
        remove(provider: String);
        list();
        has(provider: String);
        has_auth(provider: String);
        get_auth_status(provider: String);
        get_all();
        logout(provider: String);
    }

    fn get_oauth_providers(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Vec<Resource<crate::oauth_reads::Endpoint>>, String>> {
        let providers = endpoint(self, &this)?.get_oauth_providers();
        Ok(providers.and_then(|providers| {
            providers
                .into_iter()
                .map(|provider| {
                    self.ctx()
                        .table
                        .push(provider)
                        .map_err(|error| error.to_string())
                })
                .collect()
        }))
    }

    fn set_fallback_resolver(
        &mut self,
        this: Resource<Endpoint>,
        resolver: Resource<Callback>,
    ) -> wasmtime::Result<Resource<Route>> {
        let credentials = endpoint(self, &this)?;
        let handler = retain(self, &resolver)?;
        let requests = self.api().0.requests().clone();
        let route = credentials.set_fallback_resolver(handler, requests);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every synchronous credential method to its trait implementation.
pub(crate) fn add_storage(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "auth-storage")?;
    forwards! { host, "auth-storage", HostAuthStorage, Endpoint,
        "get-oauth-providers" => get_oauth_providers();
        "set-fallback-resolver" => set_fallback_resolver(resolver: Resource<Callback>);
        "set-runtime-api-key" => set_runtime_api_key(provider: String, key: String);
        "remove-runtime-api-key" => remove_runtime_api_key(provider: String);
        "reload" => reload();
        "get" => get(provider: String);
        "set" => set(provider: String, credential: String);
        "remove" => remove(provider: String);
        "list" => list();
        "has" => has(provider: String);
        "has-auth" => has_auth(provider: String);
        "get-auth-status" => get_auth_status(provider: String);
        "get-all" => get_all();
        "drain-errors" => drain_errors();
        "logout" => logout(provider: String);
    }
    Ok(())
}

/// Installs the independently retained credential resource's asynchronous key read and its
/// invocation-local update operands.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::oauth_imports::add_to_linker(host)?;
    add_callbacks(host)?;
    host.func_wrap_concurrent(
        "[method]auth-storage.get-api-key",
        |accessor, (resource, provider, include): (Resource<Endpoint>, String, Option<bool>)| {
            Box::pin(async move {
                let endpoint = accessor.with(|mut access| {
                    access
                        .get()
                        .ctx()
                        .table
                        .get(&resource)
                        .cloned()
                        .map_err(|error| error.to_string())
                });
                let result = match endpoint {
                    Ok(endpoint) => endpoint.key(provider, include).await,
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )
}

impl HostAuthLockUpdate for State {
    reads! { crate::local_resources::credential_scope::Endpoint;
        update(current: Option<String>) -> Option<String>;
    }

    fn drop(
        &mut self,
        this: Resource<crate::local_resources::credential_scope::Endpoint>,
    ) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Registers the borrowed lock update with its synchronous method forwarded to the trait.
pub(crate) fn add_lock_update(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, crate::local_resources::credential_scope::Endpoint>(
        host,
        "auth-lock-update",
    )?;
    forwards! { host, "auth-lock-update", HostAuthLockUpdate, crate::local_resources::credential_scope::Endpoint,
        "update" => update(current: Option<String>);
    }
    Ok(())
}

/// Links authored backends and invocation-local native update operands.
fn add_callbacks<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]auth-lock-update.update-async",
        |accessor,
         (resource, current): (
            Resource<crate::local_resources::credential_scope::Endpoint>,
            Option<String>,
        )| {
            Box::pin(async move {
                let update =
                    accessor.with(|mut access| access.get().ctx().table.get(&resource).cloned())?;
                Ok((update.update_async(current).await,))
            })
        },
    )?;
    Ok(())
}
