//! Custom-message renderer callbacks.
use crate::{EventBus, EventError, ExtensionExecution};
use maestro_models::{Execution, Threaded};
use maestro_session::message_records::CustomMessage;
pub use maestro_session::message_records::MessageRenderOptions;
use maestro_theme::LiveTheme;
use maestro_tui::interface::ComponentHandle;
use serde_json::Value;

/// Renders a custom message, or returns no component to keep the host's default.
pub type MessageRenderer<T = Value, E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Renderer<T>>;

/// A renderer retained with the event bus that registered it.
pub struct MessageRendererCall<T = Value, E: ExtensionExecution = Threaded> {
    /// Original typed callback, without a reporting wrapper.
    pub(crate) renderer: MessageRenderer<T, E>,
    /// Bus holding the registering extension's identity.
    events: EventBus<E>,
}
impl<T, E: ExtensionExecution> Clone for MessageRendererCall<T, E> {
    /// Shares both retained owners without requiring cloneable details.
    fn clone(&self) -> Self {
        Self {
            renderer: self.renderer.clone(),
            events: self.events.clone(),
        }
    }
}
impl<T, E: ExtensionExecution> MessageRendererCall<T, E> {
    /// Retains a supplied renderer and the bus that invokes it.
    #[must_use]
    pub fn new(renderer: MessageRenderer<T, E>, events: EventBus<E>) -> Self {
        Self { renderer, events }
    }
    /// Invokes the retained renderer through its ordinary callback frame.
    ///
    /// # Errors
    ///
    /// Returns the renderer's original failure.
    pub fn call(
        &self,
        message: E::Shared<CustomMessage<T>>,
        options: MessageRenderOptions,
        theme: LiveTheme,
    ) -> Result<Option<ComponentHandle>, EventError> {
        self.events
            .invoke_callback(|emitter| (self.renderer)(message, options, theme, emitter))
    }
}
