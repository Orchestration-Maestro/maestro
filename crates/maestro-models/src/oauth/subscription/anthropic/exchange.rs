//! Ordered token requests and complete-body response validation.
use crate::providers::http::{Raced, decode_utf8, race};
use crate::providers::json_text::{object_record, raw_json};
use crate::{Cancellation, Fetch, FetchError, HttpRequest, OAuthCredentials, OAuthError};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Public application identifier.
pub(super) const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Token endpoint.
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
/// Validation failure detail shared by both token operations.
const TOKEN_DETAILS: &str =
    "Expected string access_token and refresh_token, finite numeric expires_in, and finite expires";

/// Ordered refresh payload.
#[derive(Serialize)]
struct RefreshRequest<'a> {
    /// Refresh grant.
    grant_type: &'a str,
    /// Application identifier.
    client_id: &'a str,
    /// Current refresh token.
    refresh_token: &'a str,
}

/// Ordered authorization-code payload.
#[derive(Serialize)]
struct CodeRequest<'a> {
    /// Authorization grant.
    grant_type: &'a str,
    /// Application identifier.
    client_id: &'a str,
    /// Selected code.
    code: &'a str,
    /// Selected state.
    state: &'a str,
    /// Redirect identity of the authorization.
    redirect_uri: &'a str,
    /// Proof verifier.
    code_verifier: &'a str,
}

/// Selected authorization fields with the redirect address they were issued for.
#[derive(Debug, PartialEq)]
pub(super) struct Grant {
    /// Selected code.
    pub(super) code: String,
    /// Selected state.
    pub(super) state: String,
    /// Redirect address the authorization named.
    pub(super) redirect_uri: String,
}

/// Exchange selected authorization fields once.
pub(super) async fn exchange(
    grant: Grant,
    verifier: String,
    fetch: Fetch,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let request = CodeRequest {
        grant_type: "authorization_code",
        client_id: CLIENT_ID,
        code: &grant.code,
        state: &grant.state,
        redirect_uri: &grant.redirect_uri,
        code_verifier: &verifier,
    };
    let context = format!(
        "redirect_uri={}; response_type=authorization_code; ",
        grant.redirect_uri
    );
    token_operation(&request, fetch, "Token exchange", &context, clock).await
}

/// Required token fields, decoded after selecting last members.
#[derive(Deserialize)]
struct TokenResponse {
    /// Access token text.
    access_token: String,
    /// Rotated refresh token text.
    refresh_token: String,
    /// Duration in seconds, rounded as a double.
    expires_in: f64,
}

/// Refresh one token through the selected transport.
pub(super) async fn refresh(
    token: String,
    fetch: Fetch,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let request = RefreshRequest {
        grant_type: "refresh_token",
        client_id: CLIENT_ID,
        refresh_token: &token,
    };
    token_operation(&request, fetch, "Anthropic token refresh", "", clock).await
}

/// Send and decode one token operation with an operation-specific error context.
async fn token_operation(
    request: &impl Serialize,
    fetch: Fetch,
    label: &str,
    context: &str,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let body =
        serde_json::to_vec(request).map_err(|error| OAuthError::message(error.to_string()))?;
    let text = post_json(body, fetch).await.map_err(|cause| {
        let mut error = OAuthError::message(format!(
            "{label} request failed. url={TOKEN_URL}; {context}details={}",
            cause.details()
        ));
        error.cause = Some(Box::new(cause));
        error
    })?;
    decode_tokens(&text, label, clock)
}

/// Read complete text before interpreting the status, using one headers-plus-body deadline.
pub(super) async fn post_json(body: Vec<u8>, fetch: Fetch) -> Result<String, OAuthError> {
    let signal = Cancellation::new();
    let request = HttpRequest {
        method: "POST".into(),
        url: TOKEN_URL.into(),
        headers: [
            ("content-type".into(), "application/json".into()),
            ("accept".into(), "application/json".into()),
        ]
        .into(),
        body,
        signal: Some(signal.clone()),
    };
    let work = async {
        let response = fetch(request).await.map_err(OAuthError::from)?;
        let bytes = crate::providers::http::collect_body(response.body)
            .await
            .map_err(OAuthError::from)?;
        let text = decode_utf8(&bytes).into_owned();
        if !(200..300).contains(&response.status) {
            return Err(OAuthError::message(format!(
                "HTTP request failed. status={}; url={TOKEN_URL}; body={text}",
                response.status
            )));
        }
        Ok(text)
    };
    match race(work, Some(Duration::from_secs(30)), None).await {
        Raced::Done(result) => result,
        Raced::TimedOut | Raced::Cancelled => {
            signal.abort();
            Err(FetchError::Timeout.into())
        }
    }
}

/// Decode only surviving required fields and compute buffered expiry from the completion clock.
fn decode_tokens(
    text: &str,
    label: &str,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let raw = raw_json(text).map_err(|cause| {
        let cause = OAuthError::message(cause.to_string());
        let mut error = OAuthError::message(format!(
            "{label} returned invalid JSON. url={TOKEN_URL}; body={text}; details={}",
            cause.details()
        ));
        error.cause = Some(Box::new(cause));
        error
    })?;
    let invalid = || {
        OAuthError::message(format!(
            "{label} returned invalid token response. url={TOKEN_URL}; body={text}; details={TOKEN_DETAILS}"
        ))
    };
    let data: TokenResponse = object_record(raw).ok_or_else(invalid)?;
    let expires = clock() + data.expires_in * 1000.0 - 300_000.0;
    if !data.expires_in.is_finite() || !expires.is_finite() {
        return Err(invalid());
    }
    Ok(OAuthCredentials {
        refresh: data.refresh_token,
        access: data.access_token,
        expires,
        extra: crate::JsonObject::new(),
    })
}
