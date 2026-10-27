//! OAuth interaction uses the existing retained callback and lifecycle reply owners.
use crate::{
    guest_calls::{Call, Handle},
    lifecycle::replies::Reply,
    local_resources::{Message, Registry, SignalEndpoint},
};
use maestro_extensions::{
    OAuthAuthInfo, OAuthError, OAuthLoginCallbacks, OAuthPrompt, OAuthSelectPrompt,
};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use wasmtime::component::{ComponentType, Lift, Lower};

/// Exact synchronous notification or awaited input requested by the native flow.
#[derive(Clone, Copy, ComponentType, Lift, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Phase {
    /// Authorization URL and instructions.
    #[component(name = "auth")]
    Auth,
    /// Pasted text request.
    #[component(name = "prompt")]
    Prompt,
    /// Synchronous progress notification.
    #[component(name = "progress")]
    Progress,
    /// Concurrent manual code input.
    #[component(name = "manual")]
    Manual,
    /// Interactive selection, including cancellation.
    #[component(name = "select")]
    Select,
    /// Synchronous authored key conversion.
    #[component(name = "key")]
    Key,
    /// Synchronous authored model transformation.
    #[component(name = "models")]
    Models,
    /// Awaited authored token refresh.
    #[component(name = "refresh")]
    Refresh,
    /// Awaited authored authorization flow.
    #[component(name = "login")]
    Login,
}
/// Owned operands for an authored OAuth operation or interaction callback.
pub(crate) struct Invocation<H = Handle> {
    /// Independently retained originating callbacks.
    pub(crate) handler: H,
    /// Exact selected callback form.
    pub(crate) phase: Phase,
    /// Supplied JSON record or literal progress text.
    pub(crate) data: String,
    /// Native interaction lent only to an authored provider login.
    pub(crate) interaction: Option<crate::oauth_interaction::Interaction>,
    /// Original typed lifecycle completion.
    pub(crate) reply: Reply<Option<String>>,
}
/// Capability operands selected by the initiating guest login.
pub(crate) struct Controls {
    /// Original guest interaction identity.
    pub(crate) handler: Handle,
    /// Presence of progress, manual-code and selection callbacks, respectively.
    pub(crate) options: (bool, bool, bool),
    /// Original caller cancellation endpoint.
    pub(crate) signal: Option<SignalEndpoint>,
}
/// Send native interaction; local lookups happen only on the original credential pump.
struct Callbacks {
    /// Shared callback identity, retaining Store admission through all native aliases.
    handler: Handle,
    /// Locates the original pump for native synchronous callback delivery.
    requests: UnboundedSender<Message>,
    /// Exact optional callback declarations.
    options: (bool, bool, bool),
    /// Original native cancellation, not a sampled flag.
    signal: Option<maestro_extensions::Cancellation>,
}
impl Controls {
    /// Adapts the actual callback slots without creating a login flow.
    pub(crate) fn callbacks(
        self,
        registry: &Registry,
    ) -> Result<maestro_extensions::OAuthCallbacks, String> {
        let signal = self
            .signal
            .as_ref()
            .map(|signal| registry.model_signal(signal))
            .transpose()?;
        Ok(Arc::new(Callbacks {
            requests: registry.requests()?,
            handler: self.handler,
            options: self.options,
            signal,
        }))
    }
}
impl Callbacks {
    /// Delivers synchronous notifications before their native caller resumes.
    fn synchronous(&self, phase: Phase, data: String) -> Result<(), OAuthError> {
        Registry::on_pump(&self.requests, |registry| {
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
        .map(|_| ())
        .map_err(error)
    }
    /// Awaits the original authored callback on the existing Store driver.
    fn input(
        &self,
        phase: Phase,
        data: Result<String, String>,
    ) -> maestro_extensions::BoxFuture<Result<Option<String>, OAuthError>> {
        let handler = self.handler.clone();
        Box::pin(async move {
            let data = data.map_err(error)?;
            handler
                .endpoint()
                .submit(|reply| {
                    Call::OAuth(Invocation {
                        handler: handler.clone(),
                        phase,
                        data,
                        interaction: None,
                        reply,
                    })
                })
                .await
                .map_err(error)
        })
    }
}
impl OAuthLoginCallbacks for Callbacks {
    fn callback_presence(&self) -> (bool, bool, bool) {
        self.options
    }
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        self.synchronous(
            Phase::Auth,
            serde_json::to_string(&info).map_err(|e| error(e.to_string()))?,
        )
    }
    fn on_prompt(
        &self,
        prompt: OAuthPrompt,
    ) -> maestro_extensions::BoxFuture<Result<String, OAuthError>> {
        required(self.input(
            Phase::Prompt,
            serde_json::to_string(&prompt).map_err(|e| e.to_string()),
        ))
    }
    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        if self.options.0 {
            self.synchronous(Phase::Progress, message.into())
        } else {
            Ok(())
        }
    }
    fn on_manual_code_input(
        &self,
    ) -> Option<maestro_extensions::BoxFuture<Result<String, OAuthError>>> {
        self.options
            .1
            .then(|| required(self.input(Phase::Manual, Ok(String::new()))))
    }
    fn on_select(
        &self,
        prompt: OAuthSelectPrompt,
    ) -> Option<maestro_extensions::BoxFuture<Result<Option<String>, OAuthError>>> {
        self.options.2.then(|| {
            self.input(
                Phase::Select,
                serde_json::to_string(&prompt).map_err(|e| e.to_string()),
            )
        })
    }
    fn signal(&self) -> Option<&maestro_extensions::Cancellation> {
        self.signal.as_ref()
    }
}
/// Rejects a missing required string rather than silently inventing input.
fn required(
    input: maestro_extensions::BoxFuture<Result<Option<String>, OAuthError>>,
) -> maestro_extensions::BoxFuture<Result<String, OAuthError>> {
    Box::pin(async move {
        input
            .await?
            .ok_or_else(|| error("missing OAuth text response".into()))
    })
}
/// Preserves the callback's authored error message.
fn error(message: String) -> OAuthError {
    maestro_extensions::DiagnosticErrorInfo::error(message).into()
}
