//! Request policy shared by HTTP protocols: retries, timeouts, cancellation and failures.

#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the loopback server."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the scripted transport."
)]
#[path = "support/transport.rs"]
mod transport;

use std::time::{Duration, SystemTime};

use chat::{TestResult, rows};
use futures_util::StreamExt;
use indexmap::IndexMap;
use maestro_models::{
    Cancellation, DiagnosticErrorInfo, FetchError, HttpRequest, StopReason, StreamOptions,
    default_fetch,
};
use serde_json::{Value, json};
use tokio::time::Instant;
use transport::{Attempt, Transport, call_inputs, drain, finish, transport};

const FIXTURE: &str = include_str!("fixtures/chat_completions/http.json");

/// A connection failure with the given message.
fn broken(message: &str) -> FetchError {
    FetchError::Connection(DiagnosticErrorInfo {
        name: Some("FetchError".into()),
        message: message.into(),
        stack: None,
        code: None,
    })
}

/// Run a call through `transport` with `configure` applied to its options.
async fn run(
    transport: &Transport,
    configure: impl FnOnce(&mut StreamOptions),
) -> TestResult<transport::Outcome> {
    let (model, context, mut options) = call_inputs(transport)?;
    configure(&mut options);
    finish(model, context, options).await
}

/// Abort the request after `after` of virtual time, so a regression ends instead of retrying forever.
fn abort_after(options: &mut StreamOptions, after: Duration) {
    let signal = Cancellation::new();
    let trigger = signal.clone();
    tokio::spawn(async move {
        tokio::time::sleep(after).await;
        trigger.abort();
    });
    options.signal = Some(signal);
}

/// Each recorded status and `x-should-retry` value retries exactly when the table says so.
async fn statuses_follow_the_retry_table() -> TestResult {
    for row in rows(FIXTURE, "maestro_http_retries_transient_failures")? {
        let status = u16::try_from(row["status"].as_u64().ok_or("status")?)?;
        let hint: Vec<(&str, &str)> = row["override"]
            .as_str()
            .map(|value| ("x-should-retry", value))
            .into_iter()
            .collect();
        let target = transport(vec![
            Attempt::body(status, &hint, Vec::new()),
            Attempt::success(),
        ]);
        let outcome = run(&target, |_| {}).await?;
        if row["expected"] == json!(true) {
            assert_eq!(target.attempts(), 2, "{}", row["id"]);
            assert!(
                matches!(outcome.stop_reason, StopReason::Stop),
                "{}",
                row["id"]
            );
        } else {
            assert_eq!(target.attempts(), 1, "{}", row["id"]);
            let expected = format!("{status} status code (no body)");
            assert_eq!(outcome.error, Some(expected), "{}", row["id"]);
        }
    }
    let hinted = transport(vec![Attempt::status(200, &[("x-should-retry", "true")])]);
    assert!(matches!(
        run(&hinted, |_| {}).await?.stop_reason,
        StopReason::Stop
    ));
    assert_eq!(hinted.attempts(), 1, "a success status is never retried");
    Ok(())
}

/// Connection failures and timeouts are retried, then reported once retries run out.
async fn connection_failures_are_retried() -> TestResult {
    for (failure, text) in [
        (broken("reset"), "Connection error."),
        (FetchError::Timeout, "Request timed out."),
    ] {
        let recovering = transport(vec![Attempt::Fail(failure.clone()), Attempt::success()]);
        assert!(matches!(
            run(&recovering, |_| {}).await?.stop_reason,
            StopReason::Stop
        ));
        assert_eq!(recovering.attempts(), 2);

        let failing = transport(vec![Attempt::Fail(failure)]);
        assert_eq!(run(&failing, |_| {}).await?.error.as_deref(), Some(text));
        assert_eq!(
            failing.attempts(),
            3,
            "one attempt and two retries by default"
        );
    }
    Ok(())
}

