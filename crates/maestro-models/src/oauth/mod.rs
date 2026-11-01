#![doc = include_str!("../../../../docs/models/oauth.md")]
pub mod device;
pub mod oauth_page;
pub mod pkce;
pub mod responses;
pub mod subscription;
pub mod types;

mod callback;
mod native;
pub use device::github_copilot::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};

pub(crate) use maestro_text::is_whitespace_scalar as authorization_whitespace;

use crate::SharedHandle;
use indexmap::IndexMap;
pub use responses::openai_codex::{
    OPENAI_CODEX_OAUTH_PROVIDER, login_openai_codex, refresh_openai_codex_token,
};
pub use subscription::anthropic::{
    ANTHROPIC_OAUTH_PROVIDER, login_anthropic, refresh_anthropic_token,
};
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

/// Original account implementations in their registration order.
fn builtin_providers() -> IndexMap<OAuthProviderId, OAuthProviderHandle> {
    let providers: [OAuthProviderHandle; 3] = [
        SharedHandle::new(subscription::anthropic::AnthropicOAuthProvider),
        SharedHandle::new(device::github_copilot::GitHubCopilotOAuthProvider),
        SharedHandle::new(responses::openai_codex::OpenAICodexOAuthProvider),
    ];
    providers
        .into_iter()
        .map(|provider| (provider.id().to_owned(), provider))
        .collect()
}

/// Process-wide provider membership on native targets.
#[cfg(not(target_arch = "wasm32"))]
static PROVIDERS: std::sync::LazyLock<
    std::sync::Mutex<IndexMap<OAuthProviderId, OAuthProviderHandle>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(builtin_providers()));
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Browser-local provider membership.
    static PROVIDERS: std::cell::RefCell<IndexMap<OAuthProviderId, OAuthProviderHandle>> =
        std::cell::RefCell::new(builtin_providers());
}

crate::records::platform_store_accessor!(
    /// Perform map operations without invoking provider methods or retiring values.
    with_providers, PROVIDERS, IndexMap<OAuthProviderId, OAuthProviderHandle>
);

/// Look up an exact literal provider identifier.
#[must_use]
pub fn get_oauth_provider(id: &str) -> Option<OAuthProviderHandle> {
    with_providers(|providers| providers.get(id).cloned())
}

/// Snapshot ordered membership while sharing provider implementations.
#[must_use]
pub fn get_oauth_providers() -> Vec<OAuthProviderHandle> {
    with_providers(|providers| providers.values().cloned().collect())
}

/// Append a provider or replace its implementation without moving its slot.
pub fn register_oauth_provider(provider: OAuthProviderHandle) {
    let id = provider.id().to_owned();
    let retired = with_providers(|providers| providers.insert(id, provider));
    drop(retired);
}

/// Restore a built-in implementation or remove a custom provider.
pub fn unregister_oauth_provider(id: &str) {
    let replacement = builtin_providers().shift_remove(id);
    let retired = with_providers(|providers| match replacement {
        Some(provider) => providers.insert(id.to_owned(), provider),
        None => providers.shift_remove(id),
    });
    drop(retired);
}

/// Restore original provider membership and order.
pub fn reset_oauth_providers() {
    let replacement = builtin_providers();
    let retired = with_providers(|providers| std::mem::replace(providers, replacement));
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
                    OAuthError::message(format!("Failed to refresh OAuth token for {provider_id}"))
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
