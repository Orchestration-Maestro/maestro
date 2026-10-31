//! Owned records for extension registration.
use crate::ExtensionContext;
use crate::{EventBus, ExtensionExecution};
use indexmap::IndexMap;
use maestro_models::Local;
use maestro_models::{Execution, Threaded};
use maestro_resources::SourceInfo;
use maestro_tools::{
    ToolDefinitionRecord, ToolExecution, renderer_binding::ToolRendererBinding,
    tool_definition_wrapper::ToolDefinitionInput,
};
use maestro_tui::KeyId;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};

/// The declared flag kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagType {
    /// A boolean declaration.
    Boolean,
    /// A string declaration.
    String,
}
/// A flag value, independent of its declared kind.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum FlagValue {
    /// A boolean value.
    Boolean(bool),
    /// A string value.
    String(String),
}
/// An owned asynchronous command callback.
pub type CommandHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Command>;
/// An owned asynchronous shortcut callback.
pub type ShortcutHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Shortcut>;
/// An owned asynchronous argument completion callback.
pub type ArgumentCompletions<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Completions>;

/// Command registration options.
pub struct RegisteredCommandOptions<E: ExtensionExecution = Threaded> {
    /// Optional command description.
    pub description: Option<String>,
    /// Optional argument completion callback.
    pub get_argument_completions: Option<ArgumentCompletions<E>>,
    /// Command invocation callback.
    pub handler: CommandHandler<E>,
}

/// One owned command registration.
pub struct RegisteredCommand<E: ExtensionExecution = Threaded> {
    /// Raw command name.
    pub name: String,
    /// Retained source object.
    pub source_info: Arc<RwLock<SourceInfo>>,
    /// Optional command description.
    pub description: Option<String>,
    /// Optional argument completion callback.
    pub get_argument_completions: Option<ArgumentCompletions<E>>,
    /// Command invocation callback.
    pub handler: CommandHandler<E>,
}

/// A shallow command copy with its current invocation name.
pub struct ResolvedCommand<E: ExtensionExecution = Threaded> {
    /// Copied registration retaining callback and source owners.
    pub command: RegisteredCommand<E>,
    /// Invocation name supplied by the resolving query.
    pub invocation_name: String,
}

/// Flag registration options.
pub struct ExtensionFlagOptions {
    /// Optional flag description.
    pub description: Option<String>,
    /// Declared value kind.
    pub r#type: FlagType,
    /// Optional default shared value.
    pub default: Option<FlagValue>,
}

/// One retained flag registration.
pub struct ExtensionFlag {
    /// Raw flag name.
    pub name: String,
    /// Optional flag description.
    pub description: Option<String>,
    /// Declared value kind.
    pub r#type: FlagType,
    /// Supplied default value.
    pub default: Option<FlagValue>,
    /// Supplied registration-owner path.
    pub extension_path: String,
}

/// Shortcut registration options.
pub struct ExtensionShortcutOptions<E: ExtensionExecution = Threaded> {
    /// Optional shortcut description.
    pub description: Option<String>,
    /// Shortcut invocation callback.
    pub handler: ShortcutHandler<E>,
}

/// One retained shortcut registration.
pub struct ExtensionShortcut<E: ExtensionExecution = Threaded> {
    /// Raw shortcut identifier.
    pub shortcut: KeyId,
    /// Optional shortcut description.
    pub description: Option<String>,
    /// Shortcut invocation callback.
    pub handler: ShortcutHandler<E>,
    /// Supplied registration-owner path.
    pub extension_path: String,
}

/// A mutable owner of extension registrations.
pub struct Extension<E: ExtensionExecution = Threaded> {
    /// Supplied extension path.
    pub path: String,
    /// Supplied resolved extension path.
    pub resolved_path: String,
    /// Current owner-produced provenance object.
    pub source_info: Arc<RwLock<SourceInfo>>,
    /// Ordered event handlers, retaining duplicate callbacks.
    pub handlers: IndexMap<String, Vec<E::HandlerRegistration>>,
    /// Ordered heterogeneous tool definitions.
    pub tools: IndexMap<String, RegisteredTool<E>>,
    /// Ordered command registrations.
    pub commands: IndexMap<String, E::Shared<RwLock<RegisteredCommand<E>>>>,
    /// Ordered flag registrations.
    pub flags: IndexMap<String, Arc<RwLock<ExtensionFlag>>>,
    /// First supplied defaults, retained independently of later metadata replacements.
    pub initial_flag_values: IndexMap<String, FlagValue>,
    /// Ordered shortcut registrations.
    pub shortcuts: IndexMap<KeyId, E::Shared<RwLock<ExtensionShortcut<E>>>>,
    /// Ordered custom-message renderers by custom type.
    pub message_renderers: IndexMap<String, E::RendererRegistration>,
}

