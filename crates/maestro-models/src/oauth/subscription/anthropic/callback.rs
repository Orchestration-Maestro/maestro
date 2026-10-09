//! Authorization input recognition and callback routing.
#[cfg(not(target_arch = "wasm32"))]
use crate::{OAuthError, oauth_error_html, oauth_success_html};
use serde::Deserialize;
use url::Url;

/// Optional authorization fields, retaining absent versus empty values.
#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AuthorizationCode {
    /// Authorization code, when supplied.
    pub code: Option<String>,
    /// State, when supplied.
    pub state: Option<String>,
}

/// Trim the authorization whitespace set without stripping other format characters.
fn authorization_whitespace(character: char) -> bool {
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// Recognize a URL before hash, raw query or bare code text.
pub(super) fn parse_authorization_input(input: &str) -> AuthorizationCode {
    let value = input.trim_matches(authorization_whitespace);
    if value.is_empty() {
        return AuthorizationCode::default();
    }
    if let Ok(url) = Url::parse(value) {
        return query_fields(url.query().unwrap_or_default());
    }
    if let Some((code, rest)) = value.split_once('#') {
        return AuthorizationCode {
            code: Some(code.into()),
            state: Some(rest.split('#').next().unwrap_or_default().into()),
        };
    }
    if value.contains("code=") {
        return query_fields(value.strip_prefix('?').unwrap_or(value));
    }
    AuthorizationCode {
        code: Some(value.into()),
        state: None,
    }
}

/// Return the first occurrence of each exact query key.
fn query_fields(query: &str) -> AuthorizationCode {
    AuthorizationCode {
        code: first_query(query, "code"),
        state: first_query(query, "state"),
    }
}

/// Decode form data with first-value selection and lenient percent decoding.
fn first_query(query: &str, name: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

/// Callback response plus an optional accepted authorization.
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct CallbackResponse {
    /// HTTP response code.
    pub status: u16,
    /// Response media type.
    pub content_type: &'static str,
    /// Rendered response body.
    pub body: String,
    /// Accepted code and state, never present on rejection.
    pub accepted: Option<AuthorizationCode>,
}

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
