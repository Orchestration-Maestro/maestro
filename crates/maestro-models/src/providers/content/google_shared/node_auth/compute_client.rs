//! Metadata-server token acquisition with one resolved endpoint.

use super::base_external_client::{Effects, Token, TokenSource, invalid_token};
use super::{CLOUD_SCOPE, metadata::Metadata};
use crate::HttpRequest;
use crate::providers::http::{RequestPolicy, RetryStatuses, collect_body};
use google_cloud_auth::errors::CredentialsError;

/// Compute token source; discovery admission never changes its primary endpoint.
pub(super) struct Compute {
    /// Retained endpoint and request deadline.
    metadata: Metadata,
}

impl Compute {
    /// Retain an admitted metadata endpoint without fetching a token.
    pub(super) fn new(metadata: Metadata) -> Self {
        Self { metadata }
    }
}

impl TokenSource for Compute {
    async fn refresh(&self, effects: Effects) -> Result<Token, CredentialsError> {
        let mut url = url::Url::parse(&format!(
            "{}/instance/service-accounts/default/token",
            self.metadata.base
        ))
        .map_err(|_| invalid_token())?;
        url.query_pairs_mut().append_pair("scopes", CLOUD_SCOPE);
        let request = HttpRequest {
            method: "GET".into(),
            url: url.into(),
            headers: [("metadata-flavor".into(), "Google".into())].into(),
            body: Vec::new(),
            signal: None,
        };
        let policy = RequestPolicy {
            timeout: self.metadata.timeout,
            retries: 3,
            no_response_retries: 3,
            statuses: RetryStatuses::Authentication,
        };
        let response =
            super::oauth_client_auth_handler::request(request, effects.fetch, policy).await?;
        super::metadata::validate_flavor(&response)?;
        let bytes = collect_body(response.body)
            .await
            .map_err(|_| invalid_token())?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| invalid_token())?;
        if !value.is_object() {
            return Err(invalid_token());
        }
        Token::from_response(&value, (effects.clock)())
    }
}
