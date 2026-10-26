//! Canonical credential imports delegate through the existing local resource pump.
use crate::{
    catalog_reads,
    credential_reads::{Endpoint, Request},
    read_imports::{ReadEndpoint, add_operand, add_read, operand, resource},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs the independently retained credential resource and its catalog acquisition route.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
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
    add_read(
        host,
        "[method]auth-storage.list",
        Request::List,
        resource::<T, Endpoint>,
    )?;
    add_read(
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
    add_operand(
        host,
        "[method]auth-storage.get",
        Request::Get,
        operand::<T, Endpoint, String>,
    )?;
    add_operand(
        host,
        "[method]auth-storage.get-auth-status",
        Request::Status,
        operand::<T, Endpoint, String>,
    )?;
    for (name, request) in [
        ("has", Request::Has as fn(_, _) -> _),
        ("has-auth", Request::HasAuth),
    ] {
        add_operand(
            host,
            &format!("[method]auth-storage.{name}"),
            request,
            operand::<T, Endpoint, String>,
        )?;
    }
    Ok(())
}

/// Preserves accepted-state-before-persistence behavior by calling the original owner.
fn add_mutations<T: WasiView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    add_read(
        host,
        "[method]auth-storage.reload",
        Request::Reload,
        resource::<T, Endpoint>,
    )?;
    for (name, request) in [
        (
            "remove-runtime-api-key",
            Request::RemoveRuntime as fn(_, _) -> _,
        ),
        ("remove", Request::Remove),
        ("logout", Request::Logout),
    ] {
        add_operand(
            host,
            &format!("[method]auth-storage.{name}"),
            request,
            operand::<T, Endpoint, String>,
        )?;
    }
    for (name, request) in [
        ("set-runtime-api-key", Request::Runtime as fn(_, _, _) -> _),
        ("set", Request::Set),
    ] {
        host.func_wrap(
            &format!("[method]auth-storage.{name}"),
            move |mut store: StoreContextMut<'_, T>,
                  (resource, provider, data): (Resource<Endpoint>, String, String)| {
                let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
                Ok((endpoint.read(|reply| request(provider, data, reply)),))
            },
        )?;
    }
    Ok(())
}
