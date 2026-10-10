//! Controlled OAuth providers and callbacks shared by the key selection and refresh tests.
#![allow(dead_code)] // each test binary uses a different part of these helpers
use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, OAuthCredentials, OAuthError, OAuthProviderInterface,
    register_oauth_provider, unregister_oauth_provider,
};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Expiry far in the future, in epoch milliseconds.
pub const FUTURE: f64 = 4_102_444_800_000.0;

/// Error carrying only an authored message.
pub fn error(message: &str) -> OAuthError {
    DiagnosticErrorInfo {
        name: None,
        message: message.to_owned(),
        stack: None,
        code: None,
    }
    .into()
}

/// Credentials whose access token is also the key the provider extracts.
pub fn credentials(access: &str, expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        refresh: format!("refresh-{access}"),
        access: access.to_owned(),
        expires,
        extra: json!({"extra": ["x", "x", "y"]})
            .as_object()
            .unwrap()
            .clone(),
    }
}

/// What a controlled refresh does with the credentials it receives.
pub type RefreshBehavior =
    Box<dyn Fn(OAuthCredentials) -> BoxFuture<Result<OAuthCredentials, OAuthError>> + Send + Sync>;

/// Provider whose key is the access token; the access token `extract-fails` has no key.
pub struct ControlledProvider {
    /// Registered identifier.
    id: String,
    /// Refresh calls made so far.
    pub refreshes: AtomicUsize,
    /// Result of a refresh.
    pub on_refresh: RefreshBehavior,
    /// Result of a login.
    pub login_result: Mutex<Option<Result<OAuthCredentials, OAuthError>>>,
}

impl ControlledProvider {
    /// Provider that refreshes to access token `new`.
    pub fn new(id: &str) -> Self {
        Self::refreshing(
            id,
            Box::new(|_| Box::pin(async { Ok(credentials("new", FUTURE)) })),
        )
    }

    /// Provider with a custom refresh.
    pub fn refreshing(id: &str, on_refresh: RefreshBehavior) -> Self {
        Self {
            id: id.to_owned(),
            refreshes: AtomicUsize::new(0),
            on_refresh,
            login_result: Mutex::new(Some(Ok(credentials("login", FUTURE)))),
        }
    }

    /// Register this provider and return the shared handle.
    pub fn register(self) -> Arc<Self> {
        let provider = Arc::new(self);
        register_oauth_provider(provider.clone());
        provider
    }
}

impl OAuthProviderInterface for ControlledProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }

    fn login(
        &self,
        callbacks: maestro_models::OAuthCallbacks,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let outcome = callbacks
            .on_progress(&self.id)
            .and_then(|()| self.login_result.lock().unwrap().take().unwrap());
        Box::pin(async move { outcome })
    }

    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        (self.on_refresh)(credentials)
    }

    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        if credentials.access == "extract-fails" {
            return Err(error("extract failed"));
        }
        Ok(&credentials.access)
    }
}

/// Unregisters a provider id when dropped.
pub struct Unregister(pub String);

impl Drop for Unregister {
    fn drop(&mut self) {
        unregister_oauth_provider(&self.0);
    }
}
