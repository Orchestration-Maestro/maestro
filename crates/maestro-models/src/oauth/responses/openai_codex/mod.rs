//! Response-account authorization with caller-owned interaction.
mod callback;
mod token;

use crate::oauth::callback::parse_authorization_input;
use crate::oauth::native::{CallbackOutcome, CallbackServer};
use crate::{
    BoxFuture, EventStream, Fetch, OAuthAuthInfo, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthPrompt, Pkce,
};
#[cfg(not(target_arch = "wasm32"))]
use futures_util::FutureExt as _;
use std::fmt::Write as _;

/// Redirect identity independent of the listener host.
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";

/// Authorize response-account credentials through callback or pasted input.
///
/// # Errors
/// Returns entropy, interaction, token or account-extraction failures;
/// unsupported browser serving fails before interaction.
pub fn login_openai_codex(
    callbacks: OAuthCallbacks,
    originator: Option<String>,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(login_with(
        callbacks,
        originator,
        fetch.unwrap_or_else(crate::default_fetch),
        authorization_flow,
        callback::bind,
    ))
}

/// Generate the proof and independent hexadecimal state before reaching native binding.
fn authorization_flow(
    originator: Option<String>,
) -> Result<(Pkce, String, OAuthAuthInfo), OAuthError> {
    authorization_flow_with(
        originator,
        || crate::generate_pkce().map_err(Into::into),
        |bytes| getrandom::fill(bytes).map_err(|error| OAuthError::message(error.to_string())),
    )
}
/// Keep proof generation and independent state entropy in their observable order.
fn authorization_flow_with(
    originator: Option<String>,
    proof: impl FnOnce() -> Result<Pkce, OAuthError>,
    entropy: impl FnOnce(&mut [u8; 16]) -> Result<(), OAuthError>,
) -> Result<(Pkce, String, OAuthAuthInfo), OAuthError> {
    let pkce = proof()?;
    let mut bytes = [0; 16];
    entropy(&mut bytes)?;
    let state = bytes
        .iter()
        .fold(String::with_capacity(32), |mut state, byte| {
            let _ = write!(state, "{byte:02x}");
            state
        });
    let originator = originator.unwrap_or_else(|| "maestro".into());
    let info = auth_info(&pkce, &state, &originator);
    Ok((pkce, state, info))
}
/// Build protocol fields in their authored order.
fn auth_info(pkce: &Pkce, state: &str, originator: &str) -> OAuthAuthInfo {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("response_type", "code"),
            ("client_id", token::CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("scope", "openid profile email offline_access"),
            ("code_challenge", &pkce.challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("originator", originator),
        ])
        .finish();
    OAuthAuthInfo {
        url: format!("https://auth.openai.com/oauth/authorize?{query}"),
        instructions: Some("A browser window should open. Complete login to finish.".into()),
    }
}
/// Obtain entropy before binding and retain callback ownership through processing.
async fn login_with<B: std::future::Future<Output = Result<CallbackServer, OAuthError>>>(
    callbacks: OAuthCallbacks,
    originator: Option<String>,
    fetch: Fetch,
    entropy: impl FnOnce(Option<String>) -> Result<(Pkce, String, OAuthAuthInfo), OAuthError>,
    bind: impl FnOnce(String) -> B,
) -> Result<OAuthCredentials, OAuthError> {
    let (pkce, state, info) = entropy(originator)?;
    let server = bind(state.clone()).await?;
    let result = async {
        callbacks.on_auth(info)?;
        let code = select_code(&callbacks, &state, &server).await?;
        token::exchange(
            code,
            pkce.verifier,
            fetch,
            crate::records::diagnostics::timestamp_now,
        )
        .await
    }
    .await;
    server.close().await;
    result
}
/// Select callback or independently owned manual work before prompt fallback.
async fn select_code(
    callbacks: &OAuthCallbacks,
    state: &str,
    server: &CallbackServer,
) -> Result<String, OAuthError> {
    let manual = callbacks
        .on_manual_code_input()
        .map(|future| start_manual(future, server.wait.clone()));
    let mut code = wait_code(state, server, manual).await?;
    if code.as_deref().unwrap_or_default().is_empty() {
        let input = callbacks
            .on_prompt(OAuthPrompt {
                message: "Paste the authorization code (or full redirect URL):".into(),
                placeholder: None,
                allow_empty: None,
            })
            .await?;
        code = validated_input(&input, state)?;
    }
    code.filter(|code| !code.is_empty())
        .ok_or_else(|| OAuthError::message("Missing authorization code"))
}
/// Observe settled manual errors before choosing available callback code.
async fn wait_code(
    state: &str,
    server: &CallbackServer,
    manual: Option<EventStream<Result<String, OAuthError>, ()>>,
) -> Result<Option<String>, OAuthError> {
    let outcome = server.wait.next().await;
    match outcome {
        #[cfg(not(target_arch = "wasm32"))]
        Some(CallbackOutcome::Accepted(fields)) => {
            if let Some(manual) = &manual
                && let Some(Some(Err(error))) = manual.next().now_or_never()
            {
                return Err(error);
            }
            Ok(fields.code)
        }
        _ => match manual {
            Some(manual) => {
                validated_input(&manual.next().await.transpose()?.unwrap_or_default(), state)
            }
            None => Ok(None),
        },
    }
}
/// Preserve nonempty pasted state validation without requiring supplied state.
fn validated_input(input: &str, expected: &str) -> Result<Option<String>, OAuthError> {
    let fields = parse_authorization_input(input);
    if fields
        .state
        .as_deref()
        .is_some_and(|state| !state.is_empty() && state != expected)
    {
        return Err(OAuthError::message("State mismatch"));
    }
    Ok(fields.code)
}
/// Own manual work independently and publish its result before cancelling the wait.
fn start_manual(
    future: BoxFuture<Result<String, OAuthError>>,
    wait: EventStream<CallbackOutcome, ()>,
) -> EventStream<Result<String, OAuthError>, ()> {
    let manual = EventStream::new(|_| true, |_| ());
    let output = manual.clone();
    crate::providers::http::spawn_detached(async move {
        output.push(future.await);
        wait.push(CallbackOutcome::Cancelled);
    });
    manual
}

