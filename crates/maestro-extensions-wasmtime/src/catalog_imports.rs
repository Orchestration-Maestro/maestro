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
    read_imports::{endpoint, forwards, release},
};
use wasmtime::component::{Accessor, HasSelf, LinkerInstance, Resource, ResourceType};
use wasmtime_wasi::WasiView;

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
        release(self, this)
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
    forwards! { host, "model-registry", HostModelRegistry, Endpoint,
        "auth-storage" => auth_storage();
        "get-all" => get_all();
        "find" => find(provider: String, model_id: String);
        "get-error" => get_error();
        "get-provider-display-name" => get_provider_display_name(provider: String);
        "get-available" => get_available();
        "refresh" => refresh();
        "has-configured-auth" => has_configured_auth(model: String);
        "get-provider-auth-status" => get_provider_auth_status(provider: String);
        "is-using-oauth" => is_using_oauth(model: String);
        "register-provider" => register_provider(name: String, config: String, callbacks: Wire);
        "unregister-provider" => unregister_provider(name: String);
    }
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
