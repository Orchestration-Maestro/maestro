//! Native login callbacks remain callable through the original local pump.
use crate::{
    local_resources::{Admission, Lease, Message, Registry, SignalEndpoint},
    oauth_callbacks::Phase,
};
use maestro_extensions::{OAuthCallbacks, OAuthError};
use std::sync::Arc;
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Original interaction, retained independently by each guest callback capture.
#[derive(Clone)]
pub(crate) struct Interaction {
    /// Native callbacks, never reconstructed from JSON.
    callbacks: OAuthCallbacks,
    /// Pump admission shared with the original registered provider.
    owner: Arc<Lease>,
    /// Same live cancellation supplied by the native caller.
    signal: Option<SignalEndpoint>,
}
impl Interaction {
    /// Registers only the original signal, leaving optional callback absence explicit.
    pub(crate) fn new(callbacks: OAuthCallbacks, owner: Arc<Lease>) -> Result<Self, String> {
        let signal = Registry::on_pump(owner.requests(), |registry| {
            callbacks
                .signal()
                .cloned()
                .map(|signal| registry.signal(signal))
                .transpose()
        })??;
        Ok(Self {
            callbacks,
            owner,
            signal,
        })
    }
    /// Delivers synchronous notifications on their original native owner.
    fn notify(&self, phase: Phase, data: &str) -> Result<(), String> {
        match phase {
            Phase::Auth => self.callbacks.on_auth(crate::writer_reads::decode(data)?),
            Phase::Progress => self.callbacks.on_progress(data),
            _ => return Err("invalid OAuth notification phase".into()),
        }
        .map_err(|error| error.to_string())
    }
    /// Invokes an awaited callback only when the existing native pending set polls it.
    async fn input(&self, phase: Phase, data: &str) -> Result<Option<String>, String> {
        let result = async {
            match phase {
                Phase::Prompt => self
                    .callbacks
                    .on_prompt(crate::writer_reads::decode(data).map_err(error)?)
                    .await
                    .map(Some),
                Phase::Manual => self
                    .callbacks
                    .on_manual_code_input()
                    .ok_or_else(|| error("missing manual code callback".into()))?
                    .await
                    .map(Some),
                Phase::Select => {
                    self.callbacks
                        .on_select(crate::writer_reads::decode(data).map_err(error)?)
                        .ok_or_else(|| error("missing selection callback".into()))?
                        .await
                }
                _ => Err(error("invalid OAuth input phase".into())),
            }
        }
        .await;
        result.map_err(|error| error.to_string())
    }
}
/// Native work requested through an independently retained interaction resource.
pub(crate) enum Request {
    /// Synchronous notification with its own reentry route.
    Notify(
        Interaction,
        Phase,
        String,
        std::sync::mpsc::Sender<Result<(), String>>,
    ),
    /// Awaited input on the original native pending-work service.
    Input(
        Interaction,
        Phase,
        String,
        tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
        Admission,
    ),
}
impl Request {
    /// Answers on the native owner, returning pending work with its original admission.
    pub(crate) fn answer(self) -> Option<(crate::local_resources::Work, Admission)> {
        match self {
            Self::Notify(interaction, phase, data, reply) => {
                let _ = reply.send(interaction.notify(phase, &data));
                None
            }
            Self::Input(interaction, phase, data, reply, admission) => Some((
                Box::pin(async move {
                    let _ = reply.send(interaction.input(phase, &data).await);
                }),
                admission,
            )),
        }
    }
}
/// Links reverse callbacks without creating another registry or event queue.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Interaction>(host, "oauth-interaction")?;
    host.func_wrap(
        "[method]oauth-interaction.options",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Interaction>,)| {
            Ok((store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .callbacks
                .callback_presence(),))
        },
    )?;
    host.func_wrap(
        "[method]oauth-interaction.signal",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Interaction>,)| {
            let signal = store.data_mut().ctx().table.get(&resource)?.signal.clone();
            Ok((signal
                .map(|signal| store.data_mut().ctx().table.push(signal))
                .transpose()?,))
        },
    )?;
    host.func_wrap(
        "[method]oauth-interaction.notify",
        |mut store: StoreContextMut<'_, T>,
         (resource, phase, data): (Resource<Interaction>, Phase, String)| {
            let interaction = store.data_mut().ctx().table.get(&resource)?.clone();
            let owner = interaction.owner.clone();
            let route = owner.route(|_, reply| {
                Message::OAuthInteraction(Request::Notify(interaction, phase, data, reply))
            });
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]oauth-interaction.input",
        |accessor, (resource, phase, data): (Resource<Interaction>, Phase, String)| {
            Box::pin(async move {
                let interaction =
                    accessor.with(|mut access| access.get().ctx().table.get(&resource).cloned())?;
                let owner = interaction.owner.clone();
                Ok((owner
                    .request_async(|_, reply, admission| {
                        Message::OAuthInteraction(Request::Input(
                            interaction,
                            phase,
                            data,
                            reply,
                            admission,
                        ))
                    })
                    .await,))
            })
        },
    )
}
/// Preserves a native callback's literal failure message.
fn error(message: String) -> OAuthError {
    maestro_extensions::DiagnosticErrorInfo::error(message).into()
}