/// Refresh response-account tokens without binding a listener or storing credentials.
///
/// # Errors
/// Returns token request, response or account-extraction failures.
pub fn refresh_openai_codex_token(
    refresh_token: String,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(token::refresh(
        refresh_token,
        fetch.unwrap_or_else(crate::default_fetch),
        crate::records::diagnostics::timestamp_now,
    ))
}

#[cfg(test)]
mod tests;

/// Response-account provider with the default unchanged model projection.
struct OpenAICodexOAuthProvider;
impl crate::OAuthProviderInterface for OpenAICodexOAuthProvider {
    fn id(&self) -> &'static str {
        "openai-codex"
    }
    fn name(&self) -> &'static str {
        "ChatGPT Plus/Pro (Codex Subscription)"
    }
    fn uses_callback_server(&self) -> Option<bool> {
        Some(true)
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        login_openai_codex(callbacks, None, fetch)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        refresh_openai_codex_token(credentials.refresh, fetch)
    }
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        Ok(&credentials.access)
    }
}
/// Response-account provider operations.
#[cfg(not(target_arch = "wasm32"))]
pub const OPENAI_CODEX_OAUTH_PROVIDER: &(dyn crate::OAuthProviderInterface + Send + Sync) =
    &OpenAICodexOAuthProvider;
/// Response-account provider operations.
#[cfg(target_arch = "wasm32")]
pub const OPENAI_CODEX_OAUTH_PROVIDER: &dyn crate::OAuthProviderInterface =
    &OpenAICodexOAuthProvider;
