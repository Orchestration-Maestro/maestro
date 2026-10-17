//! Client authentication and safe buffered OAuth exchanges.

use super::base_external_client::invalid_token;
use crate::providers::http::{
    RequestFailure, RequestPolicy, RetryStatuses, buffered, collect_body, send_with_policy,
};
use crate::{Fetch, HttpRequest};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use google_cloud_auth::errors::CredentialsError;
use indexmap::IndexMap;
use serde_json::Value;

/// Add UTF-8 Basic client authentication when a client identifier is present.
pub(super) fn client_auth(value: &Value, headers: &mut IndexMap<String, String>) -> bool {
    let Some(id) = value
        .get("client_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return false;
    };
    let secret = value
        .get("client_secret")
        .and_then(Value::as_str)
        .unwrap_or_default();
    headers.insert(
        "authorization".into(),
        format!("Basic {}", STANDARD.encode(format!("{id}:{secret}"))),
    );
    true
}

/// Status-only authentication diagnostic; bodies and URLs are never rendered.
fn status_error(status: u16, _: &str) -> RequestFailure {
    RequestFailure::new(format!(
        "Authentication: credential acquisition failed (HTTP {status})"
    ))
}

/// Send through the shared retry owner, collecting bytes before the attempt deadline.
pub(super) async fn exchange(
    request: HttpRequest,
    fetch: Fetch,
    policy: RequestPolicy,
) -> Result<Value, CredentialsError> {
    let response = self::request(request, fetch, policy).await?;
    let bytes = collect_body(response.body)
        .await
        .map_err(|_| invalid_token())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid_token())?;
    if !value.is_object() {
        return Err(invalid_token());
    }
    Ok(value)
}

/// Retry policy shared by owned non-metadata credential exchanges.
pub(super) fn policy() -> RequestPolicy {
    RequestPolicy {
        timeout: None,
        retries: 3,
        no_response_retries: 2,
        statuses: RetryStatuses::Authentication,
    }
}

/// Send a buffered operation while retaining safe status and transience categories.
pub(super) async fn request(
    request: HttpRequest,
    fetch: Fetch,
    policy: RequestPolicy,
) -> Result<crate::HttpResponse, CredentialsError> {
    send_with_policy(request, buffered(fetch), policy, status_error)
        .await
        .map_err(|failure| {
            let transient = failure.message == "Connection error."
                || failure.message == "Request timed out."
                || ["HTTP 408", "HTTP 429", "HTTP 5"]
                    .iter()
                    .any(|kind| failure.message.contains(kind));
            match status_of(&failure.message) {
                Some(status) => CredentialsError::from_source(transient, Status(status)),
                None => CredentialsError::from_msg(transient, failure.message),
            }
        })
}

/// Safe HTTP status retained as the source of an authentication failure.
#[derive(Debug)]
pub(super) struct Status(pub(super) u16);

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Authentication: credential acquisition failed (HTTP {})",
            self.0
        )
    }
}

impl std::error::Error for Status {}

/// Read the status written by `status_error`.
fn status_of(message: &str) -> Option<u16> {
    message
        .rsplit_once("(HTTP ")?
        .1
        .strip_suffix(')')?
        .parse()
        .ok()
}
