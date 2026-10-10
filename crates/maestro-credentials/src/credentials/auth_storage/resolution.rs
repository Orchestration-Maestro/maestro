//! Configured-authentication availability and status without key values.
use super::state::AuthStorage;
use super::types::{AuthSource, AuthStatus};
use maestro_models::{find_env_keys, get_env_api_key};

impl AuthStorage {
    /// Whether the fallback resolver supplies a nonempty key, called without the state lock.
    fn fallback_supplies(&self, provider: &str) -> bool {
        let resolver = self.state().fallback_resolver.clone();
        resolver.is_some_and(|resolve| resolve(provider).is_some_and(|key| !key.is_empty()))
    }

    /// Whether any authentication is configured, without resolving stored helpers or refreshing tokens;
    /// the supplied fallback resolver is the caller's and runs as given.
    #[must_use]
    pub fn has_auth(&self, provider: &str) -> bool {
        let configured = {
            let state = self.state();
            state.runtime_overrides.contains_key(provider) || state.data.contains_key(provider)
        };
        configured || get_env_api_key(provider).is_some() || self.fallback_supplies(provider)
    }

    /// Configured-authentication metadata without key values, stored helper resolution or refresh;
    /// the supplied fallback resolver is the caller's and runs as given.
    #[must_use]
    pub fn get_auth_status(&self, provider: &str) -> AuthStatus {
        let (stored, runtime) = {
            let state = self.state();
            (
                state.data.contains_key(provider),
                state.runtime_overrides.contains_key(provider),
            )
        };
        let (configured, source, label) = if stored {
            (true, Some(AuthSource::Stored), None)
        } else if runtime {
            (
                false,
                Some(AuthSource::Runtime),
                Some("--api-key".to_owned()),
            )
        } else if let Some(name) = find_env_keys(provider).and_then(|keys| keys.into_iter().next())
        {
            (false, Some(AuthSource::Environment), Some(name))
        } else if self.fallback_supplies(provider) {
            (
                false,
                Some(AuthSource::Fallback),
                Some("custom provider config".to_owned()),
            )
        } else {
            (false, None, None)
        };
        AuthStatus {
            configured,
            source,
            label,
        }
    }
}
