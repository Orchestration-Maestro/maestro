//! Controlled response-account policy and recorded protocol cases.
use super::{callback, token};
use crate::{
    BoxFuture, EventStream, Fetch, FetchError, HttpResponse, OAuthCredentials, OAuthError,
};
use futures_util::FutureExt as _;
use serde::Deserialize;
use std::{collections::BTreeMap, future::Future, sync::Arc};

/// Consumed protocol observations grouped by their owning behavior.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Required-field cases.
    fields: Vec<TokenCase>,
    /// Last-member selection cases.
    surviving: Vec<TokenCase>,
    /// Error-detail formatting cases.
    compact: Vec<TokenCase>,
    /// Competing failure phases.
    phases: Vec<TokenCase>,
    /// Status and reason phrase cases.
    status: Vec<TokenCase>,
    /// Fragmented byte bodies.
    bytes: Vec<TokenCase>,
    /// Ordered authorization fields.
    flow: Vec<FlowCase>,
    /// Callback route decisions.
    routes: Vec<RouteCase>,
    /// Payload decoding cases.
    decode: Vec<AccountCase>,
    /// Account shape cases.
    account: Vec<AccountCase>,
    /// Unread account members.
    unread_account: Vec<AccountCase>,
}
/// A selected token operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Exchange,
    Refresh,
}
/// Transport effect applied after supplied chunks.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Effect {
    Normal,
    BodyError,
    FetchError,
}
/// Consumed response and expected result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenCase {
    /// Operation under test.
    operation: Operation,
    /// Response status.
    status: u16,
    /// Supplied reason phrase.
    reason: String,
    /// Bytes before completion or error.
    body: Body,
    /// Selected effect.
    effect: Effect,
    /// Observable outcome.
    expected: Expected,
}
/// A complete UTF-8 recording or deliberately fragmented byte recording.
#[derive(PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Body {
    /// Text carried as one complete chunk.
    Text(String),
    /// Supplied fragment boundaries, including invalid UTF-8.
    Chunks(Vec<Vec<u8>>),
}
impl Body {
    /// Consume the recorded text or boundaries into the actual transport body.
    fn into_chunks(self) -> Vec<Vec<u8>> {
        match self {
            Self::Text(text) => vec![text.into_bytes()],
            Self::Chunks(chunks) => chunks,
        }
    }
}
/// Credential or error outcome without copied runtime error text.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Credentials(Credentials),
    Error(String),
    NativeError(bool),
}
/// All credential fields retained by the operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    /// Access token.
    access: String,
    /// Rotated token.
    refresh: String,
    /// Completion-based expiry.
    expires: f64,
    /// Routing metadata.
    #[serde(rename = "accountId")]
    account_id: String,
}
/// Ordered authorization result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlowCase {
    /// Caller originator.
    originator: Option<String>,
    /// Controlled proof verifier.
    verifier: String,
    /// Independent hexadecimal state.
    state: String,
    /// Full expected URL.
    url: String,
}
/// Callback input and decision, without a duplicated page template.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteCase {
    /// Request target.
    target: String,
    /// Required state.
    state: String,
    /// Response status.
    status: u16,
    /// Media type.
    content_type: String,
    /// Page message.
    message: String,
    /// Accepted code.
    code: Option<String>,
}
/// Payload and selected account identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountCase {
    /// JWT input.
    token: String,
    /// Extracted account, if any.
    expected: Option<String>,
}
/// Read only the committed, consumed fixture schema.
fn corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/response_accounts.json"
    ))
    .unwrap();
    let mut requests = std::collections::HashSet::new();
    for case in corpus
        .fields
        .iter()
        .chain(&corpus.surviving)
        .chain(&corpus.compact)
        .chain(&corpus.phases)
        .chain(&corpus.status)
        .chain(&corpus.bytes)
    {
        assert!(
            requests.insert((
                case.operation,
                case.status,
                case.reason.as_str(),
                &case.body,
                case.effect
            )),
            "duplicate controlled token input"
        );
    }
    let mut tokens = std::collections::HashSet::new();
    for case in corpus
        .decode
        .iter()
        .chain(&corpus.account)
        .chain(&corpus.unread_account)
    {
        assert!(
            tokens.insert(case.token.as_str()),
            "duplicate account payload"
        );
    }
    corpus
}

