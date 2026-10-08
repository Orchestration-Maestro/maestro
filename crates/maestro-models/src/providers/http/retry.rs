//! Send one request with setup timeout, cancellation and transient-failure retries.

use std::collections::BTreeMap;
use std::time::Duration;

use super::failure::status_failure;
use super::runtime::{Raced, race, sleep, unit_random};
use super::{FetchError, HttpRequest, HttpResponse, RequestFailure, client};
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

/// Accept a whole number of at least zero, naming the setting in the failure.
fn whole_number(name: &str, value: f64) -> Result<f64, RequestFailure> {
    if value.fract() != 0.0 || !value.is_finite() {
        Err(RequestFailure::new(format!("{name} must be an integer")))
    } else if value < 0.0 {
        Err(RequestFailure::new(format!(
            "{name} must be a positive integer"
        )))
    } else {
        Ok(value)
    }
}

/// Report whether a status or an explicit header asks for another attempt.
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

/// Use the server's retry hint: `retry-after-ms` when nonzero, else `retry-after`.
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
fn backoff(retries_made: f64) -> Duration {
    let base = (BACKOFF_BASE_SECONDS * 2.0_f64.powf(retries_made)).min(BACKOFF_CAP_SECONDS);
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
                Ok(chunk) => bytes.extend(chunk),
                Err(error) => return Some(error.to_string()),
            }
        }
        None
    };
    match race(reading, None, signal).await {
        Raced::Cancelled => Err(RequestFailure::aborted()),
        Raced::Done(Some(failure)) => Ok(failure),
        Raced::Done(None) | Raced::TimedOut => Ok(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

/// Send the request, retrying connection failures and transient statuses.
///
/// Only a success status returns a response; the body is left unread.
///
/// # Errors
/// Fails on invalid timeout or retry settings, cancellation, exhausted retries and
/// non-success responses.
pub(crate) async fn send(
    request: HttpRequest,
    options: &StreamOptions,
) -> Result<HttpResponse, RequestFailure> {
    let timeout_ms = whole_number("timeout", options.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))?;
    let max_retries = whole_number("maxRetries", options.max_retries.unwrap_or(DEFAULT_RETRIES))?;
    let timeout = Duration::try_from_secs_f64(timeout_ms / 1000.0).unwrap_or(Duration::MAX);
    let signal = request.signal.clone();
    let mut remaining = max_retries;
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
                if remaining > 0.0 && should_retry(&response) {
                    let delay = hinted_delay(&response.headers)
                        .unwrap_or_else(|| backoff(max_retries - remaining));
                    drop(response);
                    pause(delay, signal.as_ref()).await?;
                    remaining -= 1.0;
                    continue;
                }
                let status = response.status;
                let text = read_text(response, signal.as_ref()).await?;
                return Err(status_failure(status, &text));
            }
        };
        if remaining <= 0.0 {
            return Err(exhausted(&failure));
        }
        pause(backoff(max_retries - remaining), signal.as_ref()).await?;
        remaining -= 1.0;
    }
}
