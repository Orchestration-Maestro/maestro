//! Native response callback ownership with assigned socket addresses.
use super::super as codex;
use super::*;

/// Observe the actual assigned port without releasing its accepting owner.
async fn bound_callback(
    state: String,
    port: u16,
    environment: impl FnOnce(&str) -> Option<String>,
    accept: impl for<'a> Fn(&'a tokio::net::TcpListener) -> crate::oauth::native::Accept<'a>
    + Send
    + 'static,
) -> (
    crate::oauth::native::CallbackServer,
    EventStream<std::net::SocketAddr, std::net::SocketAddr>,
) {
    let address = EventStream::new(|_| true, |value: &std::net::SocketAddr| *value);
    let observed = address.clone();
    let server = callback::bind_with(state, port, environment, move |listener| {
        address.push(listener.local_addr().unwrap());
        accept(listener)
    })
    .await
    .unwrap();
    (server, observed)
}

/// Read the page emitted by the native HTTP consumer.
async fn callback_response(address: std::net::SocketAddr, target: &str) -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut peer = tokio::net::TcpStream::connect(address).await.unwrap();
    peer.write_all(
        format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .await
    .unwrap();
    let mut response = String::new();
    peer.read_to_string(&mut response).await.unwrap();
    response
}

/// Callback-only interaction publishes the authorization URL.
struct Browser {
    /// Authorization identity from the actual operation.
    auth: crate::EventStream<crate::OAuthAuthInfo, crate::OAuthAuthInfo>,
}
impl crate::OAuthLoginCallbacks for Browser {
    fn on_auth(&self, info: crate::OAuthAuthInfo) -> Result<(), crate::OAuthError> {
        self.auth.push(info);
        Ok(())
    }
    fn on_prompt(
        &self,
        _: crate::OAuthPrompt,
    ) -> crate::BoxFuture<Result<String, crate::OAuthError>> {
        panic!("valid callback avoids prompt")
    }
}
/// Release the same callback-selected token request after page consumption.
async fn released_fetch(
    gate: crate::EventStream<(), ()>,
    request: crate::HttpRequest,
) -> Result<HttpResponse, crate::FetchError> {
    gate.result().await;
    let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect();
    assert_eq!(fields["code"], "browser");
    response(
        200,
        String::new(),
        vec![success_bytes("callback-account")],
        Effect::Normal,
    )(request)
    .await
}

/// Compare the complete route response through the native HTTP consumer.
async fn check_page(address: std::net::SocketAddr, target: &str, status: u16, message: &str) {
    let response = callback_response(address, target).await;
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(
        headers.starts_with(&format!("HTTP/1.1 {status} ")),
        "{headers}"
    );
    assert!(headers.contains("content-type: text/html; charset=utf-8"));
    let expected = if status == 200 {
        crate::oauth_success_html(message)
    } else {
        crate::oauth_error_html(message, None)
    };
    assert_eq!(body, expected.unwrap());
}

/// Preserve each callback parser and route response before accepting the browser code.
async fn check_callback_pages(address: std::net::SocketAddr, state: &str) {
    assert!(
        callback_response(address, "http://[")
            .await
            .starts_with("HTTP/1.1 400 ")
    );
    for (target, status, message) in [
        ("/wrong".into(), 404, "Callback route not found."),
        (
            "/auth/callback?state=wrong&code=x".into(),
            400,
            "State mismatch.",
        ),
        (
            format!("/auth/callback?state={state}"),
            400,
            "Missing authorization code.",
        ),
        (
            "//[".into(),
            500,
            "Internal error while processing OAuth callback.",
        ),
        (
            format!("/auth/callback?state={state}&code=browser"),
            200,
            "OpenAI authentication completed. You can close this window.",
        ),
    ] {
        check_page(address, &target, status, message).await;
    }
}

