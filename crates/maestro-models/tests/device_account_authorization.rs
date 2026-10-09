//! Device-account authorization through controlled interaction and transport.
#![cfg(test)]

#[test]
fn maestro_device_login_preserves_enterprise_endpoints() {
    use maestro_models::normalize_domain;
    for (input, expected) in [
        ("", None),
        (" ", None),
        ("\u{feff}", None),
        ("\u{85}", None),
        ("\u{feff}Example.COM\u{feff}", Some("example.com")),
        ("\u{85}example.com\u{85}", None),
        (
            " \tHTTPS://User:secret@Example.COM:8443/a?q=1#f\n",
            Some("example.com"),
        ),
        ("company.ghe.com", Some("company.ghe.com")),
        ("ftp://Example.COM/path", Some("example.com")),
        ("custom://EXAMPLE.COM/path", Some("EXAMPLE.COM")),
        ("file:///tmp/example", Some("")),
        ("https://", None),
        ("a b", None),
        ("://", None),
        ("//example.com", Some("example.com")),
        ("localhost:1234", Some("localhost")),
        ("[::1]:8443", Some("[::1]")),
        ("bücher.example", Some("xn--bcher-kva.example")),
        ("example.com.", Some("example.com.")),
        ("127.1", Some("127.0.0.1")),
        ("http://0x7f000001", Some("127.0.0.1")),
        ("https://%65xample.com", Some("example.com")),
        ("https://example.com\\path", Some("example.com")),
        ("https://exa\nmple.com", Some("example.com")),
        ("https://example.com\u{200b}", Some("example.com")),
        ("mailto:user@example.com", Some("example.com")),
    ] {
        assert_eq!(normalize_domain(input).as_deref(), expected, "{input:?}");
    }
    for whitespace in [
        '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}',
        '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}',
        '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
    ] {
        assert_eq!(
            normalize_domain(&format!("{whitespace}Example.COM{whitespace}")).as_deref(),
            Some("example.com")
        );
    }
    run(maestro_device_login_preserves_enterprise_endpoints_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn maestro_device_login_preserves_enterprise_endpoints_scenario() {
    for (input, expected) in [
        ("", None),
        ("\u{feff}", None),
        ("company.ghe.com", Some("company.ghe.com")),
        (" https://EXAMPLE.COM:8443/path ", Some("example.com")),
    ] {
        let interaction = Arc::new(Interaction {
            input: input.into(),
            ..Default::default()
        });
        let host = expected.unwrap_or("github.com");
        let inner = scripted(vec![
            DEVICE.into(),
            r#"{"access_token":"r"}"#.into(),
            SERVICE.into(),
        ]);
        let fetch: Fetch = Arc::new(move |request| {
            if request.url.ends_with("/device/code") {
                assert_eq!(request.url, format!("https://{host}/login/device/code"));
            }
            inner(request)
        });
        let credentials = maestro_models::login_github_copilot(interaction, Some(fetch))
            .await
            .unwrap();
        assert_eq!(
            credentials
                .extra
                .get("enterpriseUrl")
                .and_then(serde_json::Value::as_str),
            expected
        );
    }
    for input in ["\u{85}", "file:///tmp", "bad host", "\u{200b}"] {
        let interaction = Arc::new(Interaction {
            input: input.into(),
            ..Default::default()
        });
        let fetch: Fetch = Arc::new(|_| panic!("invalid login domain prevents Fetch"));
        assert_eq!(
            maestro_models::login_github_copilot(interaction, Some(fetch))
                .await
                .unwrap_err()
                .to_string(),
            "Invalid GitHub Enterprise URL/domain"
        );
    }
}

#[test]
fn device_base_urls_follow_token_priority() {
    use maestro_models::get_github_copilot_base_url as base;
    for (token, domain, expected) in [
        (None, None, "https://api.individual.githubcopilot.com"),
        (
            Some(""),
            Some(""),
            "https://api.individual.githubcopilot.com",
        ),
        (
            None,
            Some("company.ghe.com"),
            "https://copilot-api.company.ghe.com",
        ),
        (
            Some("x"),
            Some("https://Raw.EXAMPLE/a"),
            "https://copilot-api.https://Raw.EXAMPLE/a",
        ),
        (
            Some("abcproxy-ep=proxy.test;proxy-ep=proxy.later"),
            Some("ignored"),
            "https://api.test",
        ),
        (
            Some("proxy-ep=;proxy-ep=proxy.second;"),
            None,
            "https://api.second",
        ),
        (Some("proxy-ep=PROXY.X;"), None, "https://PROXY.X"),
        (Some("proxy-ep=proxy.;"), None, "https://api."),
        (
            Some("proxy-ep=proxy.proxy.example;"),
            None,
            "https://api.proxy.example",
        ),
        (
            Some("proxy-ep= api.example ;"),
            None,
            "https:// api.example ",
        ),
        (Some("proxy-ep=api.example"), None, "https://api.example"),
        (Some("proxy-ep=proxy.☃;"), None, "https://api.☃"),
        (
            Some("proxy-ep=proxy.x\nsecond;"),
            None,
            "https://api.x\nsecond",
        ),
    ] {
        assert_eq!(base(token, domain), expected);
    }
}

use maestro_models::{BoxFuture, Fetch, HttpResponse, OAuthCredentials, OAuthError};
use std::sync::Arc;

/// Run deterministic native futures with paused timers.
fn run(work: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::pause();
            work.await;
        });
}

/// One complete controlled JSON response.
fn response(text: impl Into<Vec<u8>>) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(futures_util::stream::iter([Ok(text.into())])),
    }
}

