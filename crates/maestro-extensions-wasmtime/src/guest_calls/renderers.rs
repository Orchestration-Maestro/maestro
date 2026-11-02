//! Local message renderers retain real guest components without capturing a runner.
use super::{Call, Handle};
use crate::{
    lifecycle::replies::Reply,
    local_resources::{
        Registry,
        effects::{EmitterEndpoint, ThemeEndpoint},
    },
};
use maestro_extensions::{EventBus, Local, MessageRenderer};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lower};

/// Exact owned renderer inputs on the canonical boundary.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct RenderRequest {
    /// Complete original custom message document.
    pub(crate) message: String,
    /// Actual caller expansion choice.
    pub(crate) expanded: bool,
    /// Original live theme capability.
    pub(crate) theme: wasmtime::component::Resource<ThemeEndpoint>,
}
/// Actual native renderer operands, before Store resource insertion.
pub(crate) struct RendererCall<H = Handle> {
    /// Original authored renderer.
    pub(crate) handler: H,
    /// Complete message document.
    pub(crate) message: String,
    /// Actual expansion choice.
    pub(crate) expanded: bool,
    /// Original theme.
    pub(crate) theme: ThemeEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// Actual optional component identity.
    pub(crate) reply: Reply<Option<Handle>>,
}
/// Component mutations share one canonical unit-result call shape.
pub(crate) enum ComponentMethod {
    /// Literal terminal input.
    Input(String),
    /// Changes a factory component's expansion.
    SetExpanded(bool),
    /// Assigns the component's focus.
    SetFocused(bool),
    /// Clears authored rendering caches.
    Invalidate,
}
/// Calls on retained guest components use the same Store lifecycle as creation.
pub(crate) enum ComponentCall<R = Handle> {
    /// Fallible render with the exact viewport width.
    Render(R, u32, EmitterEndpoint, Reply<Vec<String>>),
    /// Fallible input or cache invalidation.
    Unit(R, ComponentMethod, EmitterEndpoint, Reply<()>),
    /// Original authored key-release preference.
    WantsKeyRelease(R, EmitterEndpoint, Reply<bool>),
    /// Focus state, absent for a component without that capability.
    Focused(R, EmitterEndpoint, Reply<Option<bool>>),
}
impl Handle {
    /// Creates only the Local-family renderer; reporting stays with its future caller.
    pub(crate) fn renderer(
        &self,
        registry: Registry,
        events: EventBus<Local>,
    ) -> MessageRenderer<serde_json::Value, Local> {
        let handler = self.clone();
        Rc::new(move |message, options, theme, emitter| {
            let message = serde_json::to_string(&message).map_err(|error| error.to_string())?;
            let component = handler.0.endpoint.synchronous(&registry, |reply| {
                Ok(Call::Renderer(RendererCall {
                    handler: handler.clone(),
                    message,
                    expanded: options.expanded,
                    theme: registry.theme(theme)?,
                    emitter: registry.emitter(emitter.frame())?,
                    reply,
                }))
            })?;
            Ok(component.map(|component| {
                Rc::new(crate::guest_component::GuestComponent::new(
                    component,
                    registry.clone(),
                    events.clone(),
                )) as maestro_extensions::ComponentHandle
            }))
        })
    }
}