/// Probe release using the same reuse policy as the native listener.
fn probe(address: std::net::SocketAddr) -> tokio::net::TcpListener {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    #[cfg(unix)]
    socket.set_reuseaddr(true).unwrap();
    socket.bind(address).unwrap();
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
/// Keep admission through this token body and observe shutdown after its outcome.
async fn check_token_cleanup(status: u16, bytes: Vec<u8>) {
    let (server, observed) = bound_callback(
        "s".into(),
        0,
        |_| None,
        |listener| Box::pin(listener.accept()),
    )
    .await;
    let address = observed.result().await;
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    let entered = EventStream::new(|()| true, |()| ());
    let release = EventStream::new(|()| true, |()| ());
    let consumed = entered.clone();
    let gate = release.clone();
    let successful = bytes == success_bytes("acct_123");
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
    let peer = tokio::net::TcpStream::connect(address).await.unwrap();
    release.push(());
    assert_eq!(work.await.unwrap().is_ok(), successful);
    assert!(stop.is_aborted());
    stopped.result().await;
    drop(peer);
    drop(probe(address));
}
/// Observe listener release after interaction failure or future abandonment.
async fn check_interaction_cleanup(phase: &str) {
    let (server, observed) = bound_callback(
        "s".into(),
        0,
        |_| None,
        |listener| Box::pin(listener.accept()),
    )
    .await;
    let address = observed.result().await;
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
    drop(probe(address));
}

/// Publish a native accept failure only when its named producer releases it.
async fn failed_accept(
    gate: EventStream<(), ()>,
) -> std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
    gate.result().await;
    Err(std::io::Error::from_raw_os_error(5))
}

