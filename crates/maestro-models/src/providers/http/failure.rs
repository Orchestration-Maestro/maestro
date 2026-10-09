//! Failure text for requests, responses and stream error payloads.

use serde_json::value::RawValue;
use url::Url;

use super::FetchError;
use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{compact_raw, is_truthy, member, raw_json};

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

/// Read a JSON string value as its text.
fn string_text(value: &RawValue) -> Option<String> {
    serde_json::from_str(value.get()).ok()
}

/// Build the failure text from a status, a provider `error` payload and plain body text.
fn describe(
    status: Option<u16>,
    error: Option<&RawValue>,
    body: Option<&str>,
) -> Result<String, serde_json::Error> {
    let detail = match error.filter(|error| is_truthy(error)) {
        Some(error) => Some(
            match member(error, "message").filter(|message| is_truthy(message)) {
                Some(message) if message.get().starts_with('"') => {
                    serde_json::from_str(message.get())?
                }
                Some(message) => compact_raw(message)?,
                None => compact_raw(error)?,
            },
        ),
        None => body.filter(|text| !text.is_empty()).map(str::to_owned),
    };
    Ok(match (status.filter(|status| *status != 0), detail) {
        (Some(status), Some(detail)) => format!("{status} {detail}"),
        (Some(status), None) => format!("{status} status code (no body)"),
        (None, Some(detail)) => detail,
        (None, None) => "(no status code or body)".to_owned(),
    })
}

/// Wrap a provider `error` payload with the upstream explanation it carries.
fn failure(status: Option<u16>, error: Option<&RawValue>, body: Option<&str>) -> RequestFailure {
    let raw = error
        .and_then(|error| member(error, "metadata"))
        .and_then(|metadata| member(metadata, "raw"))
        .and_then(string_text)
        .filter(|raw| !raw.is_empty());
    match describe(status, error, body) {
        Ok(message) => RequestFailure { message, raw },
        Err(error) => RequestFailure::new(error.to_string()),
    }
}

/// Describe a non-success HTTP response from its status and body text.
pub(super) fn status_failure(status: u16, body: &str) -> RequestFailure {
    let parsed = raw_json(body).ok().filter(|parsed| is_truthy(parsed));
    match parsed {
        Some(parsed) => failure(Some(status), member(parsed, "error"), None),
        None => failure(Some(status), None, Some(body)),
    }
}

/// Describe a non-success HTTP response using its truthy parsed body's truthy `message`,
/// otherwise that body; non-truthy or unparsed bodies use their original text, or are bodiless
/// when empty. Selected JSON details use string text or escaped JSON; rendering failures
/// retain their native diagnostic.
pub(crate) fn envelope_failure(status: u16, body: &str) -> RequestFailure {
    let parsed = raw_json(body).ok().filter(|parsed| is_truthy(parsed));
    match parsed {
        Some(parsed) => rendered_failure(describe(Some(status), Some(parsed), None)),
        None => rendered_failure(describe(Some(status), None, Some(body))),
    }
}

/// Describe a non-success SDK response without chat-specific metadata decoration.
pub(crate) fn sdk_status_failure(status: u16, body: &str) -> RequestFailure {
    let parsed = raw_json(body).ok().filter(|parsed| is_truthy(parsed));
    rendered_failure(match parsed {
        Some(parsed) => describe(Some(status), member(parsed, "error"), None),
        None => describe(Some(status), None, Some(body)),
    })
}

/// Describe a streamed SDK error without chat-specific metadata decoration.
pub(crate) fn sdk_stream_failure(error: &RawValue) -> Result<RequestFailure, serde_json::Error> {
    describe(None, Some(error), None).map(RequestFailure::new)
}

/// Keep a rendering failure as the native diagnostic rather than an empty detail.
fn rendered_failure(result: Result<String, serde_json::Error>) -> RequestFailure {
    RequestFailure::new(result.unwrap_or_else(|error| error.to_string()))
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
