//! Request-time resolution of retained configuration sources.
use super::{CatalogError, ModelRegistry, read_provider};
use indexmap::IndexMap;
use maestro_credentials::{
    resolve_config_value_or_throw, resolve_config_value_uncached, resolve_headers_or_throw,
};
use maestro_models::Model;
use std::sync::{Arc, PoisonError, RwLock};

/// Checked provider request sources, separate from descriptor metadata.
pub(super) struct ProviderRequestConfig {
    /// Optional configured key expression.
    pub api_key: Option<String>,
    /// Optional configured header expressions.
    pub headers: Option<IndexMap<String, String>>,
    /// Synthesize Authorization after header resolution.
    pub auth_header: bool,
}
pub use maestro_request::model_registry::ResolvedRequestAuth;
impl ModelRegistry {
    /// Select a provider key with the credentials fallback disabled, running
    /// configured commands through the catalog's own handle.
    ///
    /// # Errors
    /// Returns a catalog error wrapping a lower-layer account extraction failure.
    pub async fn get_api_key_for_provider(
        &self,
        provider: &str,
    ) -> Result<Option<String>, CatalogError> {
        let key = self
            .auth_storage
            .get_api_key(provider, false, &*self.config_values)
            .await?;
        if key.is_some() {
            return Ok(key);
        }
        let config = self.request_config(provider);
        Ok(config
            .as_ref()
            .and_then(|c| c.api_key.as_deref())
            .filter(|v| !v.is_empty())
            .and_then(|v| resolve_config_value_uncached(v, &*self.config_values)))
    }
}

impl ModelRegistry {
    /// Resolve request keys, layered headers and required provider account endpoints.
    /// Returns the winning configuration or credential error.
    pub async fn get_api_key_and_headers(&self, model: &Arc<RwLock<Model>>) -> ResolvedRequestAuth {
        match self.resolve_request(model).await {
            Ok((api_key, headers, base_url)) => ResolvedRequestAuth::Ok {
                api_key,
                headers,
                base_url,
            },
            Err(error) => ResolvedRequestAuth::Error { error },
        }
    }
    /// Read request fields at their consuming phase without retaining guards across effects.
    async fn resolve_request(&self, model: &Arc<RwLock<Model>>) -> Result<RequestValues, String> {
        let provider = read_provider(model);
        let config = self.request_config(&provider);
        let stored = self
            .auth_storage
            .get_api_key(&provider, false, &*self.config_values)
            .await
            .map_err(|error| error.to_string())?;
        let api_key = self.resolve_key(
            stored,
            config.as_ref().and_then(|c| c.api_key.as_deref()),
            model,
        )?;
        let headers = self.resolve_headers(model, config.as_deref(), api_key.as_deref())?;
        let descriptor = model.read().unwrap_or_else(PoisonError::into_inner);
        let base_url = self
            .auth_storage
            .cloudflare_base_url(&descriptor)
            .map_err(|e| e.message)?;
        Ok((api_key, headers, base_url))
    }
    /// Resolve and merge provider, model and synthesized authentication headers in order.
    fn resolve_headers(
        &self,
        model: &Arc<RwLock<Model>>,
        config: Option<&ProviderRequestConfig>,
        api_key: Option<&str>,
    ) -> Result<Option<IndexMap<String, String>>, String> {
        let provider = read_provider(model);
        let provider_headers = resolve_headers_or_throw(
            config.and_then(|c| c.headers.as_ref()),
            &format!("provider \"{provider}\""),
            &*self.config_values,
        )
        .map_err(|error| error.to_string())?;
        let identity = {
            let model = model.read().unwrap_or_else(PoisonError::into_inner);
            (model.provider.clone(), model.id.clone())
        };
        let model_sources = self
            .state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .model_headers
            .get(&identity)
            .cloned();
        let model_headers = resolve_headers_or_throw(
            model_sources.as_ref(),
            &format!("model \"{}/{}\"", identity.0, identity.1),
            &*self.config_values,
        )
        .map_err(|error| error.to_string())?;
        let mut headers: IndexMap<_, _> = model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .headers
            .clone()
            .unwrap_or_default();
        headers.extend(provider_headers.into_iter().flatten());
        headers.extend(model_headers.into_iter().flatten());
        if config.is_some_and(|c| c.auth_header) {
            let Some(key) = api_key.filter(|key| !key.is_empty()) else {
                let provider = read_provider(model);
                return Err(format!("No API key found for \"{provider}\""));
            };
            headers.insert("Authorization".into(), format!("Bearer {key}"));
        }
        let headers = super::composition::ordered(Some(headers)).filter(|h| !h.is_empty());
        Ok(headers)
    }
    /// Resolve configured fallback only when storage supplied no key.
    fn resolve_key(
        &self,
        stored: Option<String>,
        config: Option<&str>,
        model: &Arc<RwLock<Model>>,
    ) -> Result<Option<String>, String> {
        let api_key = match (stored, config.filter(|value| !value.is_empty())) {
            (Some(key), _) => Some(key),
            (None, Some(value)) => {
                let provider = read_provider(model);
                Some(
                    resolve_config_value_or_throw(
                        value,
                        &format!("API key for provider \"{provider}\""),
                        &*self.config_values,
                    )
                    .map_err(|error| error.to_string())?,
                )
            }
            (None, None) => None,
        };
        Ok(api_key)
    }
}
/// Internal success payload before conversion to the public result.
type RequestValues = (
    Option<String>,
    Option<IndexMap<String, String>>,
    Option<String>,
);
