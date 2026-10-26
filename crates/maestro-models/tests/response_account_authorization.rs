//! Controlled response-account authorization through supplied effects.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use futures_util::FutureExt as _;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Fetch, HttpResponse, refresh_openai_codex_token};
#[cfg(not(target_arch = "wasm32"))]
use std::{collections::BTreeMap, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_response_refresh_keeps_stderr_clean() {
    if !maestro_test_process::selection().is_child() {
        crate::runtime::run(async {
            let output = maestro_test_process::TestProcess::current(
                "maestro_response_refresh_keeps_stderr_clean",
            )
            .unwrap()
            .nocapture()
            .case("*")
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        });
        return;
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let body = r#"{"error":{"message":"Could not validate your token. Please try signing in again.","type":"invalid_request_error"}}"#;
        let fetch: Fetch = Arc::new(move |_| Box::pin(async move {
            Ok(HttpResponse { status: 401, status_text: "Unauthorized".into(), headers: BTreeMap::new(), body: Box::pin(futures_util::stream::iter([Ok(body.as_bytes().to_vec())])) })
        }));
        assert_eq!(refresh_openai_codex_token("invalid-refresh-token".into(), Some(fetch)).await.unwrap_err().to_string(), format!("OpenAI Codex token refresh failed (401): {body}"));
    });
}

