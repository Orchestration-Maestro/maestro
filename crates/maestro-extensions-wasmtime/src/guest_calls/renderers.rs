//! Local message renderers retain real guest components without capturing a runner.
use super::{Call, Handle};
use crate::lifecycle::GuestResource;
use crate::{
    lifecycle::replies::Reply,
    local_resources::{
        Registry,
        effects::{EmitterEndpoint, ThemeEndpoint},
    },
};
use maestro_extensions::{EventBus, Local, MessageRenderer};
use std::{rc::Rc, sync::Arc};
use wasmtime::component::{ComponentType, Lower, ResourceAny};

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
pub(crate) struct RendererCall {
    /// Original authored renderer.
    pub(crate) handler: Handle,
    /// Complete message document.
    pub(crate) message: String,
    /// Actual expansion choice.
    pub(crate) expanded: bool,
    /// Original theme.
    pub(crate) theme: ThemeEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// Actual optional component resource.
    pub(crate) reply: Reply<Option<ResourceAny>>,
}
/// Infallible component methods share one canonical unit-result call shape.
pub(crate) enum ComponentMethod {
    /// Literal terminal input.
    Input(String),
    /// Clears authored rendering caches.
    Invalidate,
}
/// Calls on retained guest components use the same Store lifecycle as creation.
pub(crate) enum ComponentCall {
    /// Fallible render with the exact viewport width.
    Render(Arc<GuestResource>, u32, EmitterEndpoint, Reply<Vec<String>>),
    /// Fallible input or cache invalidation.
    Unit(
        Arc<GuestResource>,
        ComponentMethod,
        EmitterEndpoint,
        Reply<()>,
    ),
    /// Original authored key-release preference.
    WantsKeyRelease(Arc<GuestResource>, EmitterEndpoint, Reply<bool>),
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
            let resource = handler.0.endpoint.replies.synchronous(&registry, |reply| {
                let call = RendererCall {
                    handler: handler.clone(),
                    message,
                    expanded: options.expanded,
                    theme: registry.theme(theme)?,
                    emitter: registry.emitter(emitter.frame())?,
                    reply,
                };
                handler.0.endpoint.send(Call::Renderer(call))
            })?;
            Ok(resource.map(|resource| {
                Rc::new(crate::guest_component::GuestComponent::new(
                    handler.resource(resource),
                    registry.clone(),
                    events.clone(),
                )) as maestro_extensions::ComponentHandle
            }))
        })
    }
}
