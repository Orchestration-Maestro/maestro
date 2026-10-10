//! Configured-authentication availability and status without key values.
use super::backend::AuthStorageFuture;
use super::state::AuthStorage;
use super::types::{AuthCredential, AuthSource, AuthStatus, decode};
use crate::{ConfigValueOperations, resolve_config_value};
use maestro_models::{
    OAuthError, OAuthProviderHandle, find_env_keys, get_env_api_key, get_oauth_provider,
};

/// How the synchronous prefix of a key request ended.
enum Selection {
    /// The request is decided.
    Done(Result<Option<String>, OAuthError>),
    /// No stored source applies; continue with environment and fallback.
    Ambient,
    /// The stored token has expired and needs a locked refresh.
    Refresh(OAuthProviderHandle),
}

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

    /// Select the runtime override or stored record without awaiting.
    fn select_stored(
        &self,
        provider_id: &str,
        operations: &dyn ConfigValueOperations,
    ) -> Selection {
        let (runtime, record) = {
            let state = self.state();
            (
                state
                    .runtime_overrides
                    .get(provider_id)
                    .filter(|key| !key.is_empty())
                    .cloned(),
                state.data.get(provider_id).cloned(),
            )
        };
        if let Some(key) = runtime {
            return Selection::Done(Ok(Some(key)));
        }
        let Some(record) = record else {
            return Selection::Ambient;
        };
        match (
            record.get("type").and_then(|kind| kind.as_str()),
            decode(&record),
        ) {
            (Some("api_key" | "oauth"), None) => Selection::Done(Ok(None)),
            (_, Some(AuthCredential::ApiKey(stored))) => {
                Selection::Done(Ok(resolve_config_value(&stored.key, operations)))
            }
            (_, Some(AuthCredential::OAuth(stored))) => {
                let Some(provider) = get_oauth_provider(provider_id) else {
                    return Selection::Done(Ok(None));
                };
                if (self.clock)() < stored.expires {
                    return Selection::Done(
                        provider
                            .get_api_key(&stored)
                            .map(|key| Some(key.to_owned())),
                    );
                }
                Selection::Refresh(provider)
            }
            (_, None) => Selection::Ambient,
        }
    }

    /// Environment key, then the fallback resolver unless `include_fallback` is false.
    fn ambient_key(&self, provider_id: &str, include_fallback: bool) -> Option<String> {
        if let Some(key) = get_env_api_key(provider_id).filter(|key| !key.is_empty()) {
            return Some(key);
        }
        let resolver = self.state().fallback_resolver.clone();
        resolver
            .filter(|_| include_fallback)
            .and_then(|resolve| resolve(provider_id))
    }

    /// Refresh an expired token under the lock; a failure is recorded and only a newly valid
    /// stored token may then supply the key.
    async fn refreshed_key(
        &self,
        provider_id: &str,
        provider: OAuthProviderHandle,
        include_fallback: bool,
    ) -> Result<Option<String>, OAuthError> {
        match self
            .refresh_oauth_token_with_lock(provider_id, &provider)
            .await
        {
            Ok(Some(key)) => Ok(Some(key)),
            Ok(None) => Ok(self.ambient_key(provider_id, include_fallback)),
            Err(error) => {
                self.state().errors.push(error);
                self.reload();
                let record = self.state().data.get(provider_id).and_then(decode);
                match record {
                    Some(AuthCredential::OAuth(stored)) if (self.clock)() < stored.expires => {
                        Ok(Some(provider.get_api_key(&stored)?.to_owned()))
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    /// Select a request key: runtime override, stored record, environment, then fallback.
    ///
    /// The returned future is `Send` on native targets and does not borrow `operations`:
    /// stored keys resolve before it is built. Stored records decide as follows.
    /// - A complete `api_key` record resolves through the cached configured-value
    ///   resolver; `None` and the empty string stop the lookup.
    /// - A malformed `api_key` or `oauth` record, or one of an unregistered provider,
    ///   yields `None` without consulting later sources.
    /// - An unexpired token yields the provider's extracted key; an extraction failure is returned.
    /// - An expired token refreshes under the backend's asynchronous lock. Failures are
    ///   recorded for [`AuthStorage::drain_errors`] and the store is reloaded; only a stored
    ///   `oauth` record valid at that moment then supplies a key, otherwise the result is
    ///   `None` with no environment or fallback lookup. A refresh that yields nothing
    ///   continues to the environment and fallback.
    ///
    /// # Errors
    /// Returns the provider's key extraction failure.
    pub fn get_api_key<'a>(
        &'a self,
        provider_id: &'a str,
        include_fallback: bool,
        operations: &dyn ConfigValueOperations,
    ) -> AuthStorageFuture<'a, Result<Option<String>, OAuthError>> {
        match self.select_stored(provider_id, operations) {
            Selection::Done(result) => Box::pin(std::future::ready(result)),
            Selection::Ambient => Box::pin(std::future::ready(Ok(
                self.ambient_key(provider_id, include_fallback)
            ))),
            Selection::Refresh(provider) => {
                Box::pin(self.refreshed_key(provider_id, provider, include_fallback))
            }
        }
    }
}
