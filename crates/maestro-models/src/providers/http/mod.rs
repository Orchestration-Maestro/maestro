//! Replaceable HTTP transport with the shared request, retry and failure policy.

mod client;
mod failure;
mod retry;
mod runtime;
mod text;

use std::collections::BTreeMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::{BoxFuture, Cancellation, DiagnosticErrorInfo};

pub(crate) use failure::{RequestFailure, endpoint_url, stream_failure};
pub(crate) use retry::send;
pub(crate) use runtime::{Raced, race, spawn_detached};
pub(crate) use text::TextDecoder;

/// One HTTP attempt: method, final URL, ordered lowercase headers and body bytes.
#[derive(Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: String,
    /// Final request URL.
    pub url: String,
    /// Header names (lowercase) and values in wire order.
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
    /// Response headers with lowercase names.
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
/// wrap it, or pass it as `StreamOptions::fetch` to share the process-wide client.
#[must_use]
pub fn default_fetch() -> Fetch {
    Arc::new(client::fetch)
}
