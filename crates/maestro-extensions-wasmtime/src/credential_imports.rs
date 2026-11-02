//! Canonical credential imports delegate through the existing local resource pump.
use crate::{
    catalog_reads,
    credential_reads::{Endpoint, Request},
    read_imports::{
        ReadEndpoint, add_operand, add_read, add_routed, add_routed_operand, operand, resource,
    },
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs the independently retained credential resource and its catalog acquisition route.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::oauth_imports::add_to_linker(host)?;
    crate::context_imports::add_resource::<T, Endpoint>(host, "auth-storage")?;
    host.func_wrap(
        "[method]model-registry.auth-storage",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<catalog_reads::Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let result = endpoint
                .read(catalog_reads::Request::Credentials)
                .and_then(|owner| {
                    store
                        .data_mut()
                        .ctx()
                        .table
                        .push(owner)
                        .map_err(|error| error.to_string())
                });
            Ok((result,))
        },
    )?;
    add_callbacks(host)?;
    add_reads(host)?;
    add_mutations(host)?;
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

/// Reads the original accepted state and drains its own error queue.
fn add_reads<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_routed(
        host,
        "[method]auth-storage.list",
        Request::List,
        resource::<T, Endpoint>,
    )?;
    add_routed(
        host,
        "[method]auth-storage.get-all",
        Request::All,
        resource::<T, Endpoint>,
    )?;
    add_read(
        host,
        "[method]auth-storage.drain-errors",
        Request::Errors,
        resource::<T, Endpoint>,
    )?;
    add_routed_operand(
        host,
        "[method]auth-storage.get",
        Request::Get,
        operand::<T, Endpoint, String>,
    )?;
    host.func_wrap(
        "[method]auth-storage.get-auth-status",
        |mut store: StoreContextMut<'_, T>, (resource, provider): (Resource<Endpoint>, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Status(provider, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]auth-storage.has-auth",
        |mut store: StoreContextMut<'_, T>, (resource, provider): (Resource<Endpoint>, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::HasAuth(provider, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    add_operand(
        host,
        "[method]auth-storage.has",
        Request::Has,
        operand::<T, Endpoint, String>,
    )?;
    Ok(())
}

/// Preserves accepted-state-before-persistence behavior by calling the original owner.
fn add_mutations<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_routed(
        host,
        "[method]auth-storage.reload",
        Request::Reload,
        resource::<T, Endpoint>,
    )?;
    for (name, request) in [
        ("remove", Request::Remove as fn(_, _) -> _),
        ("logout", Request::Logout),
    ] {
        add_routed_operand(
            host,
            &format!("[method]auth-storage.{name}"),
            request,
            operand::<T, Endpoint, String>,
        )?;
    }
    add_operand(
        host,
        "[method]auth-storage.remove-runtime-api-key",
        Request::RemoveRuntime,
        operand::<T, Endpoint, String>,
    )?;
    host.func_wrap(
        "[method]auth-storage.set-runtime-api-key",
        |mut store: StoreContextMut<'_, T>,
         (resource, provider, key): (Resource<Endpoint>, String, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((endpoint.read(|reply| Request::Runtime(provider, key, reply)),))
        },
    )?;
    host.func_wrap(
        "[method]auth-storage.set",
        |mut store: StoreContextMut<'_, T>,
         (resource, provider, data): (Resource<Endpoint>, String, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Set(provider, data, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    Ok(())
}

/// Links authored backends and invocation-local native update operands.
fn add_callbacks<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, crate::local_resources::credential_scope::Endpoint>(
        host,
        "auth-lock-update",
    )?;
    host.func_wrap("[method]auth-storage.set-fallback-resolver", |mut store: StoreContextMut<'_, T>, (resource, handler): (Resource<Endpoint>, Resource<crate::guest_calls::Callback>)| {
        let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
        let handler = crate::callback_imports::retain(store.data_mut(), &handler)?;
        let requests = store.data().api().0.requests().clone();
        let route = endpoint.route(|reply| Request::Fallback(handler, requests, reply));
        Ok((crate::import_route::push(store.data_mut(), route)?,))
    })?;
    host.func_wrap(
        "[method]auth-lock-update.update",
        |mut store: StoreContextMut<'_, T>,
         (resource, current): (
            Resource<crate::local_resources::credential_scope::Endpoint>,
            Option<String>,
        )| {
            let update = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((update.update(current),))
        },
    )?;
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