#[test]
fn maestro_http_retries_transient_failures() -> TestResult {
    chat::block_on(true, async {
        statuses_follow_the_retry_table().await?;
        connection_failures_are_retried().await?;
        for (retries, attempts) in [
            (Some(0.0), 1),
            (Some(-0.0), 1),
            (Some(1.0), 2),
            (Some(3.0), 4),
            (None, 3),
        ] {
            let failing = transport(vec![Attempt::status(503, &[])]);
            run(&failing, |options| options.max_retries = retries).await?;
            assert_eq!(failing.attempts(), attempts, "{retries:?}");
        }
        Ok(())
    })
}

/// The wait a retry hint must produce.
enum Wait {
    /// Exactly this long.
    Exactly(Duration),
    /// Longer than the first bound and no longer than the second.
    Within(Duration, Duration),
    /// Exponential backoff for the first retry, reduced by jitter.
    Backoff,
}

/// An HTTP date `seconds` from now; negative values are in the past.
fn http_date(seconds: i64) -> String {
    let offset = Duration::from_secs(seconds.unsigned_abs());
    let now = SystemTime::now();
    httpdate::fmt_http_date(if seconds >= 0 {
        now + offset
    } else {
        now - offset
    })
}

/// Retry hints and the wait each must produce.
fn hint_cases<'a>(future: &'a str, past: &'a str) -> Vec<(Vec<(&'a str, &'a str)>, Wait)> {
    let ms = Duration::from_millis;
    vec![
        (vec![], Wait::Backoff),
        (
            vec![("retry-after-ms", "125.5")],
            Wait::Exactly(Duration::from_micros(125_500)),
        ),
        (vec![("retry-after", "1.25")], Wait::Exactly(ms(1250))),
        (
            vec![("retry-after-ms", "125"), ("retry-after", "7")],
            Wait::Exactly(ms(125)),
        ),
        (
            vec![("retry-after-ms", "0"), ("retry-after", "2")],
            Wait::Exactly(ms(2000)),
        ),
        (vec![("retry-after-ms", "0")], Wait::Exactly(Duration::ZERO)),
        (vec![("Retry-After", "2")], Wait::Exactly(ms(2000))),
        (
            vec![("retry-after", future)],
            Wait::Within(ms(87_000), ms(90_000)),
        ),
        (vec![("retry-after", past)], Wait::Exactly(Duration::ZERO)),
        (vec![("retry-after", "invalid")], Wait::Backoff),
        (vec![("retry-after-ms", "invalid")], Wait::Backoff),
        (
            vec![("retry-after-ms", "invalid"), ("retry-after", "3")],
            Wait::Exactly(ms(3000)),
        ),
        (vec![("retry-after", "-2")], Wait::Backoff),
        (vec![("retry-after-ms", "12garbage")], Wait::Backoff),
        (vec![("retry-after", "Infinity")], Wait::Backoff),
        (vec![("retry-after-ms", "NaN")], Wait::Backoff),
    ]
}

#[test]
fn maestro_http_honors_retry_headers() -> TestResult {
    chat::block_on(true, async {
        let (future, past) = (http_date(90), http_date(-60));
        let ms = Duration::from_millis;
        for (headers, wait) in hint_cases(&future, &past) {
            let target = transport(vec![Attempt::status(503, &headers), Attempt::success()]);
            run(&target, |_| {}).await?;
            let gap = *target.gaps().first().ok_or("no retry")?;
            let (low, high) = match wait {
                Wait::Exactly(exact) => (exact, exact),
                Wait::Within(low, high) => (low, high),
                Wait::Backoff => (ms(375), ms(500)),
            };
            let timer_resolution = ms(1);
            assert!(
                gap + timer_resolution > low && gap <= high + timer_resolution,
                "{headers:?} waited {gap:?}, expected {low:?} to {high:?}"
            );
        }
        Ok(())
    })
}

/// Each gap lies in the jittered window of its exponential backoff, capped at eight seconds.
fn assert_backoff_schedule(gaps: &[Duration]) -> TestResult {
    for (retries_made, gap) in gaps.iter().enumerate() {
        let base = (0.5 * 2.0_f64.powi(i32::try_from(retries_made)?)).min(8.0);
        let seconds = gap.as_secs_f64();
        assert!(
            seconds > base * 0.75 - 1e-9 && seconds <= base + 1e-9,
            "retry {retries_made} waited {seconds}s, base {base}s"
        );
    }
    Ok(())
}

