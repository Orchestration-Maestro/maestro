//! Callable native OAuth providers, independent of later registry membership.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AbortSignal, ExtensionFuture, ExtensionResult, Model, OAuthCredentials};
pub use maestro_request::oauth::{
    OAuthAuthInfo, OAuthPrompt, OAuthSelectOption, OAuthSelectPrompt,
};
use std::rc::Rc;

/// Open provider identifier.
pub type OAuthProviderId = String;
/// Provider identifier used by authorization records.
pub type OAuthProvider = OAuthProviderId;

/// Operations on one independently retained OAuth implementation.
pub trait OAuthProviderInterface {
    /// Returns the literal provider identifier.
    fn id(&self) -> &str;
    /// Returns the login display label.
    fn name(&self) -> &str;
    /// Returns callback-server metadata without filling in an absent value.
    fn uses_callback_server(&self) -> Option<bool>;
    /// Authorizes through retained interaction callbacks.
    ///
    /// # Errors
    /// The future returns the original provider or callback failure.
    fn login(
        &self,
        callbacks: Rc<OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials>;
    /// Rotates credentials while retaining this implementation independently of the facade.
    ///
    /// # Errors
    /// Returns the original provider or record-decoding failure.
    #[must_use]
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials>;
    /// Transforms descriptors when the provider supplies a model modifier.
    ///
    /// # Errors
    /// Returns the original provider or record-decoding failure.
    fn modify_models(
        &self,
        models: &[Model],
        credentials: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>>;
    /// Converts credentials through this implementation without selecting it again by identifier.
    ///
    /// # Errors
    /// Returns the original provider failure.
    fn get_api_key(&self, credentials: &OAuthCredentials) -> ExtensionResult<String>;
}

/// Synchronous authorization notification.
pub type OAuthAuthCallback = Rc<dyn Fn(OAuthAuthInfo) -> ExtensionResult<()>>;
/// Awaited text prompt.
pub type OAuthPromptCallback = Rc<dyn Fn(OAuthPrompt) -> ExtensionFuture<'static, String>>;
/// Synchronous progress notification.
pub type OAuthProgressCallback = Rc<dyn Fn(&str) -> ExtensionResult<()>>;
/// Optional concurrent pasted authorization input.
pub type OAuthManualCodeCallback = Rc<dyn Fn() -> ExtensionFuture<'static, String>>;
/// Awaited choice; None is the caller's cancellation decision.
pub type OAuthSelectCallback =
    Rc<dyn Fn(OAuthSelectPrompt) -> ExtensionFuture<'static, Option<String>>>;

/// Interaction supplied by the caller of an authorization flow.
///
/// A registered implementation may retain this record or an input future after login ends.
/// Native callback captures stay alive until their final guest alias is released.
#[derive(Clone)]
pub struct OAuthLoginCallbacks {
    /// Publishes the authorization URL synchronously.
    pub on_auth: OAuthAuthCallback,
    /// Awaits pasted authorization text.
    pub on_prompt: OAuthPromptCallback,
    /// Publishes progress synchronously when supplied.
    pub on_progress: Option<OAuthProgressCallback>,
    /// Starts optional concurrent pasted input.
    pub on_manual_code_input: Option<OAuthManualCodeCallback>,
    /// Requests an optional choice without replacing an absent callback.
    pub on_select: Option<OAuthSelectCallback>,
    /// Caller cancellation, absent unless supplied.
    pub signal: Option<AbortSignal>,
}

impl OAuthLoginCallbacks {
    /// Reports whether progress, manual-code and selection callbacks are supplied, in that order.
    #[must_use]
    pub fn callback_presence(&self) -> (bool, bool, bool) {
        (
            self.on_progress.is_some(),
            self.on_manual_code_input.is_some(),
            self.on_select.is_some(),
        )
    }
}
