//! A native OAuth implementation ends with its capability.
use super::ended_endpoint;
use crate::{
    local_resources::Capability,
    oauth_reads::{Endpoint, Request},
    read_imports::ReadEndpoint,
};
use maestro_extensions::{
    BoxFuture, Fetch, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderInterface,
};
use std::{borrow::Cow, sync::Arc};

/// An implementation that never completes an authorization or a refresh.
struct Idle;

impl OAuthProviderInterface for Idle {
    fn id(&self) -> &'static str {
        "idle"
    }

    fn name(&self) -> &'static str {
        "Idle"
    }

    fn login(
        &self,
        _callbacks: OAuthCallbacks,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::pending())
    }

    fn refresh_token(
        &self,
        _credentials: OAuthCredentials,
        _fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(std::future::pending())
    }

    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        Ok(Cow::Borrowed(&credentials.access))
    }
}

#[test]
fn released_oauth_provider_rejects_every_kind_of_operation_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::OAuthProvider(Arc::new(Idle)),
        Endpoint,
        |provider| {
            [
                provider.read(Request::Info).unwrap_err(),
                provider
                    .route(|reply| Request::Models("[]".into(), reply))
                    .result()
                    .unwrap_err(),
                provider
                    .route(|reply| Request::Key("{}".into(), reply))
                    .result()
                    .unwrap_err(),
            ]
        },
    );
    assert_eq!(errors, ["extension OAuth provider resource ended"; 3]);
}