/// Shared flag values and registration activity.
pub struct ExtensionRuntimeState {
    /// Current shared flag values.
    pub flag_values: RwLock<IndexMap<String, FlagValue>>,
    /// Registration activity, shared by every facade over the runtime.
    pub(crate) activity: ActivityGuard,
}

/// The activity of a runtime or of one runner: active until its first invalidation.
#[derive(Default)]
pub(crate) struct ActivityGuard {
    /// First invalidation message.
    pub(crate) stale_message: RwLock<Option<String>>,
}

/// Registration facade borrowing its owner and retaining runtime and event bus.
pub struct ExtensionAPI<E: ExtensionExecution = Threaded> {
    /// Existing shared event capability.
    pub events: EventBus<E>,
    /// Registration owner.
    pub(crate) extension: E::Weak<RwLock<Extension<E>>>,
    /// Shared values and activity guard.
    pub(crate) runtime: E::Shared<super::ExtensionRuntime<E>>,
    /// Directory supplied when the facade was created.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) cwd: String,
}

/// An owned mutable extension record.
pub type ExtensionHandle<E = Threaded> = <E as Execution>::Shared<RwLock<Extension<E>>>;
/// The shared current extension load order.
pub type ExtensionCollection<E = Threaded> =
    <E as Execution>::Shared<RwLock<Vec<ExtensionHandle<E>>>>;
impl<E: ExtensionExecution> Clone for RegisteredCommand<E> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            source_info: Arc::clone(&self.source_info),
            description: self.description.clone(),
            get_argument_completions: self.get_argument_completions.clone(),
            handler: self.handler.clone(),
        }
    }
}
impl<E: ExtensionExecution> Deref for ResolvedCommand<E> {
    type Target = RegisteredCommand<E>;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}
impl<E: ExtensionExecution> Clone for ExtensionAPI<E> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            extension: self.extension.clone(),
            runtime: self.runtime.clone(),
            #[cfg(not(target_arch = "wasm32"))]
            cwd: self.cwd.clone(),
        }
    }
}

/// Retained tool projections.
pub(crate) mod tools;
pub use tools::ToolDefinitionMetadata;

/// Extension-owned typed definition. Callback ownership and invocation are independent.
type Definition<P = Value, D = Value, S = Value, E = Threaded> =
    ToolDefinitionRecord<ExtensionContext, P, D, S, E, Local>;
/// Per-call Local context factory retained by wrapped tools, never by the registry.
type ContextFactory = maestro_tools::types::local::ToolContextFactory<ExtensionContext>;
/// One typed handler; mutations remain borrowed through its awaited continuation.
pub type ExtensionHandler<V, R, E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Handler<V, R>>;
/// Controlled factory callback with synchronous-ready or suspended initialization.
pub type ExtensionFactory<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Factory>;
/// Turns a resolved extension path into its factory, or none when the module exports none.
#[cfg(not(target_arch = "wasm32"))]
pub type ModuleLoader<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::ModuleLoader>;

/// One path that failed to load.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionLoadError {
    /// The path as supplied.
    pub path: String,
    /// The complete failure message.
    pub error: String,
}
/// Extensions loaded in path order, the paths that failed, and the runtime they share.
#[cfg(not(target_arch = "wasm32"))]
pub struct LoadExtensionsResult<E: ExtensionExecution = Threaded> {
    /// Loaded extensions in path order.
    pub extensions: Vec<ExtensionHandle<E>>,
    /// Failed paths in path order.
    pub errors: Vec<ExtensionLoadError>,
    /// The shared runtime; its actions fail until a runner binds it.
    pub runtime: E::Shared<super::ExtensionRuntime<E>>,
}