#[test]
fn device_refresh_preserves_tokens_expiry_and_domain() {
    run(device_refresh_preserves_tokens_expiry_and_domain_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_refresh_preserves_tokens_expiry_and_domain_scenario() {
    for (expiry, expected) in [
        ("1234", 934_000.0_f64),
        ("0", -300_000.0),
        ("-0.001", -300_001.0),
        ("0.0005", -299_999.5),
    ] {
        for domain in [None, Some(""), Some("company.ghe.com")] {
            let text = format!(r#"{{"token":"","expires_at":{expiry}}}"#);
            let fetch = fixed_fetch(text);
            let credentials = maestro_models::refresh_github_copilot_token(
                String::new(),
                domain.map(str::to_owned),
                Some(fetch),
            )
            .await
            .unwrap();
            assert_eq!(credentials.refresh, "");
            assert_eq!(credentials.access, "");
            assert_eq!(credentials.expires.to_bits(), expected.to_bits());
            assert_eq!(
                credentials.extra.get("enterpriseUrl"),
                domain.map(|d| serde_json::Value::String(d.into())).as_ref()
            );
        }
    }
}

#[test]
fn device_refresh_selects_surviving_fields() {
    run(device_refresh_selects_surviving_fields_scenario());
}

use futures_util::FutureExt as _;
use maestro_models::{
    Cancellation, OAuthAuthInfo, OAuthCallbacks, OAuthLoginCallbacks, OAuthPrompt,
};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Controlled interaction recording each delivered callback.
#[derive(Default)]
struct Interaction {
    /// Enterprise prompt answer.
    input: String,
    /// Callback that returns a controlled failure.
    fail: Option<&'static str>,
    /// Callback that aborts before returning.
    abort: Option<&'static str>,
    /// Caller cancellation.
    signal: Cancellation,
    /// Delivered interaction events.
    events: Mutex<Vec<String>>,
}
impl OAuthLoginCallbacks for Interaction {
    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        self.events.lock().unwrap().push(format!(
            "prompt:{}:{}:{:?}",
            prompt.message,
            prompt.placeholder.unwrap(),
            prompt.allow_empty
        ));
        if self.abort == Some("prompt") {
            self.signal.abort();
        }
        let input = self.input.clone();
        let fail = self.fail == Some("prompt");
        Box::pin(async move {
            if fail {
                Err(failure("prompt-failed"))
            } else {
                Ok(input)
            }
        })
    }
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        self.events.lock().unwrap().push(format!(
            "auth:{}:{}",
            info.url,
            info.instructions.unwrap()
        ));
        if self.abort == Some("auth") {
            self.signal.abort();
        }
        if self.fail == Some("auth") {
            Err(failure("auth-failed"))
        } else {
            Ok(())
        }
    }
    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        self.events
            .lock()
            .unwrap()
            .push(format!("progress:{message}"));
        if self.fail == Some("progress") {
            Err(failure("progress-failed"))
        } else {
            Ok(())
        }
    }
    fn signal(&self) -> Option<&Cancellation> {
        Some(&self.signal)
    }
}

/// Standard device authorization response.
const DEVICE: &str = r#"{"device_code":"dev","user_code":"USER","verification_uri":"https://verify.example/a","interval":5,"expires_in":900}"#;
/// Standard exchanged service token.
const SERVICE: &str = r#"{"token":"proxy-ep=proxy.test;","expires_at":1234}"#;

/// Consume controlled responses for the protocol, and immediately finish policies.
fn scripted(texts: Vec<String>) -> Fetch {
    let queue = Mutex::new(VecDeque::from(texts));
    Arc::new(move |request| {
        assert!(request.signal.is_none());
        let text = if request.url.ends_with("/policy") {
            String::new()
        } else {
            queue
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected protocol request")
        };
        Box::pin(async move { Ok(response(text)) })
    })
}

