//! Certificate-owned service-account token exchange.

use super::{
    CLOUD_SCOPE,
    base_external_client::{Token, invalid_token},
    oauth_client_auth_handler,
};
use crate::{Fetch, HttpRequest};
use google_cloud_auth::errors::CredentialsError;
use num_traits::ToPrimitive as _;
use serde_json::Value;

/// Optional certificate-source impersonation, distinct from outer native impersonation.
pub(super) struct Impersonated {
    /// Authored exchange endpoint.
    endpoint: String,
    /// Exact JSON numeric spelling with a seconds suffix.
    lifetime: String,
}

impl Impersonated {
    /// Select the source's optional impersonation URL and truthy numeric lifetime.
    pub(super) fn new(value: &Value) -> Result<Option<Self>, CredentialsError> {
        let Some(endpoint) = value
            .get("service_account_impersonation_url")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let selected = value
            .get("service_account_impersonation")
            .and_then(|value| value.get("token_lifetime_seconds"));
        let duration = selected
            .filter(|value| value.as_f64().is_some_and(|number| number != 0.0))
            .cloned()
            .unwrap_or(Value::from(3600));
        let lifetime = format!(
            "{}s",
            maestro_json::compact_json(&duration).map_err(|_| invalid_token())?
        );
        Ok(Some(Self {
            endpoint: endpoint.to_owned(),
            lifetime,
        }))
    }

    /// Exchange only a validated source token and return a validated final token.
    pub(super) async fn exchange(
        &self,
        access: &str,
        fetch: Fetch,
        now: f64,
    ) -> Result<Token, CredentialsError> {
        let body = maestro_json::compact_json(
            &serde_json::json!({"scope":[CLOUD_SCOPE],"lifetime":self.lifetime}),
        )
        .map_err(|_| invalid_token())?;
        let request = HttpRequest {
            method: "POST".into(),
            url: self.endpoint.clone(),
            headers: [
                ("content-type".into(), "application/json".into()),
                ("authorization".into(), format!("Bearer {access}")),
            ]
            .into(),
            body: body.into_bytes(),
            signal: None,
        };
        let value = oauth_client_auth_handler::exchange(
            request,
            fetch,
            oauth_client_auth_handler::policy(),
        )
        .await?;
        let access = value
            .get("accessToken")
            .and_then(Value::as_str)
            .ok_or_else(invalid_token)?
            .to_owned();
        let expiry = value
            .get("expireTime")
            .and_then(Value::as_str)
            .ok_or_else(invalid_token)?;
        let expiry = chrono::DateTime::parse_from_rfc3339(expiry)
            .map_err(|_| invalid_token())?
            .timestamp_millis()
            .to_f64()
            .ok_or_else(invalid_token)?;
        Token::new(access, Some(expiry), now)
    }
}
