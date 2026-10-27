//! Shared runtime actions and attributed failures.
use super::{ExtensionExecution, ExtensionRuntimeState, PendingProviderRegistration};
use crate::loader::runtime::ProviderRoute;
use maestro_models::{Execution, Threaded};
use std::{ops::Deref, rc::Rc, sync::RwLock};
/// Sends a custom message to the session.
pub type SendMessageHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SendMessage>;
/// Sends a user message to the agent.
pub type SendUserMessageHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SendUserMessage>;
/// Appends a custom entry to the session.
pub type AppendEntryHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::AppendEntry>;
/// Sets the session display name.
pub type SetSessionNameHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SetSessionName>;
/// Reads the session display name.
pub type GetSessionNameHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::GetSessionName>;
/// Sets or clears a label on an entry.
pub type SetLabelHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SetLabel>;
/// Reads the active tool names.
pub type GetActiveToolsHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::GetActiveTools>;
/// Reads every configured tool.
pub type GetAllToolsHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::GetAllTools>;
/// Selects the active tools by name.
pub type SetActiveToolsHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SetActiveTools>;
/// Refreshes the tool inventory.
pub type RefreshToolsHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::RefreshTools>;
/// Reads the available slash commands.
pub type GetCommandsHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::GetCommands>;
/// Selects a model.
pub type SetModelHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SetModel>;
/// Reads the thinking level.
pub type GetThinkingLevelHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::GetThinkingLevel>;
/// Sets the thinking level.
pub type SetThinkingLevelHandler<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::SetThinkingLevel>;
/// The operations a runner binds behind every extension API.
#[derive(Clone)]
pub struct ExtensionActions<E: ExtensionExecution = Threaded> {
    /// Sends a custom message.
    pub send_message: SendMessageHandler<E>,
    /// Sends a user message.
    pub send_user_message: SendUserMessageHandler<E>,
    /// Appends a custom entry.
    pub append_entry: AppendEntryHandler<E>,
    /// Sets the session name.
    pub set_session_name: SetSessionNameHandler<E>,
    /// Reads the session name.
    pub get_session_name: GetSessionNameHandler<E>,
    /// Sets or clears an entry label.
    pub set_label: SetLabelHandler<E>,
    /// Reads the active tool names.
    pub get_active_tools: GetActiveToolsHandler<E>,
    /// Reads every configured tool.
    pub get_all_tools: GetAllToolsHandler<E>,
    /// Selects the active tools.
    pub set_active_tools: SetActiveToolsHandler<E>,
    /// Refreshes the tool inventory.
    pub refresh_tools: RefreshToolsHandler<E>,
    /// Reads the available commands.
    pub get_commands: GetCommandsHandler<E>,
    /// Selects a model.
    pub set_model: SetModelHandler<E>,
    /// Reads the thinking level.
    pub get_thinking_level: GetThinkingLevelHandler<E>,
    /// Sets the thinking level.
    pub set_thinking_level: SetThinkingLevelHandler<E>,
}

/// The shared runtime behind every extension API and its runner.
pub struct ExtensionRuntime<E: ExtensionExecution = Threaded> {
    /// Original settings and package operations, available without runner binding.
    pub(crate) host_operations: RwLock<Option<E::Shared<E::HostOperation>>>,
    /// Application-supplied constructor defaults, installed before factory loading.
    pub(crate) construction: RwLock<Option<crate::CatalogConstruction>>,
    /// Application-supplied native search and shell construction owners.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) tool_construction: RwLock<Option<crate::ToolConstruction>>,
    /// The single host-admitted channel open only while base factories load.
    pub(crate) load_time: RwLock<Option<crate::loader::LoadTimeState>>,
    /// Flag values and the registration activity guard.
    pub(crate) state: ExtensionRuntimeState,
    /// The bound operations; not-initialized until a runner binds.
    pub actions: RwLock<ExtensionActions<E>>,
    /// Registrations retained until binding; callers may inspect or edit them.
    pub pending_provider_registrations:
        RwLock<std::collections::BTreeMap<u64, PendingProviderRegistration>>,
    /// The bound provider route, absent until the initial flush completes.
    pub(crate) providers: RwLock<ProviderRoute<E>>,
}

impl<E: ExtensionExecution> Deref for ExtensionRuntime<E> {
    type Target = ExtensionRuntimeState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

/// A failure attributed to an extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionError {
    /// Path of the extension the failure belongs to.
    pub extension_path: String,
    /// The operation or event that failed.
    pub event: String,
    /// The failure message.
    pub error: String,
    /// Supplied stack text.
    pub stack: Option<String>,
}

/// Receives each attributed extension failure.
pub type ExtensionErrorListener = Rc<dyn Fn(&ExtensionError) -> Result<(), crate::EventError>>;
/// Reports a retained UI callback's failure under its event name, attributed to the extension
/// that registered it.
pub type UIErrorReporter = Rc<dyn Fn(&str, &crate::EventError) -> Result<(), crate::EventError>>;