/// Read environment only at native binding and prove the configured host is reached.
async fn binding_witness(
    state: String,
    host: Option<&str>,
    observed: Arc<std::sync::atomic::AtomicUsize>,
) -> Result<crate::oauth::native::CallbackServer, OAuthError> {
    let (server, observed_address) = bound_callback(
        state,
        0,
        move |key| {
            assert_eq!(key, "MAESTRO_OAUTH_CALLBACK_HOST");
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            host.map(str::to_owned)
        },
        |listener| Box::pin(listener.accept()),
    )
    .await;
    if host == Some(" ") {
        assert!(observed_address.result().now_or_never().is_none());
    } else {
        let actual = host.filter(|host| !host.is_empty()).unwrap_or("127.0.0.1");
        let address = observed_address.result().await;
        assert_eq!(address.ip().to_string(), actual);
        drop(tokio::net::TcpStream::connect(address).await.unwrap());
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

/// Native callback tests observe the addresses assigned to their listeners.
mod oauth_callback {
    use super::*;

    #[test]
    fn response_login_closes_after_token_completion() {
        run(async {
            for (status, bytes) in [
                (200, success_bytes("acct_123")),
                (200, b"{".to_vec()),
                (
                    200,
                    b"{\"access_token\":\"h.e30.s\",\"refresh_token\":\"r\",\"expires_in\":1}"
                        .to_vec(),
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

    #[test]
    fn response_auth_callback_failure_releases_listener() {
        run(async {
            let (server, observed) = bound_callback(
                "s".into(),
                0,
                |_| None,
                |listener| Box::pin(listener.accept()),
            )
            .await;
            let address = observed.result().await;
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
            drop(probe(address));
        });
    }

    #[test]
    fn response_login_environment_is_read_at_binding() {
        run(response_login_environment_is_read_at_binding_flow());
    }

    async fn response_login_environment_is_read_at_binding_flow() {
        for host in [None, Some(""), Some("127.0.0.2"), Some(" ")] {
            let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let observed = reads.clone();
            let result = codex::login_with(
                Interaction::new(Some(Ok("manual".into())), "unused", ""),
                None,
                success_fetch(),
                |_| {
                    Ok((
                        proof(),
                        "s".into(),
                        codex::auth_info(&proof(), "s", "maestro"),
                    ))
                },
                move |state| binding_witness(state, host, observed),
            )
            .await;
            assert_eq!(result.unwrap().extra["accountId"], "acct_123");
            assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
        for phase in ["proof", "state"] {
            let result = codex::login_with(
                Interaction::new(None, "unused", ""),
                None,
                success_fetch(),
                move |originator| {
                    codex::authorization_flow_with(
                        originator,
                        || controlled_proof(phase),
                        |_| Err(OAuthError::message("state failed")),
                    )
                },
                move |state| {
                    callback::bind_with(
                        state,
                        0,
                        |_| panic!("entropy failure avoids environment access"),
                        |listener| Box::pin(listener.accept()),
                    )
                },
            )
            .await;
            assert_eq!(result.unwrap_err().to_string(), format!("{phase} failed"));
        }
    }

    #[test]
    fn response_post_bind_failure_enters_fallback() {
        run(response_post_bind_failure_enters_fallback_flow());
    }

    async fn response_post_bind_failure_enters_fallback_flow() {
        for accepted in [false, true] {
            let release = EventStream::new(|()| true, |()| ());
            let gate = release.clone();
            let (server, observed) = bound_callback(
                "s".into(),
                0,
                |_| None,
                move |_| Box::pin(failed_accept(gate.clone())),
            )
            .await;
            let address = observed.result().await;
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
            drop(probe(address));
        }
    }
    #[test]
    fn response_callback_instances_use_independent_addresses() {
        run(async {
            let (left, left_observed) = bound_callback(
                "left-state".into(),
                0,
                |_| None,
                |listener| Box::pin(listener.accept()),
            )
            .await;
            let left_address = left_observed.result().await;
            let (right, right_observed) = bound_callback(
                "right-state".into(),
                0,
                |_| None,
                |listener| Box::pin(listener.accept()),
            )
            .await;
            let right_address = right_observed.result().await;
            assert_ne!(left_address, right_address);
            assert_ne!(left_address.port(), 0);
            assert_ne!(right_address.port(), 0);
            let rejected =
                callback_response(right_address, "/auth/callback?state=left-state&code=left").await;
            assert!(rejected.starts_with("HTTP/1.1 400 "));
            assert!(rejected.contains("State mismatch."));
            let accepted =
                callback_response(left_address, "/auth/callback?state=left-state&code=left").await;
            assert!(accepted.starts_with("HTTP/1.1 200 "));
            let crate::oauth::native::CallbackOutcome::Accepted(fields) =
                left.wait.next().await.unwrap()
            else {
                panic!("left callback must be accepted");
            };
            assert_eq!(fields.code.as_deref(), Some("left"));
            left.close().await;
            let accepted =
                callback_response(right_address, "/auth/callback?state=right-state&code=right")
                    .await;
            assert!(accepted.starts_with("HTTP/1.1 200 "));
            let crate::oauth::native::CallbackOutcome::Accepted(fields) =
                right.wait.next().await.unwrap()
            else {
                panic!("right callback must be accepted");
            };
            assert_eq!(fields.code.as_deref(), Some("right"));
            right.close().await;
        });
    }

    #[test]
    fn maestro_response_login_falls_back_after_bind_failure() {
        run(async {
            let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = occupied.local_addr().unwrap().port();
            let fetch = response(
                200,
                String::new(),
                vec![success_bytes("manual-account")],
                Effect::Normal,
            );
            let checked: Fetch = Arc::new(move |request| {
                let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
                    .into_owned()
                    .collect();
                assert_eq!(fields["code"], "pasted-code");
                fetch(request)
            });
            let credentials = codex::login_with(
                Interaction::new(None, "pasted-code", ""),
                None,
                checked,
                codex::authorization_flow,
                move |state| async move {
                    let (server, address) = bound_callback(
                        state,
                        port,
                        |_| None,
                        |listener| Box::pin(listener.accept()),
                    )
                    .await;
                    assert!(address.result().now_or_never().is_none());
                    Ok(server)
                },
            )
            .await
            .unwrap();
            assert_eq!(credentials.extra["accountId"], "manual-account");
            drop(occupied);
        });
    }

    #[test]
    fn maestro_response_callback_pages_preserve_status() {
        run(async {
            let callbacks = Arc::new(Browser {
                auth: crate::EventStream::new(|_| true, Clone::clone),
            });
            let auth = callbacks.auth.clone();
            let release = crate::EventStream::new(|()| true, |()| ());
            let gate = release.clone();
            let fetch: Fetch =
                Arc::new(move |request| Box::pin(released_fetch(gate.clone(), request)));
            let authorization = codex::authorization_flow(None).unwrap();
            let (server, observed) = bound_callback(
                authorization.1.clone(),
                0,
                |_| None,
                |listener| Box::pin(listener.accept()),
            )
            .await;
            let address = observed.result().await;
            let work = tokio::spawn(codex::login_with(
                callbacks,
                None,
                fetch,
                |_| Ok(authorization),
                |_| std::future::ready(Ok(server)),
            ));
            let info = auth.result().await;
            let url = url::Url::parse(&info.url).unwrap();
            let state = url.query_pairs().find(|(key, _)| key == "state").unwrap().1;
            check_callback_pages(address, &state).await;
            release.push(());
            assert_eq!(
                work.await.unwrap().unwrap().extra["accountId"],
                "callback-account"
            );
        });
    }
}
