//! Owned HTTP and controlled-time seam.
use crate::{Cancellation, Failure};
use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};
/// Sensitive authorized HTTP request; never safe to log.
#[derive(Clone)]
pub struct ChatHttpRequest {
    /// Complete sensitive request URL.
    pub url: String,
    /// Authorized literal headers.
    pub headers: BTreeMap<String, String>,
    /// Exact encoded JSON bytes.
    pub body: Vec<u8>,
}
/// Response headers and owned streaming body.
pub struct ChatHttpResponse {
    /// HTTP status before body reads.
    pub status: u16,
    /// Lowercase response header names; values may be sensitive.
    pub headers: BTreeMap<String, String>,
    /// Owned incremental bytes, released on drop.
    pub body: Box<dyn ChatHttpBody>,
}
/// Pull-based body bytes with no idle deadline.
pub trait ChatHttpBody: Send {
    /// Exact next bytes, clean EOF or a safe read failure.
    #[allow(clippy::type_complexity)]
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, Failure>> + Send + '_>>;
}
/// Replaceable single-attempt HTTP transport with controlled time.
pub trait ChatTransport: Send + Sync {
    /// POST once, returning at headers; cancellation wins ties and zero never polls I/O.
    fn send(
        &self,
        request: ChatHttpRequest,
        timeout_ms: u64,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatHttpResponse, Failure>> + Send + '_>>;
    /// Cancellable wait retaining the full requested duration.
    fn wait(
        &self,
        delay: Duration,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + '_>>;
    /// UTC Unix milliseconds for date-based retry delays.
    fn now_unix_millis(&self) -> u64;
    /// Noncryptographic jitter in [0,0.25).
    fn jitter(&self) -> f64;
}
