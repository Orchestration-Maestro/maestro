//! A login provider that carries only its identifier and display name.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthProviderInterface,
};
use std::borrow::Cow;

/// A provider known by its identifier and display name alone.
///
/// It refuses to log in, returns the credentials it is asked to refresh and uses their
/// access field as the request key.
pub(crate) struct Named(pub(crate) &'static str, pub(crate) &'static str);

impl OAuthProviderInterface for Named {
    fn id(&self) -> &str {
        self.0
    }
    fn name(&self) -> &str {
        self.1
    }
    fn login(
        &self,
        _callbacks: OAuthCallbacks,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Err(DiagnosticErrorInfo::new(None, "no login").into()) })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { Ok(credentials) })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        Ok(Cow::Borrowed(&credentials.access))
    }
}
