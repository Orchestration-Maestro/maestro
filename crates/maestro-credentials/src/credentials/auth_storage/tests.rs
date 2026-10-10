//! Expiry boundaries decided by the substituted clock.
#![cfg(not(target_arch = "wasm32"))]
use super::AuthStorage;
use super::types::{AuthCredential, AuthStorageData, stored_record};
use crate::ConfigValueOperations;
use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface, register_oauth_provider, unregister_oauth_provider,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Run `future` to completion on a fresh current-thread runtime.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// Operations that never resolve anything.
struct Nothing;

impl ConfigValueOperations for Nothing {
    fn environment(&self, _name: &str) -> Option<String> {
        None
    }

    fn execute(&self, _command: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Provider whose key is the access token; its refresh succeeds only when `refreshes_to` is set.
struct Counting {
    /// Registered identifier.
    id: &'static str,
    /// Refresh calls made.
    refreshes: AtomicUsize,
    /// Access token returned by a refresh, or `None` to fail it.
    refreshes_to: Option<&'static str>,
}

impl OAuthProviderInterface for Counting {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.id
    }

    fn login(
        &self,
        _callbacks: OAuthCallbacks,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(failure("no login")) })
    }

    fn refresh_token(
        &self,
        _credentials: OAuthCredentials,
        _fetch: Option<maestro_models::Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        let outcome = self
            .refreshes_to
            .map(|access| credentials(access, 4_102_444_800_000.0))
            .ok_or_else(|| failure("refused"));
        Box::pin(async move { outcome })
    }

    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        Ok(&credentials.access)
    }
}

/// Error carrying only a message.
fn failure(message: &str) -> OAuthError {
    DiagnosticErrorInfo {
        name: None,
        message: message.to_owned(),
        stack: None,
        code: None,
    }
    .into()
}

/// OAuth tokens with the given access token and expiry.
fn credentials(access: &str, expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        refresh: "refresh".to_owned(),
        access: access.to_owned(),
        expires,
        extra: serde_json::Map::new(),
    }
}

/// Memory storage holding one OAuth record for `id`, with the clock fixed at 1000 ms.
fn storage_at_1000(id: &str, access: &str, expires: f64) -> AuthStorage {
    let mut data = AuthStorageData::new();
    data.insert(
        id.to_owned(),
        AuthCredential::OAuth(credentials(access, expires)),
    );
    AuthStorage::in_memory(data).with_clock(|| 1000.0)
}

/// Register a provider and return it with the id's unregistration guard.
fn register(provider: Counting) -> Arc<Counting> {
    let provider = Arc::new(provider);
    register_oauth_provider(provider.clone());
    provider
}

#[test]
fn request_expiry_equality_starts_refresh() {
    block_on(async {
        let provider = register(Counting {
            id: "unit-request",
            refreshes: AtomicUsize::new(0),
            refreshes_to: Some("refreshed"),
        });
        let storage = storage_at_1000("unit-request", "external", 5000.0);
        let stale = credentials("stale", 1000.0);
        storage.state().data.insert(
            "unit-request".to_owned(),
            stored_record(AuthCredential::OAuth(stale)),
        );
        let key = storage.get_api_key("unit-request", true, &Nothing).await;
        assert_eq!(key.unwrap().as_deref(), Some("external"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 0);
        unregister_oauth_provider("unit-request");
    });
}

#[test]
fn locked_expiry_equality_is_not_reused() {
    block_on(async {
        let provider = register(Counting {
            id: "unit-locked",
            refreshes: AtomicUsize::new(0),
            refreshes_to: Some("refreshed"),
        });
        let storage = storage_at_1000("unit-locked", "stored", 1000.0);
        let key = storage.get_api_key("unit-locked", true, &Nothing).await;
        assert_eq!(key.unwrap().as_deref(), Some("refreshed"));
        assert_eq!(provider.refreshes.load(Ordering::SeqCst), 1);
        unregister_oauth_provider("unit-locked");

        register(Counting {
            id: "unit-recovery",
            refreshes: AtomicUsize::new(0),
            refreshes_to: None,
        });
        let storage = storage_at_1000("unit-recovery", "stored", 1000.0);
        let key = storage.get_api_key("unit-recovery", true, &Nothing).await;
        assert_eq!(key.unwrap(), None);
        assert_eq!(storage.drain_errors().len(), 1);
        unregister_oauth_provider("unit-recovery");
    });
}
