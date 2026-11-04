//! Execution-family declarations for retained extension callbacks.
#[cfg(not(target_arch = "wasm32"))]
use super::registration::ExtensionFactory;
use super::registration::{ExtensionAPI, ExtensionHandler, RegisteredToolInput};
use super::{ExtensionActions, ProviderConfig};
use crate::loader::runtime::{NOT_INITIALIZED, SET_MODEL_NOT_INITIALIZED};
use crate::{
    CallbackEmitter, EventBus, EventError, EventTail,
    types::{
        ExtensionCommandContext, ExtensionContext, MessageRenderer, renderers::MessageRendererCall,
    },
};
use maestro_models::{Execution, Local, SharedHandle, Threaded};
use maestro_models::{Model, ModelThinkingLevel as ThinkingLevel, ToolInfo, UserContent};
use maestro_resources::SlashCommandInfo;
use maestro_session::messages::{CustomMessage, MessageRenderOptions};
use maestro_session::messages::{CustomMessageInput, SendMessageOptions, SendUserMessageOptions};
use maestro_theme::LiveTheme;
use maestro_tools::tool_definition_wrapper::ToolDefinitionInput;
use maestro_tui::{AutocompleteItem, tui::ComponentHandle};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

/// Callback representations for extension registration and channel delivery.
///
/// [`Local`] callbacks and their futures use `Rc` and may capture single-thread
/// values such as terminal components. [`Threaded`], the default, keeps callbacks
/// `Send + Sync` on native targets. Channel futures are `Send`, while registered
/// handler invocation futures are local and may retain a local context. A default listener
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
/// A native action cannot retain a local-only capture:
///
/// ```compile_fail,E0277
/// use maestro_extensions::{ExtensionActions, ExtensionExecution};
/// use maestro_models::Threaded;
/// use std::{rc::Rc, sync::Arc};
/// fn threaded_action_rejects_local_capture() -> ExtensionActions {
///     let local = Rc::new(());
///     let mut actions = Threaded::unbound_actions();
///     actions.get_active_tools = Arc::new(move || {
///         let _ = &local;
///         Ok(Vec::new())
///     });
///     actions
/// }
/// ```
pub trait ExtensionExecution: maestro_tools::ToolExecution + Clone + Default {
    /// Private family-owned object-safe projection, not a root SDK accessor.
    type RegisteredDefinition: super::registration::tools::RetainedDefinition + ?Sized;
    /// Opaque typed handler storage, like the existing message-renderer registration.
    type HandlerRegistration;
    /// Typed event callback.
    type Handler<V, R>: for<'a> Fn(
            &'a mut V,
            ExtensionContext,
        ) -> <Local as Execution>::Future<'a, Result<Option<R>, EventError>>
        + ?Sized;
    /// Factory callback.
    type Factory: Fn(ExtensionAPI<Self>) -> Self::Future<'static, Result<(), EventError>> + ?Sized;
    /// Module loader callback: a resolved path to its factory, or none when the module exports none.
    #[cfg(not(target_arch = "wasm32"))]
    type ModuleLoader: Fn(&str) -> Self::Future<'static, Result<Option<ExtensionFactory<Self>>, EventError>>
        + ?Sized;
    /// Retain the concrete definition without erasing its typed state or details.
    fn retain_definition<
        P: DeserializeOwned + 'static,
        D: Serialize + 'static,
        S: Default + 'static,
    >(
        definition: RegisteredToolInput<P, D, S, Self>,
    ) -> Self::Shared<Self::RegisteredDefinition>;
    /// Retain a typed callback in the existing ordered event owner.
    fn erase_handler<V: 'static, R: 'static>(
        handler: ExtensionHandler<V, R, Self>,
    ) -> Self::HandlerRegistration;
    /// Recovers the callback only when both event and result types match.
    fn recover_handler<V: 'static, R: 'static>(
        registration: &Self::HandlerRegistration,
    ) -> Option<ExtensionHandler<V, R, Self>>;
    /// Sends a custom message.
    type SendMessage: Fn(CustomMessageInput, Option<SendMessageOptions>) -> Result<(), EventError>
        + ?Sized;
    /// Sends a user message.
    type SendUserMessage: Fn(UserContent, Option<SendUserMessageOptions>) -> Result<(), EventError>
        + ?Sized;
    /// Appends a custom entry.
    type AppendEntry: Fn(String, Option<Value>) -> Result<(), EventError> + ?Sized;
    /// Sets the session name.
    type SetSessionName: Fn(String) -> Result<(), EventError> + ?Sized;
    /// Reads the session name.
    type GetSessionName: Fn() -> Result<Option<String>, EventError> + ?Sized;
    /// Sets or clears an entry label.
    type SetLabel: Fn(String, Option<String>) -> Result<(), EventError> + ?Sized;
    /// Reads the active tool names.
    type GetActiveTools: Fn() -> Result<Vec<String>, EventError> + ?Sized;
    /// Reads every configured tool.
    type GetAllTools: Fn() -> Result<
            Vec<ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>>>,
            EventError,
        > + ?Sized;
    /// Selects the active tools.
    type SetActiveTools: Fn(Vec<String>) -> Result<(), EventError> + ?Sized;
    /// Refreshes the tool inventory.
    type RefreshTools: Fn() -> Result<(), EventError> + ?Sized;
    /// Reads the available commands.
    type GetCommands: Fn() -> Result<Vec<SlashCommandInfo>, EventError> + ?Sized;
    /// Selects a model, resolving whether it was accepted.
    type SetModel: Fn(Arc<RwLock<Model>>) -> Self::Future<'static, Result<bool, EventError>>
        + ?Sized;
    /// Reads the thinking level.
    type GetThinkingLevel: Fn() -> Result<ThinkingLevel, EventError> + ?Sized;
    /// Sets the thinking level.
    type SetThinkingLevel: Fn(ThinkingLevel) -> Result<(), EventError> + ?Sized;
    /// Registers a provider; the configuration is the platform handle, browser-local or
    /// natively shared.
    type RegisterProvider: Fn(String, SharedHandle<RwLock<ProviderConfig>>) -> Result<(), EventError>
        + ?Sized;
    /// Unregisters a provider.
    type UnregisterProvider: Fn(String) -> Result<(), EventError> + ?Sized;
    /// Factory-time operations on application-owned settings and packages.
    type HostOperation: Fn(crate::host_operations::HostOperation) -> Self::Future<'static, Result<Value, String>>
        + ?Sized;
    /// Constructs a failed operation in this execution family.
    fn failed_host_operation(error: String) -> Self::Future<'static, Result<Value, String>>;
    /// The operations of a runtime no runner has bound yet.
    fn unbound_actions() -> ExtensionActions<Self>;
    /// Command invocation callback.
    type Command: Fn(String, ExtensionCommandContext) -> Self::Future<'static, Result<(), EventError>>
        + ?Sized;
    /// Shortcut invocation callback.
    type Shortcut: Fn(ExtensionContext) -> Self::Future<'static, Result<(), EventError>> + ?Sized;
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
    /// Stores a typed renderer with the bus that invokes it.
    fn erase_message_renderer<T: 'static>(
        renderer: MessageRenderer<T, Self>,
        events: EventBus<Self>,
    ) -> Self::RendererRegistration;
    /// Returns the stored renderer when it has exactly this details type.
    fn recover_message_renderer<T: 'static>(
        registration: &Self::RendererRegistration,
    ) -> Option<MessageRenderer<T, Self>>;
    /// Returns the stored renderer with the bus that invokes it.
    fn recover_message_renderer_call<T: 'static>(
        registration: &Self::RendererRegistration,
    ) -> Option<MessageRendererCall<T, Self>>;
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
            type HostOperation = dyn Fn(crate::host_operations::HostOperation) -> Self::Future<'static, Result<Value, String>> $($bound)*;
            fn failed_host_operation(error: String) -> Self::Future<'static, Result<Value, String>> {
                Box::pin(async { Err(error) })
            }
            type RegisteredDefinition=dyn super::registration::tools::RetainedDefinition $($bound)*;
            type HandlerRegistration=Box<dyn std::any::Any $($bound)*>;
            type Handler<V,R>=dyn for<'a> Fn(&'a mut V,ExtensionContext)-><Local as Execution>::Future<'a,Result<Option<R>,EventError>> $($bound)*;
            type Factory=dyn Fn(ExtensionAPI<Self>)->Self::Future<'static,Result<(),EventError>> $($bound)*;
            #[cfg(not(target_arch = "wasm32"))]
            type ModuleLoader=dyn Fn(&str)->Self::Future<'static,Result<Option<ExtensionFactory<Self>>,EventError>> $($bound)*;
            fn retain_definition<P:DeserializeOwned+'static,D:Serialize+'static,S:Default+'static>(input:RegisteredToolInput<P,D,S,Self>)->Self::Shared<Self::RegisteredDefinition> {
                let definition=match input.definition { ToolDefinitionInput::Owned(x)=>Self::share(RwLock::new(*x)),ToolDefinitionInput::Retained(x)=>x };
                Self::share(super::registration::tools::Definition::<P,D,S,Self>{definition,project_renderers:input.project_renderers})
            }
            fn erase_handler<V:'static,R:'static>(handler:ExtensionHandler<V,R,Self>)->Self::HandlerRegistration { Box::new(handler) }

            fn recover_handler<V:'static,R:'static>(registration:&Self::HandlerRegistration)->Option<ExtensionHandler<V,R,Self>> {
                registration.downcast_ref::<ExtensionHandler<V,R,Self>>().cloned()
            }
            type SendMessage = dyn Fn(CustomMessageInput, Option<SendMessageOptions>) -> Result<(), EventError> $($bound)*;
            type SendUserMessage = dyn Fn(UserContent, Option<SendUserMessageOptions>) -> Result<(), EventError> $($bound)*;
            type AppendEntry = dyn Fn(String, Option<Value>) -> Result<(), EventError> $($bound)*;
            type SetSessionName = dyn Fn(String) -> Result<(), EventError> $($bound)*;
            type GetSessionName = dyn Fn() -> Result<Option<String>, EventError> $($bound)*;
            type SetLabel = dyn Fn(String, Option<String>) -> Result<(), EventError> $($bound)*;
            type GetActiveTools = dyn Fn() -> Result<Vec<String>, EventError> $($bound)*;
            type GetAllTools = dyn Fn() -> Result<Vec<ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>>>, EventError> $($bound)*;
            type SetActiveTools = dyn Fn(Vec<String>) -> Result<(), EventError> $($bound)*;
            type RefreshTools = dyn Fn() -> Result<(), EventError> $($bound)*;
            type GetCommands = dyn Fn() -> Result<Vec<SlashCommandInfo>, EventError> $($bound)*;
            type SetModel = dyn Fn(Arc<RwLock<Model>>) -> Self::Future<'static, Result<bool, EventError>> $($bound)*;
            type GetThinkingLevel = dyn Fn() -> Result<ThinkingLevel, EventError> $($bound)*;
            type SetThinkingLevel = dyn Fn(ThinkingLevel) -> Result<(), EventError> $($bound)*;
            type RegisterProvider = dyn Fn(String, SharedHandle<RwLock<ProviderConfig>>) -> Result<(), EventError> $($bound)*;
            type UnregisterProvider = dyn Fn(String) -> Result<(), EventError> $($bound)*;
            fn unbound_actions() -> ExtensionActions<Self> {
                ExtensionActions {
                    send_message: Self::share(|_, _| Err(NOT_INITIALIZED.into())),
                    send_user_message: Self::share(|_, _| Err(NOT_INITIALIZED.into())),
                    append_entry: Self::share(|_, _| Err(NOT_INITIALIZED.into())),
                    set_session_name: Self::share(|_| Err(NOT_INITIALIZED.into())),
                    get_session_name: Self::share(|| Err(NOT_INITIALIZED.into())),
                    set_label: Self::share(|_, _| Err(NOT_INITIALIZED.into())),
                    get_active_tools: Self::share(|| Err(NOT_INITIALIZED.into())),
                    get_all_tools: Self::share(|| Err(NOT_INITIALIZED.into())),
                    set_active_tools: Self::share(|_| Err(NOT_INITIALIZED.into())),
                    refresh_tools: Self::share(|| Ok(())),
                    get_commands: Self::share(|| Err(NOT_INITIALIZED.into())),
                    set_model: Self::share(|_| {
                        Box::pin(std::future::ready(Err(SET_MODEL_NOT_INITIALIZED.into())))
                    }),
                    get_thinking_level: Self::share(|| Err(NOT_INITIALIZED.into())),
                    set_thinking_level: Self::share(|_| Err(NOT_INITIALIZED.into())),
                }
            }
            type Command =
                dyn Fn(String, ExtensionCommandContext) -> Self::Future<'static, Result<(), EventError>> $($bound)*;
            type Shortcut =
                dyn Fn(ExtensionContext) -> Self::Future<'static, Result<(), EventError>> $($bound)*;
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
                events: EventBus<Self>,
            ) -> Self::RendererRegistration {
                Box::new(MessageRendererCall::<T, Self>::new(renderer, events))
            }
            fn recover_message_renderer<T: 'static>(
                registration: &Self::RendererRegistration,
            ) -> Option<MessageRenderer<T, Self>> {
                Self::recover_message_renderer_call::<T>(registration).map(|call| call.renderer)
            }
            fn recover_message_renderer_call<T: 'static>(
                registration: &Self::RendererRegistration,
            ) -> Option<MessageRendererCall<T, Self>> {
                registration
                    .downcast_ref::<MessageRendererCall<T, Self>>()
                    .cloned()
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
