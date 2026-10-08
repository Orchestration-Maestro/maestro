//! Failure text for requests, responses and stream error payloads.

use std::borrow::Cow;

use serde_json::Value;
use url::Url;

use super::FetchError;
use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{compact_json, is_truthy};

/// Why a request ended without a usable response, with upstream detail when reported.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RequestFailure {
    /// Authored failure text.
    pub(crate) message: String,
    /// Upstream explanation the provider reported under `error.metadata.raw`.
    pub(crate) raw: Option<String>,
}

impl RequestFailure {
    /// Fail with fixed text and no upstream detail.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            raw: None,
        }
    }

    /// Fail because the request was cancelled.
    pub(crate) fn aborted() -> Self {
        Self::new(FetchError::Aborted.to_string())
    }

    /// Final text with the upstream explanation on its own line.
    pub(crate) fn into_text(self) -> String {
        match self.raw {
            Some(raw) => format!("{}\n{raw}", self.message),
            None => self.message,
        }
    }
}

impl From<DiagnosticErrorInfo> for RequestFailure {
    fn from(error: DiagnosticErrorInfo) -> Self {
        Self::new(error.to_string())
    }
}

/// Render a value as compact JSON text.
fn json_text(value: &Value) -> String {
    compact_json(value).unwrap_or_default()
}

/// Build the failure text from a status, a provider `error` payload and plain body text.
fn describe(status: Option<u16>, error: Option<&Value>, body: Option<&str>) -> String {
    let detail = match error.filter(|error| is_truthy(error)) {
        Some(error) => match error.get("message").filter(|message| is_truthy(message)) {
            Some(Value::String(message)) => Some(Cow::Borrowed(message.as_str())),
            Some(message) => Some(Cow::Owned(json_text(message))),
            None => Some(Cow::Owned(json_text(error))),
        },
        None => body.filter(|text| !text.is_empty()).map(Cow::Borrowed),
    };
    match (status.filter(|status| *status != 0), detail) {
        (Some(status), Some(detail)) => format!("{status} {detail}"),
        (Some(status), None) => format!("{status} status code (no body)"),
        (None, Some(detail)) => detail.into_owned(),
        (None, None) => "(no status code or body)".to_owned(),
    }
}

/// Wrap a provider `error` payload with the upstream explanation it carries.
fn failure(status: Option<u16>, error: Option<&Value>, body: Option<&str>) -> RequestFailure {
    let raw = error
        .and_then(|error| error.get("metadata"))
        .and_then(|metadata| metadata.get("raw"))
        .and_then(Value::as_str)
        .filter(|raw| !raw.is_empty())
        .map(str::to_owned);
    RequestFailure {
        message: describe(status, error, body),
        raw,
    }
}

/// Describe a non-success HTTP response from its status and body text.
pub(super) fn status_failure(status: u16, body: &str) -> RequestFailure {
    match serde_json::from_str::<Value>(body).ok().filter(is_truthy) {
        Some(parsed) => failure(Some(status), parsed.get("error"), None),
        None => failure(Some(status), None, Some(body)),
    }
}

/// Describe an error payload carried inside a successful event stream.
pub(crate) fn stream_failure(error: &Value) -> RequestFailure {
    failure(None, Some(error), None)
}

/// Join a base URL and an endpoint path, then normalize the result as a URL.
///
/// # Errors
/// Fails when the joined text is not a valid absolute URL.
pub(crate) fn endpoint_url(base: &str, path: &str) -> Result<String, RequestFailure> {
    let relative = if base.ends_with('/') {
        path.strip_prefix('/').unwrap_or(path)
    } else {
        path
    };
    Url::parse(&format!("{base}{relative}"))
        .map(String::from)
        .map_err(|error| RequestFailure::new(error.to_string()))
}
