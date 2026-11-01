//! Native ownership can end only after the compiled guest releases its final provider handle.
use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// A selected native implementation with independently observed final release.
pub(super) struct Provider(pub(super) Arc<AtomicUsize>);
impl Drop for Provider {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "component-retained"
    }
    fn name(&self) -> &'static str {
        " Literal login label "
    }
    fn uses_callback_server(&self) -> Option<bool> {
        Some(false)
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.login(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.refresh_token(credentials, fetch)
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, OAuthError> {
        if credentials.extra.get("deny") == Some(&serde_json::Value::Bool(true)) {
            return Err(DiagnosticErrorInfo::new(None, "literal key denial").into());
        }
        maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER.get_api_key(credentials)
    }
}
