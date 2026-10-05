//! Native single-attempt asynchronous HTTP using supplied runtime.
use crate::*;
use std::{
    collections::{BTreeMap, hash_map::RandomState},
    future::Future,
    hash::{BuildHasher, Hasher},
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
/// Native HTTP transport retaining system proxy, TLS and operating-system roots.
/// Creates no runtime, has no body deadline and disables hidden protocol retries.
pub struct NativeHttpTransport {
    client: reqwest::Client,
    random: RandomState,
    counter: AtomicU64,
}
impl NativeHttpTransport {
    /// Create a client without network I/O; the caller supplies a Tokio runtime.
    pub fn new() -> Result<Self, Failure> {
        Ok(Self {
            client: reqwest::Client::builder()
                .retry(reqwest::retry::never())
                .build()
                .map_err(|_| Failure::AdapterFailed)?,
            random: RandomState::new(),
            counter: AtomicU64::new(0),
        })
    }
}
async fn cancelled<F: Future>(
    future: F,
    cancellation: &Cancellation,
) -> Result<F::Output, Failure> {
    let mut future = std::pin::pin!(future);
    let mut cancelled = std::pin::pin!(cancellation.cancelled());
    std::future::poll_fn(|cx| {
        if cancelled.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(Err(Failure::Cancelled));
        }
        let result = future.as_mut().poll(cx);
        if cancellation.is_cancelled() {
            std::task::Poll::Ready(Err(Failure::Cancelled))
        } else {
            result.map(Ok)
        }
    })
    .await
}
impl ChatTransport for NativeHttpTransport {
    fn send(
        &self,
        request: ChatHttpRequest,
        timeout_ms: u64,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatHttpResponse, Failure>> + Send + '_>> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            if timeout_ms == 0 {
                return Err(Failure::SetupTimeout);
            }
            let mut builder = self.client.post(request.url).body(request.body);
            for (name, value) in request.headers {
                builder = builder.header(name, value);
            }
            let request = builder
                .build()
                .map_err(|_| Failure::InvalidRequestHeaders)?;
            let response = cancelled(
                tokio::time::timeout(
                    Duration::from_millis(timeout_ms),
                    self.client.execute(request),
                ),
                &cancellation,
            )
            .await?
            .map_err(|_| Failure::SetupTimeout)?
            .map_err(|_| Failure::Transport)?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().into(), v.into())))
                .collect::<BTreeMap<_, _>>();
            Ok(ChatHttpResponse {
                status,
                headers,
                body: Box::new(NativeBody(response)),
            })
        })
    }
    fn wait(
        &self,
        delay: Duration,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + '_>> {
        Box::pin(async move {
            let mut remaining = delay;
            loop {
                // Keep each sleep inside the timer wheel span without capping the total.
                let mut chunk = remaining.min(Duration::from_millis((1u64 << 36) - 1));
                while tokio::time::Instant::now().checked_add(chunk).is_none() {
                    chunk /= 2;
                }
                cancelled(tokio::time::sleep(chunk), &cancellation).await?;
                remaining -= chunk;
                if remaining.is_zero() {
                    return Ok(());
                }
            }
        })
    }
    fn now_unix_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64
    }
    fn jitter(&self) -> f64 {
        let mut hash = self.random.build_hasher();
        hash.write_u64(self.counter.fetch_add(1, Ordering::Relaxed));
        ((hash.finish() >> 11) as f64) * (0.25 / 9007199254740992.0)
    }
}
struct NativeBody(reqwest::Response);
impl ChatHttpBody for NativeBody {
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, Failure>> + Send + '_>> {
        Box::pin(async move {
            self.0
                .chunk()
                .await
                .map(|v| v.map(|b| b.to_vec()))
                .map_err(|_| Failure::Transport)
        })
    }
}
