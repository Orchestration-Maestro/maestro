//! Validated header layers for response-session requests.

use super::request::diagnostic;
use crate::providers::transport::{layer_request_headers, normalize_request};
use crate::{DiagnosticErrorInfo, Model, StreamOptions};
use indexmap::IndexMap;

/// Initial model headers combine; additional headers replace; provider headers win.
pub(super) fn build_sse_headers<E: crate::ProviderExecution>(
    model: &Model,
    options: &StreamOptions<E>,
    account: &str,
    token: &str,
    user_agent: &str,
) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
    let mut headers = IndexMap::new();
    if let Some(initial) = &model.headers {
        layer_request_headers(&mut headers, initial, true).map_err(diagnostic)?;
    }
    if let Some(additional) = &options.headers {
        layer_request_headers(&mut headers, additional, false).map_err(diagnostic)?;
    }
    headers.extend([
        ("authorization".to_owned(), format!("Bearer {token}")),
        ("chatgpt-account-id".to_owned(), account.to_owned()),
        ("originator".to_owned(), "maestro".to_owned()),
        ("user-agent".to_owned(), user_agent.to_owned()),
        (
            "openai-beta".to_owned(),
            "responses=experimental".to_owned(),
        ),
        ("accept".to_owned(), "text/event-stream".to_owned()),
        ("content-type".to_owned(), "application/json".to_owned()),
    ]);
    if let Some(session) = options.session_id.as_deref().filter(|s| !s.is_empty()) {
        headers.insert("session_id".to_owned(), session.to_owned());
        headers.insert("x-client-request-id".to_owned(), session.to_owned());
    }
    normalize_request(&mut headers).map_err(diagnostic)?;
    Ok(headers)
}

/// Derive socket headers from validated request headers: the upgrade carries no content
/// negotiation, selects the socket beta and identifies the request in both affinity fields.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn build_web_socket_headers(
    prepared: &IndexMap<String, String>,
    request_id: &str,
) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
    let mut headers: IndexMap<String, String> = prepared
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "accept" | "content-type" | "openai-beta"))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    headers.extend([
        (
            "openai-beta".to_owned(),
            "responses_websockets=2026-02-06".to_owned(),
        ),
        ("x-client-request-id".to_owned(), request_id.to_owned()),
        ("session_id".to_owned(), request_id.to_owned()),
    ]);
    normalize_request(&mut headers).map_err(diagnostic)?;
    Ok(headers)
}
