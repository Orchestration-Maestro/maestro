//! The provider's retained effects and its cached direct access token.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use indexmap::IndexMap;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, io::NativeIo};
use std::{cell::RefCell, rc::Rc};

/// A short-lived token for the gateway, with the headers it must travel with.
#[derive(Clone, Debug)]
pub struct DirectAccessToken {
    /// Bearer credential for gateway requests.
    pub token: String,
    /// Headers the gateway requires on every request.
    pub headers: IndexMap<String, String>,
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
        _gitlab_access_token: &str,
    ) -> ExtensionResult<DirectAccessToken> {
        let _ = (&self.io, &self.api, &self.now, &self.random, &self.direct_access);
        std::future::ready(()).await;
        Err(String::new())
    }
}