#[test]
fn maestro_http_bounds_retry_backoff() -> TestResult {
    chat::block_on(true, async {
        let target = transport(vec![Attempt::status(503, &[])]);
        run(&target, |options| options.max_retries = Some(7.0)).await?;
        assert_eq!(target.gaps().len(), 7);
        assert_backoff_schedule(&target.gaps())?;

        // A floating-point countdown cannot step down from 2^54; the integer budget can.
        let huge = transport(vec![Attempt::status(503, &[])]);
        run(&huge, |options| {
            options.max_retries = Some(2.0_f64.powi(54));
            abort_after(options, Duration::from_secs(60));
        })
        .await?;
        assert!(huge.gaps().len() > 5, "the backoff reaches its cap");
        assert_backoff_schedule(&huge.gaps())
    })
}

/// The timeout in milliseconds that a fixture value names.
fn timeout_value(value: &Value) -> TestResult<f64> {
    match value.as_str() {
        Some("NaN") => Ok(f64::NAN),
        Some("Infinity") => Ok(f64::INFINITY),
        Some("-Infinity") => Ok(f64::NEG_INFINITY),
        Some(other) => Err(format!("unknown value {other}").into()),
        None => Ok(value.as_f64().ok_or("number")?),
    }
}

#[test]
fn maestro_http_preserves_setup_timeout() -> TestResult {
    chat::block_on(true, async {
        for row in rows(FIXTURE, "maestro_http_preserves_setup_timeout")? {
            let target = transport(vec![Attempt::success()]);
            let timeout = timeout_value(&row["timeout"])?;
            let outcome = run(&target, |options| options.timeout_ms = Some(timeout)).await?;
            match row["expected"]["error"].as_str() {
                Some(message) => {
                    assert_eq!(outcome.error.as_deref(), Some(message), "{}", row["id"]);
                    assert_eq!(target.attempts(), 0, "{} sends nothing", row["id"]);
                }
                None => assert!(
                    matches!(outcome.stop_reason, StopReason::Stop),
                    "{}",
                    row["id"]
                ),
            }
        }

        for (timeout, waited) in [
            (None, Duration::from_millis(600_000)),
            (Some(1200.0), Duration::from_millis(1200)),
            (Some(0.0), Duration::ZERO),
        ] {
            let hanging = transport(vec![Attempt::Hang]);
            let started = Instant::now();
            let outcome = run(&hanging, |options| {
                options.timeout_ms = timeout;
                options.max_retries = Some(0.0);
            })
            .await?;
            assert_eq!(outcome.error.as_deref(), Some("Request timed out."));
            assert_eq!(started.elapsed(), waited, "{timeout:?}");
        }

        let slow = transport(vec![Attempt::slow_body(Duration::from_secs(700))]);
        let started = Instant::now();
        let outcome = run(&slow, |options| options.max_retries = Some(0.0)).await?;
        assert!(
            matches!(outcome.stop_reason, StopReason::Stop),
            "a slow body is not cut short"
        );
        assert!(started.elapsed() >= Duration::from_secs(700));
        Ok(())
    })
}

