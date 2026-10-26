//! Native component factories enter their authored guest factory through the original frame.
use super::{Call, Handle, SynchronousKind};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{
    Registry,
    effects::{EmitterEndpoint, ThemeEndpoint, TuiEndpoint},
};
use maestro_extensions::{
    ComponentHandle, EventError, ExtensionAPI, LiveTheme, Local, TUI,
    types::{ComponentDisposer, ComponentFactory, ExtensionComponent, FooterFactory},
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
    /// Original footer metadata of a footer factory.
    pub(crate) footer: Option<crate::footer_data::Endpoint>,
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
            let frame = registry.emitter(emitter.frame())?;
            self.build(&registry, &facade, (tui, theme, None), frame)
        })
    }

    /// A native footer factory bound to the original extension's callback frame.
    pub(crate) fn footer_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
    ) -> FooterFactory {
        let facade = api.clone();
        api.ui_callback(move |(tui, theme, footer), emitter| {
            let frame = registry.emitter(emitter.frame())?;
            let footer = Some(registry.footer(footer)?);
            self.build(&registry, &facade, (tui, theme, footer), frame)
        })
    }

    /// Runs the authored factory with the lent terminal, theme and footer metadata, and keeps
    /// what it built as native owners.
    fn build(
        &self,
        registry: &Registry,
        api: &ExtensionAPI<Local>,
        (tui, theme, footer): (TUI, LiveTheme, Option<crate::footer_data::Endpoint>),
        emitter: EmitterEndpoint,
    ) -> Result<ExtensionComponent, EventError> {
        let built = self.0.endpoint.synchronous(registry, |reply| {
            Ok(Call::Factory(FactoryCall {
                handler: self.clone(),
                tui: registry.tui(tui)?,
                theme: registry.theme(theme)?,
                footer,
                emitter,
                reply,
            }))
        });
        let (component, dispose) = built.map_err(EventError::from)?;
        let view = crate::guest_component::GuestComponent::new(
            component,
            registry.clone(),
            api.events.clone(),
        );
        Ok(ExtensionComponent {
            component: Rc::new(view) as ComponentHandle,
            dispose: dispose.map(|dispose| dispose.disposer(registry.clone(), api)),
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
