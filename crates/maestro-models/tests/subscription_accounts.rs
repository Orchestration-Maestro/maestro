//! Subscription account authorization through caller-supplied HTTP effects.
#![cfg(test)]
use maestro_models::{Fetch, HttpResponse, refresh_anthropic_token};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn subscription_refresh_omits_scope() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
    let fetch: Fetch = Arc::new(|request| {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://platform.claude.com/v1/oauth/token");
        assert_eq!(request.headers["content-type"], "application/json");
        assert_eq!(request.headers["accept"], "application/json");
        assert_eq!(String::from_utf8(request.body).unwrap(), r#"{"grant_type":"refresh_token","client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e","refresh_token":"old"}"#);
        Box::pin(async { Ok(HttpResponse { status: 200, headers: BTreeMap::new(), body: Box::pin(futures_util::stream::iter([Ok(br#"{"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":3600,"scope":"discard"}"#.to_vec())])) }) })
    });
    let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0;
    let credentials = refresh_anthropic_token("old".into(), Some(fetch)).await.unwrap();
    let after = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0;
    assert_eq!(credentials.access, "rotated-access");
    assert_eq!(credentials.refresh, "rotated-refresh");
    assert!(credentials.extra.is_empty());
    assert!((before.floor() + 3_300_000.0..=after.floor() + 3_300_000.0).contains(&credentials.expires));
    });
}

/// Pasted authorization interaction.
struct Manual;
impl maestro_models::OAuthLoginCallbacks for Manual {
    fn on_auth(
        &self,
        info: maestro_models::OAuthAuthInfo,
    ) -> Result<(), maestro_models::OAuthError> {
        let url = url::Url::parse(&info.url).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://claude.ai");
        assert_eq!(url.path(), "/oauth/authorize");
        let keys: Vec<_> = url.query_pairs().map(|(key, _)| key.into_owned()).collect();
        assert_eq!(
            keys,
            [
                "code",
                "client_id",
                "response_type",
                "redirect_uri",
                "scope",
                "code_challenge",
                "code_challenge_method",
                "state"
            ]
        );
        let fields: BTreeMap<_, _> = url.query_pairs().collect();
        assert_eq!(fields["redirect_uri"], "http://localhost:53692/callback");
        assert_eq!(fields["code"], "true");
        assert_eq!(fields["client_id"], "9d1c250a-e61b-44d9-88ed-5944d1962f5e");
        assert_eq!(
            fields["scope"],
            "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload"
        );
        assert_eq!(fields["response_type"], "code");
        assert_eq!(fields["code_challenge_method"], "S256");
        assert_eq!(fields["state"].len(), 43);
        assert_ne!(fields["state"], fields["code_challenge"]);
        assert_eq!(
            info.instructions.as_deref(),
            Some(
                "Complete login in your browser. If the browser is on another machine, paste the final redirect URL here."
            )
        );
        Ok(())
    }
    fn on_prompt(
        &self,
        _: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        panic!("manual code must avoid prompt")
    }
    fn on_manual_code_input(
        &self,
    ) -> Option<maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>>> {
        Some(Box::pin(async { Ok("manual-code".into()) }))
    }
    fn on_progress(&self, message: &str) -> Result<(), maestro_models::OAuthError> {
        assert_eq!(message, "Exchanging authorization code for tokens...");
        Ok(())
    }
}

/// Fixed-port scenarios share an admission guard.
static PORT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[test]
fn subscription_manual_login_uses_localhost_redirect() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let _permit = PORT.acquire().await.unwrap();
            let fetch: Fetch = Arc::new(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["code"], "manual-code");
                assert_eq!(body["state"], body["code_verifier"]);
                assert_eq!(body["redirect_uri"], "http://localhost:53692/callback");
                Box::pin(async {
                    Ok(HttpResponse {
                        status: 200,
                        headers: BTreeMap::new(),
                        body: Box::pin(futures_util::stream::iter([Ok(
                            br#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#
                                .to_vec(),
                        )])),
                    })
                })
            });
            let result = maestro_models::login_anthropic(Arc::new(Manual), Some(fetch))
                .await
                .unwrap();
            assert_eq!(result.access, "a");
            assert_eq!(result.refresh, "r");
            let _released = bind_callback_probe();
        });
}

/// Callback-only interaction starts real loopback requests after receiving state.
struct Callback {
    /// Owned peer producer, observed after login returns.
    peer: maestro_models::EventStream<tokio::task::JoinHandle<tokio::net::TcpStream>, ()>,
}
impl maestro_models::OAuthLoginCallbacks for Callback {
    fn on_auth(
        &self,
        info: maestro_models::OAuthAuthInfo,
    ) -> Result<(), maestro_models::OAuthError> {
        let url = url::Url::parse(&info.url).unwrap();
        let state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .unwrap()
            .1
            .into_owned();
        let peer = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let rejected = client
                .get("http://127.0.0.1:53692/callback?error=%3Cscript%3E%26%22%27")
                .send()
                .await
                .unwrap();
            assert_eq!(rejected.status(), 400);
            assert_eq!(
                rejected.headers()["content-type"],
                "text/html; charset=utf-8"
            );
            assert!(
                rejected
                    .text()
                    .await
                    .unwrap()
                    .contains("Error: &lt;script&gt;&amp;&quot;&#39;")
            );
            let mut connection = tokio::net::TcpStream::connect("127.0.0.1:53692")
                .await
                .unwrap();
            let accepted = raw_callback(
                &mut connection,
                &format!("/callback?code=callback-code&state={state}"),
            )
            .await;
            assert!(accepted.starts_with("HTTP/1.1 200"));
            assert!(
                accepted.contains("Anthropic authentication completed. You can close this window.")
            );
            connection
        });
        self.peer.push(peer);
        Ok(())
    }
    fn on_prompt(
        &self,
        _: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        panic!("valid callback must avoid prompt")
    }
}
#[test]
fn subscription_callback_login_rejects_then_accepts() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let _permit = PORT.acquire().await.unwrap();
        let fetch: Fetch = Arc::new(|request| {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["code"], "callback-code");
            Box::pin(async { Ok(HttpResponse { status: 200, headers: BTreeMap::new(), body: Box::pin(futures_util::stream::iter([Ok(br#"{"access_token":"callback-access","refresh_token":"callback-refresh","expires_in":1}"#.to_vec())])) }) })
        });
        let callbacks = Arc::new(Callback { peer: maestro_models::EventStream::new(|_| false, |_| ()) });
        let result = maestro_models::login_anthropic(callbacks.clone(), Some(fetch)).await.unwrap();
        let mut peer = callbacks.peer.next().await.unwrap().await.unwrap();
        // The completed response established admission on this same keep-alive peer.
        write_peer(&mut peer, b"GET /callback?code=fresh&state=wrong HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
        let mut byte = [0];
        assert_eq!(read_peer(&mut peer, &mut byte).await, 0, "stopped listener served a fresh request");
        assert_eq!(result.access, "callback-access");
    });
}
#[test]
fn subscription_provider_delegates_without_model_changes() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let _permit = PORT.acquire().await.unwrap();
            let provider = maestro_models::ANTHROPIC_OAUTH_PROVIDER;
            assert_eq!(provider.id(), "anthropic");
            assert_eq!(provider.name(), "Anthropic (Claude Pro/Max)");
            assert_eq!(provider.uses_callback_server(), Some(true));
            let fetch: Fetch = Arc::new(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                assert!(matches!(
                    body["grant_type"].as_str(),
                    Some("authorization_code" | "refresh_token")
                ));
                Box::pin(async {
                    Ok(HttpResponse {
                        status: 200,
                        headers: BTreeMap::new(),
                        body: Box::pin(futures_util::stream::iter([Ok(
                            br#"{"access_token":"key","refresh_token":"rotated","expires_in":1}"#
                                .to_vec(),
                        )])),
                    })
                })
            });
            let credentials = provider
                .login(Arc::new(Manual), Some(fetch.clone()))
                .await
                .unwrap();
            let key = provider.get_api_key(&credentials).unwrap();
            assert_eq!(key.as_ptr(), credentials.access.as_ptr());
            let mut models = maestro_models::get_models("openai");
            models.truncate(2);
            assert_eq!(models.len(), 2);
            models[0].name = "first custom".into();
            models[1].name = "second custom".into();
            let allocation = models.as_ptr();
            let result = provider.modify_models(models, &credentials).unwrap();
            assert_eq!(result.as_ptr(), allocation);
            assert_eq!(result[0].name, "first custom");
            assert_eq!(result[1].name, "second custom");
            assert_eq!(
                provider
                    .refresh_token(credentials, Some(fetch))
                    .await
                    .unwrap()
                    .refresh,
                "rotated"
            );
        });
}
/// Interaction which must never run after native binding failure.
struct Uncalled;
impl maestro_models::OAuthLoginCallbacks for Uncalled {
    fn on_auth(&self, _: maestro_models::OAuthAuthInfo) -> Result<(), maestro_models::OAuthError> {
        panic!("binding failure must precede auth")
    }
    fn on_prompt(
        &self,
        _: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        panic!("binding failure must precede prompt")
    }
}
#[test]
fn subscription_occupied_port_reports_native_error() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let _permit = PORT.acquire().await.unwrap();
            let listener = bind_callback_probe();
            let error = maestro_models::login_anthropic(Arc::new(Uncalled), None)
                .await
                .unwrap_err();
            assert!(error.errno.is_some());
            drop(listener);
            let _released = bind_callback_probe();
        });
}
#[test]
fn subscription_default_client_reads_loopback_body() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || serve_token_peer(&listener));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let client = maestro_models::default_fetch();
            let fetch: Fetch = Arc::new(move |mut request| {
                request.url = format!("http://{address}/token");
                client(request)
            });
            let credentials = refresh_anthropic_token("loopback-old".into(), Some(fetch))
                .await
                .unwrap();
            assert_eq!(credentials.access, "local");
            assert_eq!(credentials.refresh, "local-refresh");
        });
    server.join().unwrap();
}

