//! Response-account callback admission and routing.
use crate::OAuthError;
#[cfg(not(target_arch = "wasm32"))]
use crate::oauth::callback::{CallbackResponse, query_fields};
use crate::oauth::native::CallbackServer;

/// Recover from a native binding failure using the same cancelled wait identity.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind(state: String) -> Result<CallbackServer, OAuthError> {
    bind_with(
        state,
        |key| std::env::var(key).ok(),
        |listener| Box::pin(listener.accept()),
    )
    .await
}
/// Bind through supplied native effects, recovering only native listener failures.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind_with(
    state: String,
    environment: impl FnOnce(&str) -> Option<String>,
    accept: impl for<'a> Fn(&'a tokio::net::TcpListener) -> crate::oauth::native::Accept<'a>
    + Send
    + 'static,
) -> Result<CallbackServer, OAuthError> {
    Ok(
        crate::oauth::native::bind_with(state, 1455, route, environment, accept)
            .await
            .unwrap_or_else(|_| cancelled()),
    )
}
/// Serving is unavailable in the browser, independently of shared token primitives.
#[cfg(target_arch = "wasm32")]
pub(super) async fn bind(state: String) -> Result<CallbackServer, OAuthError> {
    unavailable(state).await
}
/// Refuse native serving at its effect boundary.
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn unavailable(
    _state: String,
) -> std::future::Ready<Result<CallbackServer, OAuthError>> {
    std::future::ready(Err(OAuthError::message(
        "Response-account OAuth is only available in native environments",
    )))
}
/// Represent a failed native bind without inventing another pending listener.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn cancelled() -> CallbackServer {
    let server = CallbackServer::new();
    server
        .wait
        .push(crate::oauth::native::CallbackOutcome::Cancelled);
    server.stopped.push(());
    server
}
/// Route state validation before code validation without consuming rejected callbacks.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn route(target: &str, state: &str) -> CallbackResponse {
    let url = url::Url::parse("http://localhost").and_then(|base| base.join(target));
    let (status, message, accepted) = match url {
        Err(_) => (500, "Internal error while processing OAuth callback.", None),
        Ok(url) if url.path() != "/auth/callback" => (404, "Callback route not found.", None),
        Ok(url) => {
            let fields = query_fields(url.query().unwrap_or_default());
            if fields.state.as_deref() != Some(state) {
                (400, "State mismatch.", None)
            } else if fields.code.as_deref().unwrap_or_default().is_empty() {
                (400, "Missing authorization code.", None)
            } else {
                (
                    200,
                    "OpenAI authentication completed. You can close this window.",
                    Some(fields),
                )
            }
        }
    };
    let body = if accepted.is_some() {
        crate::oauth_success_html(message)
    } else {
        crate::oauth_error_html(message, None)
    };
    CallbackResponse {
        status,
        content_type: "text/html; charset=utf-8",
        body: body.unwrap_or_else(|error| error.message),
        accepted,
    }
}
