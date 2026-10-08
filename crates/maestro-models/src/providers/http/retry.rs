//! Send one request with setup timeout, cancellation and transient-failure retries.

use std::collections::BTreeMap;
use std::time::Duration;

use super::failure::status_failure;
use super::runtime::{Raced, race, sleep, unit_random};
use super::{FetchError, HttpRequest, HttpResponse, RequestFailure, client, decode_utf8};
use crate::{Cancellation, StreamOptions};

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
    if matches!(error, FetchError::Timeout) {
        RequestFailure::new(error.to_string())
    } else {
        RequestFailure::new("Connection error.")
    }
}

/// Read a failed response body as text, reporting a read failure as that text.
async fn read_text(
    response: HttpResponse,
    signal: Option<&Cancellation>,
) -> Result<String, RequestFailure> {
    use futures_util::StreamExt;

    let mut body = response.body;
    let mut bytes = Vec::new();
    let reading = async {
        while let Some(chunk) = body.next().await {
            match chunk {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(error) => return Some(error.to_string()),
            }
        }
        None
    };
    match race(reading, None, signal).await {
        Raced::Cancelled => Err(RequestFailure::aborted()),
        Raced::Done(Some(failure)) => Ok(failure),
        Raced::Done(None) | Raced::TimedOut => Ok(decode_utf8(&bytes).into_owned()),
    }
}

/// Send the request, retrying connection failures and transient statuses.
///
/// Only a success status returns a response, and it is accepted before any retry header is
/// read; the body is left unread.
///
/// # Errors
/// Fails on invalid timeout or retry settings, cancellation, exhausted retries and
/// non-success responses.
pub(crate) async fn send(
    request: HttpRequest,
    options: &StreamOptions,
) -> Result<HttpResponse, RequestFailure> {
    let timeout_ms = whole_number("timeout", options.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))?;
    let budget = retry_budget(options.max_retries.unwrap_or(DEFAULT_RETRIES))?;
    let timeout = Duration::try_from_secs_f64(timeout_ms / 1000.0).unwrap_or(Duration::MAX);
    let signal = request.signal.clone();
    let mut remaining = budget;
    loop {
        if signal.as_ref().is_some_and(Cancellation::is_aborted) {
            return Err(RequestFailure::aborted());
        }
        let attempt = match &options.fetch {
            Some(fetch) => fetch(request.clone()),
            None => client::fetch(request.clone()),
        };
        let failure = match race(attempt, Some(timeout), signal.as_ref()).await {
            Raced::Cancelled | Raced::Done(Err(FetchError::Aborted)) => {
                return Err(RequestFailure::aborted());
            }
            Raced::TimedOut => FetchError::Timeout,
            Raced::Done(Err(error)) => error,
            Raced::Done(Ok(response)) if (200..300).contains(&response.status) => {
                return Ok(response);
            }
            Raced::Done(Ok(response)) => {
                if should_retry(&response)
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
                return Err(status_failure(status, &text));
            }
        };
        let Some(left) = remaining.checked_sub(1) else {
            return Err(exhausted(&failure));
        };
        pause(backoff(budget - remaining), signal.as_ref()).await?;
        remaining = left;
    }
}
