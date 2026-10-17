//! Demand-driven token reuse for owned credential protocols.

use super::certificate_subject_token_supplier::certificate_error;
use super::{
    ACQUISITION_FAILED, CLOUD_SCOPE, INVALID_CONFIGURATION, fixed_error, oauth_client_auth_handler,
};
use crate::{Fetch, HttpRequest};
use google_cloud_auth::credentials::{CacheableResource, CredentialsProvider, EntityTag};
use google_cloud_auth::errors::CredentialsError;
use hyper::http::{Extensions, HeaderMap};
use num_traits::ToPrimitive as _;
use serde::Deserialize;
use serde_json::Value;
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

/// Token endpoint fields the protocols read.
#[derive(Deserialize)]
pub(super) struct TokenResponse {
    /// Access value.
    access_token: Option<String>,
    /// Lifetime in seconds.
    expires_in: Option<f64>,
    /// Rotated refresh token.
    pub(super) refresh_token: Option<String>,
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
        response: &TokenResponse,
        now: f64,
    ) -> Result<Self, CredentialsError> {
        let access = response
            .access_token
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(invalid_token)?;
        Self::new(
            access,
            response.expires_in.map(|seconds| now + seconds * 1000.0),
            now,
        )
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

/// A source token and the transport that obtained it, which the optional impersonation reuses.
pub(super) struct Grant {
    /// Validated source token.
    pub(super) token: Token,
    /// Transport the source exchanged over.
    pub(super) fetch: Fetch,
}

/// One source-specific refresh returning a validated token.
pub(super) trait TokenSource: Send + Sync + 'static {
    /// Perform the protocol exchange outside cache locks.
    fn refresh(
        &self,
        effects: Effects,
    ) -> impl std::future::Future<Output = Result<Grant, CredentialsError>> + Send;
}

/// Shared in-flight result; errors are retained only for callers of that attempt.
type Attempt = Arc<tokio::sync::OnceCell<Result<Token, Arc<CredentialsError>>>>;

/// Optional service-account exchange that follows the source exchange.
pub(super) struct ServiceAccountImpersonation {
    /// Authored exchange endpoint.
    url: String,
    /// Exact JSON numeric spelling with a seconds suffix.
    lifetime: String,
}

impl ServiceAccountImpersonation {
    /// Select the optional impersonation URL and truthy numeric lifetime of a configuration.
    pub(super) fn new(value: &Value) -> Result<Option<Self>, CredentialsError> {
        let Some(url) = value
            .get("service_account_impersonation_url")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let duration = value
            .get("service_account_impersonation")
            .and_then(|value| value.get("token_lifetime_seconds"))
            .filter(|value| value.as_f64().is_some_and(|number| number != 0.0))
            .cloned()
            .unwrap_or(Value::from(3600));
        let lifetime = format!(
            "{}s",
            maestro_json::compact_json(&duration).map_err(|_| invalid_token())?
        );
        Ok(Some(Self {
            url: url.to_owned(),
            lifetime,
        }))
    }
}

/// Impersonated service-account token response.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImpersonatedToken {
    /// Final access value.
    access_token: String,
    /// RFC 3339 expiry.
    expire_time: String,
}

/// Shared cache state for one retained credential source.
pub(super) struct BaseExternalAccountClient<S> {
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
    /// Optional exchange of the source token for a service-account token.
    impersonation: Option<ServiceAccountImpersonation>,
}

impl<S> std::fmt::Debug for BaseExternalAccountClient<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BaseExternalAccountClient")
    }
}

impl<S: TokenSource> BaseExternalAccountClient<S> {
    /// Capture a protocol and its header metadata without starting work.
    pub(super) fn new(
        source: S,
        effects: Effects,
        quota: Option<String>,
        universe: Option<String>,
    ) -> Self {
        Self {
            source,
            attempt: Mutex::new(Arc::new(tokio::sync::OnceCell::new())),
            effects,
            quota,
            universe,
            impersonation: None,
        }
    }

    /// Follow every source exchange with a service-account token exchange.
    pub(super) fn with_impersonation(
        mut self,
        impersonation: Option<ServiceAccountImpersonation>,
    ) -> Self {
        self.impersonation = impersonation;
        self
    }

    /// Exchange at the source, then at the service account when one is configured.
    async fn refresh_access_token(&self) -> Result<Token, CredentialsError> {
        let grant = self.source.refresh(self.effects.clone()).await?;
        match &self.impersonation {
            Some(target) => {
                self.get_impersonated_access_token(target, &grant.token.access, grant.fetch)
                    .await
            }
            None => Ok(grant.token),
        }
    }

    /// Exchange only a validated source token and return a validated final token.
    async fn get_impersonated_access_token(
        &self,
        target: &ServiceAccountImpersonation,
        access: &str,
        fetch: Fetch,
    ) -> Result<Token, CredentialsError> {
        let body = maestro_json::compact_json(
            &serde_json::json!({"scope":[CLOUD_SCOPE],"lifetime":target.lifetime}),
        )
        .map_err(|_| invalid_token())?;
        let request = HttpRequest {
            method: "POST".into(),
            url: target.url.clone(),
            headers: [
                ("content-type".into(), "application/json".into()),
                ("authorization".into(), format!("Bearer {access}")),
            ]
            .into(),
            body: body.into_bytes(),
            signal: None,
        };
        let response: ImpersonatedToken = oauth_client_auth_handler::exchange(
            request,
            fetch,
            oauth_client_auth_handler::policy(),
        )
        .await?;
        let expiry = chrono::DateTime::parse_from_rfc3339(&response.expire_time)
            .map_err(|_| invalid_token())?
            .timestamp_millis()
            .to_f64()
            .ok_or_else(invalid_token)?;
        Token::new(response.access_token, Some(expiry), (self.effects.clock)())
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
            .get_or_init(|| async { self.refresh_access_token().await.map_err(Arc::new) })
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

impl<S: TokenSource> CredentialsProvider for BaseExternalAccountClient<S> {
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
                quota
                    .parse()
                    .map_err(|_| fixed_error(false, INVALID_CONFIGURATION, None))?,
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
        f.write_str(ACQUISITION_FAILED)
    }
}

impl std::error::Error for Shared {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0)
    }
}

/// Safe token/protocol failure without untrusted response data.
pub(super) fn invalid_token() -> CredentialsError {
    fixed_error(false, ACQUISITION_FAILED, None)
}

/// Certificate HTTP policy: verified HTTPS, no proxy and no retained idle connection pool.
pub(super) fn identity_client(identity: reqwest::Identity) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .identity(identity)
        .https_only(true)
        .no_proxy()
        .pool_max_idle_per_host(0)
        .retry(reqwest::retry::never())
}

/// Build the certificate-only transport used at refresh.
fn identity_fetch(identity: reqwest::Identity) -> Result<Fetch, CredentialsError> {
    let client = identity_client(identity)
        .build()
        .map_err(|_| certificate_error())?;
    Ok(crate::providers::http::supplied_fetch(client))
}
