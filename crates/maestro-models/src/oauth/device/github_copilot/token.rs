//! Single-attempt device-account HTTP requests and service-token exchange.
use crate::providers::http::decode_utf8;
use crate::providers::json_text::{member, raw_json, raw_number};
use crate::{Fetch, FetchError, HttpRequest, OAuthCredentials, OAuthError};
use futures_util::StreamExt as _;
use indexmap::IndexMap;
use serde_json::value::RawValue;

/// Device application identifier.
pub(super) const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
/// Account client identity headers.
const COPILOT_HEADERS: [(&str, &str); 4] = [
    ("user-agent", "GitHubCopilotChat/0.35.0"),
    ("editor-version", "vscode/1.107.0"),
    ("editor-plugin-version", "copilot-chat/0.35.0"),
    ("copilot-integration-id", "vscode-chat"),
];

/// Build headers for service-token exchange and model policies.
fn account_headers(first: (&str, &str), token: &str) -> IndexMap<String, String> {
    let mut headers = IndexMap::from([
        (first.0.into(), first.1.into()),
        ("authorization".into(), format!("Bearer {token}")),
    ]);
    headers.extend(
        COPILOT_HEADERS
            .into_iter()
            .map(|(key, value)| (key.into(), value.into())),
    );
    headers
}

/// Read the whole response before checking status; transport diagnostics remain native.
pub(super) async fn fetch_json(request: HttpRequest, fetch: &Fetch) -> Result<String, OAuthError> {
    let mut response = fetch(request).await.map_err(fetch_error)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.body.next().await {
        bytes.extend(chunk.map_err(fetch_error)?);
    }
    let text = decode_utf8(&bytes).into_owned();
    if !(200..300).contains(&response.status) {
        return Err(OAuthError::message(format!(
            "{} {}: {text}",
            response.status, response.status_text
        )));
    }
    Ok(text)
}

/// Preserve a supplied diagnostic when an attempt or body stream fails.
fn fetch_error(error: FetchError) -> OAuthError {
    match error {
        FetchError::Connection(diagnostic) => diagnostic.into(),
        other => OAuthError::message(other.to_string()),
    }
}

/// Decode one selected required string without inspecting other members.
pub(super) fn string_field(raw: &RawValue, name: &str) -> Option<String> {
    member(raw, name).and_then(|value| serde_json::from_str(value.get()).ok())
}

/// Distinguish scalar roots from object and array field validation failures.
pub(super) fn response_record<'a>(text: &'a str, label: &str) -> Result<&'a RawValue, OAuthError> {
    let raw = raw_json(text).map_err(|error| OAuthError::message(error.to_string()))?;
    if !raw.get().starts_with(['{', '[']) {
        return Err(OAuthError::message(format!("Invalid {label} response")));
    }
    Ok(raw)
}

/// Exchange a refresh token, retaining the supplied enterprise operand verbatim.
pub(super) async fn refresh(
    refresh: String,
    domain: Option<String>,
    fetch: Fetch,
) -> Result<OAuthCredentials, OAuthError> {
    let host = domain
        .as_deref()
        .filter(|domain| !domain.is_empty())
        .unwrap_or("github.com");
    let headers = account_headers(("accept", "application/json"), &refresh);
    let text = fetch_json(
        HttpRequest {
            method: "GET".into(),
            url: format!("https://api.{host}/copilot_internal/v2/token"),
            headers,
            body: Vec::new(),
            signal: None,
        },
        &fetch,
    )
    .await?;
    let raw = response_record(&text, "Copilot token")?;
    let invalid = || OAuthError::message("Invalid Copilot token response fields");
    let access = string_field(raw, "token").ok_or_else(invalid)?;
    let expiry = member(raw, "expires_at")
        .and_then(raw_number)
        .ok_or_else(invalid)?;
    let expires = expiry * 1000.0 - 300_000.0;
    if !expiry.is_finite() || !expires.is_finite() {
        return Err(invalid());
    }
    let mut extra = crate::JsonObject::new();
    if let Some(domain) = domain {
        extra.insert("enterpriseUrl".into(), domain.into());
    }
    Ok(OAuthCredentials {
        refresh,
        access,
        expires,
        extra,
    })
}

/// Enable every catalog model concurrently, swallowing each policy attempt's failure.
pub(super) async fn enable_all_github_copilot_models(
    credentials: &OAuthCredentials,
    domain: Option<&str>,
    fetch: &Fetch,
) {
    let base = super::get_github_copilot_base_url(Some(&credentials.access), domain);
    let models = crate::get_models("github-copilot");
    futures_util::future::join_all(models.into_iter().map(|model| {
        let mut headers =
            account_headers(("content-type", "application/json"), &credentials.access);
        headers.extend([
            ("openai-intent".into(), "chat-policy".into()),
            ("x-interaction-type".into(), "chat-policy".into()),
        ]);
        let request = HttpRequest {
            method: "POST".into(),
            url: format!("{base}/models/{}/policy", model.id),
            headers,
            body: br#"{"state":"enabled"}"#.to_vec(),
            signal: None,
        };
        async move {
            let _ = fetch(request).await;
        }
    }))
    .await;
}
