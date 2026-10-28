//! Subscription sign-in through the host's HTTP and interaction interfaces.
use maestro_extensions_wasm::{
    AbortSignal, ExtensionFuture, ExtensionResult, Model, OAuthAuthInfo, OAuthCredentials,
    OAuthLoginCallbacks, OAuthPrompt, OAuthProviderInterface,
    http::{Request, Response},
    io::NativeIo,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::rc::Rc;

/// The registered subscription client's public identifier.
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Interactive authorization endpoint.
const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
/// Authorization-code exchange and refresh endpoint.
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
/// The provider's hosted authorization-code page.
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

/// Provider effects retained independently of the registration callback.
#[derive(Clone)]
pub struct CustomAnthropic {
    /// The host's independently retained HTTP operations.
    pub io: Rc<dyn NativeIo>,
    /// Secure verifier bytes, supplied by WASI in the component.
    pub entropy: Rc<dyn Fn() -> ExtensionResult<[u8; 32]>>,
    /// Current Unix epoch milliseconds.
    pub now: Rc<dyn Fn() -> f64>,
}

/// Token fields accepted from a successful exchange.
#[derive(Deserialize)]
struct Tokens {
    /// Bearer credential for model requests.
    access_token: String,
    /// Credential for the next rotation.
    refresh_token: String,
    /// Lifetime in seconds from the provider.
    expires_in: f64,
}

impl OAuthProviderInterface for CustomAnthropic {
    fn id(&self) -> &'static str {
        "custom-anthropic"
    }
    fn name(&self) -> &'static str {
        "Custom Anthropic (Claude Pro/Max)"
    }
    fn uses_callback_server(&self) -> Option<bool> {
        None
    }
    fn login(
        &self,
        callbacks: Rc<OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let provider = self.clone();
        Box::pin(async move {
            let (verifier, challenge) = super::pkce::generate((provider.entropy)()?);
            (callbacks.on_auth)(OAuthAuthInfo {
                url: authorization_url(&verifier, &challenge),
                instructions: None,
            })?;
            let pasted = (callbacks.on_prompt)(OAuthPrompt {
                message: "Paste the authorization code:".into(),
                placeholder: None,
                allow_empty: None,
            })
            .await?;
            let (code, state) = pasted
                .split_once('#')
                .ok_or("Authorization code must include state")?;
            if state != verifier {
                return Err("Authorization state mismatch".into());
            }
            provider
                .exchange(
                    json!({
                        "grant_type":"authorization_code", "client_id":CLIENT_ID,
                        "code":code, "state":state, "redirect_uri":REDIRECT_URI,
                        "code_verifier":verifier
                    }),
                    callbacks.signal.clone(),
                    "exchange",
                )
                .await
        })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let provider = self.clone();
        Box::pin(async move {
            provider
                .exchange(
                    json!({
                        "grant_type":"refresh_token", "client_id":CLIENT_ID,
                        "refresh_token":credentials.refresh
                    }),
                    None,
                    "refresh",
                )
                .await
        })
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

impl CustomAnthropic {
    /// Performs one token exchange and subtracts the five-minute refresh margin.
    async fn exchange(
        &self,
        body: Value,
        signal: Option<AbortSignal>,
        phase: &str,
    ) -> ExtensionResult<OAuthCredentials> {
        let response = self
            .io
            .fetch(Request {
                url: TOKEN_URL.into(),
                method: Some("POST".into()),
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: Some(serde_json::to_vec(&body).map_err(|error| error.to_string())?),
                signal,
            })
            .await?;
        if !response.ok() {
            return Err(format!(
                "Token {phase} failed: {}",
                failure(&*response).await
            ));
        }
        let tokens: Tokens =
            serde_json::from_value(response.json().await?).map_err(|error| error.to_string())?;
        Ok(OAuthCredentials {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires: (self.now)() + tokens.expires_in * 1000.0 - 5.0 * 60.0 * 1000.0,
            extra: serde_json::Map::new(),
        })
    }
}

/// Names a failed response by its status, then its body text or the error that stopped the read.
pub async fn failure(response: &dyn Response) -> String {
    let detail = response.text().await.unwrap_or_else(|error| error);
    format!("HTTP {} {detail}", response.status())
}

/// Encodes the fixed authorization parameters beside the URL-safe proof key.
fn authorization_url(verifier: &str, challenge: &str) -> String {
    format!(
        concat!(
            "{}?code=true&client_id={}&response_type=code&",
            "redirect_uri=https%3A%2F%2Fconsole.anthropic.com%2Foauth%2Fcode%2Fcallback&",
            "scope=org%3Acreate_api_key+user%3Aprofile+user%3Ainference&",
            "code_challenge={}&code_challenge_method=S256&state={}"
        ),
        AUTHORIZE_URL, CLIENT_ID, challenge, verifier
    )
}
