//! Login, provider listing and the locked refresh of an expired token.
use super::state::AuthStorage;
use super::types::{AuthCredential, AuthStorageError, decode, parse_storage_data, stored_record};
use indexmap::IndexMap;
use maestro_models::{
    DiagnosticErrorInfo, OAuthCallbacks, OAuthError, OAuthProviderHandle, get_oauth_api_key,
    get_oauth_provider,
};

/// A selected key and the text to store, either of which may be absent.
type Refreshed = (Option<String>, Option<String>);

impl AuthStorage {
    /// Log in through the registered provider and store its credentials as `oauth` records.
    ///
    /// Persistence follows [`AuthStorage::set`].
    ///
    /// # Errors
    /// Returns `Unknown OAuth provider: <id>` for an unregistered provider, or the provider's login failure.
    pub async fn login(
        &self,
        provider_id: &str,
        callbacks: OAuthCallbacks,
    ) -> Result<(), OAuthError> {
        let Some(provider) = get_oauth_provider(provider_id) else {
            return Err(DiagnosticErrorInfo {
                name: None,
                message: format!("Unknown OAuth provider: {provider_id}"),
                stack: None,
                code: None,
            }
            .into());
        };
        let credentials = provider.login(callbacks, None).await?;
        self.set(provider_id, AuthCredential::OAuth(credentials));
        Ok(())
    }

    /// The registered OAuth providers in registration order, sharing their implementations.
    #[must_use]
    pub fn get_oauth_providers(&self) -> Vec<OAuthProviderHandle> {
        maestro_models::get_oauth_providers()
    }

    /// Refresh under the backend's asynchronous exclusion, returning the selected key.
    ///
    /// `None` means the stored record is no longer a complete OAuth record.
    pub(super) async fn refresh_oauth_token_with_lock(
        &self,
        provider_id: &str,
        provider: &OAuthProviderHandle,
    ) -> Result<Option<String>, AuthStorageError> {
        let mut key = None;
        self.storage
            .with_lock_async(Box::new(|current| {
                Box::pin(async {
                    let (selected, text) = self
                        .refresh_under_lock(provider_id, provider, current)
                        .await?;
                    key = selected;
                    Ok(text)
                })
            }))
            .await?;
        Ok(key)
    }

    /// Accept the reread document, reuse a valid token or refresh once and publish before writing.
    async fn refresh_under_lock(
        &self,
        provider_id: &str,
        provider: &OAuthProviderHandle,
        current: Option<String>,
    ) -> Result<Refreshed, AuthStorageError> {
        let record = {
            let mut state = self.state();
            state.data = parse_storage_data(current.as_deref())?;
            state.load_error = false;
            state.data.get(provider_id).cloned()
        };
        let Some(AuthCredential::OAuth(credentials)) = record.as_ref().and_then(decode) else {
            return Ok((None, None));
        };
        if (self.clock)() < credentials.expires {
            return Ok((Some(provider.get_api_key(&credentials)?.to_owned()), None));
        }
        let selected = IndexMap::from([(provider_id.to_owned(), credentials)]);
        let Some(refreshed) = get_oauth_api_key(provider_id, &selected, None).await? else {
            return Ok((None, None));
        };
        let mut state = self.state();
        state.data.insert(
            provider_id.to_owned(),
            stored_record(AuthCredential::OAuth(refreshed.new_credentials)),
        );
        state.load_error = false;
        let text = serde_json::to_string_pretty(&state.data)?;
        Ok((Some(refreshed.api_key), Some(text)))
    }
}
