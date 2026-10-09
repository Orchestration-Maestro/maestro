//! Controlled subscription policy cases.
use super::{
    callback::{AuthorizationCode, parse_authorization_input, route},
    exchange,
};
use crate::{
    DiagnosticCode, DiagnosticErrorInfo, Fetch, FetchError, HttpResponse, OAuthCredentials,
    OAuthError,
};
use futures_util::FutureExt as _;
use serde::Deserialize;
use std::{cell::Cell, collections::BTreeMap, future::Future, sync::Arc};

/// Stored oracle corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Stored precedence.
    precedence: Vec<InputCase>,
    /// Stored whitespace.
    whitespace: Vec<InputCase>,
    /// Stored query.
    query: Vec<InputCase>,
    /// Stored diagnostics.
    diagnostics: Vec<DiagnosticCase>,
    /// Stored routes.
    routes: Vec<RouteCase>,
    /// Stored tokens.
    tokens: Vec<TokenCase>,
    /// Stored requests.
    requests: Vec<RequestCase>,
    /// Stored response text.
    response_text: Vec<TextCase>,
}

/// Stored oracle inputcase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputCase {
    /// Stored input.
    input: String,
    /// Stored expected.
    expected: AuthorizationCode,
}

/// Stored oracle diagnosticcase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticCase {
    /// Stored input.
    input: ErrorInput,
    /// Stored expected.
    expected: String,
}

/// Supplied numeric marker or scalar code.
#[derive(Deserialize)]
#[serde(untagged)]
enum CodeInput {
    /// Text code.
    Text(String),
    /// Numeric code.
    Number(f64),
    /// Nonfinite or signed-zero marker.
    Marker(NumberMarker),
}
/// Stored oracle numbermarker.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberMarker {
    /// Stored number.
    number: String,
}

/// Structured error or literal text.
#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorInput {
    /// Error fields.
    Record(ErrorRecord),
    /// Literal diagnostic text.
    Text(String),
}
/// Stored oracle errorrecord.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorRecord {
    /// Stored name.
    name: String,
    /// Stored message.
    message: String,
    /// Stored code.
    code: Option<CodeInput>,
    /// Stored errno.
    errno: Option<CodeInput>,
    /// Stored stack.
    stack: Option<String>,
    /// Stored cause.
    cause: Option<Box<ErrorInput>>,
}

/// Stored oracle routecase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RouteCase {
    /// Stored path.
    path: String,
    /// Stored status.
    status: u16,
    /// Stored content type.
    content_type: String,
    /// Stored message.
    message: Option<String>,
    /// Stored details.
    details: Option<String>,
    /// Stored body.
    body: Option<String>,
}

/// Token operation selected by the oracle.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    /// Authorization-code grant.
    Exchange,
    /// Refresh grant.
    Refresh,
}
/// Oracle token outcome.
#[derive(Deserialize)]
#[serde(untagged)]
enum ExpectedToken {
    /// Credential fields.
    Success(Success),
    /// Failure phase.
    Failure(FailurePhase),
    /// HTTP failure status.
    Http(HttpStatus),
}
/// Recorded non-success status.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpStatus {
    /// Recorded response status.
    http_status: u16,
}
/// Authored token failure category.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FailurePhase {
    /// Invalid success contract.
    InvalidToken,
    /// Invalid JSON syntax.
    InvalidJson,
}
/// Stored oracle success.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Success {
    /// Stored credentials.
    credentials: ExpectedCredentials,
}

/// Stored oracle expectedcredentials.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCredentials {
    /// Stored refresh.
    refresh: String,
    /// Stored access.
    access: String,
    /// Stored expires.
    expires: f64,
}

/// Stored oracle tokencase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenCase {
    /// Stored mode.
    mode: Mode,
    /// Stored body.
    body: String,
    /// Stored status.
    status: u16,
    /// Stored clock.
    clock: f64,
    /// Stored expected.
    expected: ExpectedToken,
}

/// Stored oracle requestcase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestCase {
    /// Stored mode.
    mode: Mode,
    /// Stored input.
    input: String,
    /// Stored request.
    request: ExpectedRequest,
    /// Stored result.
    result: ExpectedCredentials,
}

/// Stored oracle expectedrequest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedRequest {
    /// Stored url.
    url: String,
    /// Stored method.
    method: String,
    /// Stored headers.
    headers: BTreeMap<String, String>,
    /// Stored body.
    body: String,
}

/// Stored oracle textcase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextCase {
    /// Stored bytes.
    bytes: Vec<u8>,
    /// Stored expected.
    expected: String,
}

