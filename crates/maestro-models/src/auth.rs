//! Supplied request credentials and metadata-only resolution.
use crate::{Cancellation, Failure};
use std::{future::Future, pin::Pin};

/// Owned sensitive text with fixed redacted Debug; not memory zeroization.
#[derive(Clone)]
pub struct SecretString(String);
impl SecretString {
    /// Own supplied text without credential discovery.
    pub fn new(value: String) -> Self {
        Self(value)
    }
    /// Deliberate sensitive access for an authorized adapter or credential owner.
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
/// Authentication for one selected provider. Source labels must be non-secret.
#[derive(Clone, Debug)]
pub enum RequestAuth {
    /// Supplied sensitive credential.
    Secret {
        /// Credential delivered only to the selected adapter.
        secret: SecretString,
        /// Caller-supplied non-secret provenance.
        source: Option<String>,
    },
    /// Explicitly configured secret-free endpoint; never an implicit fallback.
    ConfiguredWithoutSecret {
        /// Caller-supplied non-secret provenance.
        source: Option<String>,
    },
}
/// Supplied metadata, not a promise of live credential validity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthStatus {
    /// Whether authentication is configured.
    pub configured: bool,
    /// Non-secret provenance.
    pub source: Option<String>,
}
/// Request-scoped resolution without model-owned credential lifecycle.
pub trait AuthResolver: Send + Sync {
    /// Return metadata only, without resolving, refreshing or reading secrets.
    fn status(&self, provider: &str) -> AuthStatus;
    /// Resolve only this provider. Future creation must not synchronously block.
    /// Return typed safe errors and observe local cancellation.
    fn resolve(
        &self,
        provider: String,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>>;
}
pub(crate) fn validate(auth: RequestAuth) -> Result<RequestAuth, Failure> {
    if matches!(&auth, RequestAuth::Secret { secret, .. } if secret.expose().is_empty()) {
        Err(Failure::MissingAuthentication)
    } else {
        Ok(auth)
    }
}

/// Provider exchange outputs returned to the credential owner without expiry policy.
#[derive(Clone, Debug)]
pub struct TokenExchangeResult {
    /// Authentication for a later request.
    pub auth: RequestAuth,
    /// Opaque rotated provider state; not a persisted schema.
    pub state: SecretString,
    /// Optional absolute Unix-millisecond expiry, never interpreted by models.
    pub expires_at: Option<u64>,
}
/// Optional provider primitive invoked only by the credential owner under its lock.
pub trait TokenExchange: Send + Sync {
    /// Exchange owned opaque state with local cancellation and typed safe failures.
    /// Creation must not block. The owner controls invocation and persistence.
    fn exchange(
        &self,
        state: SecretString,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<TokenExchangeResult, Failure>> + Send + '_>>;
}