/// Run one operation with controlled native scheduling.
fn run<T>(work: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(work)
}
/// Fixed oracle completion time.
fn clock() -> f64 {
    1_700_000_000_123.0
}
/// Supplied transport cause, not an authored token failure.
fn failure(message: &str) -> FetchError {
    FetchError::Connection(crate::DiagnosticErrorInfo {
        message: message.into(),
        name: None,
        code: None,
        stack: None,
    })
}
/// Invoke the operation through its actual form, transport and catch boundary.
async fn operation(
    operation: Operation,
    fetch: Fetch,
    clock: fn() -> f64,
) -> Result<OAuthCredentials, OAuthError> {
    match operation {
        Operation::Exchange => token::exchange("c +&😀".into(), "v/?".into(), fetch, clock).await,
        Operation::Refresh => token::refresh("r +&😀".into(), fetch, clock).await,
    }
}
/// Supply chunks with a closed successful or failing completion.
fn response(status: u16, reason: String, chunks: Vec<Vec<u8>>, effect: Effect) -> Fetch {
    Arc::new(move |_| {
        let reason = reason.clone();
        let chunks = chunks.clone();
        Box::pin(async move {
            if matches!(effect, Effect::FetchError) {
                return Err(failure("offline"));
            }
            let mut chunks: Vec<_> = chunks.into_iter().map(Ok).collect();
            if matches!(effect, Effect::BodyError) {
                chunks.push(Err(failure("body failed")));
            }
            Ok(HttpResponse {
                status,
                status_text: reason,
                headers: BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter(chunks)),
            })
        })
    })
}
/// Consume every token fixture input and every expected field.
async fn check_tokens(cases: Vec<TokenCase>) {
    for TokenCase {
        operation: selected,
        status,
        reason,
        body,
        effect,
        expected,
    } in cases
    {
        let result = operation(
            selected,
            response(status, reason, body.into_chunks(), effect),
            clock,
        )
        .await;
        match expected {
            Expected::Credentials(Credentials {
                access,
                refresh,
                expires,
                account_id,
            }) => {
                let actual = result.unwrap();
                assert_eq!(
                    (actual.access, actual.refresh, actual.expires),
                    (access, refresh, expires)
                );
                assert_eq!(
                    actual.extra,
                    [("accountId".into(), account_id.into())]
                        .into_iter()
                        .collect()
                );
            }
            Expected::Error(expected) => assert_eq!(result.unwrap_err().to_string(), expected),
            Expected::NativeError(wrapped) => {
                let error = result.unwrap_err().to_string();
                assert_eq!(
                    error.starts_with("OpenAI Codex token refresh error: "),
                    wrapped,
                    "{error}"
                );
                assert!(!error.contains("response missing fields:"), "{error}");
                assert!(!error.contains("extract accountId"), "{error}");
                assert!(!error.is_empty());
            }
        }
    }
}

#[test]
fn response_token_fields_require_nonempty_strings_and_finite_expiry() {
    run(check_tokens(corpus().fields));
}

/// Consume exact JWT identities, including native decoding failures.
fn check_accounts(cases: Vec<AccountCase>) {
    for AccountCase {
        token: input,
        expected,
    } in cases
    {
        assert_eq!(token::account_id(&input), expected, "{input}");
    }
}

#[test]
fn response_token_reads_select_surviving_fields() {
    run(check_tokens(corpus().surviving));
}

#[test]
fn response_token_messages_keep_compact_json_spelling() {
    run(check_tokens(corpus().compact));
}

#[test]
fn response_refresh_wraps_fetch_and_json_failures_once() {
    run(check_tokens(corpus().phases));
}

#[test]
fn response_error_body_beats_reason_phrase() {
    run(check_tokens(corpus().status));
}

#[test]
fn response_token_text_handles_fragmented_utf8() {
    run(check_tokens(corpus().bytes));
}

#[test]
fn response_account_payload_decodes_url_safe_utf8() {
    check_accounts(corpus().decode);
}

#[test]
fn response_account_claim_requires_nonempty_string() {
    check_accounts(corpus().account);
}

#[test]
fn response_account_parser_discards_unread_members() {
    check_accounts(corpus().unread_account);
}