/// Deserialize only consumed fields, rejecting fixture growth without a consumer.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/oauth_subscription.json"
    ))
    .unwrap()
}
/// Execute a native operation with a controllable scheduler.
fn run<T>(work: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(work)
}
thread_local! {
    /// Completion time supplied by token cases.
    static CLOCK: Cell<f64> = const { Cell::new(0.0) };
}
/// Read the test operation's completion clock.
fn clock() -> f64 {
    CLOCK.get()
}
/// Return controlled chunked text from one Fetch call.
fn response(status: u16, bytes: Vec<u8>) -> Fetch {
    Arc::new(move |_| {
        let bytes = bytes.clone();
        Box::pin(async move {
            Ok(HttpResponse {
                status,
                headers: BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter(
                    bytes.into_iter().map(|byte| Ok(vec![byte])),
                )),
            })
        })
    })
}
/// Invoke the authored grant rather than duplicating its request policy.
async fn operation(
    mode: Mode,
    input: String,
    fetch: Fetch,
) -> Result<OAuthCredentials, OAuthError> {
    match mode {
        Mode::Exchange => {
            exchange::exchange(input.clone(), input.clone(), input, fetch, clock).await
        }
        Mode::Refresh => exchange::refresh(input, fetch, clock).await,
    }
}
/// Compare every stored credential field and rejection of unrelated fields.
fn credentials(actual: &OAuthCredentials, expected: ExpectedCredentials) {
    let ExpectedCredentials {
        refresh,
        access,
        expires,
    } = expected;
    assert_eq!(actual.refresh, refresh);
    assert_eq!(actual.access, access);
    assert_eq!(actual.expires.to_bits(), expires.to_bits());
    assert!(actual.extra.is_empty());
}
/// Verify each selected token query once through complete-body consumption.
fn token_cases(indices: &[usize]) {
    run(async {
        for (
            index,
            TokenCase {
                mode,
                body,
                status,
                clock: now,
                expected,
            },
        ) in corpus()
            .tokens
            .into_iter()
            .enumerate()
            .filter(|(index, _)| indices.contains(index))
        {
            CLOCK.set(now);
            let actual = operation(
                mode,
                "code".into(),
                response(status, body.as_bytes().to_vec()),
            )
            .await;
            match expected {
                ExpectedToken::Success(Success {
                    credentials: expected,
                }) => credentials(
                    &actual.unwrap_or_else(|error| panic!("case {index}: {error}")),
                    expected,
                ),
                ExpectedToken::Http(HttpStatus { http_status }) => {
                    let error = actual.unwrap_err();
                    assert_eq!(http_status, status);
                    assert!(error.to_string().contains(&format!("HTTP request failed. status={status}; url=https://platform.claude.com/v1/oauth/token; body={body}")));
                }
                ExpectedToken::Failure(phase) => {
                    let error = actual.unwrap_err();
                    let label = match mode {
                        Mode::Exchange => "Token exchange",
                        Mode::Refresh => "Anthropic token refresh",
                    };
                    let detail = match phase {
                        FailurePhase::InvalidToken => "returned invalid token response",
                        FailurePhase::InvalidJson => "returned invalid JSON",
                    };
                    assert!(
                        error.to_string().starts_with(&format!("{label} {detail}.")),
                        "case {index}: {error}"
                    );
                    assert!(error.to_string().contains(&format!("body={body}")));
                    assert!(
                        error
                            .to_string()
                            .contains("url=https://platform.claude.com/v1/oauth/token")
                    );
                }
            }
        }
    });
}
#[test]
fn authorization_input_preserves_precedence() {
    for InputCase { input, expected } in corpus().precedence {
        assert_eq!(parse_authorization_input(&input), expected, "{input:?}");
    }
}
#[test]
fn authorization_input_trims_authorization_whitespace() {
    for InputCase { input, expected } in corpus().whitespace {
        assert_eq!(parse_authorization_input(&input), expected, "{input:?}");
    }
}
#[test]
fn authorization_input_decodes_first_query_values() {
    for InputCase { input, expected } in corpus().query {
        assert_eq!(parse_authorization_input(&input), expected, "{input:?}");
    }
}
#[test]
fn callback_routes_preserve_status_and_escaped_text() {
    for RouteCase {
        path,
        status,
        content_type,
        message,
        details,
        body,
    } in corpus().routes
    {
        let actual = route(&path, "good");
        assert_eq!(actual.status, status, "{path}");
        assert_eq!(actual.content_type, content_type);
        if let Some(body) = body {
            assert_eq!(actual.body, body);
        } else {
            assert!(actual.body.contains(&message.unwrap()));
            if let Some(details) = details {
                assert!(actual.body.contains(&details));
            }
        }
        assert_eq!(actual.accepted.is_some(), status == 200);
    }
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        let server = super::native::bind("good".into()).await.unwrap();
        for (target, expected) in [("http://[", 400), ("//[", 500)] {
            let mut peer = tokio::net::TcpStream::connect("127.0.0.1:53692")
                .await
                .unwrap();
            let request =
                format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            write_native(&mut peer, request.as_bytes()).await;
            assert!(
                native_response(&mut peer)
                    .await
                    .starts_with(&format!("HTTP/1.1 {expected}"))
            );
        }
        server.close().await;
    });
}
/// Convert diagnostic markers into their exact supplied doubles.
fn code(input: CodeInput) -> DiagnosticCode {
    match input {
        CodeInput::Text(text) => DiagnosticCode::Text(text),
        CodeInput::Number(number) => DiagnosticCode::Number(number),
        CodeInput::Marker(NumberMarker { number }) => {
            DiagnosticCode::Number(match number.as_str() {
                "-0" => -0.0,
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => panic!("invalid marker"),
            })
        }
    }
}
/// Retain all supplied error fields including nested literal causes.
fn diagnostic(input: ErrorInput) -> OAuthError {
    match input {
        ErrorInput::Text(text) => OAuthError::message(text),
        ErrorInput::Record(ErrorRecord {
            name,
            message,
            code: supplied_code,
            errno,
            stack,
            cause,
        }) => OAuthError {
            diagnostic: Box::new(DiagnosticErrorInfo {
                name: Some(name),
                message,
                code: supplied_code.map(code),
                stack,
            }),
            errno: errno.map(code),
            cause: cause.map(|cause| Box::new(diagnostic(*cause))),
        },
    }
}
#[test]
fn oauth_errors_print_only_supplied_details() {
    for DiagnosticCase { input, expected } in corpus().diagnostics {
        assert_eq!(diagnostic(input).details(), expected);
    }
    run(async {
        for mode in [Mode::Exchange, Mode::Refresh] {
            let fetch: Fetch = Arc::new(|_| {
                Box::pin(std::future::ready(Err(FetchError::Connection(
                    DiagnosticErrorInfo {
                        name: Some("Wire".into()),
                        message: "wire failed".into(),
                        code: Some(DiagnosticCode::Text("ECONNRESET".into())),
                        stack: Some("supplied trace".into()),
                    },
                ))))
            });
            let error = operation(mode, "code".into(), fetch).await.unwrap_err();
            assert!(
                error
                    .to_string()
                    .ends_with("details=Wire: wire failed; code=ECONNRESET; stack=supplied trace")
            );
            let cause = error.cause.unwrap();
            assert!(
                matches!(cause.diagnostic.code, Some(DiagnosticCode::Text(ref code)) if code == "ECONNRESET")
            );
            assert_eq!(cause.diagnostic.stack.as_deref(), Some("supplied trace"));
            assert!(cause.errno.is_none());
            assert!(cause.cause.is_none());
        }
    });
}
#[test]
fn token_requests_keep_field_order() {
    run(async {
        for RequestCase {
            mode,
            input,
            request,
            result,
        } in corpus().requests
        {
            CLOCK.set(1_700_000_000_123.0);
            let fetch: Fetch = Arc::new(move |actual| {
                check_request(&actual, &request);
                response(
                    200,
                    br#"{"access_token":"a","refresh_token":"r","expires_in":11}"#.to_vec(),
                )(actual)
            });
            credentials(&operation(mode, input, fetch).await.unwrap(), result);
        }
    });
}
#[test]
fn token_text_decodes_fragmented_utf8() {
    run(async {
        for TextCase { bytes, expected } in corpus().response_text {
            assert_eq!(
                exchange::post_json(Vec::new(), response(200, bytes))
                    .await
                    .unwrap(),
                expected
            );
        }
    });
}

