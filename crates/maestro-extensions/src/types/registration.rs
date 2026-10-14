//! Owned records for extension registration.
use crate::{EventBus, EventError};
use indexmap::IndexMap;
use maestro_models::BoxFuture;
use maestro_resources::SourceInfo;
use maestro_tui::{AutocompleteItem, KeyId};
use std::{
    ops::Deref,
    sync::{Arc, RwLock, Weak},
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
/// An owned asynchronous `CommandHandler` callback.
#[cfg(not(target_arch = "wasm32"))]
pub type CommandHandler<C> =
    Arc<dyn Fn(String, C) -> BoxFuture<Result<(), EventError>> + Send + Sync>;
/// An owned asynchronous `CommandHandler` callback.
#[cfg(target_arch = "wasm32")]
pub type CommandHandler<C> = Arc<dyn Fn(String, C) -> BoxFuture<Result<(), EventError>>>;
/// An owned asynchronous `ShortcutHandler` callback.
#[cfg(not(target_arch = "wasm32"))]
pub type ShortcutHandler<C> = Arc<dyn Fn(C) -> BoxFuture<Result<(), EventError>> + Send + Sync>;
/// An owned asynchronous `ShortcutHandler` callback.
#[cfg(target_arch = "wasm32")]
pub type ShortcutHandler<C> = Arc<dyn Fn(C) -> BoxFuture<Result<(), EventError>>>;
/// An owned asynchronous `ArgumentCompletions` callback.
#[cfg(not(target_arch = "wasm32"))]
pub type ArgumentCompletions = Arc<
    dyn Fn(String) -> BoxFuture<Result<Option<Vec<AutocompleteItem>>, EventError>> + Send + Sync,
>;
/// An owned asynchronous `ArgumentCompletions` callback.
#[cfg(target_arch = "wasm32")]
pub type ArgumentCompletions =
    Arc<dyn Fn(String) -> BoxFuture<Result<Option<Vec<AutocompleteItem>>, EventError>>>;

/// Command registration options.
pub struct RegisteredCommandOptions<C> {
    /// Optional command description.
    pub description: Option<String>,
    /// Optional argument completion callback.
    pub get_argument_completions: Option<ArgumentCompletions>,
    /// Command invocation callback.
    pub handler: CommandHandler<C>,
}

/// One owned command registration.
pub struct RegisteredCommand<C> {
    /// Raw command name.
    pub name: String,
    /// Retained source object.
    pub source_info: Arc<RwLock<SourceInfo>>,
    /// Optional command description.
    pub description: Option<String>,
    /// Optional argument completion callback.
    pub get_argument_completions: Option<ArgumentCompletions>,
    /// Command invocation callback.
    pub handler: CommandHandler<C>,
}

/// A shallow command copy with its current invocation name.
pub struct ResolvedCommand<C> {
    /// Copied registration retaining callback and source owners.
    pub command: RegisteredCommand<C>,
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
pub struct ExtensionShortcutOptions<C> {
    /// Optional shortcut description.
    pub description: Option<String>,
    /// Shortcut invocation callback.
    pub handler: ShortcutHandler<C>,
}

/// One retained shortcut registration.
pub struct ExtensionShortcut<C> {
    /// Raw shortcut identifier.
    pub shortcut: KeyId,
    /// Optional shortcut description.
    pub description: Option<String>,
    /// Shortcut invocation callback.
    pub handler: ShortcutHandler<C>,
    /// Supplied registration-owner path.
    pub extension_path: String,
}

/// A mutable owner of extension registrations.
pub struct Extension<C, CC> {
    /// Supplied extension path.
    pub path: String,
    /// Supplied resolved extension path.
    pub resolved_path: String,
    /// Current owner-produced provenance object.
    pub source_info: Arc<RwLock<SourceInfo>>,
    /// Ordered command registrations.
    pub commands: IndexMap<String, Arc<RwLock<RegisteredCommand<CC>>>>,
    /// Ordered flag registrations.
    pub flags: IndexMap<String, Arc<RwLock<ExtensionFlag>>>,
    /// Ordered shortcut registrations.
    pub shortcuts: IndexMap<KeyId, Arc<RwLock<ExtensionShortcut<C>>>>,
}

/// Shared flag values and registration activity.
pub struct ExtensionRuntimeState {
    /// Current shared flag values.
    pub flag_values: RwLock<IndexMap<String, FlagValue>>,
    /// First invalidation message.
    pub(crate) stale_message: RwLock<Option<String>>,
}

/// Registration facade borrowing its owner and retaining runtime and event bus.
pub struct ExtensionAPI<C, CC> {
    /// Existing shared event capability.
    pub events: EventBus,
    /// Registration owner.
    pub(crate) extension: Weak<RwLock<Extension<C, CC>>>,
    /// Shared values and activity guard.
    pub(crate) runtime: Arc<ExtensionRuntimeState>,
}

/// An owned mutable extension record.
pub type ExtensionHandle<C, CC> = Arc<RwLock<Extension<C, CC>>>;
/// The shared current extension load order.
pub type ExtensionCollection<C, CC> = Arc<RwLock<Vec<ExtensionHandle<C, CC>>>>;
impl<C> Clone for RegisteredCommand<C> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            source_info: Arc::clone(&self.source_info),
            description: self.description.clone(),
            get_argument_completions: self.get_argument_completions.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}
impl<C> Deref for ResolvedCommand<C> {
    type Target = RegisteredCommand<C>;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}
impl<C, CC> Clone for ExtensionAPI<C, CC> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            extension: self.extension.clone(),
            runtime: Arc::clone(&self.runtime),
        }
    }
}
