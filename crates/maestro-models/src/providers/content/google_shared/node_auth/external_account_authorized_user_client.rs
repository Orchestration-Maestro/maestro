//! Workforce authorized-user refresh and retained refresh-token rotation.

use super::base_external_client::{
    Effects, Grant, Token, TokenResponse, TokenSource, invalid_token,
};
use super::oauth_client_auth_handler::{client_auth, exchange, policy};
use crate::HttpRequest;
use google_cloud_auth::errors::CredentialsError;
use serde_json::Value;
use std::sync::Mutex;

/// Workforce configuration with refresh-token rotation owned by this source.
pub(super) struct ExternalAccountAuthorizedUserClient {
    /// Token exchange URL.
    endpoint: String,
    /// Client identifier and secret used only at the authentication boundary.
    authentication: indexmap::IndexMap<String, String>,
    /// Rotating refresh token; no caller executes under this lock.
    refresh: Mutex<String>,
}

impl ExternalAccountAuthorizedUserClient {
    /// Decode the selected source without starting HTTP work.
    pub(super) fn new(value: &Value) -> Result<Self, CredentialsError> {
        let universe = value
            .get("universe_domain")
            .and_then(Value::as_str)
            .unwrap_or("googleapis.com");
        let endpoint = value.get("token_url").and_then(Value::as_str).map_or_else(
            || format!("https://sts.{universe}/v1/oauthtoken"),
            str::to_owned,
        );
        let refresh = value
            .get("refresh_token")
            .and_then(Value::as_str)
            .ok_or_else(invalid_token)?
            .to_owned();
        let mut authentication = indexmap::IndexMap::new();
        client_auth(value, &mut authentication);
        authentication.insert(
            "content-type".into(),
            "application/x-www-form-urlencoded".into(),
        );
        Ok(Self {
            endpoint,
            authentication,
            refresh: Mutex::new(refresh),
        })
    }
}

impl TokenSource for ExternalAccountAuthorizedUserClient {
    /// Perform refresh, retaining rotation before validating the access response.
    async fn refresh(&self, effects: Effects) -> Result<Grant, CredentialsError> {
        let refresh = self.refresh.lock().map_err(|_| invalid_token())?.clone();
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
            ])
            .finish();
        let request = HttpRequest {
            method: "POST".into(),
            url: self.endpoint.clone(),
            headers: self.authentication.clone(),
            body: body.into_bytes(),
            signal: None,
        };
        let response: TokenResponse = exchange(request, effects.fetch.clone(), policy()).await?;
        if let Some(rotated) = response
            .refresh_token
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            rotated.clone_into(&mut *self.refresh.lock().map_err(|_| invalid_token())?);
        }
        Ok(Grant {
            token: Token::from_response(&response, (effects.clock)())?,
            fetch: effects.fetch,
        })
    }
}