#[test]
fn token_expiry_uses_completion_clock() {
    token_cases(&[0, 1, 2, 3, 25, 26, 27, 35, 36, 37, 38, 60, 61, 62]);
    run(async {
        CLOCK.set(1_000_000.0);
        let fetch: Fetch = Arc::new(|_| {
            Box::pin(async {
                Ok(HttpResponse {
                    status: 200,
                    headers: BTreeMap::new(),
                    body: Box::pin(futures_util::stream::once(delayed_bytes())),
                })
            })
        });
        assert_eq!(
            exchange::refresh("old".into(), fetch, clock)
                .await
                .unwrap()
                .expires
                .to_bits(),
            1_700_500.0_f64.to_bits()
        );
    });
}
#[test]
fn malformed_success_tokens_fail_typed_decode() {
    let indices: Vec<_> = (4..22).chain(39..57).chain([33, 68, 86, 87]).collect();
    token_cases(&indices);
    run(async {
        let fetch = response(200, b"{}".to_vec());
        let error = exchange::refresh("old".into(), fetch, || {
            panic!("invalid token data must fail before reading its expiry clock")
        })
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("returned invalid token response")
        );
    });
}
#[test]
fn token_records_select_last_members_before_decode() {
    token_cases(&[28, 29, 30, 31, 32, 34, 63, 64, 65, 66, 67, 69]);
}
#[test]
fn token_json_failure_keeps_operation_context() {
    token_cases(&[22, 23, 24, 57, 58, 59]);
}
#[test]
fn token_status_is_checked_after_body() {
    token_cases(&(70..86).collect::<Vec<_>>());
    run(async {
        let fetch: Fetch = Arc::new(|_| {
            Box::pin(async {
                Ok(HttpResponse {
                    status: 500,
                    headers: BTreeMap::new(),
                    body: Box::pin(futures_util::stream::iter([Err(FetchError::Connection(
                        DiagnosticErrorInfo {
                            name: Some("Body".into()),
                            message: "body failed".into(),
                            code: None,
                            stack: None,
                        },
                    ))])),
                })
            })
        });
        let error = exchange::refresh("old".into(), fetch, clock)
            .await
            .unwrap_err();
        assert!(error.to_string().ends_with("details=Body: body failed"));
        assert!(!error.to_string().contains("status=500"));
    });
}
#[test]
fn token_fetch_is_not_retried() {
    run(async {
        for failure in [
            FetchError::Aborted,
            FetchError::Timeout,
            FetchError::Connection(DiagnosticErrorInfo {
                name: Some("Wire".into()),
                message: "wire failed".into(),
                code: Some(DiagnosticCode::Text("ECONNRESET".into())),
                stack: Some("supplied trace".into()),
            }),
        ] {
            for mode in [Mode::Exchange, Mode::Refresh] {
                check_attempt(mode, failure.clone()).await;
            }
        }
        for mode in [Mode::Exchange, Mode::Refresh] {
            for status in [200, 429, 500] {
                check_response_attempt(mode, status).await;
            }
        }
    });
}
#[test]
fn token_deadline_covers_headers_and_body() {
    run(async {
        tokio::time::pause();
        for headers_delay in [0, 10_000] {
            let signal = crate::EventStream::new(|_: &crate::Cancellation| true, Clone::clone);
            let delivered = signal.clone();
            let fetch: Fetch = Arc::new(move |request| {
                delivered.push(request.signal.unwrap());
                Box::pin(pending_body(headers_delay))
            });
            let mut work = Box::pin(exchange::refresh("old".into(), fetch, clock));
            assert!(work.as_mut().now_or_never().is_none());
            let abort = signal.result().await;
            tokio::time::advance(std::time::Duration::from_millis(29_999)).await;
            assert!(work.as_mut().now_or_never().is_none());
            tokio::time::advance(std::time::Duration::from_millis(1)).await;
            assert!(
                work.await
                    .unwrap_err()
                    .to_string()
                    .contains("Request timed out.")
            );
            assert!(abort.is_aborted());
        }
        for finish_at in [29_999, 30_000] {
            let released = crate::EventStream::new(|()| true, |()| ());
            let body_gate = released.clone();
            let fetch: Fetch = Arc::new(move |_| {
                let body_gate = body_gate.clone();
                Box::pin(gated_body_response(body_gate))
            });
            let mut work = Box::pin(exchange::refresh("old".into(), fetch, clock));
            assert!(work.as_mut().now_or_never().is_none());
            tokio::time::advance(std::time::Duration::from_millis(finish_at)).await;
            released.push(());
            assert_eq!(work.await.unwrap().access, "ready");
        }
        let fetch: Fetch = Arc::new(|_| Box::pin(std::future::pending()));
        let mut work = Box::pin(exchange::refresh("old".into(), fetch, clock));
        assert!(work.as_mut().now_or_never().is_none());
        tokio::time::advance(std::time::Duration::from_secs(30)).await;
        assert!(
            work.await
                .unwrap_err()
                .to_string()
                .contains("Request timed out.")
        );
    });
}

