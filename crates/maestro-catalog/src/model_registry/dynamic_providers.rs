//! Dynamic application and retained provider replay.
use super::provider_config::{RegisteredOAuth, nonempty};
use super::request_auth::ProviderRequestConfig;
use super::{CatalogError, CatalogState, ModelRegistry, ProviderConfigInput, ProviderModelInput};
use maestro_credentials::AuthCredential;
use maestro_models::SharedHandle;
use maestro_models::{
    ApiProvider, Invoke, Model, OAuthProviderHandle, ProviderCall, ProviderExecution,
    StreamFunction, StreamOptionsInput, register_api_provider, register_oauth_provider,
};
use std::sync::{Arc, PoisonError, RwLock};
impl ModelRegistry {
    /// Apply incoming operations before descriptor publication and optional transformation.
    pub(super) fn apply_provider_config(
        &self,
        provider: &str,
        config: &SharedHandle<RwLock<ProviderConfigInput>>,
    ) -> Result<(), CatalogError> {
        let oauth = self.prepare_provider_config(provider, config);
        if let Some(oauth) = oauth {
            self.transform_models(provider, &oauth)?;
        }
        Ok(())
    }
    /// Publish incoming sources and descriptors before supplied transformation.
    pub(super) fn prepare_provider_config(
        &self,
        provider: &str,
        config: &SharedHandle<RwLock<ProviderConfigInput>>,
    ) -> Option<OAuthProviderHandle> {
        install_operations(provider, config);
        {
            let config = config.read().unwrap_or_else(PoisonError::into_inner);
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            if nonempty(config.api_key.as_deref()).is_some()
                || config.headers.is_some()
                || config.auth_header == Some(true)
            {
                state.provider_configs.insert(
                    provider.into(),
                    Arc::new(ProviderRequestConfig {
                        api_key: config.api_key.clone(),
                        headers: super::composition::ordered(config.headers.clone()),
                        auth_header: config.auth_header.unwrap_or(false),
                    }),
                );
            }
            if let Some(definitions) = config.models.as_ref().filter(|models| !models.is_empty()) {
                state.replace_models(provider, &config, definitions);
                config.callbacks.oauth.clone()
            } else if nonempty(config.base_url.as_deref()).is_some() || config.headers.is_some() {
                state.override_models(provider, config.base_url.as_deref());
                None
            } else {
                None
            }
        }
    }
    /// Invoke supplied account operations outside configuration and descriptor guards.
    fn transform_models(
        &self,
        provider: &str,
        oauth: &OAuthProviderHandle,
    ) -> Result<(), CatalogError> {
        let Some(AuthCredential::OAuth(credentials)) = self.auth_storage.get(provider) else {
            return Ok(());
        };
        let handles = self
            .get_all()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let handles = super::transform_handles(handles, oauth, &credentials)?;
        self.publish(handles);
        Ok(())
    }
    /// Remove a retained registration before reloading; unknown names are no-ops.
    ///
    /// # Errors
    /// Returns a catalog error wrapping a surviving account transformation failure.
    pub fn unregister_provider(&self, provider_name: &str) -> Result<(), CatalogError> {
        let retired = self
            .state
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .registered_providers
            .shift_remove(provider_name);
        let removed = retired.is_some();
        drop(retired);
        if removed {
            self.refresh()?;
        }
        Ok(())
    }
    /// Merge defined fields into the first retained handle without moving its slot.
    pub(super) fn upsert_registered_provider(
        &self,
        provider: &str,
        config: SharedHandle<RwLock<ProviderConfigInput>>,
    ) {
        let existing = {
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            if let Some((_, existing)) = state.registered_providers.get(provider) {
                SharedHandle::clone(existing)
            } else {
                let ordinal = state.next_provider;
                state.next_provider += 1;
                state
                    .registered_providers
                    .insert(provider.into(), (ordinal, config));
                return;
            }
        };
        if SharedHandle::ptr_eq(&existing, &config) {
            return;
        }
        let retired = {
            let incoming = config.read().unwrap_or_else(PoisonError::into_inner);
            let mut retained = existing.write().unwrap_or_else(PoisonError::into_inner);
            (
                replace_defined(&mut retained.name, incoming.name.as_ref()),
                replace_defined(&mut retained.base_url, incoming.base_url.as_ref()),
                replace_defined(&mut retained.api_key, incoming.api_key.as_ref()),
                replace_defined(&mut retained.api, incoming.api.as_ref()),
                replace_defined(
                    &mut retained.callbacks.stream_simple,
                    incoming.callbacks.stream_simple.as_ref(),
                ),
                replace_defined(&mut retained.headers, incoming.headers.as_ref()),
                replace_defined(&mut retained.auth_header, incoming.auth_header.as_ref()),
                replace_defined(
                    &mut retained.callbacks.oauth,
                    incoming.callbacks.oauth.as_ref(),
                ),
                replace_defined(&mut retained.models, incoming.models.as_ref()),
            )
        };
        drop(retired);
    }
}
/// Install retained account/callback handles without guards over registry retirement.
fn install_operations(provider: &str, config: &SharedHandle<RwLock<ProviderConfigInput>>) {
    let oauth = config
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .callbacks
        .oauth
        .clone();
    if let Some(inner) = oauth {
        register_oauth_provider(SharedHandle::new(RegisteredOAuth {
            id: provider.into(),
            inner,
        }));
    }
    let callback = {
        let config = config.read().unwrap_or_else(PoisonError::into_inner);
        config.callbacks.stream_simple.as_ref().map(|callback| {
            (
                config.api.clone().unwrap_or_default(),
                SharedHandle::clone(callback),
            )
        })
    };
    if let Some((api, callback)) = callback {
        let owner = SharedHandle::new(ForwardProvider(callback));
        register_api_provider(
            ApiProvider {
                api,
                stream: owner.clone(),
                stream_simple: owner,
            },
            Some(format!("provider:{provider}")),
        );
    }
}
/// Copy supplied metadata into its retained descriptor, leaving headers in request storage.
fn descriptor(model: &ProviderModelInput, provider: &str, config: &ProviderConfigInput) -> Model {
    Model {
        id: model.id.clone(),
        name: model.name.clone(),
        provider: provider.into(),
        api: nonempty(model.api.as_deref())
            .or(nonempty(config.api.as_deref()))
            .unwrap_or_default()
            .into(),
        base_url: model
            .base_url
            .as_ref()
            .or(config.base_url.as_ref())
            .cloned()
            .unwrap_or_default(),
        reasoning: model.reasoning,
        thinking_level_map: model.thinking_level_map.clone(),
        input: model.input.clone(),
        cost: model.cost.clone(),
        context_window: model.context_window,
        max_tokens: model.max_tokens,
        headers: None,
        compat: model.compat.clone(),
    }
}
/// Return replaced ownership to retire it after releasing configuration guards.
fn replace_defined<T: Clone>(retained: &mut Option<T>, incoming: Option<&T>) -> Option<T> {
    incoming.and_then(|value| retained.replace(value.clone()))
}