/// Existing renderer-binding choice retained without changing execution or registration ownership.
type RendererProjection<P, D, S, E> =
    fn(&Definition<P, D, S, E>) -> Result<ToolRendererBinding, maestro_models::DiagnosticErrorInfo>;
/// One native registration argument, preserving the existing typed and raw binding routes.
pub struct RegisteredToolInput<P, D, S, E: ExtensionExecution = Threaded> {
    /// The original owned or retained definition.
    pub(super) definition: ToolDefinitionInput<ExtensionContext, P, D, S, E, Local>,
    /// No supplied callback runs during this projection.
    pub(super) project_renderers: RendererProjection<P, D, S, E>,
}
impl<P, D, S: Default + 'static, E: ExtensionExecution> RegisteredToolInput<P, D, S, E> {
    /// Selects the existing raw/no-result-renderer binding without requiring a details decoder.
    /// A typed result slot retains the existing lower-owner diagnostic when binding is requested.
    #[must_use]
    pub fn from_raw(
        definition: impl Into<ToolDefinitionInput<ExtensionContext, P, D, S, E, Local>>,
    ) -> Self {
        Self {
            definition: definition.into(),
            project_renderers: ToolRendererBinding::from_raw,
        }
    }
}
impl<P, D: DeserializeOwned + 'static, S: Default + 'static, E: ExtensionExecution>
    From<ToolDefinitionInput<ExtensionContext, P, D, S, E, Local>>
    for RegisteredToolInput<P, D, S, E>
{
    fn from(definition: ToolDefinitionInput<ExtensionContext, P, D, S, E, Local>) -> Self {
        Self {
            definition,
            project_renderers: |definition| Ok(ToolRendererBinding::from_definition(definition)),
        }
    }
}
impl<P, D: DeserializeOwned + 'static, S: Default + 'static, E: ExtensionExecution>
    From<Definition<P, D, S, E>> for RegisteredToolInput<P, D, S, E>
{
    fn from(definition: Definition<P, D, S, E>) -> Self {
        Self::from(ToolDefinitionInput::Owned(Box::new(definition)))
    }
}

/// One retained definition with source identity.
pub struct RegisteredTool<E: ExtensionExecution = Threaded> {
    /// The definition owner.
    pub definition: E::Shared<E::RegisteredDefinition>,
    /// The source info owner.
    pub source_info: Arc<RwLock<SourceInfo>>,
}
impl<E: ExtensionExecution> RegisteredTool<E> {
    /// The definition's current model-facing declaration, label, prompt text and options.
    #[must_use]
    pub fn metadata(&self) -> ToolDefinitionMetadata {
        tools::RetainedDefinition::metadata(&*self.definition)
    }
    /// The definition's renderers, bound for row rendering.
    ///
    /// # Errors
    /// Returns the lower binding diagnostic for a typed result slot on the raw route.
    pub fn renderer_binding(
        &self,
    ) -> Result<ToolRendererBinding, maestro_models::DiagnosticErrorInfo> {
        tools::RetainedDefinition::renderer_binding(&*self.definition)
    }
}
impl<E: ExtensionExecution> Clone for RegisteredTool<E> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            source_info: self.source_info.clone(),
        }
    }
}

/// A registered tool with the concrete extension context and local invocation.
///
/// ```
/// use maestro_extensions::{ToolDefinition, define_tool};
/// use maestro_models::Tool;
/// use serde_json::{Value, json};
/// use std::sync::Arc;
///
/// let tool: ToolDefinition<Value, Value, ()> = define_tool(ToolDefinition {
///     definition: Tool { name: "echo".into(), description: "Echo arguments".into(),
///         parameters: json!({"type":"object"}).into() },
///     label: "Echo".into(), prompt_snippet: None, prompt_guidelines: None,
///     render_shell: None, prepare_arguments: None, execution_mode: None,
///     execute: Arc::new(|_, params, _, _, context| Box::pin(async move {
///         assert!(context.is_some());
///         Ok(maestro_agent::AgentToolResult {
///             content: vec![], details: Some(params), terminate: None, usage: None,
///         })
///     })),
///     render_call: None, render_result: None,
/// });
/// assert_eq!(tool.definition.name, "echo");
/// ```
pub type ToolDefinition<P, D, S> = maestro_tools::ToolDefinition<ExtensionContext, P, D, S, Local>;
