//! Typed dynamic provider definitions and incoming validation.
use maestro_models::{
    Api, DiagnosticErrorInfo, ModelCompat, ModelCost, ModelInput, OAuthError, OAuthProviderHandle,
    StreamFunction, StreamOptionsInput, ThinkingLevelMap,
};
use std::borrow::Cow;

/// Partially supplied dynamic registration; absent fields retain earlier values on replay.
#[derive(Default)]
pub struct ProviderConfigInput {
    /// Provider display label, including an empty label.
    pub name: Option<String>,
    /// Provider endpoint.
    pub base_url: Option<String>,
    /// Configured request key expression.
    pub api_key: Option<String>,
    /// Protocol used by the callback and model fallback.
    pub api: Option<Api>,
    /// Callback receiving the original raw or simple options.
    pub stream_simple: Option<StreamFunction<StreamOptionsInput>>,
    /// Request header expressions.
    pub headers: Option<indexmap::IndexMap<String, String>>,
    /// Whether to synthesize Authorization.
    pub auth_header: Option<bool>,
    /// Subscription operations whose installed ID is the registration name.
    pub oauth: Option<OAuthProviderHandle>,
    /// Complete dynamic model definitions; empty lists do not replace immediately.
    pub models: Option<Vec<ProviderModelInput>>,
}

/// Complete metadata for a dynamically supplied model, without file-loader defaults.
#[derive(Clone)]
pub struct ProviderModelInput {
    /// Exact model identifier.
    pub id: String,
    /// Display label.
    pub name: String,
    /// Nonempty protocol override.
    pub api: Option<Api>,
    /// Endpoint override, including an empty endpoint.
    pub base_url: Option<String>,
    /// Whether reasoning is supported.
    pub reasoning: bool,
    /// Provider-specific thinking level values.
    pub thinking_level_map: Option<ThinkingLevelMap>,
    /// Accepted input kinds in supplied order.
    pub input: Vec<ModelInput>,
    /// Supplied usage prices.
    pub cost: ModelCost,
    /// Context capacity.
    pub context_window: f64,
    /// Output capacity.
    pub max_tokens: f64,
    /// Per-model request header expressions, not descriptor headers.
    pub headers: Option<indexmap::IndexMap<String, String>>,
    /// Open compatibility metadata.
    pub compat: Option<ModelCompat>,
}
impl ProviderConfigInput {
    /// Check prerequisites in their consuming order before registration effects.
    pub(super) fn validate(&self, provider: &str) -> Result<(), OAuthError> {
        let failure = if self.stream_simple.is_some() && nonempty(self.api.as_deref()).is_none() {
            Some(format!(
                "Provider {provider}: \"api\" is required when registering streamSimple."
            ))
        } else if self.models.as_ref().is_none_or(Vec::is_empty) {
            None
        } else if nonempty(self.base_url.as_deref()).is_none() {
            Some(format!(
                "Provider {provider}: \"baseUrl\" is required when defining models."
            ))
        } else if nonempty(self.api_key.as_deref()).is_none() && self.oauth.is_none() {
            Some(format!(
                "Provider {provider}: \"apiKey\" or \"oauth\" is required when defining models."
            ))
        } else {
            self.models
                .iter()
                .flatten()
                .find(|model| {
                    nonempty(model.api.as_deref())
                        .or(nonempty(self.api.as_deref()))
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
            Err(DiagnosticErrorInfo {
                message,
                name: None,
                stack: None,
                code: None,
            }
            .into())
        })
    }
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
    ) -> Result<&'a str, OAuthError> {
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
