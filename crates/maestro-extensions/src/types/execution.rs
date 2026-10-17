//! Execution-family declarations for retained extension callbacks.
use crate::{CallbackEmitter, EventBus, EventError, EventTail, types::MessageRenderer};
use maestro_models::{Execution, Local, Threaded};
use maestro_session::messages::{CustomMessage, MessageRenderOptions};
use maestro_theme::LiveTheme;
use maestro_tui::{AutocompleteItem, tui::ComponentHandle};
use serde_json::Value;
use std::{any::Any, sync::Arc};

/// Callback representations for extension registration and channel delivery.
///
/// [`Local`] callbacks and their futures use `Rc` and may capture single-thread
/// values such as terminal components. [`Threaded`], the default, keeps callbacks
/// `Send + Sync` and their futures `Send` on native targets, so a default listener
/// cannot capture a local value:
///
/// ```compile_fail,E0277
/// use maestro_extensions::EventListener;
/// use std::{rc::Rc, sync::Arc};
///
/// fn threaded_listener_rejects_local_capture() -> EventListener {
///     let local = Rc::new(());
///     Arc::new(move |_, _| {
///         let _ = &local;
///         Ok(None)
///     })
/// }
/// ```
pub trait ExtensionExecution: Execution + Clone + Default {
    /// Command invocation callback.
    type Command<C>: Fn(String, C) -> Self::Future<'static, Result<(), EventError>> + ?Sized;
    /// Shortcut invocation callback.
    type Shortcut<C>: Fn(C) -> Self::Future<'static, Result<(), EventError>> + ?Sized;
    /// Argument completion callback.
    type Completions: Fn(String) -> Self::Future<'static, Result<Option<Vec<AutocompleteItem>>, EventError>>
        + ?Sized;
    /// Listener prefix returning an optional owned continuation.
    type Listener: for<'a> Fn(
            Arc<Value>,
            CallbackEmitter<'a, Self>,
        ) -> Result<Option<EventTail<Self>>, EventError>
        + ?Sized;
    /// Caller-owned serial cooperative task admission.
    type Spawner: Fn(Self::Future<'static, ()>) + ?Sized;
    /// Diagnostic destination.
    type Reporter: Fn(&str) + ?Sized;
    /// Custom-message renderer with typed details.
    type Renderer<T>: for<'a> Fn(
            Self::Shared<CustomMessage<T>>,
            MessageRenderOptions,
            LiveTheme,
            CallbackEmitter<'a, Self>,
        ) -> Result<Option<ComponentHandle>, EventError>
        + ?Sized;
    /// A renderer stored without its details type.
    type RendererRegistration;
    /// Stores a typed renderer.
    fn erase_message_renderer<T: 'static>(
        renderer: MessageRenderer<T, Self>,
    ) -> Self::RendererRegistration;
    /// Returns the stored renderer when it has exactly this details type.
    fn recover_message_renderer<T: 'static>(
        registration: &Self::RendererRegistration,
    ) -> Option<MessageRenderer<T, Self>>;
    /// Erases the shared tail settlement, which keeps the bus alive until it finishes.
    fn tail_task(
        tail: EventTail<Self>,
        channel: String,
        events: EventBus<Self>,
    ) -> Self::Future<'static, ()>;
}

/// One declaration template; only pointer and auto-trait bounds vary.
macro_rules! family {
    ($mode:ty, [$($bound:tt)*]) => {
        impl ExtensionExecution for $mode {
            type Command<C> =
                dyn Fn(String, C) -> Self::Future<'static, Result<(), EventError>> $($bound)*;
            type Shortcut<C> = dyn Fn(C) -> Self::Future<'static, Result<(), EventError>> $($bound)*;
            type Completions = dyn Fn(String) -> Self::Future<
                'static,
                Result<Option<Vec<AutocompleteItem>>, EventError>,
            > $($bound)*;
            type Listener = dyn for<'a> Fn(
                Arc<Value>,
                CallbackEmitter<'a, Self>,
            ) -> Result<Option<EventTail<Self>>, EventError> $($bound)*;
            type Spawner = dyn Fn(Self::Future<'static, ()>) $($bound)*;
            type Reporter = dyn Fn(&str) $($bound)*;
            type Renderer<T> = dyn for<'a> Fn(
                Self::Shared<CustomMessage<T>>,
                MessageRenderOptions,
                LiveTheme,
                CallbackEmitter<'a, Self>,
            ) -> Result<Option<ComponentHandle>, EventError> $($bound)*;
            type RendererRegistration = Box<dyn Any $($bound)*>;
            fn erase_message_renderer<T: 'static>(
                renderer: MessageRenderer<T, Self>,
            ) -> Self::RendererRegistration {
                Box::new(renderer)
            }
            fn recover_message_renderer<T: 'static>(
                registration: &Self::RendererRegistration,
            ) -> Option<MessageRenderer<T, Self>> {
                registration.downcast_ref::<MessageRenderer<T, Self>>().cloned()
            }
            fn tail_task(
                tail: EventTail<Self>,
                channel: String,
                events: EventBus<Self>,
            ) -> Self::Future<'static, ()> {
                Box::pin(crate::event_bus::settle_tail::<Self>(tail, channel, events))
            }
        }
    };
}
family!(Local, []);
#[cfg(not(target_arch = "wasm32"))]
family!(Threaded, [+ Send + Sync]);
#[cfg(target_arch = "wasm32")]
family!(Threaded, []);