#[test]
fn maestro_http_stops_retries_on_abort() -> TestResult {
    chat::block_on(true, async {
        let target = transport(vec![Attempt::success()]);
        let signal = Cancellation::new();
        signal.abort();
        let outcome = run(&target, |options| options.signal = Some(signal.clone())).await?;
        assert!(matches!(outcome.stop_reason, StopReason::Aborted));
        assert_eq!(outcome.error.as_deref(), Some("Request was aborted."));
        assert_eq!(target.attempts(), 0, "no request after an abort");

        let hanging = transport(vec![Attempt::Hang]);
        let started = Instant::now();
        let outcome = run(&hanging, |options| {
            abort_after(options, Duration::from_secs(5));
        })
        .await?;
        assert!(matches!(outcome.stop_reason, StopReason::Aborted));
        assert_eq!(started.elapsed(), Duration::from_secs(5));
        assert_eq!(hanging.attempts(), 1);

        let reading = transport(vec![Attempt::streaming_with_status(500, || {
            Box::pin(futures_util::stream::pending())
        })]);
        let started = Instant::now();
        let outcome = run(&reading, |options| {
            abort_after(options, Duration::from_secs(7));
        })
        .await?;
        assert!(matches!(outcome.stop_reason, StopReason::Aborted));
        assert_eq!(outcome.error.as_deref(), Some("Request was aborted."));
        assert_eq!(
            started.elapsed(),
            Duration::from_secs(7),
            "the error body read is interrupted"
        );

        let waiting = transport(vec![
            Attempt::status(503, &[("retry-after", "3600")]),
            Attempt::success(),
        ]);
        let started = Instant::now();
        let outcome = run(&waiting, |options| {
            abort_after(options, Duration::from_secs(30));
        })
        .await?;
        assert!(matches!(outcome.stop_reason, StopReason::Aborted));
        assert_eq!(
            started.elapsed(),
            Duration::from_secs(30),
            "the wait is interrupted"
        );
        assert_eq!(waiting.attempts(), 1, "no attempt follows the abort");
        Ok(())
    })
}

#[test]
fn maestro_http_rejects_unbounded_retry_counts() -> TestResult {
    chat::block_on(true, async {
        for (retries, message) in [
            (-1.0, "maxRetries must be a positive integer"),
            (0.5, "maxRetries must be an integer"),
            (-0.5, "maxRetries must be an integer"),
            (f64::NAN, "maxRetries must be an integer"),
            (f64::INFINITY, "maxRetries must be an integer"),
            (f64::NEG_INFINITY, "maxRetries must be an integer"),
            (2.0_f64.powi(64), "maxRetries must be an integer"),
            (f64::MAX, "maxRetries must be an integer"),
        ] {
            let target = transport(vec![Attempt::status(503, &[])]);
            let outcome = run(&target, |options| {
                options.max_retries = Some(retries);
                abort_after(options, Duration::from_secs(3600));
            })
            .await?;
            assert_eq!(outcome.error.as_deref(), Some(message), "{retries}");
            assert_eq!(target.attempts(), 0, "{retries} sends nothing");
        }
        Ok(())
    })
}

/// Failures a scripted transport reports, with the text each produces.
async fn scripted_failures() -> TestResult {
    for row in rows(FIXTURE, "maestro_http_reports_fetch_failures")? {
        let input = &row["input"];
        let body = match (input.get("error"), input["message"].as_str()) {
            (Some(error), _) => json!({"error": error}).to_string(),
            (None, message) => message.unwrap_or_default().to_owned(),
        };
        let attempt = match input["status"].as_u64() {
            Some(status) => Attempt::body(u16::try_from(status)?, &[], body.into_bytes()),
            None => Attempt::body(
                200,
                &[],
                format!("data: {}\n\n", json!({"error": input["error"]})).into_bytes(),
            ),
        };
        let outcome = run(&transport(vec![attempt]), |options| {
            options.max_retries = Some(0.0);
        })
        .await?;
        let expected = row["expected"].as_str();
        assert_eq!(outcome.error.as_deref(), expected, "{}", row["id"]);
    }

    for (failure, text) in [
        (broken("reset"), "Connection error."),
        (FetchError::Timeout, "Request timed out."),
        (FetchError::Aborted, "Request was aborted."),
    ] {
        let target = transport(vec![Attempt::Fail(failure)]);
        let outcome = run(&target, |options| options.max_retries = Some(0.0)).await?;
        assert_eq!(outcome.error.as_deref(), Some(text));
    }

    let unreadable = transport(vec![Attempt::BrokenBody {
        status: 502,
        error: broken("socket closed"),
    }]);
    let outcome = run(&unreadable, |options| options.max_retries = Some(0.0)).await?;
    assert_eq!(outcome.error.as_deref(), Some("502 socket closed"));
    Ok(())
}

