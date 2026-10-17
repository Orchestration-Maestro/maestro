//! Ordered certificate subject-token exchange.

use super::{CLOUD_SCOPE, base_external_client::invalid_token, oauth_client_auth_handler};
use crate::{Fetch, HttpRequest};
use google_cloud_auth::errors::CredentialsError;
use serde_json::Value;

/// Retained STS configuration and client-authenticated headers.
pub(super) struct StsCredentials {
    /// Token endpoint.
    endpoint: String,
    /// Audience identifying the identity provider.
    audience: String,
    /// Protocol subject token type.
    subject_type: String,
    /// Authentication-only headers.
    headers: indexmap::IndexMap<String, String>,
    /// Optional workforce user-project when client authentication is absent.
    workforce_project: Option<String>,
}

impl StsCredentials {
    /// Decode documented STS fields once from the selected ADC object.
    pub(super) fn new(value: &Value) -> Result<Self, CredentialsError> {
        let universe = value
            .get("universe_domain")
            .and_then(Value::as_str)
            .unwrap_or("googleapis.com");
        let endpoint = value
            .get("token_url")
            .and_then(Value::as_str)
            .map_or_else(|| format!("https://sts.{universe}/v1/token"), str::to_owned);
        let audience = value
            .get("audience")
            .and_then(Value::as_str)
            .ok_or_else(invalid_token)?
            .to_owned();
        let subject_type = value
            .get("subject_token_type")
            .and_then(Value::as_str)
            .ok_or_else(invalid_token)?
            .to_owned();
        let mut headers = indexmap::IndexMap::from([(
            "content-type".into(),
            "application/x-www-form-urlencoded".into(),
        )]);
        let authenticated = oauth_client_auth_handler::client_auth(value, &mut headers);
        let workforce_project = value
            .get("workforce_pool_user_project")
            .and_then(Value::as_str)
            .filter(|project| !project.is_empty());
        if workforce_project.is_some()
            && !regex::Regex::new(
                r"//iam\.googleapis\.com/locations/[^/]+/workforcePools/[^/]+/providers/.+",
            )
            .map_err(|_| invalid_token())?
            .is_match(&audience)
        {
            return Err(invalid_token());
        }
        Ok(Self {
            endpoint,
            audience,
            subject_type,
            headers,
            workforce_project: workforce_project
                .filter(|_| !authenticated)
                .map(str::to_owned),
        })
    }

    /// Exchange the certificate subject with the reference's form field order.
    pub(super) async fn exchange(
        &self,
        subject: &str,
        fetch: Fetch,
    ) -> Result<Value, CredentialsError> {
        let body = {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.extend_pairs([
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:token-exchange",
                ),
                ("audience", self.audience.as_str()),
                ("scope", CLOUD_SCOPE),
                (
                    "requested_token_type",
                    "urn:ietf:params:oauth:token-type:access_token",
                ),
                ("subject_token", subject),
                ("subject_token_type", self.subject_type.as_str()),
            ]);
            if let Some(project) = &self.workforce_project {
                let options =
                    maestro_json::compact_json(&serde_json::json!({"userProject":project}))
                        .map_err(|_| invalid_token())?;
                form.append_pair("options", &options);
            }
            form.finish().into_bytes()
        };
        let request = HttpRequest {
            method: "POST".into(),
            url: self.endpoint.clone(),
            headers: self.headers.clone(),
            body,
            signal: None,
        };
        oauth_client_auth_handler::exchange(request, fetch, oauth_client_auth_handler::policy())
            .await
    }
}
