//! Configured-authentication availability and status, plus request-key selection.
use super::backend::AuthStorageFuture;
use super::state::AuthStorage;
use super::types::{AuthCredential, AuthSource, AuthStatus, decode};
use crate::{ConfigValueOperations, resolve_config_value};
use maestro_models::{
    OAuthError, OAuthProviderHandle, find_env_keys, get_env_api_key, get_oauth_provider,
    providers::chat::cloudflare::is_cloudflare_provider,
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
    /// Invoke the retained fallback without holding the state lock.
    fn fallback_api_key(&self, provider: &str) -> Result<Option<String>, OAuthError> {
        let resolver = self.state().fallback_resolver.clone();
        resolver.map_or(Ok(None), |resolve| resolve(provider))
    }

    /// Whether any authentication is configured, without resolving stored helpers or refreshing tokens;
    /// the supplied fallback resolver is the caller's and runs as given.
    /// Cloudflare requires a nonempty API-key source or stored OAuth with a registered handler.
    ///
    /// # Errors
    /// Returns the original fallback resolver failure.
    pub fn has_auth(&self, provider: &str) -> Result<bool, OAuthError> {
        let cloudflare = is_cloudflare_provider(provider);
        // Release handler handles before taking the credential lock.
        let oauth = cloudflare && get_oauth_provider(provider).is_some();
        let configured = {
            let state = self.state();
            let runtime = state.runtime_overrides.get(provider);
            let stored = state.data.get(provider);
            if cloudflare {
                let stored = match stored.and_then(decode) {
                    Some(AuthCredential::ApiKey(key)) => !key.key.is_empty(),
                    Some(AuthCredential::OAuth(_)) => oauth,
                    None => false,
                };
                runtime.is_some_and(|key| !key.is_empty()) || stored
            } else {
                runtime.is_some() || stored.is_some()
            }
        };
        Ok(configured
            || get_env_api_key(provider).is_some()
            || self
                .fallback_api_key(provider)?
                .is_some_and(|key| !key.is_empty()))
    }

    /// Configured-authentication metadata without key values, stored helper resolution or refresh;
    /// the supplied fallback resolver is the caller's and runs as given.
    ///
    /// # Errors
    /// Returns the original fallback resolver failure.
    pub fn get_auth_status(&self, provider: &str) -> Result<AuthStatus, OAuthError> {
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
        } else if self
            .fallback_api_key(provider)?
            .is_some_and(|key| !key.is_empty())
        {
            (
                false,
                Some(AuthSource::Fallback),
                Some("custom provider config".to_owned()),
            )
        } else {
            (false, None, None)
        };
        Ok(AuthStatus {
            configured,
            source,
            label,
        })
    }

    /// Select the runtime override or stored record without awaiting.
    fn select_stored(
        &self,
        provider_id: &str,
        operations: &dyn ConfigValueOperations,
    ) -> Selection {
        let (known_kind, decoded) = {
            let state = self.state();
            if let Some(key) = state
                .runtime_overrides
                .get(provider_id)
                .filter(|key| !key.is_empty())
            {
                return Selection::Done(Ok(Some(key.clone())));
            }
            let Some(record) = state.data.get(provider_id) else {
                return Selection::Ambient;
            };
            (
                matches!(
                    record.get("type").and_then(|kind| kind.as_str()),
                    Some("api_key" | "oauth")
                ),
                decode(record),
            )
        };
        match (known_kind, decoded) {
            (true, None) => Selection::Done(Ok(None)),
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
                            .map(|key| Some(key.into_owned())),
                    );
                }
                Selection::Refresh(provider)
            }
            (_, None) => Selection::Ambient,
        }
    }

    /// Environment key, then the fallback resolver unless `include_fallback` is false.
    fn ambient_key(
        &self,
        provider_id: &str,
        include_fallback: bool,
    ) -> Result<Option<String>, OAuthError> {
        if let Some(key) = get_env_api_key(provider_id).filter(|key| !key.is_empty()) {
            return Ok(Some(key));
        }
        if include_fallback {
            self.fallback_api_key(provider_id)
        } else {
            Ok(None)
        }
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
            Ok(None) => self.ambient_key(provider_id, include_fallback),
            Err(error) => {
                self.state().errors.push(error);
                self.reload();
                let record = self.state().data.get(provider_id).and_then(decode);
                match record {
                    Some(AuthCredential::OAuth(stored)) if (self.clock)() < stored.expires => {
                        Ok(Some(provider.get_api_key(&stored)?.into_owned()))
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
    /// - A malformed `api_key` or `oauth` record, or an `oauth` record of an unregistered provider,
    ///   yields `None` without consulting later sources.
    /// - An unexpired token yields the provider's extracted key; an extraction failure is returned.
    /// - An expired token refreshes through the backend's `with_lock_async`. Failures are
    ///   recorded for [`AuthStorage::drain_errors`] and the store is reloaded; only a stored
    ///   `oauth` record valid at that moment then supplies a key, otherwise the result is
    ///   `None` with no environment or fallback lookup. A refresh that yields nothing
    ///   continues to the environment and fallback.
    ///
    /// # Errors
    /// Returns the original provider key extraction or fallback resolver failure.
    pub fn get_api_key<'a>(
        &'a self,
        provider_id: &'a str,
        include_fallback: bool,
        operations: &dyn ConfigValueOperations,
    ) -> AuthStorageFuture<'a, Result<Option<String>, OAuthError>> {
        match self.select_stored(provider_id, operations) {
            Selection::Done(result) => Box::pin(std::future::ready(result)),
            Selection::Ambient => Box::pin(std::future::ready(
                self.ambient_key(provider_id, include_fallback),
            )),
            Selection::Refresh(provider) => {
                Box::pin(self.refreshed_key(provider_id, provider, include_fallback))
            }
        }
    }
}
