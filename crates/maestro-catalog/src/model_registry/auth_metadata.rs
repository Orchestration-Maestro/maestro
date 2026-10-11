//! Authentication presence and provider metadata without request resolution.
use super::{ModelRegistry, read_provider};
use maestro_credentials::{AuthSource, AuthStatus, ConfigValueOperations};
use maestro_models::Model;
use std::sync::{Arc, PoisonError, RwLock};
impl ModelRegistry {
    /// Check credential presence, then freshly read the configured provider identity.
    #[must_use]
    pub fn has_configured_auth(&self, model: &Arc<RwLock<Model>>) -> bool {
        let provider = read_provider(model);
        if self.auth_storage.has_auth(&provider) {
            return true;
        }
        let provider = read_provider(model);
        self.provider_configs
            .get(&provider)
            .is_some_and(|c| c.api_key.is_some())
    }
    /// Filter current slots up to the captured list length, retaining shared descriptors.
    #[must_use]
    pub fn get_available(&self) -> Vec<Arc<RwLock<Model>>> {
        let list = self.get_all();
        let length = list.read().unwrap_or_else(PoisonError::into_inner).len();
        (0..length)
            .filter_map(|index| {
                let model = list
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(index)
                    .cloned()?;
                self.has_configured_auth(&model).then_some(model)
            })
            .collect()
    }
    /// Prefer credential metadata, otherwise describe the configured key without executing it.
    #[must_use]
    pub fn get_provider_auth_status(
        &self,
        provider: &str,
        operations: &dyn ConfigValueOperations,
    ) -> AuthStatus {
        let status = self.auth_storage.get_auth_status(provider);
        if status.source.is_some() {
            return status;
        }
        let Some(key) = self
            .provider_configs
            .get(provider)
            .and_then(|c| c.api_key.as_ref())
        else {
            return status;
        };
        let (source, label) = if key.starts_with('!') {
            (AuthSource::ModelsJsonCommand, None)
        } else if operations
            .environment(key)
            .is_some_and(|value| !value.is_empty())
        {
            (AuthSource::Environment, Some(key.clone()))
        } else {
            (AuthSource::ModelsJsonKey, None)
        };
        AuthStatus {
            configured: true,
            source: Some(source),
            label,
        }
    }
    /// Prefer the current OAuth label, then the shared shipped table, then the identifier.
    #[must_use]
    pub fn get_provider_display_name(&self, provider: &str) -> String {
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
