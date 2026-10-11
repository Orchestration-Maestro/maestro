//! Dynamic application and retained provider replay.
use super::provider_config::{RegisteredOAuth, nonempty};
use super::request_auth::ProviderRequestConfig;
use super::{ModelRegistry, ProviderConfigInput, ProviderModelInput};
use maestro_credentials::AuthCredential;
use maestro_models::{
    ApiProvider, Model, OAuthError, OAuthProviderHandle, StreamOptionsInput, register_api_provider,
    register_oauth_provider,
};
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};
impl ModelRegistry {
    /// Apply incoming operations before descriptor publication and optional transformation.
    pub(super) fn apply_provider_config(
        &mut self,
        provider: &str,
        config: &Arc<RwLock<ProviderConfigInput>>,
    ) -> Result<(), OAuthError> {
        install_operations(provider, config);
        let oauth = {
            let config = config.read().unwrap_or_else(PoisonError::into_inner);
            if nonempty(config.api_key.as_deref()).is_some()
                || config.headers.is_some()
                || config.auth_header == Some(true)
            {
                self.provider_configs.insert(
                    provider.into(),
                    ProviderRequestConfig {
                        api_key: config.api_key.clone(),
                        headers: super::composition::ordered(config.headers.clone()),
                        auth_header: config.auth_header.unwrap_or(false),
                    },
                );
            }
            if let Some(definitions) = config.models.as_ref().filter(|models| !models.is_empty()) {
                self.replace_models(provider, &config, definitions);
                config.oauth.clone()
            } else if nonempty(config.base_url.as_deref()).is_some() || config.headers.is_some() {
                self.override_models(provider, config.base_url.as_deref());
                None
            } else {
                None
            }
        };
        if let Some(oauth) = oauth {
            self.transform_models(provider, &oauth)?;
        }
        Ok(())
    }
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
            if let Some(headers) = super::composition::ordered(definition.headers.clone())
                .filter(|headers| !headers.is_empty())
            {
                self.model_headers.insert(identity, headers);
            } else {
                self.model_headers.remove(&identity);
            }
            models.push(Arc::new(RwLock::new(
                definition.descriptor(provider, config),
            )));
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
    /// Invoke supplied account operations outside configuration and descriptor guards.
    fn transform_models(
        &mut self,
        provider: &str,
        oauth: &OAuthProviderHandle,
    ) -> Result<(), OAuthError> {
        let Some(AuthCredential::OAuth(credentials)) = self.auth_storage.get(provider) else {
            return Ok(());
        };
        let handles = self
            .models
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let models: Vec<_> = handles
            .iter()
            .map(|model| model.read().unwrap_or_else(PoisonError::into_inner).clone())
            .collect();
        let Some(transformed) = oauth.modify_models(&models, &credentials) else {
            return Ok(());
        };
        let transformed = transformed?;
        let mut originals: HashMap<_, Vec<_>> = HashMap::new();
        for (snapshot, handle) in models.iter().zip(&handles) {
            originals
                .entry((&snapshot.provider, &snapshot.id))
                .or_default()
                .push((snapshot, handle));
        }
        self.models = Arc::new(RwLock::new(
            transformed
                .into_iter()
                .map(|model| {
                    if let Some((_, handle)) = originals
                        .get(&(&model.provider, &model.id))
                        .and_then(|candidates| {
                            candidates.iter().find(|(snapshot, _)| **snapshot == model)
                        })
                    {
                        Arc::clone(handle)
                    } else {
                        Arc::new(RwLock::new(model))
                    }
                })
                .collect(),
        ));
        Ok(())
    }
    /// Remove a retained registration before reloading; unknown names are no-ops.
    ///
    /// # Errors
    /// Returns an underlying load or surviving provider transformation failure.
    pub fn unregister_provider(&mut self, provider_name: &str) -> Result<(), OAuthError> {
        if self
            .registered_providers
            .shift_remove(provider_name)
            .is_some()
        {
            self.refresh()?;
        }
        Ok(())
    }
    /// Merge defined fields into the first retained handle without moving its slot.
    pub(super) fn upsert_registered_provider(
        &mut self,
        provider: &str,
        config: Arc<RwLock<ProviderConfigInput>>,
    ) {
        let Some(existing) = self.registered_providers.get(provider) else {
            self.registered_providers.insert(provider.into(), config);
            return;
        };
        if Arc::ptr_eq(existing, &config) {
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
                replace_defined(&mut retained.stream_simple, incoming.stream_simple.as_ref()),
                replace_defined(&mut retained.headers, incoming.headers.as_ref()),
                replace_defined(&mut retained.auth_header, incoming.auth_header.as_ref()),
                replace_defined(&mut retained.oauth, incoming.oauth.as_ref()),
                replace_defined(&mut retained.models, incoming.models.as_ref()),
            )
        };
        drop(retired);
    }
}
/// Install retained account/callback handles without guards over registry retirement.
fn install_operations(provider: &str, config: &Arc<RwLock<ProviderConfigInput>>) {
    let oauth = config
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .oauth
        .clone();
    if let Some(inner) = oauth {
        register_oauth_provider(Arc::new(RegisteredOAuth {
            id: provider.into(),
            inner,
        }));
    }
    let callback = {
        let config = config.read().unwrap_or_else(PoisonError::into_inner);
        config
            .stream_simple
            .as_ref()
            .map(|callback| (config.api.clone().unwrap_or_default(), Arc::clone(callback)))
    };
    if let Some((api, callback)) = callback {
        let raw = Arc::clone(&callback);
        register_api_provider(
            ApiProvider {
                api,
                stream: Arc::new(move |model, context, options| {
                    raw(model, context, options.map(StreamOptionsInput::Raw))
                }),
                stream_simple: Arc::new(move |model, context, options| {
                    callback(model, context, options.map(StreamOptionsInput::Simple))
                }),
            },
            Some(format!("provider:{provider}")),
        );
    }
}
impl ProviderModelInput {
    /// Copy supplied metadata into its retained descriptor, leaving headers in request storage.
    fn descriptor(&self, provider: &str, config: &ProviderConfigInput) -> Model {
        Model {
            id: self.id.clone(),
            name: self.name.clone(),
            provider: provider.into(),
            api: nonempty(self.api.as_deref())
                .or(nonempty(config.api.as_deref()))
                .unwrap_or_default()
                .into(),
            base_url: self
                .base_url
                .as_ref()
                .or(config.base_url.as_ref())
                .cloned()
                .unwrap_or_default(),
            reasoning: self.reasoning,
            thinking_level_map: self.thinking_level_map.clone(),
            input: self.input.clone(),
            cost: self.cost.clone(),
            context_window: self.context_window,
            max_tokens: self.max_tokens,
            headers: None,
            compat: self.compat.clone(),
        }
    }
}
/// Return replaced ownership to retire it after releasing configuration guards.
fn replace_defined<T: Clone>(retained: &mut Option<T>, incoming: Option<&T>) -> Option<T> {
    incoming.and_then(|value| retained.replace(value.clone()))
}