/// Controlled interaction choices and a producer witness.
struct Interaction {
    /// Manual producer, supplied at most once.
    manual: std::sync::Mutex<Option<crate::BoxFuture<Result<String, OAuthError>>>>,
    /// Prompt answer or error.
    prompt: String,
    /// Phase that deliberately fails.
    failure: &'static str,
    /// Ordered completed callback phases.
    events: crate::EventStream<&'static str, ()>,
    /// Ignored caller cancellation.
    signal: crate::Cancellation,
}
impl crate::OAuthLoginCallbacks for Interaction {
    fn on_auth(&self, _: crate::OAuthAuthInfo) -> Result<(), OAuthError> {
        self.phase("auth")
    }
    fn on_prompt(
        &self,
        prompt: crate::OAuthPrompt,
    ) -> crate::BoxFuture<Result<String, OAuthError>> {
        assert_eq!(
            prompt.message,
            "Paste the authorization code or full redirect URL:"
        );
        assert_eq!(prompt.placeholder.as_deref(), Some(super::REDIRECT_URI));
        assert_eq!(prompt.allow_empty, None);
        let answer = self.prompt.clone();
        let result = self.phase("prompt");
        Box::pin(async move { result.map(|()| answer) })
    }
    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        assert_eq!(message, "Exchanging authorization code for tokens...");
        self.phase("progress")
    }
    fn on_manual_code_input(&self) -> Option<crate::BoxFuture<Result<String, OAuthError>>> {
        self.events.push("manual");
        self.manual.lock().unwrap().take()
    }
    fn on_select(
        &self,
        _: crate::OAuthSelectPrompt,
    ) -> Option<crate::BoxFuture<Result<Option<String>, OAuthError>>> {
        panic!("selector is not used")
    }
    fn signal(&self) -> Option<&crate::Cancellation> {
        panic!("signal is not used")
    }
}
impl Interaction {
    /// Construct controlled manual input with prompt fallback.
    fn new(
        input: Option<Result<String, OAuthError>>,
        prompt: &str,
        failure: &'static str,
    ) -> Arc<Self> {
        Arc::new(Self {
            manual: std::sync::Mutex::new(
                input.map(|input| Box::pin(async move { input }) as crate::BoxFuture<_>),
            ),
            prompt: prompt.into(),
            failure,
            events: crate::EventStream::new(|_| false, |_| ()),
            signal: crate::Cancellation::new(),
        })
    }
    /// Publish a completed callback phase and optionally fail it.
    fn phase(&self, phase: &'static str) -> Result<(), OAuthError> {
        self.events.push(phase);
        if self.failure == phase {
            Err(OAuthError::message(format!("{phase} failed")))
        } else {
            Ok(())
        }
    }
}
/// Fixed proof input, independent of code and prompt strings.
fn pkce() -> crate::Pkce {
    crate::Pkce {
        verifier: "verifier".into(),
        challenge: "challenge".into(),
    }
}
/// Owned listener seam whose stopped witness follows shutdown observation.
fn listener() -> super::native::CallbackServer {
    let server = super::native::CallbackServer::new();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    tokio::spawn(async move {
        stop.cancelled().await;
        stopped.push(());
    });
    server
}
/// Accepted callback input used at the real routing boundary.
fn accept(wait: &crate::EventStream<super::native::CallbackOutcome, ()>, code: &str) {
    let response = route(&format!("/callback?code={code}&state=verifier"), "verifier");
    assert_eq!(response.status, 200);
    wait.push(super::native::CallbackOutcome::Accepted(
        response.accepted.unwrap(),
    ));
}
/// Successful token response for controlled interaction flows.
fn success_fetch() -> Fetch {
    response(
        200,
        br#"{"access_token":"access","refresh_token":"refresh","expires_in":3600}"#.to_vec(),
    )
}