#[test]
fn response_callback_route_decisions_preserve_first_fields() {
    for RouteCase {
        target,
        state,
        status,
        content_type,
        message,
        code,
    } in corpus().routes
    {
        let actual = callback::route(&target, &state);
        assert_eq!(
            (actual.status, actual.content_type),
            (status, content_type.as_str())
        );
        assert_eq!(actual.accepted.and_then(|fields| fields.code), code);
        let page = if status == 200 {
            crate::oauth_success_html(&message)
        } else {
            crate::oauth_error_html(&message, None)
        };
        assert_eq!(actual.body, page.unwrap());
    }
}

#[test]
fn response_entropy_is_separate_and_precedes_binding() {
    for FlowCase {
        originator,
        verifier,
        state,
        url,
    } in corpus().flow
    {
        let order = std::cell::Cell::new(0);
        let (proof, actual_state, info) = super::authorization_flow_with(
            originator,
            || {
                assert_eq!(order.replace(1), 0);
                Ok(crate::Pkce {
                    verifier: verifier.clone(),
                    challenge: "6oZqdX5MOLq_qBJ8vppAnT4fk6AP8UiP9zX8-Rev_9A".into(),
                })
            },
            |bytes| {
                assert_eq!(order.replace(2), 1);
                for (value, byte) in (0xf0..=0xff).zip(bytes) {
                    *byte = value;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            (proof.verifier, actual_state, info.url),
            (verifier, state, url)
        );
        assert_eq!(order.get(), 2);
    }
    for phase in ["proof", "state"] {
        let result = super::authorization_flow_with(
            None,
            || {
                if phase == "proof" {
                    Err(OAuthError::message("proof failed"))
                } else {
                    Ok(proof())
                }
            },
            |_| {
                assert_eq!(phase, "state");
                Err(OAuthError::message("state failed"))
            },
        );
        assert_eq!(result.unwrap_err().to_string(), format!("{phase} failed"));
    }
}
/// Controlled proof without duplicating its generation algorithm.
fn proof() -> crate::Pkce {
    crate::Pkce {
        verifier: "verifier".into(),
        challenge: "challenge".into(),
    }
}

/// Supplied interactions at the coordinator boundary.
struct Interaction {
    /// Independently owned manual work.
    manual: std::sync::Mutex<Option<BoxFuture<Result<String, OAuthError>>>>,
    /// Prompt answer.
    prompt: String,
    /// Interaction failure phase, if any.
    fail: &'static str,
    /// Auth notification witness.
    auth: EventStream<(), ()>,
    /// Supplied signal whose cancellation is not this flow's policy.
    signal: crate::Cancellation,
}
impl Interaction {
    /// Supply an optional immediate manual result and a prompt answer.
    fn new(
        manual: Option<Result<String, OAuthError>>,
        prompt: &str,
        fail: &'static str,
    ) -> Arc<Self> {
        Arc::new(Self {
            manual: std::sync::Mutex::new(
                manual.map(|result| Box::pin(std::future::ready(result)) as BoxFuture<_>),
            ),
            prompt: prompt.into(),
            fail,
            auth: EventStream::new(|()| true, |()| ()),
            signal: crate::Cancellation::new(),
        })
    }
}
impl crate::OAuthLoginCallbacks for Interaction {
    fn on_auth(&self, _: crate::OAuthAuthInfo) -> Result<(), OAuthError> {
        self.auth.push(());
        if self.fail == "auth" {
            Err(OAuthError::message("auth failed"))
        } else {
            Ok(())
        }
    }
    fn on_prompt(&self, prompt: crate::OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        assert_eq!(
            prompt,
            crate::OAuthPrompt {
                message: "Paste the authorization code (or full redirect URL):".into(),
                placeholder: None,
                allow_empty: None
            }
        );
        let answer = self.prompt.clone();
        let fail = self.fail;
        Box::pin(async move {
            if fail == "prompt" {
                Err(OAuthError::message("prompt failed"))
            } else {
                Ok(answer)
            }
        })
    }
    fn on_manual_code_input(&self) -> Option<BoxFuture<Result<String, OAuthError>>> {
        assert_ne!(self.fail, "auth");
        assert!(self.auth.result().now_or_never().is_some());
        self.manual.lock().unwrap().take()
    }
    fn on_progress(&self, _: &str) -> Result<(), OAuthError> {
        panic!("progress is not called")
    }
    fn on_select(
        &self,
        _: crate::OAuthSelectPrompt,
    ) -> Option<BoxFuture<Result<Option<String>, OAuthError>>> {
        panic!("selector is not called")
    }
    fn signal(&self) -> Option<&crate::Cancellation> {
        Some(&self.signal)
    }
}
/// Owned listener effect with a shutdown witness, without a socket.
fn listener() -> crate::oauth::native::CallbackServer {
    let server = crate::oauth::native::CallbackServer::new();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    tokio::spawn(async move {
        stop.cancelled().await;
        stopped.push(());
    });
    server
}
/// Accepted callback applied to the same pending listener identity.
fn accept(wait: &EventStream<crate::oauth::native::CallbackOutcome, ()>, code: &str) {
    let response = callback::route(&format!("/auth/callback?state=s&code={code}"), "s");
    assert_eq!(response.status, 200);
    wait.push(crate::oauth::native::CallbackOutcome::Accepted(
        response.accepted.unwrap(),
    ));
}
/// Successful token bytes for interaction tests.
fn success_bytes() -> Vec<u8> {
    br#"{"access_token":"h.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjdF8xMjMifX0=.s","refresh_token":"rotated","expires_in":0.5}"#.to_vec()
}
/// Controlled successful transport.
fn success_fetch() -> Fetch {
    response(200, String::new(), vec![success_bytes()], Effect::Normal)
}
/// Exercise the real coordinator while controlling only entropy and listener effects.
async fn login(
    callbacks: crate::OAuthCallbacks,
    fetch: Fetch,
    server: crate::oauth::native::CallbackServer,
) -> Result<OAuthCredentials, OAuthError> {
    super::login_with(
        callbacks,
        None,
        fetch,
        |_| {
            Ok((
                proof(),
                "s".into(),
                super::auth_info(&proof(), "s", "maestro"),
            ))
        },
        |_| std::future::ready(Ok(server)),
    )
    .await
}

#[test]
fn response_prompt_fallback_validates_optional_state() {
    run(async {
        for (manual, prompt, expected) in [
            ("", "prompt", Ok("prompt")),
            (" ", "prompt#s", Ok("prompt")),
            ("#s", "code=p", Ok("p")),
            ("code=m&state=", "unused", Ok("m")),
            ("m#s", "unused", Ok("m")),
            ("manual", "unused", Ok("manual")),
            ("", "code=p&state=", Ok("p")),
            ("code=&state=s&code=late", "prompt", Ok("prompt")),
            ("https://x/?state=s", "prompt", Ok("prompt")),
            ("", "https://x/?state=wrong", Err("State mismatch")),
            ("#wrong", "p", Err("State mismatch")),
            ("m#wrong", "unused", Err("State mismatch")),
            ("", "p#wrong", Err("State mismatch")),
            ("", "", Err("Missing authorization code")),
            ("", " ", Err("Missing authorization code")),
            ("", "code=&state=s", Err("Missing authorization code")),
        ] {
            let callbacks = Interaction::new(Some(Ok(manual.into())), prompt, "");
            let checked: Fetch = Arc::new(move |request| {
                let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
                    .into_owned()
                    .collect();
                assert_eq!(fields["code"], expected.unwrap());
                success_fetch()(request)
            });
            let result = login(callbacks, checked, callback::cancelled()).await;
            match expected {
                Ok(_) => assert_eq!(result.unwrap().refresh, "rotated"),
                Err(error) => assert_eq!(result.unwrap_err().to_string(), error),
            }
        }
        let callbacks = Interaction::new(None, "unused", "prompt");
        assert_eq!(
            login(callbacks, success_fetch(), callback::cancelled())
                .await
                .unwrap_err()
                .to_string(),
            "prompt failed"
        );
    });
}

#[test]
fn response_manual_and_browser_have_controlled_winners() {
    run(async {
        for winner in ["manual", "browser", "cancelled"] {
            check_winner(winner).await;
        }
    });
}
/// Keep one winning input and complete the losing producer before releasing that operation.
async fn check_winner(winner: &str) {
    let producer = EventStream::new(|_: &String| false, |_| ());
    let input = producer.clone();
    let completed = EventStream::new(|()| true, |()| ());
    let witness = completed.clone();
    let callbacks = Interaction::new(None, "unused", "");
    *callbacks.manual.lock().unwrap() = Some(Box::pin(async move {
        let input = input.next().await.unwrap();
        witness.push(());
        Ok(input)
    }));
    let server = if winner == "cancelled" {
        callback::cancelled()
    } else {
        listener()
    };
    let wait = server.wait.clone();
    let entered = EventStream::new(|_: &String| true, Clone::clone);
    let observed = entered.clone();
    let release = EventStream::new(|()| true, |()| ());
    let body_gate = release.clone();
    let fetch: Fetch = Arc::new(move |request| {
        let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
            .into_owned()
            .collect();
        observed.push(fields["code"].clone());
        let gate = body_gate.clone();
        Box::pin(released_token(gate, request))
    });
    let mut work = Box::pin(login(callbacks, fetch, server));
    assert!(work.as_mut().now_or_never().is_none());
    if winner == "browser" {
        accept(&wait, "browser-code");
    } else {
        producer.push("manual-code".into());
    }
    let work = tokio::spawn(work);
    let selected = entered.result().await;
    assert_eq!(
        selected,
        if winner == "browser" {
            "browser-code"
        } else {
            "manual-code"
        }
    );
    if winner == "browser" {
        producer.push("late-manual".into());
    }
    completed.result().await;
    assert!(!work.is_finished());
    release.push(());
    assert_eq!(work.await.unwrap().unwrap().refresh, "rotated");
    assert_eq!(entered.result().await, selected);
}

/// Release the same selected token request through a named gate.
async fn released_token(
    gate: EventStream<(), ()>,
    request: crate::HttpRequest,
) -> Result<HttpResponse, FetchError> {
    gate.result().await;
    success_fetch()(request).await
}

#[test]
fn response_native_refusal_leaves_shared_primitives_available() {
    run(async {
        let callbacks = Interaction::new(None, "unused", "");
        let result = super::login_with(
            callbacks.clone(),
            None,
            success_fetch(),
            |_| {
                Ok((
                    proof(),
                    "s".into(),
                    super::auth_info(&proof(), "s", "maestro"),
                ))
            },
            callback::unavailable,
        )
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "Response-account OAuth is only available in native environments"
        );
        assert!(callbacks.auth.result().now_or_never().is_none());
        assert_eq!(
            super::refresh_openai_codex_token("old".into(), Some(success_fetch()))
                .await
                .unwrap()
                .extra["accountId"],
            "acct_123"
        );
        assert!(!crate::generate_pkce().unwrap().verifier.is_empty());
        assert!(
            crate::oauth_success_html("usable")
                .unwrap()
                .contains("<p>usable</p>")
        );
    });
}

#[test]
fn response_token_forms_preserve_order_and_encoding() {
    run(async {
        for selected in [Operation::Exchange, Operation::Refresh] {
            let fetch: Fetch = Arc::new(move |request| {
                assert_eq!(request.method, "POST");
                assert_eq!(request.url, "https://auth.openai.com/oauth/token");
                assert_eq!(
                    request.headers.iter().collect::<Vec<_>>(),
                    vec![(
                        &"content-type".to_owned(),
                        &"application/x-www-form-urlencoded".to_owned()
                    )]
                );
                assert!(request.signal.is_none());
                assert_eq!(
                    std::str::from_utf8(&request.body).unwrap(),
                    expected_form(selected)
                );
                success_fetch()(request)
            });
            assert_eq!(
                operation(selected, fetch, clock).await.unwrap().refresh,
                "rotated"
            );
        }
    });
}

thread_local! {
    /// Completion clock for one current-thread operation.
    static CLOCK: std::cell::Cell<f64> = const { std::cell::Cell::new(100.0) };
}
/// Read the controlled completion clock.
fn controlled_clock() -> f64 {
    CLOCK.get()
}
/// Make premature clock reads fail at their decision boundary.
fn unread_clock() -> f64 {
    panic!("invalid token fields must not consult the clock")
}
/// Body operation that witnesses consumption before waiting for its release.
async fn held_body(
    entered: EventStream<(), ()>,
    gate: EventStream<(), ()>,
) -> Result<Vec<u8>, FetchError> {
    entered.push(());
    gate.result().await;
    Ok(success_bytes())
}
#[test]
fn response_expiry_uses_unbuffered_completion_clock() {
    run(async {
        for selected in [Operation::Exchange, Operation::Refresh] {
            CLOCK.set(100.0);
            let entered = EventStream::new(|()| true, |()| ());
            let release = EventStream::new(|()| true, |()| ());
            let gate = release.clone();
            let consumed = entered.clone();
            let fetch: Fetch = Arc::new(move |_| {
                let body = held_body(consumed.clone(), gate.clone());
                Box::pin(std::future::ready(Ok(HttpResponse {
                    status: 200,
                    status_text: String::new(),
                    headers: BTreeMap::new(),
                    body: Box::pin(futures_util::stream::once(body)),
                })))
            });
            let mut work = Box::pin(operation(selected, fetch, controlled_clock));
            assert!(work.as_mut().now_or_never().is_none());
            entered.result().await;
            CLOCK.set(7000.0);
            release.push(());
            assert_eq!(work.await.unwrap().expires.to_bits(), 7500.0_f64.to_bits());
            let invalid = response(
                200,
                String::new(),
                vec![b"{\"access_token\":\"\",\"refresh_token\":\"r\",\"expires_in\":1}".to_vec()],
                Effect::Normal,
            );
            assert!(
                operation(selected, invalid, unread_clock)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("response missing fields:")
            );
        }
    });
}

#[test]
fn response_settled_manual_errors_precede_code_selection() {
    run(async {
        for callback_ready in [false, true] {
            let server = listener();
            if callback_ready {
                accept(&server.wait, "browser");
            }
            let manual = super::start_manual(
                Box::pin(std::future::ready(Err(OAuthError::message(
                    "manual failed",
                )))),
                server.wait.clone(),
            );
            manual.result().await;
            if !callback_ready {
                accept(&server.wait, "later");
            }
            assert_eq!(
                super::wait_code("s", &server, Some(manual))
                    .await
                    .unwrap_err()
                    .to_string(),
                "manual failed"
            );
            server.close().await;
        }
        for late_failure in [false, true] {
            let server = listener();
            let producer = EventStream::new(|_: &bool| false, |_| ());
            let input = producer.clone();
            let manual = super::start_manual(Box::pin(late_manual(input)), server.wait.clone());
            let completed = manual.clone();
            accept(&server.wait, "selected");
            let code = super::wait_code("s", &server, Some(manual))
                .await
                .unwrap()
                .unwrap();
            producer.push(late_failure);
            completed.result().await;
            let credentials = token::exchange(code, "verifier".into(), success_fetch(), clock)
                .await
                .unwrap();
            assert_eq!(credentials.refresh, "rotated");
            server.close().await;
        }
    });
}

/// Unit-level native callback scenarios share one fixed-port admission owner.
static NATIVE_PORT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
/// Probe release using the same reuse policy as the native listener.
fn probe() -> tokio::net::TcpListener {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    #[cfg(unix)]
    socket.set_reuseaddr(true).unwrap();
    socket.bind("127.0.0.1:1455".parse().unwrap()).unwrap();
    socket.listen(128).unwrap()
}
/// Hold supplied token bytes behind a body-consumed witness and release gate.
async fn controlled_body(
    entered: EventStream<(), ()>,
    release: EventStream<(), ()>,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, FetchError> {
    entered.push(());
    release.result().await;
    Ok(bytes)
}
#[test]
fn response_login_closes_after_token_completion() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        for (status, bytes) in [
            (200, success_bytes()),
            (200, b"{".to_vec()),
            (
                200,
                b"{\"access_token\":\"h.e30.s\",\"refresh_token\":\"r\",\"expires_in\":1}".to_vec(),
            ),
            (401, b"denied".to_vec()),
        ] {
            check_token_cleanup(status, bytes).await;
        }
        for phase in ["prompt", "manual", "drop"] {
            check_interaction_cleanup(phase).await;
        }
    });
}
/// Keep admission through this token body and observe shutdown after its outcome.
async fn check_token_cleanup(status: u16, bytes: Vec<u8>) {
    let server = crate::oauth::native::bind("s".into(), 1455, callback::route)
        .await
        .unwrap();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    let entered = EventStream::new(|()| true, |()| ());
    let release = EventStream::new(|()| true, |()| ());
    let consumed = entered.clone();
    let gate = release.clone();
    let successful = bytes == success_bytes();
    let fetch: Fetch = Arc::new(move |_| {
        let body = controlled_body(consumed.clone(), gate.clone(), bytes.clone());
        Box::pin(std::future::ready(Ok(HttpResponse {
            status,
            status_text: String::new(),
            headers: BTreeMap::new(),
            body: Box::pin(futures_util::stream::once(body)),
        })))
    });
    let work = tokio::spawn(login(
        Interaction::new(Some(Ok("manual".into())), "unused", ""),
        fetch,
        server,
    ));
    entered.result().await;
    assert!(!stop.is_aborted());
    let peer = tokio::net::TcpStream::connect("127.0.0.1:1455")
        .await
        .unwrap();
    release.push(());
    assert_eq!(work.await.unwrap().is_ok(), successful);
    assert!(stop.is_aborted());
    stopped.result().await;
    drop(peer);
    drop(probe());
}
/// Observe listener release after interaction failure or future abandonment.
async fn check_interaction_cleanup(phase: &str) {
    let server = crate::oauth::native::bind("s".into(), 1455, callback::route)
        .await
        .unwrap();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    let callbacks = if phase == "manual" {
        Interaction::new(
            Some(Err(OAuthError::message("manual failed"))),
            "unused",
            "",
        )
    } else {
        Interaction::new(None, "unused", "prompt")
    };
    if phase == "prompt" {
        server
            .wait
            .push(crate::oauth::native::CallbackOutcome::Cancelled);
    }
    let mut work = Box::pin(login(callbacks, success_fetch(), server));
    if phase == "drop" {
        assert!(work.as_mut().now_or_never().is_none());
        drop(work);
    } else {
        assert_eq!(
            work.await.unwrap_err().to_string(),
            format!("{phase} failed")
        );
    }
    assert!(stop.is_aborted());
    stopped.result().await;
    drop(probe());
}