/// Controlled scenario for the named authorization behavior.
async fn device_refresh_selects_surviving_fields_scenario() {
    for (text, expected) in [
        ("null", "Invalid Copilot token response"),
        ("false", "Invalid Copilot token response"),
        ("1", "Invalid Copilot token response"),
        (r#""text""#, "Invalid Copilot token response"),
        ("[]", "Invalid Copilot token response fields"),
        ("{}", "Invalid Copilot token response fields"),
        (r#"["token",123]"#, "Invalid Copilot token response fields"),
        (
            r#"{"token":12,"expires_at":1}"#,
            "Invalid Copilot token response fields",
        ),
        (
            r#"{"token":"ok","expires_at":"1"}"#,
            "Invalid Copilot token response fields",
        ),
        (
            r#"{"token":"first","token":0,"expires_at":1}"#,
            "Invalid Copilot token response fields",
        ),
        (r#"{"token":"ok"}"#, "Invalid Copilot token response fields"),
        (
            r#"{"expires_at":1}"#,
            "Invalid Copilot token response fields",
        ),
        (
            r#"{"token":"\ud800","expires_at":1}"#,
            "Invalid Copilot token response fields",
        ),
    ] {
        let fetch: Fetch = Arc::new(move |_| Box::pin(async move { Ok(response(text)) }));
        let error = maestro_models::refresh_github_copilot_token("r".into(), None, Some(fetch))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected, "{text}");
    }
    let deep = format!("{}0{}", "[".repeat(300), "]".repeat(300));
    let text = format!(r#"{{"token":0,"to\u006ben":"last","expires_at":1,"unused":{deep}}}"#);
    let fetch: Fetch = Arc::new(move |_| {
        let text = text.clone();
        Box::pin(async move { Ok(response(text)) })
    });
    assert_eq!(
        maestro_models::refresh_github_copilot_token("r".into(), None, Some(fetch))
            .await
            .unwrap()
            .access,
        "last"
    );
}

#[test]
fn maestro_device_login_waits_before_polling() {
    run(maestro_device_login_waits_before_polling_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn maestro_device_login_waits_before_polling_scenario() {
    let start = tokio::time::Instant::now();
    let times = Arc::new(Mutex::new(Vec::new()));
    let observed = times.clone();
    let queue = Mutex::new(VecDeque::from([
        DEVICE,
        r#"{"error":"authorization_pending"}"#,
        r#"{"error":"slow_down","interval":10}"#,
        r#"{"access_token":"account"}"#,
        SERVICE,
    ]));
    let fetch: Fetch = Arc::new(move |request| {
        let text = if request.url.ends_with("/policy") {
            ""
        } else {
            if request.url.ends_with("/access_token") {
                observed.lock().unwrap().push(start.elapsed().as_millis());
            }
            queue.lock().unwrap().pop_front().unwrap()
        };
        Box::pin(async move { Ok(response(text)) })
    });
    let mut login =
        maestro_models::login_github_copilot(Arc::new(Interaction::default()), Some(fetch));
    assert!(login.as_mut().now_or_never().is_none());
    // Tokio rounds a timer deadline upward by at most one millisecond tick.
    for expected in [vec![6001], vec![6001, 12002], vec![6001, 12002, 26003]] {
        let wait = if expected.len() == 3 { 14000 } else { 6000 };
        tokio::time::advance(std::time::Duration::from_millis(wait - 1)).await;
        assert!(login.as_mut().now_or_never().is_none());
        assert_eq!(times.lock().unwrap().len(), expected.len() - 1);
        tokio::time::advance(std::time::Duration::from_millis(2)).await;
        let result = login.as_mut().now_or_never();
        assert_eq!(*times.lock().unwrap(), expected);
        if expected.len() == 3 {
            let credentials = result.expect("last poll completed login").unwrap();
            assert_eq!(credentials.refresh, "account");
            assert_eq!(credentials.access, "proxy-ep=proxy.test;");
        } else {
            assert!(result.is_none());
        }
    }
}

#[test]
fn device_provider_exposes_shared_contract() {
    let provider = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER;
    assert_eq!(provider.id(), "github-copilot");
    assert_eq!(provider.name(), "GitHub Copilot");
    assert_eq!(provider.uses_callback_server(), None);
    let credentials = OAuthCredentials {
        access: "access-only".into(),
        refresh: "refresh".into(),
        expires: 0.0,
        extra: maestro_models::JsonObject::new(),
    };
    assert!(std::ptr::eq(
        provider.get_api_key(&credentials).unwrap(),
        credentials.access.as_str()
    ));
    assert!(std::ptr::eq(
        provider,
        maestro_models::oauth::device::github_copilot::GITHUB_COPILOT_OAUTH_PROVIDER
    ));
    assert!(std::ptr::eq(
        provider,
        maestro_models::oauth::GITHUB_COPILOT_OAUTH_PROVIDER
    ));
    let _: fn(OAuthCallbacks, Option<Fetch>) -> BoxFuture<Result<OAuthCredentials, OAuthError>> =
        maestro_models::oauth::login_github_copilot;

    run(device_provider_exposes_shared_contract_scenario());
}

/// Caller-authored diagnostic used by controlled effects.
fn failure(message: &str) -> OAuthError {
    maestro_models::DiagnosticErrorInfo {
        message: message.into(),
        name: None,
        stack: None,
        code: None,
    }
    .into()
}

/// Controlled scenario for the named authorization behavior.
async fn device_provider_exposes_shared_contract_scenario() {
    let result = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
        .login(
            Arc::new(Interaction::default()),
            Some(scripted(vec![
                DEVICE.into(),
                r#"{"access_token":"refresh"}"#.into(),
                SERVICE.into(),
            ])),
        )
        .await
        .unwrap();
    assert_eq!(result.refresh, "refresh");
}

#[test]
fn device_requests_keep_wire_fields() {
    run(device_requests_keep_wire_fields_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_requests_keep_wire_fields_scenario() {
    let interaction = Arc::new(Interaction {
        input: " HTTPS://ENTERPRISE.example:8443/path ".into(),
        ..Default::default()
    });
    let phases = Arc::new(Mutex::new(Vec::new()));
    let observed = phases.clone();
    let fetch: Fetch = Arc::new(move |request| {
        let text = wire_response(&request, &observed);
        Box::pin(async move { Ok(response(text)) })
    });
    maestro_models::login_github_copilot(interaction.clone(), Some(fetch))
        .await
        .unwrap();
    assert_eq!(*phases.lock().unwrap(), ["device", "poll", "refresh"]);
    assert!(
        interaction
            .events
            .lock()
            .unwrap()
            .contains(&"auth:not a url:Enter code: <code>\n".into())
    );
}

#[test]
fn device_prompt_precedes_cancellation() {
    run(device_prompt_precedes_cancellation_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_prompt_precedes_cancellation_scenario() {
    for (preabort, abort, fail, expected) in [
        (true, None, None, "Login cancelled"),
        (false, Some("prompt"), None, "Login cancelled"),
        (true, None, Some("prompt"), "prompt-failed"),
    ] {
        let interaction = Arc::new(Interaction {
            input: "bad host".into(),
            abort,
            fail,
            ..Default::default()
        });
        if preabort {
            interaction.signal.abort();
        }
        let fetch: Fetch = Arc::new(|_| panic!("the settled prompt decision must prevent Fetch"));
        let error = maestro_models::login_github_copilot(interaction.clone(), Some(fetch))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(
            *interaction.events.lock().unwrap(),
            [
                "prompt:GitHub Enterprise URL/domain (blank for github.com):company.ghe.com:Some(true)"
            ]
        );
    }
}

#[test]
fn device_callbacks_preserve_order_and_failures() {
    run(device_callbacks_preserve_order_and_failures_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_callbacks_preserve_order_and_failures_scenario() {
    for fail in [None, Some("auth"), Some("progress")] {
        let interaction = Arc::new(Interaction {
            fail,
            ..Default::default()
        });
        let effects = interaction.clone();
        let fetch: Fetch = Arc::new(move |request| {
            let (phase, text) = if request.url.ends_with("/device/code") {
                ("device", DEVICE)
            } else if request.url.ends_with("/access_token") {
                ("poll", r#"{"access_token":"r"}"#)
            } else if request.url.ends_with("/token") {
                ("refresh", SERVICE)
            } else {
                ("policy", "")
            };
            effects.events.lock().unwrap().push(phase.into());
            Box::pin(async move { Ok(response(text)) })
        });
        let result = maestro_models::login_github_copilot(interaction.clone(), Some(fetch)).await;
        let events = interaction.events.lock().unwrap();
        let prefix = [
            "prompt:GitHub Enterprise URL/domain (blank for github.com):company.ghe.com:Some(true)",
            "device",
            "auth:https://verify.example/a:Enter code: USER",
        ];
        assert_eq!(&events[..3], prefix);
        match fail {
            Some("auth") => {
                assert_eq!(result.unwrap_err().to_string(), "auth-failed");
                assert_eq!(events.len(), 3);
            }
            Some("progress") => {
                assert_eq!(result.unwrap_err().to_string(), "progress-failed");
                assert_eq!(
                    &events[3..],
                    ["poll", "refresh", "progress:Enabling models..."]
                );
            }
            None => {
                result.unwrap();
                assert_eq!(
                    &events[3..6],
                    ["poll", "refresh", "progress:Enabling models..."]
                );
                assert_eq!(
                    events.len(),
                    6 + maestro_models::get_models("github-copilot").len()
                );
                assert!(events[6..].iter().all(|event| event == "policy"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn device_code_fields_are_selected_before_decoding() {
    run(device_code_fields_are_selected_before_decoding_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_code_fields_are_selected_before_decoding_scenario() {
    device_root_errors().await;
    for key in [
        "device_code",
        "user_code",
        "verification_uri",
        "interval",
        "expires_in",
    ] {
        for wrong in [false, true] {
            let fields = invalid_device_fields(key, wrong);
            let interaction = Arc::new(Interaction::default());
            let result = maestro_models::login_github_copilot(
                interaction.clone(),
                Some(scripted(vec![fields])),
            )
            .await
            .unwrap_err();
            assert_eq!(result.to_string(), "Invalid device code response fields");
            assert_eq!(interaction.events.lock().unwrap().len(), 1);
        }
    }
    let deep = format!("{}1e400{}", "[".repeat(1000), "]".repeat(1000));
    for text in [
        r#"{"device_code":"","user_code":"","verification_uri":"","interval":5,"expires_in":900}"#
            .to_owned(),
        format!(
            r#"{{"device_code":12,"\u0064evice_code":"last","user_code":"code","verification_uri":"url","interval":5,"expires_in":900,"ignored":{deep},"bad\ud800":1}}"#
        ),
    ] {
        let result = maestro_models::login_github_copilot(
            Arc::new(Interaction::default()),
            Some(scripted(vec![
                text,
                r#"{"access_token":"r"}"#.into(),
                SERVICE.into(),
            ])),
        )
        .await
        .unwrap();
        assert_eq!(result.refresh, "r");
    }
    let result = maestro_models::login_github_copilot(Arc::new(Interaction::default()), Some(scripted(vec![r#"{"device_code":"first","device_code":12,"user_code":"code","verification_uri":"url","interval":5,"expires_in":900}"#.into()]))).await.unwrap_err();
    assert_eq!(result.to_string(), "Invalid device code response fields");
}

#[test]
fn device_token_poll_selects_access_before_error() {
    run(device_token_poll_selects_access_before_error_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_token_poll_selects_access_before_error_scenario() {
    for (text, expected) in [
        (r#"{"access_token":"","error":"access_denied"}"#, ""),
        (
            r#"{"access_token":"ok","error":"slow_down","interval":1e400,"error_description":"\ud800"}"#,
            "ok",
        ),
        (
            r#"{"access_token":12,"access_token":"last","error_description":{}}"#,
            "last",
        ),
    ] {
        let result = maestro_models::login_github_copilot(
            Arc::new(Interaction::default()),
            Some(scripted(vec![DEVICE.into(), text.into(), SERVICE.into()])),
        )
        .await
        .unwrap();
        assert_eq!(result.refresh, expected);
    }
    let result = maestro_models::login_github_copilot(
        Arc::new(Interaction::default()),
        Some(scripted(vec![
            DEVICE.into(),
            r#"{"error":"authorization_pending","interval":1e400,"error_description":"\ud800"}"#
                .into(),
            r#"{"access_token":"next"}"#.into(),
            SERVICE.into(),
        ])),
    )
    .await
    .unwrap();
    assert_eq!(result.refresh, "next");
}

#[test]
fn device_unknown_token_bodies_keep_polling() {
    run(device_unknown_token_bodies_keep_polling_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_unknown_token_bodies_keep_polling_scenario() {
    for text in [
        "null",
        "false",
        "2",
        r#""unexpected""#,
        "[]",
        "{}",
        r#"{"access_token":23,"error":42}"#,
        r#"{"access_token":12,"error":"authorization_pending"}"#,
    ] {
        let start = tokio::time::Instant::now();
        let inner = scripted(vec![
            DEVICE.into(),
            text.into(),
            r#"{"access_token":"next"}"#.into(),
            SERVICE.into(),
        ]);
        let times = Arc::new(Mutex::new(Vec::new()));
        let observed = times.clone();
        let fetch: Fetch = Arc::new(move |request| {
            if request.url.ends_with("/access_token") {
                observed.lock().unwrap().push(start.elapsed());
            }
            inner(request)
        });
        let result =
            maestro_models::login_github_copilot(Arc::new(Interaction::default()), Some(fetch))
                .await
                .unwrap();
        assert_eq!(result.refresh, "next");
        let times = times.lock().unwrap();
        assert_eq!(times.len(), 2);
        for (time, expected) in times.iter().zip([6000, 12000]) {
            assert!(
                (std::time::Duration::from_millis(expected)
                    ..=std::time::Duration::from_millis(expected + 2))
                    .contains(time)
            );
        }
    }
}

#[test]
fn device_errors_keep_status_reason_and_body() {
    run(device_errors_keep_status_reason_and_body_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_errors_keep_status_reason_and_body_scenario() {
    for stage in ["/device/code", "/access_token", "/token"] {
        for mode in ["empty", "reason", "body200", "body403", "transport"] {
            let fetch = failure_fetch(stage, mode);
            let error =
                maestro_models::login_github_copilot(Arc::new(Interaction::default()), Some(fetch))
                    .await
                    .unwrap_err();
            assert_eq!(
                error.to_string(),
                match mode {
                    "transport" => "transport-failed",
                    "body200" | "body403" => "body-failed",
                    "reason" => "403 Custom failure: raw\nbody",
                    _ => "403 : raw\nbody",
                }
            );
        }
    }
}

#[test]
fn device_errors_keep_server_description() {
    run(device_errors_keep_server_description_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_errors_keep_server_description_scenario() {
    for error in ["access_denied", "expired_token", "unknown", ""] {
        for (description, suffix) in [
            (None, ""),
            (Some(r#""""#), ""),
            (Some(r#""details""#), ": details"),
        ] {
            let description =
                description.map_or_else(String::new, |raw| format!(",\"error_description\":{raw}"));
            let text = format!(r#"{{"error":"{error}"{description},"interval":1e400}}"#);
            let result = maestro_models::login_github_copilot(
                Arc::new(Interaction::default()),
                Some(scripted(vec![DEVICE.into(), text])),
            )
            .await
            .unwrap_err();
            assert_eq!(
                result.to_string(),
                format!("Device flow failed: {error}{suffix}")
            );
        }
    }
    for description in [
        "null",
        "false",
        "0",
        "1",
        "true",
        "{}",
        "[]",
        r#"["a","b"]"#,
    ] {
        let text = format!(r#"{{"error":"access_denied","error_description":{description}}}"#);
        let error = maestro_models::login_github_copilot(
            Arc::new(Interaction::default()),
            Some(scripted(vec![DEVICE.into(), text])),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "Device flow failed: access_denied");
    }
    let text = r#"{"error":"denied","error_description":" \ufeffdetail\u0085 "}"#;
    let error = maestro_models::login_github_copilot(
        Arc::new(Interaction::default()),
        Some(scripted(vec![DEVICE.into(), text.into()])),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Device flow failed: denied:  \u{feff}detail\u{85} "
    );
}

#[test]
fn device_refresh_delegation_keeps_raw_enterprise_domain() {
    run(device_refresh_delegation_keeps_raw_enterprise_domain_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_refresh_delegation_keeps_raw_enterprise_domain_scenario() {
    for domain in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::Value::String(String::new())),
        Some("https://Raw.EXAMPLE/path".into()),
    ] {
        let mut credentials = OAuthCredentials {
            refresh: "r".into(),
            access: "old".into(),
            expires: 1.0,
            extra: maestro_models::JsonObject::new(),
        };
        if let Some(domain) = domain {
            credentials.extra.insert("enterpriseUrl".into(), domain);
        }
        let expected_domain = credentials
            .extra
            .get("enterpriseUrl")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let host = expected_domain
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("github.com");
        let expected_url = format!("https://api.{host}/copilot_internal/v2/token");
        let fetch: Fetch = Arc::new(move |request| {
            assert_eq!(request.url, expected_url);
            Box::pin(async { Ok(response(SERVICE)) })
        });
        let result = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
            .refresh_token(credentials, Some(fetch))
            .await
            .unwrap();
        assert_eq!(
            result.extra.get("enterpriseUrl"),
            expected_domain.map(serde_json::Value::String).as_ref()
        );
    }
    refresh_rejects_metadata().await;
}

#[test]
fn device_provider_changes_only_matching_models() {
    let mut models = maestro_models::get_models("github-copilot");
    models.truncate(3);
    models[0].base_url = "changed-from".into();
    models[1].provider = "other".into();
    models[1].base_url = "untouched".into();
    models[2].base_url = "different-from".into();
    let original = models.clone();
    for (access, domain, expected) in [
        (
            "proxy-ep=proxy.token;",
            Some("bad host"),
            "https://api.token",
        ),
        (
            "plain",
            Some(" https://CAPS.example:999/path "),
            "https://copilot-api.caps.example",
        ),
        (
            "",
            Some("bad host"),
            "https://api.individual.githubcopilot.com",
        ),
        (
            "plain",
            Some(""),
            "https://api.individual.githubcopilot.com",
        ),
        ("proxy-ep=proxy.token;", None, "https://api.token"),
    ] {
        let mut credentials = OAuthCredentials {
            access: access.into(),
            refresh: "r".into(),
            expires: 0.0,
            extra: maestro_models::JsonObject::new(),
        };
        credentials.extra.insert(
            "enterpriseUrl".into(),
            domain.map_or(serde_json::Value::Null, Into::into),
        );
        let result = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
            .modify_models(models.clone(), &credentials)
            .unwrap();
        let mut expected_models = original.clone();
        expected_models[0].base_url = expected.into();
        expected_models[2].base_url = expected.into();
        assert_eq!(result, expected_models);
        assert_eq!(models, original);
    }
    modifier_rejects_metadata(&models);
}

#[test]
fn maestro_device_tokens_enable_catalog_models() {
    run(maestro_device_tokens_enable_catalog_models_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn maestro_device_tokens_enable_catalog_models_scenario() {
    let models = maestro_models::get_models("github-copilot");
    assert!(!models.is_empty());
    let (started, mut requests) = tokio::sync::mpsc::unbounded_channel();
    let fetch: Fetch = Arc::new(move |request| {
        if request.url.ends_with("/policy") {
            let (release, response) = tokio::sync::oneshot::channel();
            started.send((request, release)).unwrap();
            Box::pin(async move { response.await.unwrap() })
        } else {
            let text = if request.url.ends_with("/device/code") {
                DEVICE
            } else if request.url.ends_with("/access_token") {
                r#"{"access_token":"r"}"#
            } else {
                SERVICE
            };
            Box::pin(async move { Ok(response(text)) })
        }
    });
    let interaction = Arc::new(Interaction::default());
    let login = tokio::spawn(maestro_models::login_github_copilot(
        interaction.clone(),
        Some(fetch),
    ));
    let mut releases = Vec::new();
    let mut expected: std::collections::BTreeSet<_> = models
        .iter()
        .map(|model| format!("https://api.test/models/{}/policy", model.id))
        .collect();
    for _ in &models {
        let (request, release) = requests.recv().await.expect("all-policy-started barrier");
        assert!(expected.remove(&request.url));
        check_policy_wire(&request);
        releases.push(release);
    }
    assert!(expected.is_empty());
    assert!(!login.is_finished());
    for (index, release) in releases.into_iter().rev().enumerate() {
        let result = policy_outcome(index);
        release
            .send(result)
            .unwrap_or_else(|_| panic!("login dropped a policy attempt"));
    }
    let credentials = login.await.unwrap().unwrap();
    assert_eq!(credentials.access, "proxy-ep=proxy.test;");
    assert!(requests.recv().await.is_none());
    assert_eq!(
        interaction
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.starts_with("progress:"))
            .collect::<Vec<_>>(),
        [&"progress:Enabling models...".to_string()]
    );
}

#[test]
fn device_cancel_does_not_race_fetch() {
    run(device_cancel_does_not_race_fetch_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_cancel_does_not_race_fetch_scenario() {
    for (stage, pending) in [
        ("/access_token", false),
        ("/access_token", true),
        ("/token", false),
        ("/policy", false),
    ] {
        let (started, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let fetch = held_fetch(stage, pending, started);
        let interaction = Arc::new(Interaction::default());
        let login = tokio::spawn(maestro_models::login_github_copilot(
            interaction.clone(),
            Some(fetch),
        ));
        let release = requests.recv().await.expect("held-phase-started witness");
        interaction.signal.abort();
        assert!(!login.is_finished());
        release.send(()).unwrap();
        let result = login.await.unwrap();
        if pending {
            assert_eq!(result.unwrap_err().to_string(), "Login cancelled");
        } else {
            assert_eq!(result.unwrap().access, "proxy-ep=proxy.test;");
        }
    }
}

#[test]
fn maestro_device_login_cancels_scoped_waits() {
    run(maestro_device_login_cancels_scoped_waits_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn maestro_device_login_cancels_scoped_waits_scenario() {
    for completed in 0..3 {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let fetch = pending_fetch(count.clone());
        let interaction = Arc::new(Interaction::default());
        let mut login = maestro_models::login_github_copilot(interaction.clone(), Some(fetch));
        assert!(login.as_mut().now_or_never().is_none());
        for _ in 0..completed {
            tokio::time::advance(std::time::Duration::from_millis(6001)).await;
            assert!(login.as_mut().now_or_never().is_none());
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), completed);
        interaction.signal.abort();
        assert_eq!(login.await.unwrap_err().to_string(), "Login cancelled");
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), completed);
    }
    let interaction = Arc::new(Interaction {
        abort: Some("auth"),
        ..Default::default()
    });
    let result =
        maestro_models::login_github_copilot(interaction, Some(scripted(vec![DEVICE.into()])))
            .await
            .unwrap_err();
    assert_eq!(result.to_string(), "Login cancelled");
}

#[test]
fn device_decode_uses_native_json_causes_and_shared_text() {
    run(device_decode_uses_native_json_causes_and_shared_text_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_decode_uses_native_json_causes_and_shared_text_scenario() {
    for stage in ["/device/code", "/access_token", "/token"] {
        let fetch: Fetch = Arc::new(move |request| {
            let text = if request.url.ends_with(stage) {
                "{"
            } else if request.url.ends_with("/device/code") {
                DEVICE
            } else {
                r#"{"access_token":"r"}"#
            };
            Box::pin(async move { Ok(response(text)) })
        });
        let error =
            maestro_models::login_github_copilot(Arc::new(Interaction::default()), Some(fetch))
                .await
                .unwrap_err();
        assert!(error.to_string().contains("line 1 column"));
    }
    for field in ["access_token", "error", "error_description"] {
        let text = if field == "error_description" {
            r#"{"error":"access_denied","error_description":"\ud800"}"#.into()
        } else {
            format!(r#"{{"{field}":"\ud800"}}"#)
        };
        let result = maestro_models::login_github_copilot(
            Arc::new(Interaction::default()),
            Some(scripted(vec![
                DEVICE.into(),
                text,
                r#"{"access_token":"next"}"#.into(),
                SERVICE.into(),
            ])),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("line 1 column"));
    }
    response_text_decoding().await;
}

/// One selected transport or body failure at a protocol phase.
fn failure_fetch(stage: &'static str, mode: &'static str) -> Fetch {
    Arc::new(move |request| {
        Box::pin(async move {
            let ordinary = if request.url.ends_with("/device/code") {
                DEVICE
            } else {
                r#"{"access_token":"r"}"#
            };
            if !request.url.ends_with(stage) {
                return Ok(response(ordinary));
            }
            if mode == "transport" {
                return Err(maestro_models::FetchError::Connection(
                    *failure("transport-failed").diagnostic,
                ));
            }
            let mut response = response("raw\nbody");
            response.status = if mode == "body200" { 200 } else { 403 };
            response.status_text = if mode == "reason" {
                "Custom failure"
            } else {
                ""
            }
            .into();
            if mode.starts_with("body") {
                response.body = Box::pin(futures_util::stream::iter([Err(
                    maestro_models::FetchError::Connection(*failure("body-failed").diagnostic),
                )]));
            }
            Ok(response)
        })
    })
}

/// Hold the first selected request behind its release channel.
fn held_fetch(
    stage: &'static str,
    pending: bool,
    started: tokio::sync::mpsc::UnboundedSender<tokio::sync::oneshot::Sender<()>>,
) -> Fetch {
    let held = std::sync::atomic::AtomicBool::new(false);
    Arc::new(move |request| {
        assert!(request.signal.is_none());
        let text = if request.url.ends_with("/device/code") {
            DEVICE
        } else if request.url.ends_with("/access_token") {
            if pending {
                r#"{"error":"authorization_pending"}"#
            } else {
                r#"{"access_token":"r"}"#
            }
        } else if request.url.ends_with("/token") {
            SERVICE
        } else {
            ""
        };
        if request.url.ends_with(stage) && !held.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let (release, response) = tokio::sync::oneshot::channel();
            started.send(release).unwrap();
            Box::pin(async move {
                response.await.unwrap();
                Ok(self::response(text))
            })
        } else {
            Box::pin(async move { Ok(response(text)) })
        }
    })
}

/// Pending then slow-down responses that never complete login.
fn pending_fetch(count: Arc<std::sync::atomic::AtomicUsize>) -> Fetch {
    let observed = count;
    Arc::new(move |request| {
        let text = if request.url.ends_with("/device/code") {
            DEVICE
        } else {
            assert!(request.url.ends_with("/access_token"));
            let index = observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if index == 0 {
                r#"{"error":"authorization_pending"}"#
            } else {
                r#"{"error":"slow_down"}"#
            }
        };
        Box::pin(async move { Ok(response(text)) })
    })
}

/// One missing or wrong-typed required device field.
fn invalid_device_fields(key: &str, wrong: bool) -> String {
    let mut fields: serde_json::Value = serde_json::from_str(DEVICE).unwrap();
    if wrong {
        fields[key] = if key == "interval" || key == "expires_in" {
            "12".into()
        } else {
            12.into()
        };
    } else {
        fields.as_object_mut().unwrap().remove(key);
    }
    fields.to_string()
}
/// A reusable fixed response, independently owned per request.
fn fixed_fetch(text: String) -> Fetch {
    Arc::new(move |_| {
        let text = text.clone();
        Box::pin(async move { Ok(response(text)) })
    })
}

/// Assert local wire framing and delegate phase-specific fields.
fn wire_response(
    request: &maestro_models::HttpRequest,
    phases: &Mutex<Vec<&'static str>>,
) -> &'static str {
    assert!(request.signal.is_none());
    if request.url.ends_with("/policy") {
        return "";
    }
    assert_eq!(request.headers["user-agent"], "GitHubCopilotChat/0.35.0");
    assert_eq!(request.headers["accept"], "application/json");
    if request.method == "GET" {
        check_refresh_wire(request);
        phases.lock().unwrap().push("refresh");
        return SERVICE;
    }
    assert_eq!(request.method, "POST");
    assert_eq!(request.headers.len(), 3);
    assert_eq!(
        request.headers["content-type"],
        "application/x-www-form-urlencoded"
    );
    if request.url.ends_with("/device/code") {
        assert_eq!(request.url, "https://enterprise.example/login/device/code");
        assert_eq!(
            request.body,
            b"client_id=Iv1.b507a08c87ecfe98&scope=read%3Auser"
        );
        phases.lock().unwrap().push("device");
        return r#"{"device_code":"a +&é/\n","user_code":"<code>\n","verification_uri":"not a url","interval":5,"expires_in":900}"#;
    }
    assert_eq!(
        request.url,
        "https://enterprise.example/login/oauth/access_token"
    );
    assert_eq!(request.body, b"client_id=Iv1.b507a08c87ecfe98&device_code=a+%2B%26%C3%A9%2F%0A&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code");
    phases.lock().unwrap().push("poll");
    r#"{"access_token":"account"}"#
}
/// Assert the account exchange's header operands.
fn check_refresh_wire(request: &maestro_models::HttpRequest) {
    assert_eq!(
        request.url,
        "https://api.enterprise.example/copilot_internal/v2/token"
    );
    assert!(request.body.is_empty());
    assert_eq!(request.headers["authorization"], "Bearer account");
    assert_eq!(request.headers["editor-version"], "vscode/1.107.0");
    assert_eq!(
        request.headers["editor-plugin-version"],
        "copilot-chat/0.35.0"
    );
    assert_eq!(request.headers["copilot-integration-id"], "vscode-chat");
    assert_eq!(request.headers.len(), 6);
}

/// Malformed metadata fails before descriptor transformation.
fn modifier_rejects_metadata(models: &[maestro_models::Model]) {
    for domain in [
        serde_json::json!(0),
        serde_json::json!(false),
        serde_json::json!(12),
        serde_json::json!({}),
        serde_json::json!([]),
    ] {
        let mut credentials = OAuthCredentials {
            access: "proxy-ep=proxy.token;".into(),
            refresh: "r".into(),
            expires: 0.0,
            extra: maestro_models::JsonObject::new(),
        };
        credentials.extra.insert("enterpriseUrl".into(), domain);
        assert!(
            maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
                .modify_models(models.to_vec(), &credentials)
                .unwrap_err()
                .to_string()
                .contains("invalid type")
        );
    }
}

/// Malformed enterprise operands fail before transport.
async fn refresh_rejects_metadata() {
    for domain in [
        serde_json::json!(0),
        serde_json::json!(false),
        serde_json::json!(12),
        serde_json::json!({}),
        serde_json::json!([]),
    ] {
        let mut credentials = OAuthCredentials {
            refresh: "r".into(),
            access: "old".into(),
            expires: 1.0,
            extra: maestro_models::JsonObject::new(),
        };
        credentials.extra.insert("enterpriseUrl".into(), domain);
        let fetch: Fetch = Arc::new(|_| panic!("invalid metadata prevents transport"));
        let error = maestro_models::GITHUB_COPILOT_OAUTH_PROVIDER
            .refresh_token(credentials, Some(fetch))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid type"));
    }
}

/// Scalar roots and arrays keep their distinct validation messages.
async fn device_root_errors() {
    for text in [
        "null",
        "false",
        "1",
        r#""text""#,
        "[]",
        "{}",
        r#"["a","b","c",5,900]"#,
    ] {
        let result = maestro_models::login_github_copilot(
            Arc::new(Interaction::default()),
            Some(scripted(vec![text.into()])),
        )
        .await
        .unwrap_err();
        assert_eq!(
            result.to_string(),
            if text.starts_with(['[', '{']) {
                "Invalid device code response fields"
            } else {
                "Invalid device code response"
            }
        );
    }
}

/// Complete model policy wire operands.
fn check_policy_wire(request: &maestro_models::HttpRequest) {
    assert_eq!(request.method, "POST");
    assert_eq!(request.body, br#"{"state":"enabled"}"#);
    assert!(request.signal.is_none());
    assert_eq!(
        request
            .headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>(),
        [
            ("content-type", "application/json"),
            ("authorization", "Bearer proxy-ep=proxy.test;"),
            ("user-agent", "GitHubCopilotChat/0.35.0"),
            ("editor-version", "vscode/1.107.0"),
            ("editor-plugin-version", "copilot-chat/0.35.0"),
            ("copilot-integration-id", "vscode-chat"),
            ("openai-intent", "chat-policy"),
            ("x-interaction-type", "chat-policy"),
        ]
    );
}

/// Shared UTF-8 text decoding precedes token selection.
async fn response_text_decoding() {
    for (bytes, expected) in [
        (
            [
                vec![0xef, 0xbb, 0xbf],
                br#"{"token":"ok","expires_at":1}"#.to_vec(),
            ]
            .concat(),
            "ok",
        ),
        (
            [
                br#"{"token":""#.to_vec(),
                vec![255],
                br#"","expires_at":1}"#.to_vec(),
            ]
            .concat(),
            "�",
        ),
    ] {
        let fetch: Fetch = Arc::new(move |_| {
            let bytes = bytes.clone();
            Box::pin(async move { Ok(response(bytes)) })
        });
        assert_eq!(
            maestro_models::refresh_github_copilot_token("r".into(), None, Some(fetch))
                .await
                .unwrap()
                .access,
            expected
        );
    }
}

/// Mixed policy outcomes without readable bodies.
fn policy_outcome(index: usize) -> Result<HttpResponse, maestro_models::FetchError> {
    if index.is_multiple_of(3) {
        Err(maestro_models::FetchError::Timeout)
    } else {
        let mut response = response("");
        response.status = if index % 3 == 1 { 403 } else { 200 };
        response.body = Box::pin(futures_util::stream::poll_fn(|_| {
            panic!("policy body must not be read")
        }));
        Ok(response)
    }
}
