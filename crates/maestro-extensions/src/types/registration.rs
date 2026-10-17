//! Owned records for extension registration.
use crate::{EventBus, ExtensionExecution};
use indexmap::IndexMap;
use maestro_models::{Execution, Threaded};
use maestro_resources::SourceInfo;
use maestro_tui::KeyId;
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Ordered command registrations.
    pub commands: IndexMap<String, E::Shared<RwLock<RegisteredCommand<E>>>>,
    /// Ordered flag registrations.
    pub flags: IndexMap<String, Arc<RwLock<ExtensionFlag>>>,
    /// Ordered shortcut registrations.
    pub shortcuts: IndexMap<KeyId, E::Shared<RwLock<ExtensionShortcut<E>>>>,
    /// Ordered custom-message renderers by custom type.
    pub message_renderers: IndexMap<String, E::RendererRegistration>,
}

/// Shared flag values and registration activity.
pub struct ExtensionRuntimeState {
    /// Current shared flag values.
    pub flag_values: RwLock<IndexMap<String, FlagValue>>,
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
    pub(crate) runtime: Arc<ExtensionRuntimeState>,
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
            runtime: Arc::clone(&self.runtime),
        }
    }
}
