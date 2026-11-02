//! Authored OAuth implementations use the existing callback and provider owners.
use crate::{
    guest_calls::{Call, Handle},
    local_resources::{Lease, Registry},
    oauth_callbacks::{Invocation, Phase},
    oauth_interaction::Interaction,
};
use maestro_extensions::{OAuthCredentials, OAuthError, OAuthProviderInterface};
use std::{borrow::Cow, sync::Arc};

/// Optional authored capability supplied beside ordinary provider metadata.
pub(crate) type Wire = Option<(
    wasmtime::component::Resource<crate::guest_calls::Callback>,
    String,
    Option<bool>,
)>;

/// Retains this Store's callback before the registration request crosses to the local owner.
pub(crate) fn retain<T: crate::callback_imports::CallbackView>(
    state: &mut T,
    id: &str,
    wire: Wire,
) -> wasmtime::Result<Option<maestro_extensions::OAuthProviderHandle>> {
    wire.map(|(handler, name, server)| {
        let handler = crate::callback_imports::retain(state, &handler)?;
        Ok(Arc::new(Provider {
            id: id.to_owned(),
            handler,
            name,
            server,
            owner: state.api().0.clone(),
        }) as maestro_extensions::OAuthProviderHandle)
    })
    .transpose()
}
/// Independently retained guest implementation, not a registry lookup by identifier.
struct Provider {
    /// Installed provider identifier.
    id: String,
    /// Original callback identity and native final-release owner.
    handler: Handle,
    /// Literal display label supplied by the author.
    name: String,
    /// Absent, false or true callback-server metadata.
    server: Option<bool>,
    /// Original pump retained for synchronous calls and reverse interaction.
    owner: Arc<Lease>,
}
impl Provider {
    /// Calls a synchronous operation through the existing reentry route.
    fn synchronous(&self, phase: Phase, data: String) -> Result<String, OAuthError> {
        Registry::on_pump(self.owner.requests(), |registry| {
            self.handler.endpoint().synchronous(registry, |reply| {
                Ok(Call::OAuth(Invocation {
                    handler: self.handler.clone(),
                    phase,
                    data,
                    interaction: None,
                    reply,
                }))
            })
        })
        .and_then(std::convert::identity)
        .and_then(required)
        .map_err(error)
    }
    /// Returns independently owned work through the existing Store and pending-work lifecycle.
    fn asynchronous(
        &self,
        phase: Phase,
        data: Result<String, String>,
        interaction: Option<Interaction>,
    ) -> maestro_extensions::BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let handler = self.handler.clone();
        Box::pin(async move {
            let data = data.map_err(error)?;
            let result = handler
                .endpoint()
                .submit(|reply| {
                    Call::OAuth(Invocation {
                        handler: handler.clone(),
                        phase,
                        data,
                        interaction,
                        reply,
                    })
                })
                .await
                .map_err(error)?;
            crate::writer_reads::decode(&required(result).map_err(error)?).map_err(error)
        })
    }
}
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn uses_callback_server(&self) -> Option<bool> {
        self.server
    }
    fn login(
        &self,
        callbacks: maestro_extensions::OAuthCallbacks,
        _fetch: Option<maestro_extensions::Fetch>,
    ) -> maestro_extensions::BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let interaction = Interaction::new(callbacks, self.owner.clone());
        match interaction {
            Ok(interaction) => {
                self.asynchronous(Phase::Login, Ok(String::new()), Some(interaction))
            }
            Err(message) => Box::pin(std::future::ready(Err(error(message)))),
        }
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        _fetch: Option<maestro_extensions::Fetch>,
    ) -> maestro_extensions::BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.asynchronous(
            Phase::Refresh,
            serde_json::to_string(&credentials).map_err(|e| e.to_string()),
            None,
        )
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        self.synchronous(
            Phase::Key,
            serde_json::to_string(credentials).map_err(|e| error(e.to_string()))?,
        )
        .map(Cow::Owned)
    }
    fn modify_models<'m>(
        &self,
        models: &'m [maestro_extensions::Model],
        credentials: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, maestro_extensions::Model>>, OAuthError>> {
        let result = serde_json::to_string(&(models, credentials))
            .map_err(|e| error(e.to_string()))
            .and_then(|data| self.synchronous(Phase::Models, data))
            .and_then(|data| {
                crate::writer_reads::decode::<Option<Vec<maestro_extensions::Model>>>(&data)
                    .map_err(error)
            });
        match result {
            Ok(None) => None,
            Ok(Some(models)) => Some(Ok(models.into_iter().map(Cow::Owned).collect())),
            Err(error) => Some(Err(error)),
        }
    }
}
/// Provider operations always return a value; empty text remains a real value.
fn required(value: Option<String>) -> Result<String, String> {
    value.ok_or_else(|| "missing OAuth provider response".into())
}
/// Preserves the original authored failure text.
fn error(message: String) -> OAuthError {
    maestro_extensions::DiagnosticErrorInfo::error(message).into()
}
