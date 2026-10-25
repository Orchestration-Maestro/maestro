//! Renderer invocation with owned themes and returned components.
use super::event_bus::Emitter;
use super::{Exports, Imports, callbacks};
use crate::{CallbackEmitter, Component, ExtensionResult, MessageRenderOptions, MessageRenderer};

/// Inputs lent or transferred to a renderer invocation.
pub(crate) struct RenderRequest<'a, I: Imports> {
    /// Complete custom-message document.
    pub(crate) message: &'a str,
    /// Expanded rendering choice.
    pub(crate) expanded: bool,
    /// Independently owned theme capability.
    pub(crate) theme: I::Theme,
    /// Emission scope of this synchronous invocation.
    pub(crate) emitter: &'a I::Emitter,
}
impl<I: Imports> Exports<I> {
    /// Invokes one renderer and preserves its optional retained view.
    pub(crate) fn invoke_renderer(
        &self,
        handler: u32,
        request: RenderRequest<'_, I>,
    ) -> ExtensionResult<Option<Component>> {
        let renderer = callbacks::find::<MessageRenderer>(handler)?;
        let message = super::session::decode(request.message)?;
        let theme = super::theme::theme(self.imports.clone(), request.theme);
        with_emitter(&self.imports, request.emitter, |emitter| {
            renderer(
                message,
                MessageRenderOptions {
                    expanded: request.expanded,
                },
                theme,
                emitter,
            )
        })
    }
}
#[cfg(target_arch = "wasm32")]
impl<I: Imports> Exports<I> {
    /// Runs one method of the authored component retained under a shared identity.
    pub(crate) fn invoke_component<R>(
        &self,
        id: u32,
        emitter: &I::Emitter,
        method: impl FnOnce(&Component, CallbackEmitter<'_>) -> ExtensionResult<R>,
    ) -> ExtensionResult<R> {
        let component = callbacks::find::<Component>(id)?;
        with_emitter(&self.imports, emitter, |emitter| {
            method(&component, emitter)
        })
    }
}

/// Transfers an authored object to the host through the shared identity table.
#[cfg(target_arch = "wasm32")]
pub(super) fn retain<I: Imports, O: callbacks::Stored>(
    imports: &I,
    object: Option<O>,
) -> Option<I::Callback> {
    object.map(|object| callbacks::Identity::<I>::new(imports).transfer(object))
}

/// Lends the existing callback emission port for one synchronous call.
pub(crate) fn with_emitter<I: Imports, R>(
    imports: &I,
    handle: &I::Emitter,
    call: impl FnOnce(CallbackEmitter<'_>) -> R,
) -> R {
    let port = Emitter { imports, handle };
    call(CallbackEmitter::new(&port))
}
