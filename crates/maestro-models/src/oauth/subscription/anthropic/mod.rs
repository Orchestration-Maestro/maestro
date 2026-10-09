//! Anthropic subscription authorization with caller-owned interaction.
mod callback;
mod exchange;
use crate::oauth::native;
#[cfg(test)]
mod tests;

use crate::providers::http::spawn_detached;
use crate::{
    BoxFuture, EventStream, Fetch, OAuthAuthInfo, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthPrompt, OAuthProviderInterface, Pkce,
};
use callback::{AuthorizationCode, parse_authorization_input};
#[cfg(not(target_arch = "wasm32"))]
use futures_util::FutureExt as _;
use native::{CallbackOutcome, CallbackServer};
use std::future::poll_fn;
use std::task::Poll;

/// Redirect identity independent of the listener's binding host.
const REDIRECT_URI: &str = "http://localhost:53692/callback";
/// Requested subscription permissions.
const SCOPES: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";

/// Authorize subscription credentials using a native callback listener or pasted input.
/// This provider does not consult the caller's selector or cancellation signal.
///
/// # Errors
/// Returns entropy, binding, interaction, transport or token validation failures.
pub fn login_anthropic(
    callbacks: OAuthCallbacks,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(login_with(
        callbacks,
        fetch.unwrap_or_else(crate::default_fetch),
        crate::generate_pkce,
        callback::bind,
    ))
}

/// Refresh subscription tokens without storing the returned credentials.
///
/// # Errors
/// Returns transport, JSON or token validation failures.
pub fn refresh_anthropic_token(
    refresh_token: String,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(exchange::refresh(
        refresh_token,
        fetch.unwrap_or_else(crate::default_fetch),
        crate::records::diagnostics::timestamp_now,
    ))
}

/// Obtain entropy before binding, keeping native restrictions inside the selected adapter.
async fn login_with<B: std::future::Future<Output = Result<CallbackServer, OAuthError>>>(
    callbacks: OAuthCallbacks,
    fetch: Fetch,
    entropy: impl FnOnce() -> Result<Pkce, crate::DiagnosticErrorInfo>,
    bind: impl FnOnce(String) -> B,
) -> Result<OAuthCredentials, OAuthError> {
    let pkce = entropy()?;
    let server = bind(pkce.verifier.clone()).await?;
    login(callbacks, fetch, pkce, server).await
}

/// Coordinate interaction and always observe listener shutdown before returning an outcome.
async fn login(
    callbacks: OAuthCallbacks,
    fetch: Fetch,
    pkce: Pkce,
    server: CallbackServer,
) -> Result<OAuthCredentials, OAuthError> {
    let selected = select_code(&callbacks, &pkce, &server).await;
    match selected {
        Ok((code, state)) => {
            let mut exchange = Box::pin(exchange::exchange(
                code,
                state,
                pkce.verifier,
                fetch,
                crate::records::diagnostics::timestamp_now,
            ));
            let ready = poll_fn(|context| {
                Poll::Ready(match exchange.as_mut().poll(context) {
                    Poll::Ready(result) => Some(result),
                    Poll::Pending => None,
                })
            })
            .await;
            server.close().await;
            match ready {
                Some(result) => result,
                None => exchange.await,
            }
        }
        Err(error) => {
            server.close().await;
            Err(error)
        }
    }
}

/// Build the ordered authorization form using space-to-plus encoding.
fn auth_info(pkce: &Pkce) -> OAuthAuthInfo {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("code", "true"),
            ("client_id", exchange::CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPES),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", pkce.verifier.as_str()),
        ])
        .finish();
    OAuthAuthInfo { url: format!("https://claude.ai/oauth/authorize?{query}"), instructions: Some("Complete login in your browser. If the browser is on another machine, paste the final redirect URL here.".into()) }
}

/// Select code from callback/manual input, then use prompt fallback and ordered validation.
async fn select_code(
    callbacks: &OAuthCallbacks,
    pkce: &Pkce,
    server: &CallbackServer,
) -> Result<(String, String), OAuthError> {
    callbacks.on_auth(auth_info(pkce))?;
    let manual = callbacks
        .on_manual_code_input()
        .map(|future| start_manual(future, server.wait.clone()));
    let outcome = server.wait.next().await;
    let mut fields = match outcome {
        #[cfg(not(target_arch = "wasm32"))]
        Some(CallbackOutcome::Accepted(fields)) => {
            if let Some(manual) = &manual
                && let Some(Some(Err(error))) = manual.next().now_or_never()
            {
                return Err(error);
            }
            fields
        }
        #[cfg(not(target_arch = "wasm32"))]
        Some(CallbackOutcome::Failed(error)) => return Err(error),
        Some(CallbackOutcome::Cancelled) | None => match manual {
            Some(manual) => validated_input(
                &manual.next().await.transpose()?.unwrap_or_default(),
                &pkce.verifier,
            )?,
            None => AuthorizationCode::default(),
        },
    };
    if fields.code.as_deref().unwrap_or_default().is_empty() {
        let input = callbacks
            .on_prompt(OAuthPrompt {
                message: "Paste the authorization code or full redirect URL:".into(),
                placeholder: Some(REDIRECT_URI.into()),
                allow_empty: None,
            })
            .await?;
        fields = validated_input(&input, &pkce.verifier)?;
    }
    let code = fields
        .code
        .filter(|code| !code.is_empty())
        .ok_or_else(|| OAuthError::message("Missing authorization code"))?;
    let state = fields
        .state
        .filter(|state| !state.is_empty())
        .ok_or_else(|| OAuthError::message("Missing OAuth state"))?;
    callbacks.on_progress("Exchanging authorization code for tokens...")?;
    Ok((code, state))
}

/// Own manual work even after an accepted callback makes its result irrelevant.
fn start_manual(
    future: BoxFuture<Result<String, OAuthError>>,
    wait: EventStream<CallbackOutcome, ()>,
) -> EventStream<Result<String, OAuthError>, ()> {
    let manual = EventStream::new(|_| true, |_| ());
    let output = manual.clone();
    spawn_detached(async move {
        output.push(future.await);
        wait.push(CallbackOutcome::Cancelled);
    });
    manual
}

/// Validate nonempty supplied state before applying the absent-state verifier fallback.
fn validated_input(input: &str, verifier: &str) -> Result<AuthorizationCode, OAuthError> {
    let mut fields = parse_authorization_input(input);
    if fields
        .state
        .as_deref()
        .is_some_and(|state| !state.is_empty() && state != verifier)
    {
        return Err(OAuthError::message("OAuth state mismatch"));
    }
    if fields.state.is_none() {
        fields.state = Some(verifier.into());
    }
    Ok(fields)
}

/// Subscription provider with unchanged model descriptors.
struct AnthropicOAuthProvider;
impl OAuthProviderInterface for AnthropicOAuthProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    fn name(&self) -> &'static str {
        "Anthropic (Claude Pro/Max)"
    }
    fn uses_callback_server(&self) -> Option<bool> {
        Some(true)
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        login_anthropic(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        refresh_anthropic_token(credentials.refresh, fetch)
    }
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        Ok(&credentials.access)
    }
}

/// Anthropic subscription provider operations.
#[cfg(not(target_arch = "wasm32"))]
pub const ANTHROPIC_OAUTH_PROVIDER: &(dyn OAuthProviderInterface + Send + Sync) =
    &AnthropicOAuthProvider;
/// Anthropic subscription provider operations.
#[cfg(target_arch = "wasm32")]
pub const ANTHROPIC_OAUTH_PROVIDER: &dyn OAuthProviderInterface = &AnthropicOAuthProvider;