#[cfg(not(target_arch = "wasm32"))]
/// Token response with supplied account metadata.
fn token_fetch(account: &str) -> Fetch {
    use base64::Engine as _;
    let payload =
        serde_json::json!({"https://api.openai.com/auth": {"chatgpt_account_id": account}});
    let access = format!(
        "h.{}.s",
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap())
    );
    let body = serde_json::json!({"access_token": access, "refresh_token": "rotated", "expires_in": 0.5, "discard": true}).to_string();
    Arc::new(move |_| {
        let body = body.clone();
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: BTreeMap::new(),
                body: Box::pin(futures_util::stream::iter([Ok(body.into_bytes())])),
            })
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Interaction that recovers with a pasted code.
struct Manual;
#[cfg(not(target_arch = "wasm32"))]
impl maestro_models::OAuthLoginCallbacks for Manual {
    fn on_auth(
        &self,
        info: maestro_models::OAuthAuthInfo,
    ) -> Result<(), maestro_models::OAuthError> {
        assert_eq!(
            info.instructions.as_deref(),
            Some("A browser window should open. Complete login to finish.")
        );
        Ok(())
    }
    fn on_prompt(
        &self,
        prompt: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        assert_eq!(
            prompt.message,
            "Paste the authorization code (or full redirect URL):"
        );
        assert_eq!(prompt.placeholder, None);
        assert_eq!(prompt.allow_empty, None);
        Box::pin(async { Ok("pasted-code".into()) })
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Protocol-aware manual interaction retaining the actual authorization URL.
struct Protocol {
    /// Expected explicit or default originator.
    originator: &'static str,
    /// Auth notification retained for the token-request witness.
    auth: maestro_models::EventStream<maestro_models::OAuthAuthInfo, maestro_models::OAuthAuthInfo>,
}
#[cfg(not(target_arch = "wasm32"))]
impl maestro_models::OAuthLoginCallbacks for Protocol {
    fn callback_presence(&self) -> (bool, bool, bool) {
        (false, true, false)
    }
    fn on_auth(
        &self,
        info: maestro_models::OAuthAuthInfo,
    ) -> Result<(), maestro_models::OAuthError> {
        let url = url::Url::parse(&info.url).unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            "https://auth.openai.com"
        );
        assert_eq!(url.path(), "/oauth/authorize");
        assert_eq!(
            url.query_pairs()
                .map(|(key, _)| key.into_owned())
                .collect::<Vec<_>>(),
            [
                "response_type",
                "client_id",
                "redirect_uri",
                "scope",
                "code_challenge",
                "code_challenge_method",
                "state",
                "id_token_add_organizations",
                "codex_cli_simplified_flow",
                "originator"
            ]
        );
        let fields: BTreeMap<_, _> = url.query_pairs().collect();
        assert_eq!(fields["response_type"], "code");
        assert_eq!(fields["client_id"], "app_EMoamEEZ73f0CkXaXp7hrann");
        assert_eq!(
            fields["redirect_uri"],
            "http://localhost:1455/auth/callback"
        );
        assert_eq!(fields["scope"], "openid profile email offline_access");
        assert_eq!(fields["code_challenge_method"], "S256");
        assert_eq!(fields["id_token_add_organizations"], "true");
        assert_eq!(fields["codex_cli_simplified_flow"], "true");
        assert_eq!(fields["originator"], self.originator);
        assert_eq!(fields["state"].len(), 32);
        assert!(
            fields["state"]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        assert_eq!(
            info.instructions.as_deref(),
            Some("A browser window should open. Complete login to finish.")
        );
        self.auth.push(info);
        Ok(())
    }
    fn on_prompt(
        &self,
        _: maestro_models::OAuthPrompt,
    ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
        panic!("manual code avoids prompt")
    }
    fn on_manual_code_input(
        &self,
    ) -> Option<maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>>> {
        let info = self.auth.clone();
        Some(Box::pin(async move {
            let url = url::Url::parse(&info.result().await.url).unwrap();
            let state = url
                .query_pairs()
                .find(|(name, _)| name == "state")
                .unwrap()
                .1;
            Ok(format!("protocol-code#{state}"))
        }))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Write bytes after readiness, without fixed delays.
async fn write_native(peer: &tokio::net::TcpStream, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        peer.writable().await.unwrap();
        match peer.try_write(bytes) {
            Ok(count) => {
                assert_ne!(count, 0);
                bytes = &bytes[count..];
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Read the request body using the client's framing, keeping received bytes intact.
async fn read_request(peer: &tokio::net::TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        peer.readable().await.unwrap();
        let mut buffer = [0; 4096];
        match peer.try_read(&mut buffer) {
            Ok(0) => panic!("request ended before its body"),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(error) => panic!("{error}"),
        }
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            if bytes.len() == end + 4 + length {
                return bytes;
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn response_default_fetch_reads_complete_loopback_response() {
    crate::runtime::run(async {
        for reason in [Some("Odd Reason"), Some(""), None] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(loopback_token(listener, reason));
            let default = maestro_models::default_fetch();
            let fetch: Fetch = Arc::new(move |mut request| {
                request.url = format!("http://{address}/token");
                default(request)
            });
            let result = refresh_openai_codex_token("local +&=é".into(), Some(fetch)).await;
            if let Some(reason) = reason {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    format!("OpenAI Codex token refresh failed (401): {reason}")
                );
            } else {
                let credentials = result.unwrap();
                assert_eq!(credentials.extra["accountId"], "loopback-account");
                assert_eq!(credentials.refresh, "fragmented-é猫");
            }
            server.await.unwrap();
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
/// Produce one complete fragmented reply after reading the actual default client's POST.
async fn loopback_token(listener: tokio::net::TcpListener, reason: Option<&str>) {
    use base64::Engine as _;
    let (peer, _) = listener.accept().await.unwrap();
    let request = read_request(&peer).await;
    let request = std::str::from_utf8(&request).unwrap();
    let (headers, body) = request.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("POST /token HTTP/1.1\r\n"));
    assert!(headers.contains("content-type: application/x-www-form-urlencoded"));
    assert_eq!(
        body,
        "grant_type=refresh_token&refresh_token=local+%2B%26%3D%C3%A9&client_id=app_EMoamEEZ73f0CkXaXp7hrann"
    );
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"loopback-account"}}"#);
    let body = if reason.is_some() {
        String::new()
    } else {
        format!(
            "{{\"access_token\":\"h.{payload}.s\",\"refresh_token\":\"fragmented-é猫\",\"expires_in\":1}}"
        )
    };
    let status = reason.map_or("200 OK".into(), |reason| format!("401 {reason}"));
    write_native(
        &peer,
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    )
    .await;
    for byte in body.as_bytes() {
        write_native(&peer, std::slice::from_ref(byte)).await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Run public fallback assertions in a checked child without opening a callback listener.
fn public_fallback_child(test: &str) -> bool {
    if std::env::var("MAESTRO_OAUTH_CALLBACK_HOST").as_deref() == Ok(" ") {
        return false;
    }
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .env("MAESTRO_OAUTH_CALLBACK_HOST", " ")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    true
}

/// Public callback checks exercise manual fallback in an isolated process.
mod oauth_callback {
    #[cfg(not(target_arch = "wasm32"))]
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maestro_response_tokens_require_account_claim() {
        if public_fallback_child("maestro_response_tokens_require_account_claim") {
            return;
        }
        crate::runtime::run(maestro_response_tokens_require_account_claim_flow());
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn maestro_response_tokens_require_account_claim_flow() {
        for (account, operation) in ["é猫😀", " ", ""]
            .into_iter()
            .flat_map(|account| [(account, "login"), (account, "refresh")])
        {
            let result = if operation == "login" {
                maestro_models::login_openai_codex(
                    Arc::new(Manual),
                    None,
                    Some(token_fetch(account)),
                )
                .await
            } else {
                refresh_openai_codex_token("old".into(), Some(token_fetch(account))).await
            };
            if account.is_empty() {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    "Failed to extract accountId from token"
                );
            } else {
                let credentials = result.unwrap();
                assert_eq!(
                    credentials.extra,
                    [(
                        "accountId".into(),
                        serde_json::Value::String(account.into())
                    )]
                    .into_iter()
                    .collect()
                );
                assert_eq!(credentials.refresh, "rotated");
                assert_eq!(credentials.access.split('.').count(), 3);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn response_provider_delegates_and_borrows_access_key() {
        if public_fallback_child("response_provider_delegates_and_borrows_access_key") {
            return;
        }
        crate::runtime::run(async {
            let provider = maestro_models::OPENAI_CODEX_OAUTH_PROVIDER;
            assert_eq!(
                (
                    provider.id(),
                    provider.name(),
                    provider.uses_callback_server()
                ),
                (
                    "openai-codex",
                    "ChatGPT Plus/Pro (Codex Subscription)",
                    Some(true)
                )
            );
            let credentials = provider
                .login(
                    Arc::new(Protocol {
                        originator: "maestro",
                        auth: maestro_models::EventStream::new(|_| true, Clone::clone),
                    }),
                    Some(token_fetch("provider-account")),
                )
                .await
                .unwrap();
            let access = provider.get_api_key(&credentials).unwrap();
            assert!(std::ptr::eq(access.as_ptr(), credentials.access.as_ptr()));
            let checked: Fetch = Arc::new(|request| {
                let fields: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body)
                    .into_owned()
                    .collect();
                assert_eq!(fields["refresh_token"], "rotated");
                token_fetch("refreshed-account")(request)
            });
            let rotated = provider
                .refresh_token(credentials, Some(checked))
                .await
                .unwrap();
            assert_eq!(rotated.extra["accountId"], "refreshed-account");
            let models = vec![
                maestro_models::get_model("google", "gemini-2.5-flash").unwrap(),
                maestro_models::get_model("anthropic", "claude-sonnet-4-5").unwrap(),
            ];
            assert!(provider.modify_models(&models, &rotated).is_none());
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn maestro_response_authorization_keeps_protocol_fields() {
        if public_fallback_child("maestro_response_authorization_keeps_protocol_fields") {
            return;
        }
        crate::runtime::run(maestro_response_authorization_keeps_protocol_fields_flow());
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn maestro_response_authorization_keeps_protocol_fields_flow() {
        for originator in [None, Some(""), Some("x +&=😀")] {
            let callbacks = Arc::new(Protocol {
                originator: originator.unwrap_or("maestro"),
                auth: maestro_models::EventStream::new(|_| true, Clone::clone),
            });
            let auth = callbacks.auth.clone();
            let fetch: Fetch = Arc::new(move |request| {
                use base64::Engine as _;
                use sha2::{Digest as _, Sha256};
                let info = auth.result().now_or_never().unwrap();
                let url = url::Url::parse(&info.url).unwrap();
                let fields: BTreeMap<_, _> = url.query_pairs().collect();
                let form: BTreeMap<_, _> = url::form_urlencoded::parse(&request.body).collect();
                assert_eq!(form["code"], "protocol-code");
                assert_eq!(form["code_verifier"].len(), 43);
                assert_ne!(form["code_verifier"], fields["state"]);
                let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(Sha256::digest(form["code_verifier"].as_bytes()));
                assert_eq!(challenge, fields["code_challenge"]);
                assert_eq!(form["redirect_uri"], fields["redirect_uri"]);
                token_fetch("protocol-account")(request)
            });
            let credentials = maestro_models::login_openai_codex(
                callbacks,
                originator.map(str::to_owned),
                Some(fetch),
            )
            .await
            .unwrap();
            assert_eq!(credentials.extra["accountId"], "protocol-account");
        }
    }
}