#[test]
fn response_auth_callback_failure_releases_listener() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        let server = crate::oauth::native::bind("s".into(), 1455, callback::route)
            .await
            .unwrap();
        let stopped = server.stopped.clone();
        let fetch: Fetch = Arc::new(|_| panic!("auth failure precedes token exchange"));
        assert_eq!(
            login(Interaction::new(None, "unused", "auth"), fetch, server)
                .await
                .unwrap_err()
                .to_string(),
            "auth failed"
        );
        assert!(stopped.result().now_or_never().is_some());
        drop(probe());
    });
}

#[test]
fn response_login_environment_is_read_at_binding() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        for host in [None, Some(""), Some("127.0.0.2"), Some(" ")] {
            let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = reads.clone();
            let result = super::login_with(
                Interaction::new(Some(Ok("manual".into())), "unused", ""),
                None,
                success_fetch(),
                |_| {
                    Ok((
                        proof(),
                        "s".into(),
                        super::auth_info(&proof(), "s", "maestro"),
                    ))
                },
                move |state| binding_witness(state, host, observed),
            )
            .await;
            assert_eq!(result.unwrap().extra["accountId"], "acct_123");
            assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
        for phase in ["proof", "state"] {
            let result = super::login_with(
                Interaction::new(None, "unused", ""),
                None,
                success_fetch(),
                move |originator| {
                    super::authorization_flow_with(
                        originator,
                        || controlled_proof(phase),
                        |_| Err(OAuthError::message("state failed")),
                    )
                },
                move |state| {
                    callback::bind_with(
                        state,
                        |_| panic!("entropy failure avoids environment access"),
                        |listener| Box::pin(listener.accept()),
                    )
                },
            )
            .await;
            assert_eq!(result.unwrap_err().to_string(), format!("{phase} failed"));
        }
    });
}

