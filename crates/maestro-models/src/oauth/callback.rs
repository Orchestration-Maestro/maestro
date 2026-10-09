//! Shared authorization input recognition and callback response records.
use crate::oauth::authorization_whitespace;
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
pub(super) fn query_fields(query: &str) -> AuthorizationCode {
    AuthorizationCode {
        code: first_query(query, "code"),
        state: first_query(query, "state"),
    }
}

/// Decode form data with first-value selection and lenient percent decoding.
pub(super) fn first_query(query: &str, name: &str) -> Option<String> {
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
