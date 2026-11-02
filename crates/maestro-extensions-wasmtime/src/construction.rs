//! Constructors retain native owners through the original capability table.
use crate::{api_imports::ApiView, local_resources::Registry};
use maestro_extensions::{AuthStorage, ModelRegistry, SharedHandle};
use std::sync::{Arc, mpsc::Sender};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Selects memory construction or a file with its independently optional default path.
pub(crate) enum Source {
    /// No file observation.
    Memory,
    /// Explicit file or the application's lazy default.
    File(Option<String>),
}
/// Constructor requests use the same pump as every retained owner.
pub(crate) enum Request {
    /// Retained author backend with its original Send pump endpoint.
    Backend(
        crate::guest_calls::credentials::Backend,
        Sender<Result<crate::credential_reads::Endpoint, String>>,
    ),
    /// File credentials with independently optional path.
    Auth(
        Option<String>,
        Sender<Result<crate::credential_reads::Endpoint, String>>,
    ),
    /// Credentials seeded in memory without file access.
    Memory(
        String,
        Sender<Result<crate::credential_reads::Endpoint, String>>,
    ),
    /// Catalog over the exact supplied credentials; absence of the outer path means memory.
    Catalog(
        crate::credential_reads::Endpoint,
        Source,
        Sender<Result<crate::catalog_reads::Endpoint, String>>,
    ),
}
impl Request {
    /// Constructs outside the capability-table borrow.
    pub(crate) fn answer(
        self,
        api: &maestro_extensions::ExtensionAPI<maestro_extensions::Local>,
        registry: &Registry,
    ) {
        match self {
            Self::Backend(backend, reply) => {
                let result = api.catalog_construction().and_then(|services| {
                    registry.credentials(
                        Arc::new(AuthStorage::from_storage(backend)),
                        services.config_values,
                    )
                });
                let _ = reply.send(result);
            }
            Self::Auth(path, reply) => {
                let result = api.catalog_construction().and_then(|services| {
                    api.create_auth_storage(path.as_deref())
                        .and_then(|owner| registry.credentials(owner, services.config_values))
                });
                let _ = reply.send(result);
            }
            Self::Memory(data, reply) => {
                let result = crate::writer_reads::decode(&data).and_then(|data| {
                    registry.credentials(
                        Arc::new(AuthStorage::in_memory(data)),
                        api.catalog_construction()?.config_values,
                    )
                });
                let _ = reply.send(result);
            }
            Self::Catalog(auth, path, reply) => {
                let result = catalog(api, registry, &auth, path)
                    .and_then(|catalog| registry.catalog(catalog));
                let _ = reply.send(result);
            }
        }
    }
    /// Rejects a retired facade without stranding a constructor.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Auth(_, reply) | Self::Memory(_, reply) | Self::Backend(_, reply) => {
                let _ = reply.send(Err(error.into()));
            }
            Self::Catalog(_, _, reply) => {
                let _ = reply.send(Err(error.into()));
            }
        }
    }
}
/// Checks that the original credential resource belongs to this pump before construction.
fn catalog(
    api: &maestro_extensions::ExtensionAPI<maestro_extensions::Local>,
    registry: &Registry,
    endpoint: &crate::credential_reads::Endpoint,
    path: Source,
) -> Result<SharedHandle<ModelRegistry>, String> {
    let services = api.catalog_construction()?;
    let (auth, effects) = registry.credential_owner(endpoint)?;
    match path {
        Source::File(path) => {
            ModelRegistry::create(auth, services.model_path(path.as_deref())?, effects)
                .map(SharedHandle::new)
                .map_err(|error| error.to_string())
        }
        Source::Memory => ModelRegistry::in_memory(auth, effects)
            .map(SharedHandle::new)
            .map_err(|error| error.to_string()),
    }
}
/// Installs constructors on the canonical host interface.
pub(crate) fn link<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    link_backend(host)?;
    host.func_wrap(
        "create-auth-storage",
        |mut store: StoreContextMut<'_, T>, (path,): (Option<String>,)| {
            let result = store.data().api().0.request(|id, reply| {
                crate::local_resources::Message::Registration(
                    id,
                    crate::registration_reads::Request::Construct(Request::Auth(path, reply)),
                )
            });
            let result = result.and_then(|owner| {
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
    host.func_wrap(
        "auth-storage-in-memory",
        |mut store: StoreContextMut<'_, T>, (data,): (String,)| {
            let result = store.data().api().0.request(|id, reply| {
                crate::local_resources::Message::Registration(
                    id,
                    crate::registration_reads::Request::Construct(Request::Memory(data, reply)),
                )
            });
            let result = result.and_then(|owner| {
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
    host.func_wrap(
        "create-model-registry",
        |store: StoreContextMut<'_, T>,
         (auth, path): (Resource<crate::credential_reads::Endpoint>, Option<String>)| {
            construct_catalog(store, &auth, Source::File(path))
        },
    )?;
    host.func_wrap(
        "in-memory-model-registry",
        |store: StoreContextMut<'_, T>, (auth,): (Resource<crate::credential_reads::Endpoint>,)| {
            construct_catalog(store, &auth, Source::Memory)
        },
    )
}
/// Retains a new catalog through a route, so a credential backend the construction reads
/// re-enters its own guest; the supplied credential borrow stays live.
fn construct_catalog<T: ApiView>(
    mut store: StoreContextMut<'_, T>,
    auth: &Resource<crate::credential_reads::Endpoint>,
    path: Source,
) -> wasmtime::Result<(Resource<crate::import_route::Route>,)> {
    let auth = store.data_mut().ctx().table.get(auth)?.clone();
    let route = store.data().api().0.route(|id, reply| {
        crate::local_resources::Message::Registration(
            id,
            crate::registration_reads::Request::Construct(Request::Catalog(auth, path, reply)),
        )
    });
    Ok((crate::import_route::push(store.data_mut(), route)?,))
}

/// Installs the value-returning constructor that calls an authored backend during reload.
fn link_backend<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "auth-storage-from-backend",
        |mut store: StoreContextMut<'_, T>,
         (handler,): (Resource<crate::guest_calls::Callback>,)| {
            let handler = crate::callback_imports::retain(store.data_mut(), &handler)?;
            let owner = store.data().api().0.clone();
            let backend = crate::guest_calls::credentials::Backend {
                handler,
                owner: owner.clone(),
            };
            let route = owner.route(|id, reply| {
                crate::local_resources::Message::Registration(
                    id,
                    crate::registration_reads::Request::Construct(Request::Backend(backend, reply)),
                )
            });
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )
}