#[test]
fn response_post_bind_failure_enters_fallback() {
    run(async {
        let _permit = NATIVE_PORT.acquire().await.unwrap();
        for accepted in [false, true] {
            let release = EventStream::new(|()| true, |()| ());
            let gate = release.clone();
            let server = callback::bind_with(
                "s".into(),
                |_| None,
                move |_| Box::pin(failed_accept(gate.clone())),
            )
            .await
            .unwrap();
            let stopped = server.stopped.clone();
            if accepted {
                accept(&server.wait, "accepted");
            }
            let checked: Fetch = Arc::new(move |request| {
                let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
                    .into_owned()
                    .collect();
                assert_eq!(
                    fields["code"],
                    if accepted { "accepted" } else { "fallback" }
                );
                success_fetch()(request)
            });
            let mut work = Box::pin(login(
                Interaction::new(None, "fallback", ""),
                checked,
                server,
            ));
            if !accepted {
                assert!(work.as_mut().now_or_never().is_none());
            }
            release.push(());
            stopped.result().await;
            assert_eq!(work.await.unwrap().extra["accountId"], "acct_123");
            drop(probe());
        }
    });
}
/// Publish a native accept failure only when its named producer releases it.
async fn failed_accept(
    gate: EventStream<(), ()>,
) -> std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
    gate.result().await;
    Err(std::io::Error::from_raw_os_error(5))
}

