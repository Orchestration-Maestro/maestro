//! Replaceable HTTP transport with the shared request, retry and failure policy.

mod client;
mod failure;
mod headers;
mod line_decoder;
mod retry;
mod runtime;
mod server_sent_events;
mod text;

use std::collections::BTreeMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::{BoxFuture, Cancellation, DiagnosticErrorInfo};

pub(crate) use failure::{
    RequestFailure, endpoint_url, envelope_failure, sdk_status_failure, sdk_stream_failure,
    stream_failure,
};
pub(crate) use headers::{client_pairs, edge_whitespace, normalize_request};
pub(crate) use retry::{send, send_with_status_error};
pub(crate) use runtime::sleep;
pub(crate) use runtime::{Raced, race, spawn_detached};
pub(crate) use server_sent_events::{ServerSentEvent, SseMessages};
pub(crate) use text::decode_utf8;

/// One HTTP attempt: method, final URL, lowercase headers and body bytes.
#[derive(Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: String,
    /// Final request URL.
    pub url: String,
    /// Header names (lowercase) and values in insertion order. A value is text whose characters
    /// each name one byte. The sender trims tab, line feed, carriage return and space from both
    /// ends of each value, keeps a value that becomes empty, and rejects an invalid name, a
    /// character above U+00FF or a value holding NUL, a carriage return or a line feed before
    /// any attempt. Every other character reaches the client, whose own rules decide whether it
    /// can carry the value.
    pub headers: IndexMap<String, String>,
    /// Request body.
    pub body: Vec<u8>,
    /// Signal the sender races each attempt against; a replacement client may watch it too,
    /// but the plain default client ignores it.
    pub signal: Option<Cancellation>,
}

/// Response status and headers with a body that streams after the headers arrive.
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Status text supplied by the selected client. The default client uses native parsed text
    /// and empty browser text; see [`default_fetch`] for its platform policy.
    pub status_text: String,
    /// Response headers with lowercase names, one text per name. The default client reads each
    /// native value byte by byte as a character, joins repeated values with `, ` (`cookie`
    /// values with `; `) and keeps only the last `set-cookie`. In a browser the platform has
    /// decoded the values already, so they carry through as UTF-8 text.
    pub headers: BTreeMap<String, String>,
    /// Streamed body chunks.
    pub body: HttpBody,
}

/// Why an HTTP attempt produced no response.
#[derive(Clone, Debug, PartialEq)]
pub enum FetchError {
    /// The connection failed; retryable.
    Connection(DiagnosticErrorInfo),
    /// The setup timeout elapsed; retryable.
    Timeout,
    /// The request was cancelled; never retried.
    Aborted,
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(cause) => formatter.write_str(&cause.message),
            Self::Timeout => formatter.write_str("Request timed out."),
            Self::Aborted => formatter.write_str("Request was aborted."),
        }
    }
}

impl std::error::Error for FetchError {}

/// Streamed response body.
#[cfg(not(target_arch = "wasm32"))]
pub type HttpBody = Pin<Box<dyn futures_core::Stream<Item = Result<Vec<u8>, FetchError>> + Send>>;
/// Streamed response body.
#[cfg(target_arch = "wasm32")]
pub type HttpBody = Pin<Box<dyn futures_core::Stream<Item = Result<Vec<u8>, FetchError>>>>;

/// Replacement for the default HTTP client; called once per attempt.
#[cfg(not(target_arch = "wasm32"))]
pub type Fetch =
    Arc<dyn Fn(HttpRequest) -> BoxFuture<Result<HttpResponse, FetchError>> + Send + Sync>;
/// Replacement for the default HTTP client; called once per attempt.
#[cfg(target_arch = "wasm32")]
pub type Fetch = Arc<dyn Fn(HttpRequest) -> BoxFuture<Result<HttpResponse, FetchError>>>;

/// The default `reqwest`-backed client that requests use when `StreamOptions::fetch` is unset.
///
/// It sends one attempt without timeout, retry or cancellation policy; callers that need those
/// wrap it, or pass it as `StreamOptions::fetch` to share the process-wide client. A header it
/// cannot send (see [`HttpRequest::headers`]; the client also refuses a control character other
/// than tab, and DEL) is reported as a connection failure of that attempt, which a sender
/// retries. On native targets it needs a Tokio runtime with the I/O driver enabled.
///
/// Native HTTP/1.0 and HTTP/1.1 responses carry the parsed reason phrase, including an explicitly
/// empty phrase. Only an absent reason extension falls back to the status's canonical reason,
/// or empty when there is none. Other native versions return empty status text. The native parser
/// discards a phrase containing non-ASCII bytes; the default browser client returns empty status
/// text until its platform adapter supplies it.
#[must_use]
pub fn default_fetch() -> Fetch {
    Arc::new(client::fetch)
}