/// Preserve the retained catalog callback and original option family across replay.
struct ForwardProvider(StreamFunction<StreamOptionsInput>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for ForwardProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        self.0.call(
            call.model,
            call.context,
            E::submission(call.options.map(Into::into), call.submit),
        )
    }
}

impl CatalogState {
    /// Publish a replacement list before a fallible account transformation.
    fn replace_models(
        &mut self,
        provider: &str,
        config: &ProviderConfigInput,
        definitions: &[ProviderModelInput],
    ) {
        let mut models: Vec<_> = self
            .models
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|model| {
                model
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .provider
                    != provider
            })
            .cloned()
            .collect();
        for definition in definitions {
            let identity = (provider.into(), definition.id.clone());
            let overridden = self
                .model_overrides
                .get(provider)
                .and_then(|overrides| overrides.get(&definition.id))
                .cloned();
            let mut headers = definition.headers.clone();
            if let Some(extra) = overridden
                .as_ref()
                .and_then(|fields| fields.headers.clone())
            {
                headers.get_or_insert_default().extend(extra);
            }
            if let Some(headers) =
                super::composition::ordered(headers).filter(|headers| !headers.is_empty())
            {
                self.model_headers.insert(identity, headers);
            } else {
                self.model_headers.remove(&identity);
            }
            let mut model = descriptor(definition, provider, config);
            if let Some(fields) = overridden {
                super::composition::apply(&mut model, fields);
            }
            models.push(Arc::new(RwLock::new(model)));
        }
        self.models = Arc::new(RwLock::new(models));
    }
    /// Replace matching descriptors while retaining unrelated handles.
    fn override_models(&mut self, provider: &str, url: Option<&str>) {
        let models = self
            .models
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|handle| {
                let model = handle.read().unwrap_or_else(PoisonError::into_inner);
                if model.provider != provider {
                    return Arc::clone(handle);
                }
                let mut replacement = model.clone();
                if let Some(url) = url {
                    replacement.base_url = url.into();
                }
                Arc::new(RwLock::new(replacement))
            })
            .collect();
        self.models = Arc::new(RwLock::new(models));
    }
}