#[test]
fn response_ignores_progress_selector_signal_and_deadlines() {
    run(async {
        tokio::time::pause();
        for pre_aborted in [false, true] {
            let callbacks = Interaction::new(None, "unused", "");
            if pre_aborted {
                callbacks.signal.abort();
            }
            let server = listener();
            let wait = server.wait.clone();
            let entered = EventStream::new(|()| true, |()| ());
            let release = EventStream::new(|()| true, |()| ());
            let consumed = entered.clone();
            let gate = release.clone();
            let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = calls.clone();
            let fetch: Fetch = Arc::new(move |request| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert!(request.signal.is_none());
                let body = held_body(consumed.clone(), gate.clone());
                Box::pin(std::future::ready(Ok(HttpResponse {
                    status: 200,
                    status_text: String::new(),
                    headers: BTreeMap::new(),
                    body: Box::pin(futures_util::stream::once(body)),
                })))
            });
            let mut work = Box::pin(login(callbacks.clone(), fetch, server));
            assert!(work.as_mut().now_or_never().is_none());
            callbacks.signal.abort();
            tokio::time::advance(std::time::Duration::from_secs(30)).await;
            assert!(work.as_mut().now_or_never().is_none());
            tokio::time::advance(std::time::Duration::from_hours(24)).await;
            assert!(work.as_mut().now_or_never().is_none());
            accept(&wait, "after-day");
            assert!(work.as_mut().now_or_never().is_none());
            entered.result().await;
            tokio::time::advance(std::time::Duration::from_hours(24)).await;
            assert!(work.as_mut().now_or_never().is_none());
            release.push(());
            assert_eq!(work.await.unwrap().refresh, "rotated");
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    });
}

