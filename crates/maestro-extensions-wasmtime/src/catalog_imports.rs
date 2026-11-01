//! Canonical catalog imports preserve current descriptors and exact lookup operands.
use crate::read_imports::{
    ReadEndpoint, add_operand, add_read, add_routed, add_routed_operand, operand, resource,
};
use crate::{
    catalog_reads::{Endpoint, Request},
    context_imports::add_resource,
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};

/// Defines catalog reads and credential resolution through the original local owner.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "model-registry")?;
    add_metadata(host)?;
    add_mutations(host)?;
    add_provider_key(host)?;
    add_read(
        host,
        "[method]model-registry.get-all",
        Request::All,
        resource::<T, Endpoint>,
    )?;
    add_routed(
        host,
        "[method]model-registry.get-available",
        Request::Available,
        resource::<T, Endpoint>,
    )?;
    host.func_wrap(
        "[method]model-registry.find",
        |mut store: StoreContextMut<'_, T>,
         (resource, provider, id): (Resource<Endpoint>, String, String)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((endpoint.read(|reply| Request::Find(provider, id, reply)),))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]model-registry.get-api-key-and-headers",
        |accessor, (resource, model): (Resource<Endpoint>, String)| {
            Box::pin(async move {
                let endpoint = accessor.with(|mut access| {
                    Ok::<_, wasmtime::Error>(access.get().ctx().table.get(&resource)?.clone())
                })?;
                let result = match crate::writer_reads::decode(&model) {
                    Ok(model) => endpoint.auth(model).await.map(ResolvedRequestAuth::from),
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )?;
    Ok(())
}

/// Canonical authentication retains optional key, endpoint and header values.
#[derive(ComponentType, Lower)]
#[component(record)]
struct RequestAuth {
    /// Selected key, absent when no source supplies one.
    #[component(name = "api-key")]
    api_key: Option<String>,
    /// Resolved provider endpoint.
    #[component(name = "base-url")]
    base_url: Option<String>,
    /// Supplied ordered headers, retaining the absent/present distinction.
    headers: Option<Vec<(String, String)>>,
}

/// Credential denial is data, distinct from a catchable import conversion error.
#[derive(ComponentType, Lower)]
#[component(variant)]
enum ResolvedRequestAuth {
    /// Native resolution completed successfully.
    #[component(name = "ok")]
    Ok(RequestAuth),
    /// Native resolution returned its original failure text.
    #[component(name = "error")]
    Error(String),
}

impl From<maestro_extensions::ResolvedRequestAuth> for ResolvedRequestAuth {
    fn from(result: maestro_extensions::ResolvedRequestAuth) -> Self {
        match result {
            maestro_extensions::ResolvedRequestAuth::Ok {
                api_key,
                headers,
                base_url,
            } => Self::Ok(RequestAuth {
                api_key,
                base_url,
                headers: headers.map(|headers| headers.into_iter().collect()),
            }),
            maestro_extensions::ResolvedRequestAuth::Error { error } => Self::Error(error),
        }
    }
}

/// Links current metadata and model-authentication predicates.
fn add_metadata<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_read(
        host,
        "[method]model-registry.get-error",
        Request::Error,
        resource::<T, Endpoint>,
    )?;
    add_routed_operand(
        host,
        "[method]model-registry.get-provider-auth-status",
        Request::Status,
        operand::<T, Endpoint, String>,
    )?;
    add_operand(
        host,
        "[method]model-registry.get-provider-display-name",
        Request::Display,
        operand::<T, Endpoint, String>,
    )?;
    add_routed_operand(
        host,
        "[method]model-registry.has-configured-auth",
        Request::HasConfiguredAuth,
        operand::<T, Endpoint, String>,
    )?;
    add_routed_operand(
        host,
        "[method]model-registry.is-using-oauth",
        Request::IsUsingOAuth,
        operand::<T, Endpoint, String>,
    )?;
    Ok(())
}
/// Mutates this catalog directly, never the extension's registration queue.
fn add_mutations<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_routed(
        host,
        "[method]model-registry.refresh",
        Request::Refresh,
        resource::<T, Endpoint>,
    )?;
    add_routed_operand(
        host,
        "[method]model-registry.unregister-provider",
        Request::Unregister,
        operand::<T, Endpoint, String>,
    )?;
    host.func_wrap(
        "[method]model-registry.register-provider",
        |mut store: StoreContextMut<'_, T>,
         (resource, name, config, callbacks): (
            Resource<Endpoint>,
            String,
            String,
            crate::provider_registration::Wire,
        )| {
            let callbacks =
                crate::provider_registration::retain(store.data_mut(), &name, callbacks)?;
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = endpoint.route(|reply| Request::Register(name, config, callbacks, reply));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}
/// Resolves the provider key on the original local catalog without blocking other requests.
fn add_provider_key<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]model-registry.get-api-key-for-provider",
        |accessor, (resource, provider): (Resource<Endpoint>, String)| {
            Box::pin(async move {
                let endpoint = accessor.with(|mut access| {
                    Ok::<_, wasmtime::Error>(access.get().ctx().table.get(&resource)?.clone())
                })?;
                Ok((endpoint.provider_key(provider).await,))
            })
        },
    )
}
