//! Native custom factories separate borrowed prefixes from owned continuations.
use super::{Call, Handle, factories::RetainedComponent};
use crate::{
    lifecycle::replies::Reply,
    local_resources::{
        Registry,
        effects::{EmitterEndpoint, ThemeEndpoint, TuiEndpoint},
    },
};
use maestro_extensions::{
    EventError, ExtensionAPI, Local,
    types::{CustomComponentFactory, CustomFactoryOutcome, UIErrorReporter},
};
use wasmtime::component::{ComponentType, Lift, Lower, Resource};

/// Capabilities lent to a custom factory without transferring native policy.
pub(crate) struct Input {
    /// Original terminal.
    pub(crate) tui: TuiEndpoint,
    /// Original live theme.
    pub(crate) theme: ThemeEndpoint,
    /// Original keybindings.
    pub(crate) keybindings: crate::keys_reads::KeysEndpoint,
    /// Native accepted-close operation.
    pub(crate) close: crate::custom_reads::CloseEndpoint,
}
/// Canonical owned operands of one prefix.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct WireInput {
    /// Original terminal.
    pub(crate) tui: Resource<TuiEndpoint>,
    /// Original live theme.
    pub(crate) theme: Resource<ThemeEndpoint>,
    /// Original keybindings.
    pub(crate) keybindings: Resource<crate::keys_reads::KeysEndpoint>,
    /// Native accepted-close operation.
    pub(crate) close: Resource<crate::custom_reads::CloseEndpoint>,
}
/// Canonical outcome transferred by the prefix.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum Built {
    /// Already constructed component.
    #[component(name = "ready")]
    Ready(super::factories::Built),
    /// Owned asynchronous continuation.
    #[component(name = "pending")]
    Pending(Resource<super::Callback>),
}
/// Actual native owners transferred by a prefix.
pub(crate) enum Outcome {
    /// Already constructed component.
    Ready(RetainedComponent),
    /// Owned asynchronous continuation.
    Pending(Handle),
}
/// One admitted prefix and its borrowed emission frame.
pub(crate) struct Prefix<H = Handle> {
    /// Original authored identity.
    pub(crate) handler: H,
    /// Native factory operands.
    pub(crate) input: Input,
    /// Original callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// Native settlement owner.
    pub(crate) reply: Reply<Outcome>,
}
/// An independently admitted prefix or continuation.
pub(crate) enum Invocation<H = Handle> {
    /// Calls the synchronous prefix under a fresh borrowed emission frame.
    Prefix(Prefix<H>),
    /// Awaits the retained future without a borrowed frame.
    Future(H, Reply<RetainedComponent>),
}
impl<H> Invocation<H> {
    /// Transfers only callback admission while retaining its original operands.
    pub(crate) fn map<T>(self, convert: impl FnOnce(H) -> T) -> Invocation<T> {
        match self {
            Self::Prefix(call) => Invocation::Prefix(Prefix {
                handler: convert(call.handler),
                input: call.input,
                emitter: call.emitter,
                reply: call.reply,
            }),
            Self::Future(handler, reply) => Invocation::Future(convert(handler), reply),
        }
    }
}
impl Handle {
    /// Uses the native callback owner to create one fresh prefix frame per factory call.
    pub(crate) fn custom_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
        reporter: Option<UIErrorReporter>,
    ) -> CustomComponentFactory {
        let facade = api.clone();
        api.ui_callback(move |(tui, theme, keys, close), emitter| {
            let input = Input {
                tui: registry.tui(tui, reporter.clone())?,
                theme: registry.theme(theme)?,
                keybindings: registry.keybindings(keys)?,
                close: registry.custom_close(close)?,
            };
            let emitter = registry.emitter(emitter.frame())?;
            self.endpoint()
                .synchronous(&registry, |reply| {
                    Ok(Call::Custom(Invocation::Prefix(Prefix {
                        handler: self.clone(),
                        input,
                        emitter,
                        reply,
                    })))
                })
                .map(|outcome| outcome.resolve(registry.clone(), facade.clone()))
                .map_err(EventError::from)
        })
    }
}
impl Outcome {
    /// Keeps the native custom-component construction and settlement policy unchanged.
    fn resolve(self, registry: Registry, api: ExtensionAPI<Local>) -> CustomFactoryOutcome {
        match self {
            Self::Ready(built) => {
                CustomFactoryOutcome::Ready(built.into_component(&registry, &api))
            }
            Self::Pending(pending) => CustomFactoryOutcome::Async(Box::pin(async move {
                pending
                    .endpoint()
                    .call(&registry, |reply| {
                        Call::Custom(Invocation::Future(pending.clone(), reply))
                    })
                    .await
                    .map(|built| built.into_component(&registry, &api))
                    .map_err(EventError::from)
            })),
        }
    }
}

impl Input {
    /// Transfers handles to the original native operands, without copying their state.
    pub(crate) fn transfer(
        &self,
        table: &mut wasmtime::component::ResourceTable,
    ) -> wasmtime::Result<WireInput> {
        Ok(WireInput {
            tui: table.push(self.tui.clone())?,
            theme: table.push(self.theme.clone())?,
            keybindings: table.push(self.keybindings.clone())?,
            close: table.push(self.close.clone())?,
        })
    }
}
