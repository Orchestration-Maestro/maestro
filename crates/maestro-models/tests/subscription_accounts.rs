//! Subscription account authorization through caller-supplied HTTP effects.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Fetch, HttpResponse, refresh_anthropic_token};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn subscription_refresh_omits_scope() {
    crate::runtime::run(async {
        let fetch: Fetch = Arc::new(|request| {
            assert_eq!(request.method, "POST");
            assert_eq!(request.url, "https://platform.claude.com/v1/oauth/token");
            assert_eq!(request.headers["content-type"], "application/json");
            assert_eq!(request.headers["accept"], "application/json");
            assert_eq!(
                String::from_utf8(request.body).unwrap(),
                r#"{"grant_type":"refresh_token","client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e","refresh_token":"old"}"#
            );
            Box::pin(async {
                Ok(HttpResponse { status_text: String::new(), status: 200, headers: BTreeMap::new(), body: Box::pin(futures_util::stream::iter([Ok(br#"{"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":3600,"scope":"discard"}"#.to_vec())])) })
            })
        });
        let before = crate::clock::epoch_millis();
        let credentials = refresh_anthropic_token("old".into(), Some(fetch))
            .await
            .unwrap();
        let after = crate::clock::epoch_millis();
        assert_eq!(credentials.access, "rotated-access");
        assert_eq!(credentials.refresh, "rotated-refresh");
        assert!(credentials.extra.is_empty());
        assert!((before + 3_300_000.0..=after + 3_300_000.0).contains(&credentials.expires));
    });
}

#[cfg(not(target_arch = "wasm32"))]
/// Pasted authorization interaction.
struct Manual;
#[cfg(not(target_arch = "wasm32"))]
impl maestro_models::OAuthLoginCallbacks for Manual {
    fn callback_presence(&self) -> (bool, bool, bool) {
        (true, true, false)
    }
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

#[cfg(not(target_arch = "wasm32"))]
/// Fixed-port scenarios share an admission guard.
static PORT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[cfg(not(target_arch = "wasm32"))]
/// Callback-only interaction starts real loopback requests after receiving state.
struct Callback {
    /// Owned peer producer, observed after login returns.
    peer: maestro_models::EventStream<tokio::task::JoinHandle<tokio::net::TcpStream>, ()>,
}
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
/// Interaction that records the announced authorization address, then declines to continue.
#[derive(Default)]
struct Announcing {
    /// Address announced by the sign-in.
    url: std::sync::Mutex<Option<String>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl maestro_models::OAuthLoginCallbacks for Announcing {
    fn on_auth(
        &self,
        info: maestro_models::OAuthAuthInfo,
    ) -> Result<(), maestro_models::OAuthError> {
        *self.url.lock().unwrap() = Some(info.url);
        Err(maestro_models::DiagnosticErrorInfo::error("declined").into())
    }
    fn on_prompt(
        &self,
        _: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        panic!("declining the announcement must precede the prompt")
    }
}
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
/// Probe accepting-listener release with the native adapter's Unix reuse policy.
fn bind_callback_probe() -> tokio::net::TcpListener {
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    #[cfg(unix)]
    socket.set_reuseaddr(true).unwrap();
    socket.bind("127.0.0.1:53692".parse().unwrap()).unwrap();
    socket.listen(128).unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
/// Attempt a complete request write, stopping quietly if the peer has closed.
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
#[cfg(not(target_arch = "wasm32"))]
/// Read peer bytes after readiness, including the shutdown EOF.
///
/// A closed listener answers bytes written after its close with a reset, which some hosts,
/// macOS among them, report before the end of stream, and Windows may report as an aborted
/// connection, so either also reads as the end.
async fn read_peer(peer: &mut tokio::net::TcpStream, bytes: &mut [u8]) -> usize {
    loop {
        peer.readable().await.unwrap();
        match peer.try_read(bytes) {
            Ok(count) => return count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                return 0;
            }
            Err(error) => panic!("{error}"),
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
/// A successful JSON response carrying `body`.
fn json_response(body: &'static [u8]) -> HttpResponse {
    HttpResponse {
        status_text: String::new(),
        status: 200,
        headers: BTreeMap::new(),
        body: Box::pin(futures_util::stream::iter([Ok(body.to_vec())])),
    }
}

/// Tests that bind the fixed callback port live in an `oauth_callback` module.
mod oauth_callback {
    #[cfg(not(target_arch = "wasm32"))]
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn subscription_manual_login_uses_localhost_redirect() {
        crate::runtime::run(async {
            let _permit = PORT.acquire().await.unwrap();
            let fetch: Fetch = Arc::new(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["code"], "manual-code");
                assert_eq!(body["state"], body["code_verifier"]);
                assert_eq!(body["redirect_uri"], "http://localhost:53692/callback");
                Box::pin(std::future::ready(Ok(json_response(
                    br#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#,
                ))))
            });
            let result = maestro_models::login_anthropic(Arc::new(Manual), Some(fetch))
                .await
                .unwrap();
            assert_eq!(result.access, "a");
            assert_eq!(result.refresh, "r");
            let _released = bind_callback_probe();
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn subscription_callback_login_rejects_then_accepts() {
        crate::runtime::run(async {
            let _permit = PORT.acquire().await.unwrap();
            let fetch: Fetch = Arc::new(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["code"], "callback-code");
                Box::pin(std::future::ready(Ok(json_response(br#"{"access_token":"callback-access","refresh_token":"callback-refresh","expires_in":1}"#))))
            });
            let callbacks = Arc::new(Callback {
                peer: maestro_models::EventStream::new(|_| false, |_| ()),
            });
            let result = maestro_models::login_anthropic(callbacks.clone(), Some(fetch))
                .await
                .unwrap();
            let mut peer = callbacks.peer.next().await.unwrap().await.unwrap();
            // The completed response established admission on this same keep-alive peer.
            write_peer(
                &mut peer,
                b"GET /callback?code=fresh&state=wrong HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await;
            let mut byte = [0];
            assert_eq!(
                read_peer(&mut peer, &mut byte).await,
                0,
                "stopped listener served a fresh request"
            );
            assert_eq!(result.access, "callback-access");
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn subscription_provider_delegates_without_model_changes() {
        crate::runtime::run(async {
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
                Box::pin(std::future::ready(Ok(json_response(
                    br#"{"access_token":"key","refresh_token":"rotated","expires_in":1}"#,
                ))))
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
            assert!(provider.modify_models(&models, &credentials).is_none());
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn subscription_occupied_port_falls_back_to_a_free_port() {
        crate::runtime::run(async {
            let _permit = PORT.acquire().await.unwrap();
            let listener = bind_callback_probe();
            let callbacks = Arc::new(Announcing::default());
            let error = maestro_models::login_anthropic(callbacks.clone(), None)
                .await
                .unwrap_err();
            assert_eq!(error.to_string(), "declined");
            let announced = callbacks.url.lock().unwrap().clone().unwrap();
            let announced = url::Url::parse(&announced).unwrap();
            let redirect = announced
                .query_pairs()
                .find(|(key, _)| key == "redirect_uri")
                .unwrap()
                .1
                .into_owned();
            let redirect = url::Url::parse(&redirect).unwrap();
            assert_ne!(redirect.port(), Some(53692));
            assert!(redirect.port().is_some());
            drop(listener);
            let _released = bind_callback_probe();
        });
    }
}
