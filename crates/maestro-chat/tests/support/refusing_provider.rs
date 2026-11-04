//! A catalog provider whose stored sign-in refuses every transformation.
use maestro_app::{
    BoxFuture, Fetch, Model, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderInterface,
};
use std::borrow::Cow;

/// A provider named `id`, shown as `name`, whose stored sign-in refuses with `error`.
pub struct RefusingProvider {
    /// The provider identifier.
    id: &'static str,
    /// The provider's display name.
    name: &'static str,
    /// The failure every transformation reports.
    error: fn() -> OAuthError,
}

impl RefusingProvider {
    /// The provider named `id`, shown as `name`, refusing with `error`.
    pub fn new(id: &'static str, name: &'static str, error: fn() -> OAuthError) -> Self {
        Self { id, name, error }
    }
}

impl OAuthProviderInterface for RefusingProvider {
    fn id(&self) -> &str {
        self.id
    }
    fn name(&self) -> &str {
        self.name
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Err((self.error)())))
    }
    fn refresh_token(
        &self,
        _: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::ready(Err((self.error)())))
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        Ok(Cow::Borrowed(&credentials.access))
    }
    fn modify_models<'m>(
        &self,
        _: &'m [Model],
        _: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, Model>>, OAuthError>> {
        Some(Err((self.error)()))
    }
}