/// Authored form spellings from controlled protocol observations.
fn expected_form(operation: Operation) -> &'static str {
    match operation {
        Operation::Exchange => {
            "grant_type=authorization_code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code=c+%2B%26%F0%9F%98%80&code_verifier=v%2F%3F&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"
        }
        Operation::Refresh => {
            "grant_type=refresh_token&refresh_token=r+%2B%26%F0%9F%98%80&client_id=app_EMoamEEZ73f0CkXaXp7hrann"
        }
    }
}

/// Complete losing manual work through the same producer's success or failure branch.
async fn late_manual(input: EventStream<bool, ()>) -> Result<String, OAuthError> {
    if input.next().await.unwrap() {
        Err(OAuthError::message("late failed"))
    } else {
        Ok("late-success".into())
    }
}

/// Read environment only at native binding and prove the configured host is reached.
async fn binding_witness(
    state: String,
    host: Option<&str>,
    observed: Arc<std::sync::atomic::AtomicUsize>,
) -> Result<crate::oauth::native::CallbackServer, OAuthError> {
    let server = callback::bind_with(
        state,
        move |key| {
            assert_eq!(key, "MAESTRO_OAUTH_CALLBACK_HOST");
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            host.map(str::to_owned)
        },
        |listener| Box::pin(listener.accept()),
    )
    .await?;
    if host != Some(" ") {
        let actual = host.filter(|host| !host.is_empty()).unwrap_or("127.0.0.1");
        drop(
            tokio::net::TcpStream::connect((actual, 1455))
                .await
                .unwrap(),
        );
    }
    Ok(server)
}

/// Fail the selected proof phase without replacing the shared primitive's implementation.
fn controlled_proof(phase: &str) -> Result<crate::Pkce, OAuthError> {
    if phase == "proof" {
        Err(OAuthError::message("proof failed"))
    } else {
        Ok(proof())
    }
}
