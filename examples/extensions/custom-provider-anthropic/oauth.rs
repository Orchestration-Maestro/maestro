//! Subscription sign-in through the host's HTTP and interaction interfaces.
use maestro_extensions_wasm::{
    ExtensionFuture, ExtensionResult, Model, OAuthCredentials, OAuthLoginCallbacks,
    OAuthProviderInterface, io::NativeIo,
};
use std::rc::Rc;

/// Authorization effects retained independently of the registration callback.
pub struct CustomAnthropic {
    /// The host's independently retained HTTP operations.
    pub io: Rc<dyn NativeIo>,
    /// Secure verifier bytes, supplied by WASI in the component.
    pub entropy: Rc<dyn Fn() -> ExtensionResult<[u8; 32]>>,
    /// Current Unix epoch milliseconds.
    pub now: Rc<dyn Fn() -> f64>,
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
        let _ = (&self.io, &self.entropy, &self.now, callbacks);
        Box::pin(async { Err("Authorization is unavailable".into()) })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let _ = credentials;
        Box::pin(async { Err("Token refresh is unavailable".into()) })
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
