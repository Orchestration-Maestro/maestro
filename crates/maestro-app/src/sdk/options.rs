//! The inputs and result of creating a session.
use crate::agent_session::{AgentSession, ToolDefinitionHandle};
use crate::resources::resource_loader::{ResourceLoader, ResourceLoaderOptions};
use maestro_catalog::{ModelRegistry, ScopedModel};
use maestro_credentials::AuthStorage;
use maestro_extensions::{LoadExtensionsResult, SessionStartEvent};
use maestro_models::{Local, Model, ModelThinkingLevel};
use maestro_session::SessionManager;
use maestro_settings::SettingsManager;
use maestro_theme::LiveTheme;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Which default tools a session without an explicit tool list starts without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoTools {
    /// No tool is enabled.
    All,
    /// The default built-in tools (read, bash, edit, write) are off; extension and SDK tools
    /// stay enabled.
    Builtin,
}

/// Where a session's skills, prompts, context files, extensions and system prompt come from.
pub enum SessionResources {
    /// A loader the caller built and loaded.
    Loader(Rc<dyn ResourceLoader<Local>>),
    /// The default loader for the session's directories, built from these inputs and loaded
    /// before the session is created. A caller that supplied no loader passes the task set
    /// and the extension module loader here.
    Default(Box<ResourceLoaderOptions<Local>>),
}

/// What [`create_agent_session`](super::create_agent_session) builds a session from; every
/// absent value takes its default.
pub struct CreateAgentSessionOptions {
    /// The working directory; absent means the supplied session manager's, else the process
    /// directory.
    pub cwd: Option<String>,
    /// The user configuration directory; absent means the default agent directory.
    pub agent_dir: Option<String>,
    /// Credentials; absent means `auth.json` in the agent directory.
    pub auth_storage: Option<Arc<AuthStorage>>,
    /// The model catalog; absent means one over `models.json` in the agent directory.
    pub model_registry: Option<Arc<ModelRegistry>>,
    /// The model; absent means the recorded one when it can be restored, else the settings
    /// default, else the first available.
    pub model: Option<Arc<RwLock<Model>>>,
    /// The thinking level; absent means the recorded one, else the settings default, else
    /// medium, clamped to the model.
    pub thinking_level: Option<ModelThinkingLevel>,
    /// Models to cycle through, in order.
    pub scoped_models: Vec<ScopedModel>,
    /// The default tools to leave off when no tool list is supplied.
    pub no_tools: Option<NoTools>,
    /// The only tools enabled, when present.
    pub tools: Option<Vec<String>>,
    /// Tool definitions registered outside extensions.
    pub custom_tools: Vec<ToolDefinitionHandle>,
    /// The resource loader, or the inputs of the default one.
    pub resources: SessionResources,
    /// The session history; absent means a new session file for the working directory.
    pub session_manager: Option<Rc<RefCell<SessionManager>>>,
    /// The preferences; absent means settings for the working and agent directories.
    pub settings_manager: Option<Rc<RefCell<SettingsManager>>>,
    /// The start event extensions receive when they are bound.
    pub session_start_event: Option<SessionStartEvent>,
    /// The published theme extensions see while no user interface is bound.
    pub theme: LiveTheme,
    /// A caller's offline selection for search programs, or the ambient flag.
    pub offline: Option<bool>,
}

/// A created session and what a frontend needs beside it.
pub struct CreateAgentSessionResult {
    /// The session.
    pub session: Rc<AgentSession>,
    /// The loaded extensions, for setting up a user interface.
    pub extensions_result: Rc<LoadExtensionsResult<Local>>,
    /// Why the recorded model was not restored, or that no model is available.
    pub model_fallback_message: Option<String>,
}
