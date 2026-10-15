//! Request-time resolution of retained configuration sources.
use super::{CatalogError, ModelRegistry, read_provider};
use indexmap::IndexMap;
use maestro_credentials::{
    ConfigValueOperations, resolve_config_value_or_throw, resolve_config_value_uncached,
    resolve_headers_or_throw,
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
    /// Select a provider key with the credentials fallback disabled.
    ///
    /// # Errors
    /// Returns a catalog error wrapping a lower-layer account extraction failure.
    pub async fn get_api_key_for_provider(
        &self,
        provider: &str,
        operations: &dyn ConfigValueOperations,
    ) -> Result<Option<String>, CatalogError> {
        let key = self
            .auth_storage
            .get_api_key(provider, false, operations)
            .await?;
        Ok(key.or_else(|| {
            self.provider_configs
                .get(provider)
                .and_then(|config| config.api_key.as_deref())
                .filter(|value| !value.is_empty())
                .and_then(|value| resolve_config_value_uncached(value, operations))
        }))
    }
}

impl ModelRegistry {
    /// Resolve keys and headers in layer order, returning the winning error.
    pub async fn get_api_key_and_headers(
        &self,
        model: &Arc<RwLock<Model>>,
        operations: &dyn ConfigValueOperations,
    ) -> ResolvedRequestAuth {
        match self.resolve_request(model, operations).await {
            Ok((api_key, headers)) => ResolvedRequestAuth::Ok { api_key, headers },
            Err(error) => ResolvedRequestAuth::Error { error },
        }
    }
    /// Read request fields at their consuming phase without retaining guards across effects.
    async fn resolve_request(
        &self,
        model: &Arc<RwLock<Model>>,
        operations: &dyn ConfigValueOperations,
    ) -> Result<RequestValues, String> {
        let provider = read_provider(model);
        let config = self.provider_configs.get(&provider);
        let stored = self
            .auth_storage
            .get_api_key(&provider, false, operations)
            .await
            .map_err(|error| error.to_string())?;
        let api_key = resolve_key(
            stored,
            config.and_then(|c| c.api_key.as_deref()),
            model,
            operations,
        )?;
        let provider = read_provider(model);
        let provider_headers = resolve_headers_or_throw(
            config.and_then(|c| c.headers.as_ref()),
            &format!("provider \"{provider}\""),
            operations,
        )
        .map_err(|error| error.to_string())?;
        let identity = {
            let model = model.read().unwrap_or_else(PoisonError::into_inner);
            (model.provider.clone(), model.id.clone())
        };
        let model_headers = resolve_headers_or_throw(
            self.model_headers.get(&identity),
            &format!("model \"{}/{}\"", identity.0, identity.1),
            operations,
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
            let Some(key) = api_key.as_ref().filter(|key| !key.is_empty()) else {
                let provider = read_provider(model);
                return Err(format!("No API key found for \"{provider}\""));
            };
            headers.insert("Authorization".into(), format!("Bearer {key}"));
        }
        let headers = super::composition::ordered(Some(headers)).filter(|h| !h.is_empty());
        Ok((api_key, headers))
    }
}
/// Internal success payload before conversion to the public result.
type RequestValues = (Option<String>, Option<IndexMap<String, String>>);

/// Resolve configured fallback only when storage supplied no key.
fn resolve_key(
    stored: Option<String>,
    config: Option<&str>,
    model: &Arc<RwLock<Model>>,
    operations: &dyn ConfigValueOperations,
) -> Result<Option<String>, String> {
    let api_key = match (stored, config.filter(|value| !value.is_empty())) {
        (Some(key), _) => Some(key),
        (None, Some(value)) => {
            let provider = read_provider(model);
            Some(
                resolve_config_value_or_throw(
                    value,
                    &format!("API key for provider \"{provider}\""),
                    operations,
                )
                .map_err(|error| error.to_string())?,
            )
        }
        (None, None) => None,
    };
    Ok(api_key)
}
