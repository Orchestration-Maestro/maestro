//! Typed dynamic provider definitions and incoming validation.
use super::CatalogError;
use maestro_models::{
    DiagnosticErrorInfo, OAuthError, OAuthProviderHandle, StreamFunction, StreamOptionsInput,
};
use maestro_request::provider_config::ProviderConfig;
use std::borrow::Cow;

/// Native callable operations supplied with a dynamic registration.
#[derive(Default)]
pub struct ProviderCallbacks {
    /// Callback receiving the original raw or simple options.
    pub stream_simple: Option<StreamFunction<StreamOptionsInput>>,
    /// Subscription operations whose installed ID is the registration name.
    pub oauth: Option<OAuthProviderHandle>,
}
/// Partially supplied dynamic registration; absent fields retain earlier values on replay.
pub type ProviderConfigInput = ProviderConfig<ProviderCallbacks>;
/// Complete metadata for a dynamically supplied model, without file-loader defaults.
pub use maestro_request::provider_config::ProviderModelConfig as ProviderModelInput;
/// Check prerequisites in their consuming order before registration effects.
pub(super) fn validate(config: &ProviderConfigInput, provider: &str) -> Result<(), CatalogError> {
    let models = config.models.as_deref().unwrap_or_default();
    let failure = if config.callbacks.stream_simple.is_some()
        && nonempty(config.api.as_deref()).is_none()
    {
        Some(format!(
            "Provider {provider}: \"api\" is required when registering streamSimple."
        ))
    } else if models.is_empty() {
        None
    } else if nonempty(config.base_url.as_deref()).is_none() {
        Some(format!(
            "Provider {provider}: \"baseUrl\" is required when defining models."
        ))
    } else if nonempty(config.api_key.as_deref()).is_none() && config.callbacks.oauth.is_none() {
        Some(format!(
            "Provider {provider}: \"apiKey\" or \"oauth\" is required when defining models."
        ))
    } else {
        models
            .iter()
            .find(|model| {
                nonempty(model.api.as_deref())
                    .or(nonempty(config.api.as_deref()))
                    .is_none()
            })
            .map(|model| {
                format!(
                    "Provider {provider}, model {}: no \"api\" specified.",
                    model.id
                )
            })
    };
    failure.map_or(Ok(()), |message| {
        Err(DiagnosticErrorInfo::new(None, message).into())
    })
}
/// String truthiness at the typed registration boundary.
pub(super) fn nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
/// Account adapter changing only the installed identifier.
pub(super) struct RegisteredOAuth {
    /// Registration identity.
    pub id: String,
    /// Supplied account operations.
    pub inner: OAuthProviderHandle,
}
impl maestro_models::OAuthProviderInterface for RegisteredOAuth {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn uses_callback_server(&self) -> Option<bool> {
        self.inner.uses_callback_server()
    }
    fn login(
        &self,
        callbacks: maestro_models::OAuthCallbacks,
        fetch: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<Result<maestro_models::OAuthCredentials, OAuthError>> {
        self.inner.login(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: maestro_models::OAuthCredentials,
        fetch: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<Result<maestro_models::OAuthCredentials, OAuthError>> {
        self.inner.refresh_token(credentials, fetch)
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_models::OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        self.inner.get_api_key(credentials)
    }
    fn modify_models<'m>(
        &self,
        models: &'m [maestro_models::Model],
        credentials: &maestro_models::OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, maestro_models::Model>>, OAuthError>> {
        self.inner.modify_models(models, credentials)
    }
}
