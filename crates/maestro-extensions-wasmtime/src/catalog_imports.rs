//! Canonical catalog imports: the generated resource traits over the original local catalog.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{
        HostModelRegistry, HostModelRegistryWithStore, RequestAuth, ResolvedRequestAuth,
    },
    catalog_reads::Endpoint,
    import_route::{Route, push},
    lifecycle::State,
    provider_registration::Wire,
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, HasSelf, LinkerInstance, Resource, ResourceType},
};
use wasmtime_wasi::WasiView;

/// Clones the catalog endpoint a guest handle names before native code runs.
fn endpoint(state: &mut State, this: &Resource<Endpoint>) -> wasmtime::Result<Endpoint> {
    Ok(state.ctx().table.get(this)?.clone())
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

impl HostModelRegistry for State {
    fn auth_storage(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Resource<crate::credential_reads::Endpoint>, String>> {
        let catalog = endpoint(self, &this)?;
        Ok(catalog.credentials().and_then(|owner| {
            self.ctx()
                .table
                .push(owner)
                .map_err(|error| error.to_string())
        }))
    }

    fn get_all(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.get_all())
    }

    fn get_available(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.get_available();
        push(self, route)
    }

    fn find(
        &mut self,
        this: Resource<Endpoint>,
        provider: String,
        model_id: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(endpoint(self, &this)?.find(provider, model_id))
    }

    fn refresh(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.refresh();
        push(self, route)
    }

    fn get_error(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(endpoint(self, &this)?.get_error())
    }

    fn has_configured_auth(
        &mut self,
        this: Resource<Endpoint>,
        model: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.has_configured_auth(model);
        push(self, route)
    }

    fn get_provider_auth_status(
        &mut self,
        this: Resource<Endpoint>,
        provider: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.get_provider_auth_status(provider);
        push(self, route)
    }

    fn get_provider_display_name(
        &mut self,
        this: Resource<Endpoint>,
        provider: String,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.get_provider_display_name(provider))
    }

    fn is_using_oauth(
        &mut self,
        this: Resource<Endpoint>,
        model: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.is_using_oauth(model);
        push(self, route)
    }

    fn register_provider(
        &mut self,
        this: Resource<Endpoint>,
        name: String,
        config: String,
        callbacks: Wire,
    ) -> wasmtime::Result<Resource<Route>> {
        let callbacks = crate::provider_registration::retain(self, &name, callbacks)?;
        let route = endpoint(self, &this)?.register_provider(name, config, callbacks);
        push(self, route)
    }

    fn unregister_provider(
        &mut self,
        this: Resource<Endpoint>,
        name: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.unregister_provider(name);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        self.ctx().table.delete(this)?;
        Ok(())
    }
}

impl HostModelRegistryWithStore<State> for HasSelf<State> {
    async fn get_api_key_and_headers(
        accessor: &Accessor<State, Self>,
        this: Resource<Endpoint>,
        model: String,
    ) -> wasmtime::Result<Result<ResolvedRequestAuth, String>> {
        let catalog = accessor.with(|mut access| endpoint(access.get(), &this))?;
        Ok(match crate::writer_reads::decode(&model) {
            Ok(model) => catalog.auth(model).await.map(ResolvedRequestAuth::from),
            Err(error) => Err(error),
        })
    }

    async fn get_api_key_for_provider(
        accessor: &Accessor<State, Self>,
        this: Resource<Endpoint>,
        provider: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let catalog = accessor.with(|mut access| endpoint(access.get(), &this))?;
        Ok(catalog.provider_key(provider).await)
    }
}

/// Registers the catalog resource's destructor and every method as a forward to the traits.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    host.resource(
        "model-registry",
        ResourceType::host::<Endpoint>(),
        |mut store, key| HostModelRegistry::drop(store.data_mut(), Resource::new_own(key)),
    )?;
    add_reads(host)?;
    add_routes(host)?;
    host.func_wrap(
        "[method]model-registry.register-provider",
        |mut store: StoreContextMut<'_, State>,
         (this, name, config, callbacks): (Resource<Endpoint>, String, String, Wire)| {
            Ok((HostModelRegistry::register_provider(
                store.data_mut(),
                this,
                name,
                config,
                callbacks,
            )?,))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]model-registry.get-api-key-and-headers",
        |accessor, (this, model): (Resource<Endpoint>, String)| {
            Box::pin(async move {
                Ok((
                    <HasSelf<State> as HostModelRegistryWithStore<State>>::get_api_key_and_headers(
                        accessor, this, model,
                    )
                    .await?,
                ))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]model-registry.get-api-key-for-provider",
        |accessor, (this, provider): (Resource<Endpoint>, String)| {
            Box::pin(async move {
                Ok((
                    <HasSelf<State> as HostModelRegistryWithStore<State>>::get_api_key_for_provider(
                        accessor, this, provider,
                    )
                    .await?,
                ))
            })
        },
    )
}

/// Forwards the methods that answer on the Store thread without a route.
fn add_reads(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]model-registry.auth-storage",
        |mut store: StoreContextMut<'_, State>, (this,): (Resource<Endpoint>,)| {
            Ok((HostModelRegistry::auth_storage(store.data_mut(), this)?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.get-all",
        |mut store: StoreContextMut<'_, State>, (this,): (Resource<Endpoint>,)| {
            Ok((HostModelRegistry::get_all(store.data_mut(), this)?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.find",
        |mut store: StoreContextMut<'_, State>,
         (this, provider, model_id): (Resource<Endpoint>, String, String)| {
            Ok((HostModelRegistry::find(
                store.data_mut(),
                this,
                provider,
                model_id,
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.get-error",
        |mut store: StoreContextMut<'_, State>, (this,): (Resource<Endpoint>,)| {
            Ok((HostModelRegistry::get_error(store.data_mut(), this)?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.get-provider-display-name",
        |mut store: StoreContextMut<'_, State>, (this, provider): (Resource<Endpoint>, String)| {
            Ok((HostModelRegistry::get_provider_display_name(
                store.data_mut(),
                this,
                provider,
            )?,))
        },
    )
}

/// Forwards the methods that return a route for the guest to drive.
fn add_routes(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]model-registry.get-available",
        |mut store: StoreContextMut<'_, State>, (this,): (Resource<Endpoint>,)| {
            Ok((HostModelRegistry::get_available(store.data_mut(), this)?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.refresh",
        |mut store: StoreContextMut<'_, State>, (this,): (Resource<Endpoint>,)| {
            Ok((HostModelRegistry::refresh(store.data_mut(), this)?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.has-configured-auth",
        |mut store: StoreContextMut<'_, State>, (this, model): (Resource<Endpoint>, String)| {
            Ok((HostModelRegistry::has_configured_auth(
                store.data_mut(),
                this,
                model,
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.get-provider-auth-status",
        |mut store: StoreContextMut<'_, State>, (this, provider): (Resource<Endpoint>, String)| {
            Ok((HostModelRegistry::get_provider_auth_status(
                store.data_mut(),
                this,
                provider,
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.is-using-oauth",
        |mut store: StoreContextMut<'_, State>, (this, model): (Resource<Endpoint>, String)| {
            Ok((HostModelRegistry::is_using_oauth(
                store.data_mut(),
                this,
                model,
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]model-registry.unregister-provider",
        |mut store: StoreContextMut<'_, State>, (this, name): (Resource<Endpoint>, String)| {
            Ok((HostModelRegistry::unregister_provider(
                store.data_mut(),
                this,
                name,
            )?,))
        },
    )
}