#[test]
fn callback_wait_settles_once() {
    run(async {
        let server = listener();
        let mut wait = Box::pin(server.wait.next());
        assert!(wait.as_mut().now_or_never().is_none());
        let rejection = route("/callback?code=c&state=wrong", "verifier");
        assert!(rejection.accepted.is_none());
        assert!(wait.as_mut().now_or_never().is_none());
        accept(&server.wait, "first");
        accept(&server.wait, "second");
        server.wait.push(super::native::CallbackOutcome::Cancelled);
        let super::native::CallbackOutcome::Accepted(fields) = wait.await.unwrap() else {
            panic!("first callback must win")
        };
        assert_eq!(fields.code.as_deref(), Some("first"));
        let cancelled = listener();
        cancelled
            .wait
            .push(super::native::CallbackOutcome::Cancelled);
        accept(&cancelled.wait, "late");
        assert!(matches!(
            cancelled.wait.next().await,
            Some(super::native::CallbackOutcome::Cancelled)
        ));
    });
}
#[test]
fn prompt_fallback_validates_code_and_state() {
    run(async {
        for (manual, prompt, expected) in [
            ("", "prompt", Ok("prompt")),
            (" ", "prompt#verifier", Ok("prompt")),
            ("#verifier", "code=p", Ok("p")),
            ("", "", Err("Missing authorization code")),
            ("", " ", Err("Missing authorization code")),
            ("", "#", Err("Missing authorization code")),
            ("", "code=p&state=", Err("Missing OAuth state")),
            ("", "p#wrong", Err("OAuth state mismatch")),
            ("#wrong", "p", Err("OAuth state mismatch")),
            ("", "https://x/?state=wrong", Err("OAuth state mismatch")),
            ("code=m&state=", "unused", Err("Missing OAuth state")),
            ("manual", "unused", Ok("manual")),
        ] {
            let callbacks = Interaction::new(Some(Ok(manual.into())), prompt, "");
            let server = listener();
            let handle: crate::OAuthCallbacks = callbacks;
            let result = super::select_code(&handle, &pkce(), &server).await;
            match expected {
                Ok(code) => assert_eq!(result.unwrap(), (code.into(), "verifier".into())),
                Err(error) => assert_eq!(result.unwrap_err().to_string(), error),
            }
        }
    });
}
#[test]
fn login_callbacks_run_in_source_order_and_fail_cleanly() {
    run(async {
        for phase in ["auth", "manual", "prompt", "progress", ""] {
            let manual = if phase == "manual" {
                Err(OAuthError::message("manual failed"))
            } else {
                Ok(String::new())
            };
            let callbacks = Interaction::new(Some(manual), "prompt", phase);
            let events = callbacks.events.clone();
            let server = listener();
            let stopped = server.stopped.clone();
            let fetch: Fetch = Arc::new(move |request| {
                events.push("exchange");
                success_fetch()(request)
            });
            let result = super::login(callbacks.clone(), fetch, pkce(), server).await;
            assert!(stopped.result().now_or_never().is_some());
            let mut observed = Vec::new();
            while let Some(Some(event)) = callbacks.events.next().now_or_never() {
                observed.push(event);
            }
            let expected = match phase {
                "auth" => vec!["auth"],
                "manual" => vec!["auth", "manual"],
                "prompt" => vec!["auth", "manual", "prompt"],
                "progress" => vec!["auth", "manual", "prompt", "progress"],
                _ => vec!["auth", "manual", "prompt", "progress", "exchange"],
            };
            assert_eq!(observed, expected);
            if phase.is_empty() {
                assert_eq!(result.unwrap().access, "access");
            } else {
                assert_eq!(result.unwrap_err().to_string(), format!("{phase} failed"));
            }
        }
    });
}
#[test]
fn listener_failure_reaches_pending_login() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        let callbacks = Interaction::new(None, "unused", "");
        let release = crate::EventStream::new(|()| true, |()| ());
        let failure = release.clone();
        let server = super::native::bind_with(
            "verifier".into(),
            |_| None,
            move |_| {
                let failure = failure.clone();
                Box::pin(async move {
                    failure.result().await;
                    Err(std::io::Error::from_raw_os_error(5))
                })
            },
        )
        .await
        .unwrap();
        let stopped = server.stopped.clone();
        let mut login = Box::pin(super::login(callbacks, success_fetch(), pkce(), server));
        assert!(login.as_mut().now_or_never().is_none());
        release.push(());
        let error = login.await.unwrap_err();
        assert_eq!(
            error.to_string(),
            std::io::Error::from_raw_os_error(5).to_string()
        );
        assert!(matches!(error.errno, Some(DiagnosticCode::Number(5.0))));
        assert!(stopped.result().now_or_never().is_some());
        let _released = native_probe("127.0.0.1");
    });
}
#[test]
fn login_wait_has_no_deadline() {
    run(async {
        tokio::time::pause();
        let callbacks = Interaction::new(None, "unused", "");
        let server = listener();
        let wait = server.wait.clone();
        let mut login = Box::pin(super::login(callbacks, success_fetch(), pkce(), server));
        assert!(login.as_mut().now_or_never().is_none());
        tokio::time::advance(std::time::Duration::from_secs(30)).await;
        assert!(login.as_mut().now_or_never().is_none());
        tokio::time::advance(std::time::Duration::from_hours(24)).await;
        assert!(login.as_mut().now_or_never().is_none());
        accept(&wait, "later");
        assert_eq!(login.await.unwrap().access, "access");
    });
}
#[test]
fn subscription_ignores_signal_and_selector() {
    run(async {
        for pre_aborted in [false, true] {
            let callbacks = Interaction::new(None, "unused", "");
            if pre_aborted {
                callbacks.signal.abort();
            }
            let server = listener();
            let wait = server.wait.clone();
            let mut login = Box::pin(super::login(
                callbacks.clone(),
                success_fetch(),
                pkce(),
                server,
            ));
            assert!(login.as_mut().now_or_never().is_none());
            callbacks.signal.abort();
            accept(&wait, "accepted");
            assert_eq!(login.await.unwrap().access, "access");
        }
    });
}
#[test]
fn callback_host_changes_bind_only() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        for (input, expected) in [
            (None, "127.0.0.1"),
            (Some(""), "127.0.0.1"),
            (Some("127.0.0.2"), "127.0.0.2"),
            (Some(" "), " "),
        ] {
            assert_eq!(
                super::native::callback_host(input.map(str::to_owned)),
                expected
            );
            let info = super::auth_info(&pkce());
            let url = url::Url::parse(&info.url).unwrap();
            assert_eq!(
                url.query_pairs()
                    .find(|(key, _)| key == "redirect_uri")
                    .unwrap()
                    .1,
                "http://localhost:53692/callback"
            );
            let result = super::native::bind_with(
                "verifier".into(),
                |key| {
                    assert_eq!(key, "MAESTRO_OAUTH_CALLBACK_HOST");
                    input.map(str::to_owned)
                },
                |listener| Box::pin(listener.accept()),
            )
            .await;
            if expected == " " {
                assert!(result.is_err());
                continue;
            }
            let server = result.unwrap();
            let address = format!("{expected}:53692");
            let mut peer = tokio::net::TcpStream::connect(address).await.unwrap();
            write_native(
                &mut peer,
                b"GET /callback?code=bound&state=verifier HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await;
            let super::native::CallbackOutcome::Accepted(fields) =
                server.wait.next().await.unwrap()
            else {
                panic!("bound callback");
            };
            assert_eq!(fields.code.as_deref(), Some("bound"));
            server.close().await;
            drop(peer);
            let _released = native_probe(expected);
        }
    });
}
#[test]
fn pkce_failure_prevents_binding() {
    run(async {
        let callbacks = Interaction::new(None, "unused", "");
        let result = super::login_with(
            callbacks.clone(),
            success_fetch(),
            || {
                Err(DiagnosticErrorInfo {
                    message: "entropy failed".into(),
                    name: Some("Entropy".into()),
                    code: Some(DiagnosticCode::Number(7.0)),
                    stack: None,
                })
            },
            |_| async { panic!("entropy failure must precede binding") },
        )
        .await;
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "entropy failed");
        assert_eq!(error.diagnostic.name.as_deref(), Some("Entropy"));
        assert!(callbacks.events.next().now_or_never().is_none());
    });
}

