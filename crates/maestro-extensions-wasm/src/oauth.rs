//! Callable native OAuth providers, independent of later registry membership.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult, Model, OAuthCredentials};

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