/// Probe accepting-listener release with the native adapter's Unix reuse policy.
fn bind_callback_probe() -> tokio::net::TcpListener {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    #[cfg(unix)]
    socket.set_reuseaddr(true).unwrap();
    socket.bind("127.0.0.1:53692".parse().unwrap()).unwrap();
    socket.listen(128).unwrap()
}

/// Read a complete loopback request and write deliberately fragmented response bytes.
fn serve_token_peer(listener: &std::net::TcpListener) {
    use std::io::{Read as _, Write as _};
    let (mut connection, _) = listener.accept().unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = connection.read(&mut buffer).unwrap();
        assert_ne!(count, 0);
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            if bytes.len() >= end + 4 + length {
                break;
            }
        }
    }
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("\"refresh_token\":\"loopback-old\"")
    );
    let body = br#"{"access_token":"local","refresh_token":"local-refresh","expires_in":2}"#;
    write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    for byte in body {
        connection.write_all(&[*byte]).unwrap();
    }
}

/// Write a complete request without relying on buffering helpers.
async fn write_peer(peer: &mut tokio::net::TcpStream, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        peer.writable().await.unwrap();
        match peer.try_write(bytes) {
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => return,
            Err(error) => panic!("{error}"),
        }
    }
}
/// Read peer bytes after readiness, including the shutdown EOF.
async fn read_peer(peer: &mut tokio::net::TcpStream, bytes: &mut [u8]) -> usize {
    loop {
        peer.readable().await.unwrap();
        match peer.try_read(bytes) {
            Ok(count) => return count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
}
/// Read one complete HTTP response while keeping the admitted connection open.
async fn raw_callback(peer: &mut tokio::net::TcpStream, target: &str) -> String {
    write_peer(
        peer,
        format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes(),
    )
    .await;
    let mut response = Vec::new();
    loop {
        let mut byte = [0];
        assert_eq!(read_peer(peer, &mut byte).await, 1);
        response.push(byte[0]);
        if let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&response[..end]).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_owned)
                })
                .unwrap()
                .parse()
                .unwrap();
            if response.len() == end + 4 + length {
                return String::from_utf8(response).unwrap();
            }
        }
    }
}
