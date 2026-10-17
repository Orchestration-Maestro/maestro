//! Client authentication and safe buffered OAuth exchanges.

use super::base_external_client::invalid_token;
use super::{ACQUISITION_FAILED, fixed_error};
use crate::providers::http::{
    Ending, RequestFailure, RequestPolicy, RetryStatuses, buffered, collect_body, send_with_policy,
};
use crate::providers::json_text::{raw_json, try_object_record};
use crate::{Fetch, HttpRequest, HttpResponse};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use google_cloud_auth::errors::CredentialsError;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
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
fn status_error(_: u16, _: &str) -> RequestFailure {
    RequestFailure::new(ACQUISITION_FAILED)
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

/// Send through the shared retry owner and decode the JSON object response as `T`.
pub(super) async fn exchange<T: DeserializeOwned>(
    request: HttpRequest,
    fetch: Fetch,
    policy: RequestPolicy,
) -> Result<T, CredentialsError> {
    decode(self::request(request, fetch, policy).await?).await
}

/// Collect a response body before the attempt deadline and decode it as a JSON object record.
pub(super) async fn decode<T: DeserializeOwned>(
    response: HttpResponse,
) -> Result<T, CredentialsError> {
    let bytes = collect_body(response.body)
        .await
        .map_err(|_| invalid_token())?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid_token())?;
    raw_json(text)
        .and_then(try_object_record)
        .map_err(|_| invalid_token())
}

/// Send a buffered operation while retaining its typed status and transience.
pub(super) async fn request(
    request: HttpRequest,
    fetch: Fetch,
    policy: RequestPolicy,
) -> Result<HttpResponse, CredentialsError> {
    send_with_policy(request, buffered(fetch), policy, status_error)
        .await
        .map_err(|failure| match failure.ending {
            Some(Ending::Status(status)) => fixed_error(
                matches!(status, 408 | 429 | 500..=599),
                ACQUISITION_FAILED,
                Some(status),
            ),
            Some(Ending::NoResponse) => CredentialsError::from_msg(true, failure.message),
            None => CredentialsError::from_msg(false, failure.message),
        })
}