#[test]
fn manual_code_and_callback_have_controlled_winners() {
    run(async {
        for winner in ["manual", "callback", "cancelled"] {
            let producer = crate::EventStream::new(|_: &String| false, |_| ());
            let input = producer.clone();
            let completed = crate::EventStream::new(|()| true, |()| ());
            let witness = completed.clone();
            let callbacks = Interaction::new(None, "unused", "");
            *callbacks.manual.lock().unwrap() = Some(Box::pin(
                async move { Ok(input.next().await.unwrap()) }.map(move |result| {
                    witness.push(());
                    result
                }),
            ));
            let server = listener();
            let wait = server.wait.clone();
            let chosen = crate::EventStream::new(|_: &String| true, Clone::clone);
            let observed = chosen.clone();
            let release = crate::EventStream::new(|()| true, |()| ());
            let headers = release.clone();
            let fetch: Fetch = Arc::new(move |request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                observed.push(body["code"].as_str().unwrap().into());
                Box::pin(released_headers(headers.clone(), request, false))
            });
            let mut login = Box::pin(super::login(callbacks, fetch, pkce(), server));
            assert!(login.as_mut().now_or_never().is_none());
            match winner {
                "callback" => accept(&wait, "callback-code"),
                "cancelled" => {
                    wait.push(super::native::CallbackOutcome::Cancelled);
                    assert!(login.as_mut().now_or_never().is_none());
                    producer.push("manual-code".into());
                }
                _ => producer.push("manual-code".into()),
            }
            let login = tokio::spawn(login);
            assert_eq!(
                chosen.result().await,
                if winner == "callback" {
                    "callback-code"
                } else {
                    "manual-code"
                }
            );
            if winner == "callback" {
                assert!(completed.result().now_or_never().is_none());
                producer.push("late".into());
            }
            completed.result().await;
            assert!(!login.is_finished());
            release.push(());
            assert_eq!(login.await.unwrap().unwrap().access, "access");
        }
    });
}
#[test]
fn manual_failure_propagates_and_late_input_cannot_replace_code() {
    run(async {
        for late_failure in [false, true] {
            let producer = crate::EventStream::new(|_: &bool| false, |_| ());
            let input = producer.clone();
            let completed = crate::EventStream::new(|()| true, |()| ());
            let witness = completed.clone();
            let callbacks = Interaction::new(None, "unused", "");
            *callbacks.manual.lock().unwrap() = Some(Box::pin(
                async move { manual_answer(input.next().await.unwrap()) }.map(move |result| {
                    witness.push(());
                    result
                }),
            ));
            let server = listener();
            let wait = server.wait.clone();
            let entered = crate::EventStream::new(|_: &crate::HttpRequest| true, Clone::clone);
            let request = entered.clone();
            let release = crate::EventStream::new(|()| true, |()| ());
            let headers = release.clone();
            let fetch: Fetch = Arc::new(move |actual| {
                request.push(actual.clone());
                Box::pin(released_headers(headers.clone(), actual, false))
            });
            let mut login = Box::pin(super::login(callbacks, fetch, pkce(), server));
            assert!(login.as_mut().now_or_never().is_none());
            accept(&wait, "selected");
            let login = tokio::spawn(login);
            let request = entered.result().await;
            producer.push(late_failure);
            completed.result().await;
            assert!(!login.is_finished());
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["code"], "selected");
            assert_eq!(body["state"], "verifier");
            release.push(());
            assert_eq!(login.await.unwrap().unwrap().access, "access");
        }
        let callbacks = Interaction::new(
            Some(Err(OAuthError::message("manual failed"))),
            "unused",
            "",
        );
        let server = listener();
        assert_eq!(
            super::login(callbacks, success_fetch(), pkce(), server)
                .await
                .unwrap_err()
                .to_string(),
            "manual failed"
        );
    });
}
#[test]
fn listener_stops_after_exchange_starts() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        for (token_failure, body_pending) in [(false, false), (true, false), (false, true)] {
            let callbacks = Interaction::new(Some(Ok("manual".into())), "unused", "");
            let server = super::native::bind("verifier".into()).await.unwrap();
            let stopped = server.stopped.clone();
            let release = crate::EventStream::new(|()| true, |()| ());
            let entered = crate::EventStream::new(|()| true, |()| ());
            let headers = release.clone();
            let exchange_entered = entered.clone();
            let body_entered = crate::EventStream::new(|()| true, |()| ());
            let body_witness = body_entered.clone();
            let stop_witness = stopped.clone();
            let fetch: Fetch = Arc::new(move |request| {
                assert!(stop_witness.result().now_or_never().is_none());
                exchange_entered.push(());
                let headers = headers.clone();
                Box::pin(held_token(
                    headers,
                    request,
                    token_failure,
                    body_pending.then(|| body_witness.clone()),
                ))
            });
            let login = tokio::spawn(super::login(callbacks, fetch, pkce(), server));
            entered.result().await;
            stopped.result().await;
            if body_pending {
                body_entered.result().await;
            }
            assert!(!login.is_finished());
            let released = native_probe("127.0.0.1");
            drop(released);
            release.push(());
            assert_eq!(login.await.unwrap().is_err(), token_failure);
        }
        let server = super::native::bind("verifier".into()).await.unwrap();
        let stopped = server.stopped.clone();
        let callbacks = Interaction::new(None, "unused", "");
        let mut login = Box::pin(super::login(callbacks, success_fetch(), pkce(), server));
        assert!(login.as_mut().now_or_never().is_none());
        drop(login);
        stopped.result().await;
        let released = native_probe("127.0.0.1");
        drop(released);
        let server = super::native::bind("verifier".into()).await.unwrap();
        let callbacks = Interaction::new(None, "unused", "auth");
        assert_eq!(
            super::login(callbacks, success_fetch(), pkce(), server)
                .await
                .unwrap_err()
                .to_string(),
            "auth failed"
        );
        let _released = native_probe("127.0.0.1");
    });
}
#[test]
fn native_unavailable_does_not_block_refresh() {
    run(async {
        let callbacks = Interaction::new(None, "unused", "");
        let result = super::login_with(
            callbacks.clone(),
            success_fetch(),
            || Ok(pkce()),
            super::native::unavailable,
        )
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "Anthropic OAuth requires a native callback listener"
        );
        assert!(callbacks.events.next().now_or_never().is_none());
        assert_eq!(
            super::refresh_anthropic_token("old".into(), Some(success_fetch()))
                .await
                .unwrap()
                .access,
            "access"
        );
        assert!(!crate::generate_pkce().unwrap().verifier.is_empty());
        assert!(
            crate::oauth_success_html("usable")
                .unwrap()
                .contains("usable")
        );
    });
}

