//! Send one request with setup timeout, cancellation and transient-failure retries.

use std::collections::BTreeMap;
use std::time::Duration;

use super::failure::Ending;
use super::failure::status_failure;
use super::headers::normalize_request;
use super::runtime::{Raced, race, sleep, unit_random};
use super::{FetchError, HttpRequest, HttpResponse, RequestFailure, decode_utf8};
use crate::{Cancellation, ProviderExecution, StreamOptions};

/// Milliseconds a response may take to start before the attempt times out.
const DEFAULT_TIMEOUT_MS: f64 = 600_000.0;
/// Retries after the first attempt.
const DEFAULT_RETRIES: f64 = 2.0;
/// Base of the exponential backoff, in seconds.
const BACKOFF_BASE_SECONDS: f64 = 0.5;
/// Longest backoff before jitter, in seconds.
const BACKOFF_CAP_SECONDS: f64 = 8.0;
/// Largest share of the backoff that jitter may remove.
const JITTER_SHARE: f64 = 0.25;

/// Response-status admission for the selected protocol.
#[derive(Clone, Copy)]
pub(crate) enum RetryStatuses {
    /// Legacy provider headers and status admission.
    Legacy,
    /// Authentication library status ranges, independent of retry headers.
    #[cfg(not(target_arch = "wasm32"))]
    Authentication,
}

/// Per-operation policy consumed by the shared attempt loop.
#[derive(Clone, Copy)]
pub(crate) struct RequestPolicy {
    /// Headers deadline; a buffered adapter also includes body collection.
    pub(crate) timeout: Option<Duration>,
    /// Global retry budget, including all preceding failures.
    pub(crate) retries: u64,
    /// Maximum preceding retries that permit a no-response retry.
    pub(crate) no_response_retries: u64,
    /// Status admission policy.
    pub(crate) statuses: RetryStatuses,
}

impl RetryStatuses {
    /// Decide admission without replacing the global retry counter.
    fn admits(self, response: &HttpResponse) -> bool {
        match self {
            Self::Legacy => should_retry(response),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Authentication => matches!(response.status, 100..=199 | 408 | 429 | 500..=599),
        }
    }
}

/// The failure for a setting that is not a usable whole number.
fn not_an_integer(name: &str) -> RequestFailure {
    RequestFailure::new(format!("{name} must be an integer"))
}

/// Accept a whole number of at least zero, naming the setting in the failure.
fn whole_number(name: &str, value: f64) -> Result<f64, RequestFailure> {
    if value.fract() != 0.0 || !value.is_finite() {
        Err(not_an_integer(name))
    } else if value < 0.0 {
        Err(RequestFailure::new(format!(
            "{name} must be a positive integer"
        )))
    } else {
        Ok(value)
    }
}

/// The longest wait of one request as a duration: a whole number of milliseconds of at least
/// zero, where one too long to represent never elapses.
///
/// # Errors
/// Fails when the setting is fractional, nonfinite or negative.
pub(crate) fn request_timeout(milliseconds: f64) -> Result<Duration, RequestFailure> {
    let milliseconds = whole_number("timeout", milliseconds)?;
    Ok(Duration::try_from_secs_f64(milliseconds / 1000.0).unwrap_or(Duration::MAX))
}

/// Count the retries a request may make as an exact integer.
///
/// A whole number converts exactly through the seconds of a duration, so no lossy cast is
/// needed; counts beyond what an integer holds are rejected.
fn retry_budget(count: f64) -> Result<u64, RequestFailure> {
    Duration::try_from_secs_f64(whole_number("maxRetries", count)?)
        .map(|seconds| seconds.as_secs())
        .map_err(|_| not_an_integer("maxRetries"))
}

/// Report whether a non-success response asks for another attempt: `x-should-retry` decides
/// when it is `true` or `false`, otherwise the status 408, 409, 429 or at least 500 does.
fn should_retry(response: &HttpResponse) -> bool {
    match header(&response.headers, "x-should-retry") {
        Some("true") => true,
        Some("false") => false,
        _ => matches!(response.status, 408 | 409 | 429 | 500..),
    }
}

/// Look up a response header by name, ignoring case.
fn header<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Read a finite, non-negative number of seconds.
fn seconds(text: &str) -> Option<Duration> {
    text.trim()
        .parse::<f64>()
        .ok()
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
}

/// Read a hint given as milliseconds.
fn milliseconds(text: &str) -> Option<Duration> {
    text.trim()
        .parse::<f64>()
        .ok()
        .and_then(|millis| Duration::try_from_secs_f64(millis / 1000.0).ok())
}

/// Read a hint given as an HTTP date; dates already past mean no wait.
fn until_date(text: &str) -> Option<Duration> {
    let date = httpdate::parse_http_date(text.trim()).ok()?;
    let target = date.duration_since(std::time::UNIX_EPOCH).ok()?;
    let now = Duration::try_from_secs_f64(crate::records::diagnostics::timestamp_now() / 1000.0)
        .unwrap_or_default();
    Some(target.saturating_sub(now))
}

/// Use the retry hint of a response about to be retried: `retry-after-ms` when nonzero, else
/// `retry-after`.
fn hinted_delay(headers: &BTreeMap<String, String>) -> Option<Duration> {
    let millis = header(headers, "retry-after-ms").and_then(milliseconds);
    if let Some(delay) = millis.filter(|delay| !delay.is_zero()) {
        return Some(delay);
    }
    let after =
        header(headers, "retry-after").and_then(|text| seconds(text).or_else(|| until_date(text)));
    after.or(millis)
}

