//! Demand-driven token reuse for owned credential protocols.

use super::google_auth::configuration_error;
use crate::Fetch;
use google_cloud_auth::credentials::{CacheableResource, CredentialsProvider, EntityTag};
use google_cloud_auth::errors::CredentialsError;
use hyper::http::{Extensions, HeaderMap};
use std::sync::{Arc, Mutex};

/// Captured effects shared by refresh operations, separate from content transport.
#[derive(Clone)]
pub(super) struct Effects {
    /// HTTP effect below response validation.
    pub(super) fetch: Fetch,
    /// Construct a certificate-only identity client at refresh, separate from content transport.
    pub(super) identity: fn(reqwest::Identity) -> Result<Fetch, CredentialsError>,
    /// Completion clock in milliseconds.
    pub(super) clock: Arc<dyn Fn() -> f64 + Send + Sync>,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            fetch: crate::default_fetch(),
            identity: identity_fetch,
            clock: Arc::new(crate::records::diagnostics::timestamp_now),
        }
    }
}

/// A validated access token with optional known expiry.
#[derive(Clone)]
pub(super) struct Token {
    /// Nonempty access value.
    pub(super) access: String,
    /// Optional expiry in milliseconds.
    pub(super) expires: Option<f64>,
}

impl Token {
    /// Reject unusable tokens before caching or impersonation.
    pub(super) fn from_response(
        value: &serde_json::Value,
        now: f64,
    ) -> Result<Self, CredentialsError> {
        let access = value
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(invalid_token)?
            .to_owned();
        let expires = match value.get("expires_in") {
            None => None,
            Some(value) => Some(
                now + value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(invalid_token)?
                    * 1000.0,
            ),
        };
        Self::new(access, expires, now)
    }

    /// Validate a computed expiry without formatting token data.
    pub(super) fn new(
        access: String,
        expires: Option<f64>,
        now: f64,
    ) -> Result<Self, CredentialsError> {
        if access.is_empty()
            || expires.is_some_and(|expires| !expires.is_finite() || expires <= now)
        {
            return Err(invalid_token());
        }
        Ok(Self { access, expires })
    }
}

/// One source-specific refresh returning a validated token.
pub(super) trait TokenSource: Send + Sync + 'static {
    /// Perform the protocol exchange outside cache locks.
    fn refresh(
        &self,
        effects: Effects,
    ) -> impl std::future::Future<Output = Result<Token, CredentialsError>> + Send;
}

/// Shared in-flight result; errors are retained only for callers of that attempt.
type Attempt = Arc<tokio::sync::OnceCell<Result<Token, Arc<CredentialsError>>>>;

/// Shared cache state for one retained credential source.
pub(super) struct BaseExternalClient<S> {
    /// Source-specific protocol configuration.
    source: S,
    /// The one current acquisition generation.
    attempt: Mutex<Attempt>,
    /// Shared HTTP and completion-clock effects.
    effects: Effects,
    /// Outer quota selection, distinct from nested credentials.
    quota: Option<String>,
    /// Optional source universe.
    universe: Option<String>,
}

impl<S> std::fmt::Debug for BaseExternalClient<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BaseExternalClient")
    }
}

impl<S: TokenSource> BaseExternalClient<S> {
    /// Capture a protocol and its header metadata without starting work.
    pub(super) fn new(source: S, effects: Effects, value: &serde_json::Value) -> Self {
        Self {
            source,
            attempt: Mutex::new(Arc::new(tokio::sync::OnceCell::new())),
            effects,
            quota: value
                .get("quota_project_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            universe: value
                .get("universe_domain")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        }
    }

    /// Join a current acquisition or replace an expired/failed generation before calling out.
    async fn token(&self) -> Result<Token, CredentialsError> {
        let now = (self.effects.clock)();
        let attempt = {
            let mut slot = self.attempt.lock().map_err(|_| invalid_token())?;
            let replace = slot.get().is_some_and(|result| match result {
                Ok(token) => token
                    .expires
                    .is_some_and(|expiry| now >= expiry - 300_000.0),
                Err(_) => true,
            });
            if replace {
                *slot = Arc::new(tokio::sync::OnceCell::new());
            }
            Arc::clone(&slot)
        };
        let result = attempt
            .get_or_init(|| async {
                self.source
                    .refresh(self.effects.clone())
                    .await
                    .map_err(Arc::new)
            })
            .await;
        match result {
            Ok(token) => Token::new(token.access.clone(), token.expires, (self.effects.clock)()),
            Err(error) => Err(CredentialsError::from_source(
                error.is_transient(),
                Shared(Arc::clone(error)),
            )),
        }
    }
}

impl<S: TokenSource> CredentialsProvider for BaseExternalClient<S> {
    async fn headers(
        &self,
        _: Extensions,
    ) -> Result<CacheableResource<HeaderMap>, CredentialsError> {
        let token = self.token().await?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", token.access)
                .parse()
                .map_err(|_| invalid_token())?,
        );
        if let Some(quota) = &self.quota {
            headers.insert(
                "x-goog-user-project",
                quota.parse().map_err(|_| {
                    CredentialsError::from_msg(false, configuration_error().message)
                })?,
            );
        }
        Ok(CacheableResource::New {
            entity_tag: EntityTag::new(),
            data: headers,
        })
    }

    fn universe_domain(&self) -> impl std::future::Future<Output = Option<String>> + Send {
        std::future::ready(self.universe.clone())
    }
}

/// One failed acquisition shared by every caller of that attempt, keeping its cause chain.
#[derive(Debug)]
struct Shared(Arc<CredentialsError>);

impl std::fmt::Display for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Authentication: credential acquisition failed")
    }
}

impl std::error::Error for Shared {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0)
    }
}

/// Safe token/protocol failure without untrusted response data.
pub(super) fn invalid_token() -> CredentialsError {
    CredentialsError::from_msg(false, "Authentication: credential acquisition failed")
}

/// Certificate HTTP policy: verified HTTPS, no proxy and no retained idle connection pool.
fn identity_fetch(identity: reqwest::Identity) -> Result<Fetch, CredentialsError> {
    let client = reqwest::Client::builder()
        .identity(identity)
        .https_only(true)
        .no_proxy()
        .pool_max_idle_per_host(0)
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| super::certificate_subject_token_supplier::certificate_error())?;
    Ok(crate::providers::http::supplied_fetch(client))
}
