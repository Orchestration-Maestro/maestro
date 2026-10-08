//! Failure text for requests, responses and stream error payloads.

use serde_json::value::RawValue;
use url::Url;

use super::FetchError;
use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{compact_raw, is_truthy, member};

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
fn json_text(value: &RawValue) -> String {
    compact_raw(value).unwrap_or_default()
}

/// Read a JSON string value as its text.
fn string_text(value: &RawValue) -> Option<String> {
    serde_json::from_str(value.get()).ok()
}

/// Build the failure text from a status, a provider `error` payload and plain body text.
fn describe(status: Option<u16>, error: Option<&RawValue>, body: Option<&str>) -> String {
    let detail = match error.filter(|error| is_truthy(error)) {
        Some(error) => Some(
            match member(error, "message").filter(|message| is_truthy(message)) {
                Some(message) => string_text(message).unwrap_or_else(|| json_text(message)),
                None => json_text(error),
            },
        ),
        None => body.filter(|text| !text.is_empty()).map(str::to_owned),
    };
    match (status.filter(|status| *status != 0), detail) {
        (Some(status), Some(detail)) => format!("{status} {detail}"),
        (Some(status), None) => format!("{status} status code (no body)"),
        (None, Some(detail)) => detail,
        (None, None) => "(no status code or body)".to_owned(),
    }
}

/// Wrap a provider `error` payload with the upstream explanation it carries.
fn failure(status: Option<u16>, error: Option<&RawValue>, body: Option<&str>) -> RequestFailure {
    let raw = error
        .and_then(|error| member(error, "metadata"))
        .and_then(|metadata| member(metadata, "raw"))
        .and_then(string_text)
        .filter(|raw| !raw.is_empty());
    RequestFailure {
        message: describe(status, error, body),
        raw,
    }
}

/// Describe a non-success HTTP response from its status and body text.
pub(super) fn status_failure(status: u16, body: &str) -> RequestFailure {
    let parsed = serde_json::from_str::<&RawValue>(body)
        .ok()
        .filter(|parsed| is_truthy(parsed));
    match parsed {
        Some(parsed) => failure(Some(status), member(parsed, "error"), None),
        None => failure(Some(status), None, Some(body)),
    }
}

/// Describe an error payload carried inside a successful event stream.
pub(crate) fn stream_failure(error: &RawValue) -> RequestFailure {
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