/// Read every recorded request field through the production transport seam.
fn check_request(actual: &crate::HttpRequest, expected: &ExpectedRequest) {
    assert_eq!(actual.url, expected.url);
    assert_eq!(actual.method, expected.method);
    assert_eq!(std::str::from_utf8(&actual.body).unwrap(), expected.body);
    for (name, value) in &expected.headers {
        assert_eq!(&actual.headers[&name.to_lowercase()], value);
    }
}
/// Release token bytes after changing the completion clock.
fn delayed_bytes() -> impl Future<Output = Result<Vec<u8>, FetchError>> {
    std::future::poll_fn(|_| {
        CLOCK.set(2_000_000.0);
        std::task::Poll::Ready(Ok(
            br#"{"access_token":"a","refresh_token":"r","expires_in":0.5}"#.to_vec(),
        ))
    })
}

/// Observe the closed single-attempt failure decision.
async fn check_attempt(mode: Mode, failure: FetchError) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let fetch: Fetch = Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        let failure = failure.clone();
        Box::pin(async move { Err(failure) })
    });
    let error = operation(mode, "code".into(), fetch).await.unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(error.cause.is_some());
    if matches!(mode, Mode::Exchange) {
        assert!(error.to_string().contains(
            "redirect_uri=http://localhost:53692/callback; response_type=authorization_code;"
        ));
    }
}