/// Exponential backoff for the given number of retries already made, reduced by jitter.
fn backoff(retries_made: u64) -> Duration {
    let doublings = i32::try_from(retries_made).unwrap_or(i32::MAX);
    let base = (BACKOFF_BASE_SECONDS * 2.0_f64.powi(doublings)).min(BACKOFF_CAP_SECONDS);
    Duration::from_secs_f64(base * (1.0 - unit_random() * JITTER_SHARE))
}

/// Wait out a retry delay unless the signal aborts first.
async fn pause(delay: Duration, signal: Option<&Cancellation>) -> Result<(), RequestFailure> {
    match race(sleep(delay), None, signal).await {
        Raced::Cancelled => Err(RequestFailure::aborted()),
        Raced::Done(()) | Raced::TimedOut => Ok(()),
    }
}

/// Describe the last failed attempt after retries are exhausted.
fn exhausted(error: &FetchError) -> RequestFailure {
    let mut failure = if matches!(error, FetchError::Timeout) {
        RequestFailure::new(error.to_string())
    } else {
        RequestFailure::new("Connection error.")
    };
    failure.ending = Some(Ending::NoResponse);
    failure
}

/// Read a failed response body as text, reporting a read failure as that text.
async fn read_text(
    response: HttpResponse,
    signal: Option<&Cancellation>,
) -> Result<String, RequestFailure> {
    match race(super::collect_body(response.body), None, signal).await {
        Raced::Cancelled => Err(RequestFailure::aborted()),
        Raced::Done(Err(error)) => Ok(error.to_string()),
        Raced::Done(Ok(bytes)) => Ok(decode_utf8(&bytes).into_owned()),
        Raced::TimedOut => Ok(String::new()),
    }
}

/// Send the request, retrying connection failures and transient statuses.
///
/// Only a success status returns a response, and it is accepted before any retry header is
/// read; the body is left unread. A non-success response ends as the failure the chat
/// protocol describes.
///
/// # Errors
/// Fails on a header that breaks the Fetch Standard's header rules (see
/// [`HttpRequest::headers`]), invalid timeout or retry settings, cancellation, exhausted
/// retries and non-success responses.
pub(crate) async fn send<E: ProviderExecution>(
    request: HttpRequest,
    options: &StreamOptions<E>,
) -> Result<HttpResponse, RequestFailure> {
    send_with_status_error(request, options, status_failure).await
}

/// Send the request as [`send`] does, describing the final non-success response with
/// `format_error`, which receives the status and the body text.
///
/// # Errors
/// Fails as [`send`] does.
pub(crate) async fn send_with_status_error<E: ProviderExecution>(
    request: HttpRequest,
    options: &StreamOptions<E>,
    format_error: fn(u16, &str) -> RequestFailure,
) -> Result<HttpResponse, RequestFailure> {
    let timeout = request_timeout(options.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))?;
    let budget = retry_budget(options.max_retries.unwrap_or(DEFAULT_RETRIES))?;
    let policy = RequestPolicy {
        timeout: Some(timeout),
        retries: budget,
        no_response_retries: budget,
        statuses: RetryStatuses::Legacy,
    };
    send_with_policy(
        request,
        options.fetch.clone().unwrap_or_else(super::default_fetch),
        policy,
        format_error,
    )
    .await
}

/// Send through the single attempt loop with the caller's protocol policy.
pub(crate) async fn send_with_policy(
    mut request: HttpRequest,
    fetch: super::Fetch,
    policy: RequestPolicy,
    format_error: fn(u16, &str) -> RequestFailure,
) -> Result<HttpResponse, RequestFailure> {
    normalize_request(&mut request.headers).map_err(RequestFailure::new)?;
    let budget = policy.retries;
    let signal = request.signal.clone();
    let mut remaining = budget;
    loop {
        if signal.as_ref().is_some_and(Cancellation::is_aborted) {
            return Err(RequestFailure::aborted());
        }
        let attempt = fetch(request.clone());
        let failure = match race(attempt, policy.timeout, signal.as_ref()).await {
            Raced::Cancelled | Raced::Done(Err(FetchError::Aborted)) => {
                return Err(RequestFailure::aborted());
            }
            Raced::TimedOut => FetchError::Timeout,
            Raced::Done(Err(error)) => error,
            Raced::Done(Ok(response)) if (200..300).contains(&response.status) => {
                return Ok(response);
            }
            Raced::Done(Ok(response)) => {
                if policy.statuses.admits(&response)
                    && let Some(left) = remaining.checked_sub(1)
                {
                    let delay = hinted_delay(&response.headers)
                        .unwrap_or_else(|| backoff(budget - remaining));
                    drop(response);
                    pause(delay, signal.as_ref()).await?;
                    remaining = left;
                    continue;
                }
                let status = response.status;
                let text = read_text(response, signal.as_ref()).await?;
                let mut failure = format_error(status, &text);
                failure.ending = Some(Ending::Status(status));
                return Err(failure);
            }
        };
        if budget - remaining >= policy.no_response_retries {
            return Err(exhausted(&failure));
        }
        let Some(left) = remaining.checked_sub(1) else {
            return Err(exhausted(&failure));
        };
        pause(backoff(budget - remaining), signal.as_ref()).await?;
        remaining = left;
    }
}

#[cfg(test)]
mod tests;
