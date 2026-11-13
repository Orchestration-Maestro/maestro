//! The login provider registry, its records and the proof-key helper.
//!
//! The registry starts empty. A composition installs the built-in providers as its base
//! layer with [`install_built_in_oauth_providers`]; `maestro-providers` owns those providers
//! and their login flows.
pub mod proof_key;
pub mod types;

use crate::SharedHandle;
use indexmap::IndexMap;
pub use types::{
    OAuthAuthInfo, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthLoginCallbacks, OAuthPrompt,
    OAuthProvider, OAuthProviderId, OAuthProviderInterface, OAuthSelectOption, OAuthSelectPrompt,
};

/// Shared provider implementation on native targets.
#[cfg(not(target_arch = "wasm32"))]
pub type OAuthProviderHandle = SharedHandle<dyn OAuthProviderInterface + Send + Sync>;
/// Shared provider implementation on browser targets.
#[cfg(target_arch = "wasm32")]
pub type OAuthProviderHandle = SharedHandle<dyn OAuthProviderInterface>;

/// Provider membership: the installed built-ins under the live registrations.
#[derive(Default)]
struct Registry {
    /// Built-in implementations in their installed order; they hold the first positions.
    built_ins: IndexMap<OAuthProviderId, OAuthProviderHandle>,
    /// Registrations in their order; one carrying a built-in's identifier shadows that built-in.
    registered: IndexMap<OAuthProviderId, OAuthProviderHandle>,
}

impl Registry {
    /// The registration for `id`, or else its built-in.
    fn get(&self, id: &str) -> Option<&OAuthProviderHandle> {
        self.registered.get(id).or_else(|| self.built_ins.get(id))
    }

    /// Built-ins in their positions, each replaced by its registration, then the other
    /// registrations in their order.
    fn ordered(&self) -> impl Iterator<Item = &OAuthProviderHandle> {
        let built_ins = self
            .built_ins
            .iter()
            .map(move |(id, built_in)| self.registered.get(id).unwrap_or(built_in));
        let others = self
            .registered
            .iter()
            .filter(move |(id, _)| !self.built_ins.contains_key(id.as_str()))
            .map(|(_, provider)| provider);
        built_ins.chain(others)
    }
}

/// Process-wide provider membership on native targets.
#[cfg(not(target_arch = "wasm32"))]
static PROVIDERS: std::sync::LazyLock<std::sync::Mutex<Registry>> =
    std::sync::LazyLock::new(std::sync::Mutex::default);
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Browser-local provider membership.
    static PROVIDERS: std::cell::RefCell<Registry> = std::cell::RefCell::default();
}

crate::records::platform_store_accessor!(
    /// Perform map operations without invoking provider methods or retiring values.
    with_providers, PROVIDERS, Registry
);

/// Look up an exact literal provider identifier.
#[must_use]
pub fn get_oauth_provider(id: &str) -> Option<OAuthProviderHandle> {
    with_providers(|registry| registry.get(id).cloned())
}

/// Snapshot ordered membership while sharing provider implementations.
#[must_use]
pub fn get_oauth_providers() -> Vec<OAuthProviderHandle> {
    with_providers(|registry| registry.ordered().cloned().collect())
}

/// Append a provider or replace its implementation without moving its slot.
pub fn register_oauth_provider(provider: OAuthProviderHandle) {
    let id = provider.id().to_owned();
    let retired = with_providers(|registry| registry.registered.insert(id, provider));
    drop(retired);
}

/// Restore a built-in implementation or remove a custom provider.
pub fn unregister_oauth_provider(id: &str) {
    let retired = with_providers(|registry| registry.registered.shift_remove(id));
    drop(retired);
}

/// Drop every registration, leaving the built-ins in their positions.
pub fn reset_oauth_providers() {
    let retired = with_providers(|registry| std::mem::take(&mut registry.registered));
    drop(retired);
}

/// Places `providers` as the registry's base layer, in the order given.
///
/// Built-ins hold the first positions wherever registrations already stand. A registration
/// carrying a built-in's identifier shadows that built-in in its position, and removing the
/// registration restores the built-in there. Each call replaces the earlier base layer, so
/// installing the same providers again changes neither membership nor order.
pub fn install_built_in_oauth_providers(providers: impl IntoIterator<Item = OAuthProviderHandle>) {
    let built_ins = providers
        .into_iter()
        .map(|provider| (provider.id().to_owned(), provider))
        .collect();
    let retired = with_providers(|registry| std::mem::replace(&mut registry.built_ins, built_ins));
    drop(retired);
}

/// Delegate one refresh without an expiry decision or key extraction.
#[must_use]
pub fn refresh_oauth_token(
    provider_id: &str,
    credentials: OAuthCredentials,
    fetch: Option<crate::Fetch>,
) -> crate::BoxFuture<Result<OAuthCredentials, OAuthError>> {
    let provider = get_oauth_provider(provider_id)
        .ok_or_else(|| OAuthError::message(format!("Unknown OAuth provider: {provider_id}")));
    Box::pin(async move { provider?.refresh_token(credentials, fetch).await })
}

/// Selected credentials and the provider's extracted request key.
pub struct OAuthApiKey {
    /// Credentials selected or returned by the single refresh.
    pub new_credentials: OAuthCredentials,
    /// Key returned by the selected provider.
    pub api_key: String,
}

/// Resolve a key, refreshing once when the supplied credentials have expired.
pub fn get_oauth_api_key(
    provider_id: &str,
    credentials: &IndexMap<OAuthProviderId, OAuthCredentials>,
    fetch: Option<crate::Fetch>,
) -> crate::BoxFuture<Result<Option<OAuthApiKey>, OAuthError>> {
    get_oauth_api_key_with_clock(
        provider_id,
        credentials,
        fetch,
        crate::records::diagnostics::timestamp_now,
    )
}

/// Resolve using a clock read only after provider and credential selection.
fn get_oauth_api_key_with_clock(
    provider_id: &str,
    credentials: &IndexMap<OAuthProviderId, OAuthCredentials>,
    fetch: Option<crate::Fetch>,
    clock: impl FnOnce() -> f64,
) -> crate::BoxFuture<Result<Option<OAuthApiKey>, OAuthError>> {
    let Some(provider) = get_oauth_provider(provider_id) else {
        let error = OAuthError::message(format!("Unknown OAuth provider: {provider_id}"));
        return Box::pin(async { Err(error) });
    };
    let Some(credentials) = credentials.get(provider_id) else {
        return Box::pin(async { Ok(None) });
    };
    let expired = clock() >= credentials.expires;
    let credentials = credentials.clone();
    let provider_id = provider_id.to_owned();
    Box::pin(async move {
        let credentials = if expired {
            provider
                .refresh_token(credentials, fetch)
                .await
                .map_err(|_| {
                    OAuthError::message(format!(
                        "Could not refresh the OAuth token for {provider_id}"
                    ))
                })?
        } else {
            credentials
        };
        let api_key = provider.get_api_key(&credentials)?.into_owned();
        Ok(Some(OAuthApiKey {
            new_credentials: credentials,
            api_key,
        }))
    })
}

#[cfg(test)]
mod tests;