/// Check one completed body/status failure without relying on an observation window.
async fn check_response_attempt(mode: Mode, status: u16) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let fetch: Fetch = Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let chunk = if status == 200 {
                Err(FetchError::Aborted)
            } else {
                Ok(b"denied".to_vec())
            };
            Ok(HttpResponse {
                status,
                headers: BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([chunk])),
            })
        })
    });
    let error = operation(mode, "code".into(), fetch).await.unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    if status == 200 {
        assert!(error.to_string().contains("Request was aborted."));
    } else {
        assert!(error.to_string().contains(&format!("status={status}")));
    }
}

/// Admit headers immediately and release successful bytes through the named body gate.
fn gated_body_response(
    gate: crate::EventStream<(), ()>,
) -> std::future::Ready<Result<HttpResponse, FetchError>> {
    std::future::ready(Ok(HttpResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: Box::pin(futures_util::stream::once(async move {
            gate.result().await;
            Ok(br#"{"access_token":"ready","refresh_token":"r","expires_in":1}"#.to_vec())
        })),
    }))
}

/// Admit headers after a controlled delay, keeping body unresolved.
async fn pending_body(delay: u64) -> Result<HttpResponse, FetchError> {
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    Ok(HttpResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: Box::pin(futures_util::stream::pending()),
    })
}

/// Produce late manual success or failure without replacing the selected callback.
fn manual_answer(failure: bool) -> Result<String, OAuthError> {
    if failure {
        Err(OAuthError::message("late failed"))
    } else {
        Ok("late-code".into())
    }
}
/// Release headers of the same controlled token operation.
async fn released_headers(
    headers: crate::EventStream<(), ()>,
    request: crate::HttpRequest,
    failure: bool,
) -> Result<HttpResponse, FetchError> {
    headers.result().await;
    if failure {
        Err(FetchError::Aborted)
    } else {
        success_fetch()(request).await
    }
}

/// Serialize cases that bind the native callback port.
static NATIVE_PORT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

/// Observe real accepting socket release with the adapter's address reuse policy.
fn native_probe(host: &str) -> tokio::net::TcpListener {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    #[cfg(unix)]
    socket.set_reuseaddr(true).unwrap();
    socket
        .bind(format!("{host}:53692").parse().unwrap())
        .unwrap();
    socket.listen(128).unwrap()
}

/// Send a complete native request using readiness rather than sleeps.
async fn write_native(peer: &mut tokio::net::TcpStream, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        peer.writable().await.unwrap();
        match peer.try_write(bytes) {
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
}

/// Read native response bytes or the peer's orderly EOF.
async fn read_native(peer: &mut tokio::net::TcpStream, bytes: &mut [u8]) -> usize {
    loop {
        peer.readable().await.unwrap();
        match peer.try_read(bytes) {
            Ok(count) => return count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
}

/// Read the response set closed by the native peer's EOF.
async fn native_response(peer: &mut tokio::net::TcpStream) -> String {
    let mut response = Vec::new();
    loop {
        let mut bytes = [0; 1024];
        let count = read_native(peer, &mut bytes).await;
        if count == 0 {
            break;
        }
        response.extend_from_slice(&bytes[..count]);
    }
    String::from_utf8(response).unwrap()
}

/// Hold either headers or the body of this one token operation behind its release gate.
async fn held_token(
    release: crate::EventStream<(), ()>,
    request: crate::HttpRequest,
    failure: bool,
    body_entered: Option<crate::EventStream<(), ()>>,
) -> Result<HttpResponse, FetchError> {
    let Some(body_entered) = body_entered else {
        return released_headers(release, request, failure).await;
    };
    Ok(HttpResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: Box::pin(futures_util::stream::once(async move {
            body_entered.push(());
            release.result().await;
            Ok(
                br#"{"access_token":"access","refresh_token":"refresh","expires_in":3600}"#
                    .to_vec(),
            )
        })),
    })
}