/// A call to the loopback address, returning the update labels and the final message.
async fn call_loopback(url: &str) -> TestResult<(Vec<String>, maestro_models::AssistantMessage)> {
    let mut model = chat::model(&json!({}))?;
    model.base_url = format!("{url}/v1");
    let history = chat::context(&json!({"messages": [{"role": "user", "content": "hello"}]}))?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        max_retries: Some(0.0),
        ..StreamOptions::default()
    };
    let options = maestro_models::OpenAICompletionsOptions {
        common,
        ..Default::default()
    };
    let stream = maestro_models::stream_openai_completions(model, history, Some(options));
    drain(&stream).await
}

/// Failures of the default client: a refused connection and a body cut short.
async fn production_client_failures() -> TestResult {
    let closed = std::net::TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", closed.local_addr()?);
    drop(closed);
    let (labels, message) = call_loopback(&url).await?;
    assert_eq!(labels, ["error"]);
    assert_eq!(message.error_message.as_deref(), Some("Connection error."));

    let event = json!({"id": "r", "model": "model",
        "choices": [{"index": 0, "delta": {"content": "so far"}}]});
    let server = loopback::serve(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 100000\r\n\r\n",
        vec![format!("data: {event}\n\n").into_bytes()],
        Duration::from_millis(5),
    )?;
    let (labels, message) = call_loopback(&server.url).await?;
    server.finish()?;
    assert_eq!(labels, ["start", "text_start", "text_delta", "error"]);
    assert!(matches!(message.stop_reason, StopReason::Error));
    assert!(message.error_message.is_some_and(|text| !text.is_empty()));
    Ok(())
}

#[test]
fn maestro_http_reports_fetch_failures() -> TestResult {
    if child_process::child_case().is_some() {
        return chat::block_on(false, production_client_failures());
    }
    chat::block_on(true, scripted_failures())?;
    child_process::rerun("maestro_http_reports_fetch_failures", "production", &[])
}

/// One request through the default client against a loopback server.
async fn default_fetch_exchange() -> TestResult {
    let server = loopback::serve(
        "HTTP/1.1 201 Created\r\nx-result: fixture\r\ncontent-length: 5\r\nconnection: close\r\n\r\n",
        vec![b"he".to_vec(), b"llo".to_vec()],
        Duration::from_millis(2),
    )?;
    let request = HttpRequest {
        method: "POST".into(),
        url: format!("{}/echo", server.url),
        headers: IndexMap::from([("x-test".to_owned(), "1".to_owned())]),
        body: b"payload".to_vec(),
        signal: None,
    };
    let mut response = default_fetch()(request).await?;
    assert_eq!(response.status, 201);
    assert_eq!(
        response.headers.get("x-result").map(String::as_str),
        Some("fixture")
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend(chunk?);
    }
    assert_eq!(body, b"hello");
    let received = server.finish()?;
    assert_eq!(received.request_line, "POST /echo HTTP/1.1");
    assert_eq!(received.header("x-test"), Some("1"));
    assert_eq!(received.body, b"payload");
    Ok(())
}

#[test]
fn maestro_http_default_fetch_sends_requests() -> TestResult {
    if child_process::child_case().is_some() {
        return chat::block_on(false, default_fetch_exchange());
    }
    child_process::rerun("maestro_http_default_fetch_sends_requests", "default", &[])
}

#[test]
fn maestro_http_joins_endpoint_urls() -> TestResult {
    chat::block_on(true, async {
        for row in rows(FIXTURE, "maestro_http_joins_endpoint_urls")? {
            let target = transport(vec![Attempt::success()]);
            let (mut model, context, options) = call_inputs(&target)?;
            model.base_url = row["base"].as_str().ok_or("base")?.to_owned();
            finish(model, context, options).await?;
            let sent = target.requests.lock().map_err(|e| e.to_string())?;
            assert_eq!(
                sent.first().map(|r| r.url.as_str()),
                row["expected"].as_str(),
                "{}",
                row["id"]
            );
        }
        Ok(())
    })
}
