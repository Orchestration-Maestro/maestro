//! A native OAuth implementation ends with its capability.
use super::{Pump, ended_endpoint, handles};
use crate::{
    local_resources::Capability, oauth_callbacks::Controls, oauth_reads::Endpoint,
    tests::ended_errors,
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
                provider.info().unwrap_err(),
                provider.modify_models("[]".into()).result().unwrap_err(),
                provider.get_api_key("{}".into()).result().unwrap_err(),
            ]
        },
    );
    assert_eq!(errors, ["extension OAuth provider resource ended"; 3]);
}

#[test]
fn released_oauth_provider_rejects_login_and_refresh_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let ([handler], _releases, _calls) = handles::<1>(&registry);
    let provider = registry.oauth_provider(Arc::new(Idle)).unwrap();
    let lease = provider.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        let controls = Controls {
            handler,
            options: (false, false, false),
            signal: None,
        };
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                [
                    provider.login(controls).await.unwrap_err(),
                    provider.refresh("{}".into()).await.unwrap_err(),
                ]
            })
    });
    assert_eq!(errors, ["extension OAuth provider resource ended"; 2]);
}
