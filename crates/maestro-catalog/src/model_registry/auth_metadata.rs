//! Authentication presence and provider metadata without request resolution.
use super::{ModelRegistry, read_provider};
use crate::CatalogError;
use maestro_credentials::{AuthSource, AuthStatus};
use maestro_models::{Model, SharedHandle};
use std::sync::{Arc, PoisonError, RwLock};
impl ModelRegistry {
    /// Check credential presence, then freshly read the configured provider identity.
    ///
    /// # Errors
    /// Returns the original credential lookup failure.
    pub fn has_configured_auth(&self, model: &Arc<RwLock<Model>>) -> Result<bool, CatalogError> {
        let descriptor = model.read().unwrap_or_else(PoisonError::into_inner).clone();
        if self.auth_storage.cloudflare_base_url(&descriptor).is_err() {
            return Ok(false);
        }
        let provider = read_provider(model);
        if self.auth_storage.has_auth(&provider)? {
            return Ok(true);
        }
        let provider = read_provider(model);
        Ok(self
            .request_config(&provider)
            .is_some_and(|c| c.api_key.is_some()))
    }
    /// Filter current slots up to the captured list length, retaining shared descriptors.
    ///
    /// # Errors
    /// Stops at the first credential lookup failure, returning no partial list.
    pub fn get_available(&self) -> Result<Vec<Arc<RwLock<Model>>>, CatalogError> {
        let list = self.get_all();
        let length = list.read().unwrap_or_else(PoisonError::into_inner).len();
        let mut available = Vec::new();
        for index in 0..length {
            let model = list
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .get(index)
                .cloned();
            if let Some(model) = model
                && self.has_configured_auth(&model)?
            {
                available.push(model);
            }
        }
        Ok(available)
    }
    /// Prefer credential metadata, otherwise describe the configured key without executing it.
    ///
    /// # Errors
    /// Returns the original credential metadata failure.
    pub fn get_provider_auth_status(&self, provider: &str) -> Result<AuthStatus, CatalogError> {
        let status = self.auth_storage.get_auth_status(provider)?;
        if status.source.is_some() {
            return Ok(status);
        }
        let config = self.request_config(provider);
        let Some(key) = config
            .as_ref()
            .and_then(|c| c.api_key.as_ref())
            .filter(|key| !key.is_empty())
        else {
            return Ok(status);
        };
        let (source, label) = if key.starts_with('!') {
            (AuthSource::ModelsJsonCommand, None)
        } else if std::env::var(key).is_ok_and(|value| !value.is_empty()) {
            (AuthSource::Environment, Some(key.clone()))
        } else {
            (AuthSource::ModelsJsonKey, None)
        };
        Ok(AuthStatus {
            configured: true,
            source: Some(source),
            label,
        })
    }
    /// Prefer retained labels, then current OAuth, shipped labels and the identifier.
    #[must_use]
    pub fn get_provider_display_name(&self, provider: &str) -> String {
        let retained = self
            .state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .registered_providers
            .get(provider)
            .map(|(_, c)| SharedHandle::clone(c));
        let registered = retained.map(|config| {
            let config = config.read().unwrap_or_else(PoisonError::into_inner);
            (config.name.clone(), config.callbacks.oauth.clone())
        });
        if let Some((name, oauth)) = registered {
            if let Some(name) = name {
                return name;
            }
            if let Some(oauth) = oauth {
                return oauth.name().to_owned();
            }
        }
        if let Some(oauth) = maestro_models::get_oauth_provider(provider) {
            return oauth.name().to_owned();
        }
        crate::BUILT_IN_PROVIDER_DISPLAY_NAMES
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(provider)
            .cloned()
            .unwrap_or_else(|| provider.to_owned())
    }
    /// Inspect the accepted stored credential kind, independently of current key selection.
    #[must_use]
    pub fn is_using_oauth(&self, model: &Arc<RwLock<Model>>) -> bool {
        let provider = read_provider(model);
        matches!(
            self.auth_storage.get(&provider),
            Some(maestro_credentials::AuthCredential::OAuth(_))
        )
    }
}
