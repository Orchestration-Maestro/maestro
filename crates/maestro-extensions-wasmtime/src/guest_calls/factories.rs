//! Native component factories enter their authored guest factory through the original frame.
use super::{
    Call, Handle, SynchronousKind,
    renderers::{ComponentCall, ComponentMethod},
};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{
    Registry,
    effects::{EmitterEndpoint, ThemeEndpoint, TuiEndpoint},
};
use maestro_extensions::{
    ComponentHandle, EventError, ExtensionAPI, LiveTheme, Local,
    types::{
        ComponentDisposer, ComponentFactory, ExtensionComponent, FooterFactory, UIErrorReporter,
        UIHostCall,
    },
};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift, Resource};

/// A factory component's literal width and transferred callback identities.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Built {
    /// The built component.
    pub(crate) component: Resource<super::Callback>,
    /// The component's disposal.
    pub(crate) dispose: Option<Resource<super::Callback>>,
    /// The component's expansion callback.
    #[component(name = "set-expanded")]
    pub(crate) set_expanded: Option<Resource<super::Callback>>,
    /// The component's implicit overlay width.
    pub(crate) width: Option<f64>,
}

/// The native owners and literal width of a built component.
pub(crate) struct RetainedComponent {
    /// The component's owner.
    pub(crate) component: Handle,
    /// The disposal's owner.
    pub(crate) dispose: Option<Handle>,
    /// The expansion callback's owner.
    pub(crate) set_expanded: Option<Handle>,
    /// The supplied implicit width.
    pub(crate) width: Option<f64>,
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
    pub(crate) reply: Reply<RetainedComponent>,
}

impl Handle {
    /// A native component factory bound to the original extension's callback frame.
    pub(crate) fn component_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
        reporter: Option<UIErrorReporter>,
    ) -> ComponentFactory {
        let facade = api.clone();
        api.ui_callback(move |(tui, theme), emitter| {
            let tui = registry.tui(tui, reporter.clone())?;
            let frame = registry.emitter(emitter.frame())?;
            self.build(&registry, &facade, (tui, theme, None), frame)
        })
    }

    /// A native footer factory bound to the original extension's callback frame.
    pub(crate) fn footer_factory(
        self,
        registry: Registry,
        api: &ExtensionAPI<Local>,
        reporter: Option<UIErrorReporter>,
    ) -> FooterFactory {
        let facade = api.clone();
        api.ui_callback(move |(tui, theme, footer), emitter| {
            let tui = registry.tui(tui, reporter.clone())?;
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
        (tui, theme, footer): (TuiEndpoint, LiveTheme, Option<crate::footer_data::Endpoint>),
        emitter: EmitterEndpoint,
    ) -> Result<ExtensionComponent, EventError> {
        let built = self.0.endpoint.synchronous(registry, |reply| {
            Ok(Call::Factory(FactoryCall {
                handler: self.clone(),
                tui,
                theme: registry.theme(theme)?,
                footer,
                emitter,
                reply,
            }))
        });
        let built = built.map_err(EventError::from)?;
        Ok(built.into_component(registry, api))
    }

    /// A retained expansion callback using each caller's original frame.
    fn expansion(self, registry: Registry, api: &ExtensionAPI<Local>) -> UIHostCall<bool, ()> {
        api.ui_callback(move |expanded, emitter| {
            let frame = registry.emitter(emitter.frame())?;
            self.0
                .endpoint
                .synchronous(&registry, |reply| {
                    Ok(Call::Component(ComponentCall::Unit(
                        self.clone(),
                        ComponentMethod::SetExpanded(expanded),
                        frame,
                        reply,
                    )))
                })
                .map_err(EventError::from)
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

impl RetainedComponent {
    /// Wraps the transferred component and callbacks using the original native toolkit.
    pub(crate) fn into_component(
        self,
        registry: &Registry,
        api: &ExtensionAPI<Local>,
    ) -> ExtensionComponent {
        let view = crate::guest_component::GuestComponent::new(
            self.component,
            registry.clone(),
            api.events.clone(),
        );
        ExtensionComponent {
            component: Rc::new(view) as ComponentHandle,
            dispose: self
                .dispose
                .map(|dispose| dispose.disposer(registry.clone(), api)),
            set_expanded: self
                .set_expanded
                .map(|handler| handler.expansion(registry.clone(), api)),
            width: self.width,
        }
    }
}
