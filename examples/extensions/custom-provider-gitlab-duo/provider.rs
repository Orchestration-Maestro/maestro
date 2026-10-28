//! The provider's retained effects and its cached direct access token.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::models::GITLAB_COM_URL;
use indexmap::IndexMap;
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    http::{Request, Response},
    io::NativeIo,
};
use serde::Deserialize;
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

/// How long a direct access token is reused: 25 minutes, in milliseconds.
const DIRECT_ACCESS_TTL: f64 = 25.0 * 60.0 * 1000.0;

/// A short-lived token for the gateway, with the headers it must travel with.
#[derive(Clone, Debug, Deserialize)]
pub struct DirectAccessToken {
    /// Bearer credential for gateway requests.
    pub token: String,
    /// Headers the gateway requires on every request.
    pub headers: IndexMap<String, String>,
    /// Moment after which the token is requested again, in Unix epoch milliseconds.
    #[serde(skip)]
    pub expires_at: f64,
}

/// Provider effects retained independently of the registration callbacks.
#[derive(Clone)]
pub struct GitLabDuo {
    /// The host's independently retained HTTP operations.
    pub io: Rc<dyn NativeIo>,
    /// The host interface that generates proof keys.
    pub api: ExtensionAPI,
    /// Current Unix epoch milliseconds.
    pub now: Rc<dyn Fn() -> f64>,
    /// Sixteen secure random bytes, supplied by WASI in the component.
    pub random: Rc<dyn Fn() -> ExtensionResult<[u8; 16]>>,
    /// The token in use, shared by every clone of this provider.
    pub direct_access: Rc<RefCell<Option<DirectAccessToken>>>,
}

impl GitLabDuo {
    /// Reuses the cached token until it expires, otherwise requests a new one.
    ///
    /// # Errors
    /// Returns the denial, the failed status with its body, or the transport failure.
    pub async fn get_direct_access_token(
        &self,
        gitlab_access_token: &str,
    ) -> ExtensionResult<DirectAccessToken> {
        let now = (self.now)();
        if let Some(cached) = self.direct_access.borrow().as_ref().filter(|cached| cached.expires_at > now) {
            return Ok(cached.clone());
        }
        let response = self
            .io
            .fetch(Request {
                url: format!("{GITLAB_COM_URL}/api/v4/ai/third_party_agents/direct_access"),
                method: Some("POST".into()),
                headers: vec![
                    ("Authorization".into(), format!("Bearer {gitlab_access_token}")),
                    ("Content-Type".into(), "application/json".into()),
                ],
                body: Some(json!({"feature_flags": {"DuoAgentPlatformNext": true}}).to_string().into_bytes()),
                signal: None,
            })
            .await?;
        if !response.ok() {
            let error = body_text(&*response).await;
            return Err(if response.status() == 403 {
                format!("GitLab Duo access denied. Ensure GitLab Duo is enabled for your account. Error: {error}")
            } else {
                format!("Failed to get direct access token: {} {error}", response.status())
            });
        }
        let mut granted: DirectAccessToken =
            serde_json::from_value(response.json().await?).map_err(|error| error.to_string())?;
        granted.expires_at = now + DIRECT_ACCESS_TTL;
        *self.direct_access.borrow_mut() = Some(granted.clone());
        Ok(granted)
    }

    /// Drops the cached token, so the next request fetches one for the new credentials.
    pub fn invalidate_direct_access_token(&self) {
        self.direct_access.borrow_mut().take();
    }
}

/// The body of a failed response, or the error that stopped the read, so the status survives.
pub async fn body_text(response: &dyn Response) -> String {
    response.text().await.unwrap_or_else(|error| error)
}
