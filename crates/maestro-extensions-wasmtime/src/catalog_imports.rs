//! Canonical catalog imports preserve current descriptors and exact lookup operands.
use crate::read_imports::ReadEndpoint;
use crate::{
    catalog_reads::{Endpoint, Request},
    context_imports::add_resource,
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, LinkerInstance, Lower, Resource},
};
use wasmtime_wasi::WasiView;

/// Defines catalog reads and credential resolution through the original local owner.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "model-registry")?;
    for (method, request) in [
        ("get-all", Request::All as fn(_) -> _),
        ("get-available", Request::Available),
    ] {
        host.func_wrap(
            &format!("[method]model-registry.{method}"),
            move |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
                let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
                Ok((endpoint.read(request),))
            },
        )?;
    }
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

/// Canonical successful authentication retains optional empty key and header values.
#[derive(ComponentType, Lower)]
#[component(record)]
struct RequestAuth {
    /// Selected key, absent when no source supplies one.
    #[component(name = "api-key")]
    api_key: Option<String>,
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
            maestro_extensions::ResolvedRequestAuth::Ok { api_key, headers } => {
                Self::Ok(RequestAuth {
                    api_key,
                    headers: headers.map(|headers| headers.into_iter().collect()),
                })
            }
            maestro_extensions::ResolvedRequestAuth::Error { error } => Self::Error(error),
        }
    }
}
