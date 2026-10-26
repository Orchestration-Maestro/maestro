//! Native component factories enter their authored guest factory through the original frame.
use super::{Call, Handle, SynchronousKind};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{
    Registry,
    effects::{EmitterEndpoint, ThemeEndpoint, TuiEndpoint},
};
use maestro_extensions::{
    ComponentHandle, EventError, ExtensionAPI, Local,
    types::{ComponentDisposer, ComponentFactory, ExtensionComponent},
};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift, Resource};

/// The guest component a factory built and its optional disposal, as transferred identities.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Built {
    /// The built component.
    pub(crate) component: Resource<super::Callback>,
    /// The component's disposal.
    pub(crate) dispose: Option<Resource<super::Callback>>,
}

/// One factory invocation with the terminal and theme the native owner lent.
pub(crate) struct FactoryCall<H = Handle> {
    /// Retains the authored factory and its admission.
    pub(crate) handler: H,
    /// Original terminal.
    pub(crate) tui: TuiEndpoint,
    /// Original live theme.
    pub(crate) theme: ThemeEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// The built component and optional disposal identities.
    pub(crate) reply: Reply<(Handle, Option<Handle>)>,
}

impl Handle {
    /// A native component factory bound to the original extension's callback frame.
    pub(crate) fn component_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
    ) -> ComponentFactory {
        let facade = api.clone();
        api.ui_callback(move |(tui, theme), emitter| {
            let built = self.0.endpoint.synchronous(&registry, |reply| {
                Ok(Call::Factory(FactoryCall {
                    handler: self.clone(),
                    tui: registry.tui(tui)?,
                    theme: registry.theme(theme)?,
                    emitter: registry.emitter(emitter.frame())?,
                    reply,
                }))
            });
            let (component, dispose) = built.map_err(EventError::from)?;
            let view = crate::guest_component::GuestComponent::new(
                component,
                registry.clone(),
                facade.events.clone(),
            );
            Ok(ExtensionComponent {
                component: Rc::new(view) as ComponentHandle,
                dispose: dispose.map(|dispose| dispose.disposer(registry.clone(), &facade)),
            })
        })
    }

    /// A native disposal that runs the authored disposal once under the original frame.
    fn disposer(self, registry: Registry, api: &ExtensionAPI<Local>) -> ComponentDisposer {
        api.ui_callback(move |(), emitter| {
            let emitter: EmitterEndpoint = registry.emitter(emitter.frame())?;
            self.synchronous(&registry, SynchronousKind::Dispose, String::new(), emitter)
                .map_err(EventError::from)
        })
    }
}
