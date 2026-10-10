//! Validated header layers for response-session requests.

use super::request::diagnostic;
use crate::providers::http::normalize_request;
use crate::{DiagnosticErrorInfo, Model, StreamOptions};
use indexmap::IndexMap;

/// Validate each incoming header before combining initial names or replacing later names.
fn layer(
    target: &mut IndexMap<String, String>,
    entries: &IndexMap<String, String>,
    initial: bool,
) -> Result<(), DiagnosticErrorInfo> {
    for (name, value) in entries {
        let mut entry = IndexMap::from([(name.to_ascii_lowercase(), value.to_owned())]);
        normalize_request(&mut entry).map_err(diagnostic)?;
        for (name, value) in entry {
            let separator = if name == "cookie" { "; " } else { ", " };
            if initial && let Some(previous) = target.get_mut(&name) {
                previous.push_str(separator);
                previous.push_str(&value);
            } else {
                target.insert(name, value);
            }
        }
    }
    Ok(())
}

/// Initial model headers combine; additional headers replace; provider headers win.
pub(super) fn build_sse_headers(
    model: &Model,
    options: &StreamOptions,
    account: &str,
    token: &str,
    user_agent: &str,
) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
    let mut headers = IndexMap::new();
    if let Some(initial) = &model.headers {
        layer(&mut headers, initial, true)?;
    }
    if let Some(additional) = &options.headers {
        layer(&mut headers, additional, false)?;
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
pub(super) fn build_web_socket_headers(
    prepared: &IndexMap<String, String>,
    request_id: &str,
) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
    let mut headers = prepared.clone();
    for name in ["accept", "content-type", "openai-beta"] {
        headers.shift_remove(name);
    }
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
