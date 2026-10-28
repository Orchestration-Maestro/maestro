//! GitLab sign-in and token refresh through the host's HTTP and interaction interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::provider::GitLabDuo;
use maestro_extensions_wasm::{
    ExtensionFuture, ExtensionResult, Model, OAuthCredentials, OAuthLoginCallbacks,
    OAuthProviderInterface,
};
use std::rc::Rc;

impl OAuthProviderInterface for GitLabDuo {
    fn id(&self) -> &'static str {
        ""
    }
    fn name(&self) -> &'static str {
        ""
    }
    fn uses_callback_server(&self) -> Option<bool> {
        None
    }
    fn login(&self, _: Rc<OAuthLoginCallbacks>) -> ExtensionFuture<'static, OAuthCredentials> {
        Box::pin(async { Err(String::new()) })
    }
    fn refresh_token(&self, _: OAuthCredentials) -> ExtensionFuture<'static, OAuthCredentials> {
        Box::pin(async { Err(String::new()) })
    }
    fn modify_models(
        &self,
        _: &[Model],
        _: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>> {
        Ok(None)
    }
    fn get_api_key(&self, _: &OAuthCredentials) -> ExtensionResult<String> {
        Ok(String::new())
    }
}
