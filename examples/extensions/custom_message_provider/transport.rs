//! The extension client's transient-failure retries over host-owned HTTP attempts.
use super::CustomAnthropic;
use maestro_extensions_wasm::{
    AbortController, AbortSignal, ExtensionResult, Subscription,
    http::{Request, Response},
};
use std::{
    future::poll_fn,
    rc::Rc,
    task::Poll,
    time::{Duration, UNIX_EPOCH},
};

/// Retries after the initial message request.
const RETRIES: u32 = 2;
/// Deadline for receiving response headers on each attempt.
const HEADERS_TIMEOUT: Duration = Duration::from_secs(600);
/// Initial retry delay before exponential growth and jitter, in seconds.
const INITIAL_DELAY: f64 = 0.5;
/// Largest fraction removed from the backoff by random jitter.
const JITTER: f64 = 0.25;
/// Retry hints at or above this duration use the client backoff instead.
const MAX_HINT_SECONDS: f64 = 60.0;

/// A response that retains caller cancellation until its body has been consumed.
pub(super) struct Received {
    /// The host's independently retained response.
    pub(super) response: Rc<dyn Response>,
    /// Removes caller-to-attempt forwarding when response consumption ends.
    _forwarding: Forwarding,
}

/// The caller's cancellation observer for one attempt.
struct Forwarding(Option<Subscription>);

impl Drop for Forwarding {
    fn drop(&mut self) {
        if let Some(subscription) = &self.0 {
            let _ = subscription.unsubscribe();
        }
    }
}

/// Sends a message request, retrying transient responses and connection failures.
///
/// # Errors
/// Returns exhausted connection failures, cancellation, timer failures or body-close failures.
pub(super) async fn send(
    provider: &CustomAnthropic,
    request: Request,
) -> ExtensionResult<Received> {
    let mut retries = 0;
    loop {
        if request.signal.as_ref().is_some_and(AbortSignal::aborted) {
            return Err("request was aborted".into());
        }
        let headers = match attempt(provider, &request).await {
            Ok(received)
                if received.response.ok()
                    || retries == RETRIES
                    || !retryable(&*received.response) =>
            {
                return Ok(received);
            }
            Ok(received) => {
                let headers = received.response.headers();
                if let Some(body) = received.response.body() {
                    body.close()?;
                }
                headers
            }
            Err(error) => {
                if retries == RETRIES || request.signal.as_ref().is_some_and(AbortSignal::aborted) {
                    return Err(error);
                }
                Vec::new()
            }
        };
        let delay = delay(provider, &headers, retries);
        provider.io.sleep(delay, request.signal.clone()).await?;
        retries += 1;
    }
}

/// Aborts the native attempt when its response-headers deadline elapses.
async fn attempt(provider: &CustomAnthropic, request: &Request) -> ExtensionResult<Received> {
    let controller = AbortController::new();
    let forwarding = Forwarding(
        request
            .signal
            .as_ref()
            .map(|signal| {
                let controller = controller.clone();
                signal.subscribe(Rc::new(move |_| controller.abort()))
            })
            .transpose()?,
    );
    if request.signal.as_ref().is_some_and(AbortSignal::aborted) {
        controller.abort()?;
    }
    let mut response = provider.io.fetch(Request {
        url: request.url.clone(),
        method: request.method.clone(),
        headers: request.headers.clone(),
        body: request.body.clone(),
        signal: Some(controller.signal()),
    });
    let mut timeout = provider.io.sleep(HEADERS_TIMEOUT, request.signal.clone());
    let response = poll_fn(|cx| {
        if let Poll::Ready(result) = response.as_mut().poll(cx) {
            return Poll::Ready(result);
        }
        if let Poll::Ready(result) = timeout.as_mut().poll(cx) {
            result?;
            controller.abort()?;
            return Poll::Ready(Err("message request timed out".into()));
        }
        Poll::Pending
    })
    .await?;
    Ok(Received {
        response,
        _forwarding: forwarding,
    })
}

/// Honors explicit retry admission before falling back to transient status codes.
fn retryable(response: &dyn Response) -> bool {
    let headers = response.headers();
    match header(&headers, "x-should-retry") {
        Some("true") => true,
        Some("false") => false,
        _ => matches!(response.status(), 408 | 409 | 429 | 500..),
    }
}

/// Finds the first matching response header, ignoring name case.
fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Accepts usable server hints; invalid or out-of-range hints use jittered exponential backoff.
fn delay(provider: &CustomAnthropic, headers: &[(String, String)], retries: u32) -> Duration {
    let millis = header(headers, "retry-after-ms").and_then(|text| text.trim().parse::<f64>().ok());
    let seconds = millis
        .filter(|millis| !millis.is_nan() && *millis != 0.0)
        .map(|millis| millis / 1000.0)
        .or_else(|| {
            let text = header(headers, "retry-after")?;
            text.trim().parse::<f64>().ok().or_else(|| {
                let date = httpdate::parse_http_date(text.trim()).ok()?;
                let epoch = date.duration_since(UNIX_EPOCH).ok()?.as_secs_f64();
                Some(epoch - (provider.now)() / 1000.0)
            })
        });
    let seconds = seconds
        .filter(|seconds| *seconds > 0.0 && *seconds < MAX_HINT_SECONDS)
        .unwrap_or_else(|| {
            INITIAL_DELAY * f64::from(1_u32 << retries) * (1.0 - (provider.random)() * JITTER)
        });
    Duration::from_secs_f64(seconds)
}
