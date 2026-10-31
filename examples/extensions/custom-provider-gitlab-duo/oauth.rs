//! GitLab sign-in and token refresh through the host's HTTP and interaction interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::models::GITLAB_COM_URL;
use super::provider::{GitLabDuo, body_text};
use maestro_extensions_wasm::{
    ExtensionFuture, ExtensionResult, Model, OAuthAuthInfo, OAuthCredentials, OAuthLoginCallbacks,
    OAuthPrompt, OAuthProviderInterface, generate_pkce, http::Request,
};
use serde::Deserialize;
use std::{fmt::Write as _, rc::Rc};

/// The client identifier GitLab issued for the bundled application.
const BUNDLED_CLIENT_ID: &str = "da4edff2e6ebd2bc3208611e2768bc1c1dd7be791dc5ff26ca34ca9ee44f7d4b";
/// The scopes the sign-in requests.
const OAUTH_SCOPES: [&str; 1] = ["api"];
/// Where GitLab sends the browser after authorization.
const REDIRECT_URI: &str = "http://127.0.0.1:8080/callback";

/// Token fields accepted from a successful exchange.
#[derive(Deserialize)]
struct Tokens {
    /// Bearer credential for GitLab requests.
    access_token: String,
    /// Credential for the next rotation.
    refresh_token: String,
    /// Lifetime in seconds.
    expires_in: f64,
    /// Issue time in Unix epoch seconds, as GitLab reports it.
    created_at: f64,
}

impl OAuthProviderInterface for GitLabDuo {
    fn id(&self) -> &'static str {
        "gitlab-duo"
    }
    fn name(&self) -> &'static str {
        "GitLab Duo"
    }
    fn uses_callback_server(&self) -> Option<bool> {
        None
    }
    fn login(
        &self,
        callbacks: Rc<OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let provider = self.clone();
        Box::pin(async move { provider.login_gitlab(&callbacks).await })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let provider = self.clone();
        Box::pin(async move { provider.refresh_gitlab_token(&credentials).await })
    }
    fn modify_models(
        &self,
        _: &[Model],
        _: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>> {
        Ok(None)
    }
    fn get_api_key(&self, credentials: &OAuthCredentials) -> ExtensionResult<String> {
        Ok(credentials.access.clone())
    }
}

impl GitLabDuo {
    /// Authorizes with a proof key: the user pastes the callback URL GitLab redirected to.
    ///
    /// # Errors
    /// Returns a callback without a code or with another state, or the failed exchange.
    pub async fn login_gitlab(
        &self,
        callbacks: &OAuthLoginCallbacks,
    ) -> ExtensionResult<OAuthCredentials> {
        let pkce = generate_pkce(&self.api)?;
        let state = uuid_v4((self.random)()?);
        let query = form(&[
            ("client_id", BUNDLED_CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("response_type", "code"),
            ("scope", &OAUTH_SCOPES.join(" ")),
            ("code_challenge", &pkce.challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
        ]);
        (callbacks.on_auth)(OAuthAuthInfo {
            url: format!("{GITLAB_COM_URL}/oauth/authorize?{query}"),
            instructions: None,
        })?;
        let callback_url = (callbacks.on_prompt)(OAuthPrompt {
            message: "Paste the callback URL:".into(),
            placeholder: None,
            allow_empty: None,
        })
        .await?;
        let code = authorization_code(&callback_url, &state)?;
        self.exchange(
            &[
                ("client_id", BUNDLED_CLIENT_ID),
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("code_verifier", &pkce.verifier),
                ("redirect_uri", REDIRECT_URI),
            ],
            "Token exchange failed",
        )
        .await
    }

    /// Rotates the credentials with their refresh token.
    ///
    /// # Errors
    /// Returns the failed refresh with GitLab's response.
    pub async fn refresh_gitlab_token(
        &self,
        credentials: &OAuthCredentials,
    ) -> ExtensionResult<OAuthCredentials> {
        self.exchange(
            &[
                ("client_id", BUNDLED_CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", &credentials.refresh),
            ],
            "Token refresh failed",
        )
        .await
    }

    /// Posts one token request, then expires the direct access token minted for the old credentials.
    async fn exchange(
        &self,
        fields: &[(&str, &str)],
        failure: &str,
    ) -> ExtensionResult<OAuthCredentials> {
        let response = self
            .io
            .fetch(Request {
                url: format!("{GITLAB_COM_URL}/oauth/token"),
                method: Some("POST".into()),
                headers: vec![(
                    "Content-Type".into(),
                    "application/x-www-form-urlencoded".into(),
                )],
                body: Some(form(fields).into_bytes()),
                signal: None,
            })
            .await?;
        if !response.ok() {
            return Err(format!("{failure}: {}", body_text(&*response).await));
        }
        let tokens: Tokens =
            serde_json::from_value(response.json().await?).map_err(|error| error.to_string())?;
        self.invalidate_direct_access_token();
        Ok(OAuthCredentials {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires: (tokens.created_at + tokens.expires_in) * 1000.0 - 5.0 * 60.0 * 1000.0,
            extra: serde_json::Map::new(),
        })
    }
}

/// Reads the first `code` and `state` of the pasted URL's query, and checks the state.
fn authorization_code(callback_url: &str, expected_state: &str) -> ExtensionResult<String> {
    let query = callback_url.split_once('?').map_or("", |(_, rest)| rest);
    let query = query.split('#').next().unwrap_or_default();
    let first = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
            .find(|(key, _)| decode(key) == name)
            .map(|(_, value)| decode(value))
    };
    let code = first("code")
        .filter(|code| !code.is_empty())
        .ok_or("No authorization code found in callback URL")?;
    if first("state").as_deref() != Some(expected_state) {
        return Err("Authorization state mismatch".into());
    }
    Ok(code)
}

/// Serializes name-value pairs as a form does.
fn form(fields: &[(&str, &str)]) -> String {
    let mut encoded = String::new();
    for (name, value) in fields {
        if !encoded.is_empty() {
            encoded.push('&');
        }
        escape(name, &mut encoded);
        encoded.push('=');
        escape(value, &mut encoded);
    }
    encoded
}

/// Appends one form component: unreserved bytes stay, a space becomes `+`, the rest `%XX`.
fn escape(text: &str, encoded: &mut String) {
    for byte in text.bytes() {
        match byte {
            b'*' | b'-' | b'.' | b'_' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
}

/// Reads one form component: `+` is a space and `%XX` is a byte; a stray `%` stays as written.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let (mut decoded, mut at) = (Vec::with_capacity(bytes.len()), 0);
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| bytes.get(at + 1..at + 3).and_then(hex_byte))
            .flatten();
        match (bytes[at], escaped) {
            (_, Some(byte)) => {
                decoded.push(byte);
                at += 2;
            }
            (b'+', None) => decoded.push(b' '),
            (byte, None) => decoded.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// The byte two hexadecimal digits name.
fn hex_byte(digits: &[u8]) -> Option<u8> {
    let digit = |byte: &u8| char::from(*byte).to_digit(16);
    let [high, low] = digits else { return None };
    u8::try_from(digit(high)? * 16 + digit(low)?).ok()
}

/// Formats 16 random bytes as a version 4 UUID.
fn uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut hex = String::new();
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
