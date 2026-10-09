//! Subscription callback policy.
pub(super) use crate::oauth::callback::{AuthorizationCode, parse_authorization_input};
#[cfg(not(target_arch = "wasm32"))]
use crate::oauth::callback::{CallbackResponse, first_query, query_fields};
#[cfg(not(target_arch = "wasm32"))]
use crate::{OAuthError, oauth_error_html, oauth_success_html};
#[cfg(not(target_arch = "wasm32"))]
use url::Url;
/// Route a target using the fixed localhost base without settling rejected input.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn route(target: &str, expected_state: &str) -> CallbackResponse {
    route_result(target, expected_state).unwrap_or_else(|_| CallbackResponse {
        status: 500,
        content_type: "text/plain; charset=utf-8",
        body: "Internal error".into(),
        accepted: None,
    })
}

/// Classify a callback and render its escaped response text.
#[cfg(not(target_arch = "wasm32"))]
fn route_result(target: &str, expected_state: &str) -> Result<CallbackResponse, OAuthError> {
    let url = Url::parse("http://localhost")
        .and_then(|base| base.join(target))
        .map_err(|error| OAuthError::message(error.to_string()))?;
    if url.path() != "/callback" {
        return html_error(404, "Callback route not found.", None);
    }
    let error = first_query(url.query().unwrap_or_default(), "error");
    if let Some(error) = error.filter(|error| !error.is_empty()) {
        return html_error(
            400,
            "Anthropic authentication did not complete.",
            Some(&format!("Error: {error}")),
        );
    }
    let fields = query_fields(url.query().unwrap_or_default());
    if fields.code.as_deref().unwrap_or_default().is_empty()
        || fields.state.as_deref().unwrap_or_default().is_empty()
    {
        return html_error(400, "Missing code or state parameter.", None);
    }
    if fields.state.as_deref() != Some(expected_state) {
        return html_error(400, "State mismatch.", None);
    }
    Ok(CallbackResponse {
        status: 200,
        content_type: "text/html; charset=utf-8",
        body: oauth_success_html("Anthropic authentication completed. You can close this window.")?,
        accepted: Some(fields),
    })
}

/// Render a rejection without consuming the callback wait.
#[cfg(not(target_arch = "wasm32"))]
fn html_error(
    status: u16,
    message: &str,
    details: Option<&str>,
) -> Result<CallbackResponse, OAuthError> {
    Ok(CallbackResponse {
        status,
        content_type: "text/html; charset=utf-8",
        body: oauth_error_html(message, details)?,
        accepted: None,
    })
}

/// Bind the subscription callback listener.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind(
    state: String,
) -> Result<crate::oauth::native::CallbackServer, OAuthError> {
    crate::oauth::native::bind(state, 53692, route).await
}
/// Reject native serving only at the browser effect boundary.
#[cfg(target_arch = "wasm32")]
pub(super) async fn bind(
    state: String,
) -> Result<crate::oauth::native::CallbackServer, crate::OAuthError> {
    unavailable(state).await
}
/// Refuse unavailable subscription callback serving.
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) async fn unavailable(
    _state: String,
) -> Result<crate::oauth::native::CallbackServer, crate::OAuthError> {
    Err(crate::OAuthError::message(
        "Anthropic OAuth requires a native callback listener",
    ))
}
