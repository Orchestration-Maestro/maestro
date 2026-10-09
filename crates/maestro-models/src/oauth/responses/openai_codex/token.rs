//! Ordered token requests and selected account metadata.
use crate::providers::http::decode_utf8;
use crate::providers::json_text::{compact_raw, member, number, object_record, raw_json};
use crate::{Fetch, FetchError, HttpRequest, OAuthCredentials, OAuthError};
use base64::{
    Engine as _, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use futures_util::StreamExt as _;
use serde::Deserialize;

/// Public application identifier.
pub(super) const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// Token endpoint.
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

/// Refresh through one selected transport attempt.
pub(super) async fn refresh(
    token: String,
    fetch: Fetch,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("grant_type", "refresh_token"),
            ("refresh_token", &token),
            ("client_id", CLIENT_ID),
        ])
        .finish();
    let result = request(body, fetch, "refresh", clock)
        .await
        .map_err(|cause| {
            OAuthError::message(format!("OpenAI Codex token refresh error: {cause}"))
        })?;
    with_account(result?)
}

/// Required surviving token fields.
#[derive(Deserialize)]
struct TokenResponse {
    /// Access token.
    access_token: String,
    /// Rotated token.
    refresh_token: String,
    /// Numeric duration, without coercion.
    #[serde(default, deserialize_with = "number")]
    expires_in: Option<f64>,
}

/// Separate authored token failures from fallible transport and decoding effects.
async fn request(
    body: String,
    fetch: Fetch,
    operation: &str,
    clock: fn() -> f64,
) -> Result<Result<OAuthCredentials, OAuthError>, OAuthError> {
    let response = fetch(HttpRequest {
        method: "POST".into(),
        url: TOKEN_URL.into(),
        headers: [(
            "content-type".into(),
            "application/x-www-form-urlencoded".into(),
        )]
        .into(),
        body: body.into_bytes(),
        signal: None,
    })
    .await
    .map_err(fetch_error)?;
    if !(200..300).contains(&response.status) {
        let text = read_text(response.body).await.unwrap_or_default();
        let detail = if text.is_empty() {
            response.status_text
        } else {
            text
        };
        return Ok(Err(OAuthError::message(format!(
            "OpenAI Codex token {operation} failed ({}): {detail}",
            response.status
        ))));
    }
    decode_tokens(&read_text(response.body).await?, operation, clock)
}

/// Validate selected response fields before reading the clock; materialize details only on failure.
fn decode_tokens(
    text: &str,
    operation: &str,
    clock: fn() -> f64,
) -> Result<Result<OAuthCredentials, OAuthError>, OAuthError> {
    let raw = raw_json(text).map_err(|error| json_error(&error))?;
    if let Some(data) = object_record::<TokenResponse>(raw)
        && !data.access_token.is_empty()
        && !data.refresh_token.is_empty()
        && let Some(duration) = data.expires_in.filter(|duration| duration.is_finite())
    {
        let expires = clock() + duration * 1000.0;
        if expires.is_finite() {
            return Ok(Ok(OAuthCredentials {
                access: data.access_token,
                refresh: data.refresh_token,
                expires,
                extra: crate::JsonObject::new(),
            }));
        }
    }
    let detail = compact_raw(raw).map_err(|error| json_error(&error))?;
    Ok(Err(OAuthError::message(format!(
        "OpenAI Codex token {operation} response missing fields: {detail}"
    ))))
}

/// Add only the required routing account metadata after token validation.
fn with_account(mut credentials: OAuthCredentials) -> Result<OAuthCredentials, OAuthError> {
    let account = account_id(&credentials.access)
        .ok_or_else(|| OAuthError::message("Failed to extract accountId from token"))?;
    credentials.extra.insert("accountId".into(), account.into());
    Ok(credentials)
}

/// Decode the middle JWT segment without verifying its header or signature.
pub(super) fn account_id(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    parts.next()?;
    let payload = parts.next()?;
    parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let encoded: Vec<u8> = payload
        .bytes()
        .filter(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
        .collect();
    let bytes = [
        DecodePaddingMode::RequireCanonical,
        DecodePaddingMode::RequireNone,
    ]
    .into_iter()
    .find_map(|padding| {
        let config = GeneralPurposeConfig::new()
            .with_decode_padding_mode(padding)
            .with_decode_allow_trailing_bits(true);
        GeneralPurpose::new(&alphabet::URL_SAFE, config)
            .decode(&encoded)
            .or_else(|_| GeneralPurpose::new(&alphabet::STANDARD, config).decode(&encoded))
            .ok()
    })?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let raw = raw_json(text).ok()?;
    let auth = member(raw, "https://api.openai.com/auth")?;
    let account = member(auth, "chatgpt_account_id")?;
    let account: String = serde_json::from_str(account.get()).ok()?;
    (!account.is_empty()).then_some(account)
}

/// Read all bytes before decoding UTF-8.
async fn read_text(mut body: crate::HttpBody) -> Result<String, OAuthError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        bytes.extend(chunk.map_err(fetch_error)?);
    }
    Ok(decode_utf8(&bytes).into_owned())
}
/// Preserve supplied transport messages.
fn fetch_error(error: FetchError) -> OAuthError {
    match error {
        FetchError::Connection(diagnostic) => diagnostic.into(),
        other => OAuthError::message(other.to_string()),
    }
}
/// Retain native JSON diagnostics.
fn json_error(error: &serde_json::Error) -> OAuthError {
    OAuthError::message(error.to_string())
}

/// Exchange a selected code with the proof verifier.
pub(super) async fn exchange(
    code: String,
    verifier: String,
    fetch: Fetch,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", super::REDIRECT_URI),
        ])
        .finish();
    with_account(request(body, fetch, "exchange", clock).await??)
}
