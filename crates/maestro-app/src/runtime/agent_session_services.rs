//! Credentials, settings, the model catalog and resources bound to one session working
//! directory.
use crate::agent_session::ToolDefinitionHandle;
use crate::configuration::config::{
    get_agent_dir,
    paths::{get_auth_path, get_models_path},
};
use crate::resources::resource_loader::{
    DefaultEventBus, DefaultResourceLoader, DefaultResourceLoaderOptions, ResourceLoader,
    ResourceLoaderOptions,
};
use crate::sdk::{
    CreateAgentSessionOptions, CreateAgentSessionResult, NoTools, SessionResources,
    create_agent_session,
};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_catalog::{ConfigValues, ModelRegistry, ScopedModel};
use maestro_credentials::{AuthStorage, ProcessConfigValueOperations};
use maestro_extensions::SessionStartEvent;
use maestro_extensions::{
    ExtensionExecution, LoadExtensionsResult,
    types::{FlagType, FlagValue},
};
use maestro_models::{
    DiagnosticErrorInfo, Local, Model, ModelThinkingLevel, SharedHandle, Threaded,
};
use maestro_resources::paths::CONFIG_DIR_NAME;
use maestro_session::SessionManager;
use maestro_settings::SettingsManager;
use maestro_theme::LiveTheme;
use maestro_tools::utils::shell::get_shell_config;
use std::sync::RwLock;
use std::{cell::RefCell, collections::HashMap, io, rc::Rc, sync::Arc};

/// The severity of an [`AgentSessionRuntimeDiagnostic`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentSessionRuntimeDiagnosticType {
    /// Information.
    Info,
    /// A warning.
    Warning,
    /// An error the caller may treat as fatal.
    Error,
}

/// A non-fatal issue found while creating services; the caller decides what to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSessionRuntimeDiagnostic {
    /// The severity.
    pub r#type: AgentSessionRuntimeDiagnosticType,
    /// The complete message.
    pub message: String,
}

/// Inputs for services bound to one working directory.
///
/// Resource paths should already be absolute, so a later change of working directory
/// does not reinterpret them.
pub struct CreateAgentSessionServicesOptions<E: ExtensionExecution = Threaded> {
    /// The session working directory.
    pub cwd: String,
    /// The user configuration directory; absent means [`get_agent_dir`].
    pub agent_dir: Option<String>,
    /// Credentials; absent means `auth.json` in the agent directory.
    pub auth_storage: Option<Arc<AuthStorage>>,
    /// Settings; absent means settings for `cwd` and the agent directory.
    pub settings_manager: Option<Rc<RefCell<SettingsManager>>>,
    /// The model catalog; absent means one over `models.json` in the agent directory.
    pub model_registry: Option<Arc<ModelRegistry>>,
    /// Extension flag values by flag name.
    pub extension_flag_values: Option<IndexMap<String, FlagValue>>,
    /// The resource loader inputs.
    pub resource_loader_options: ResourceLoaderOptions<E>,
    /// A caller's offline selection for search programs, or the ambient flag.
    pub offline: Option<bool>,
}

/// Coherent infrastructure for one session working directory; no session is created.
pub struct AgentSessionServices<E: ExtensionExecution = Threaded> {
    /// The session working directory.
    pub cwd: String,
    /// The user configuration directory.
    pub agent_dir: String,
    /// Credentials.
    pub auth_storage: Arc<AuthStorage>,
    /// Settings.
    pub settings_manager: Rc<RefCell<SettingsManager>>,
    /// The model catalog, with the extensions' providers registered.
    pub model_registry: Arc<ModelRegistry>,
    /// The reloaded resources.
    pub resource_loader: Rc<dyn ResourceLoader<E>>,
    /// Problems found while creating the services.
    pub diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
    /// The startup offline selection sessions created from these services inherit.
    pub offline: Option<bool>,
}

/// An error diagnostic.
fn error(message: String) -> AgentSessionRuntimeDiagnostic {
    AgentSessionRuntimeDiagnostic {
        r#type: AgentSessionRuntimeDiagnosticType::Error,
        message,
    }
}

/// Sets each value of a registered flag on the extensions' runtime: `true` for a boolean
/// flag, the text for a string flag. Unregistered names become one error after the others.
fn apply_extension_flag_values<E: ExtensionExecution>(
    extensions: &LoadExtensionsResult<E>,
    values: IndexMap<String, FlagValue>,
) -> Vec<AgentSessionRuntimeDiagnostic> {
    let mut registered = HashMap::new();
    for extension in &extensions.extensions {
        let extension = read(extension);
        for (name, flag) in &extension.flags {
            let flag = read(flag);
            registered.entry(name.clone()).or_insert(flag.r#type);
        }
    }
    let mut diagnostics = Vec::new();
    let mut unknown = Vec::new();
    let mut flag_values = write(&extensions.runtime.flag_values);
    for (name, value) in values {
        match (registered.get(&name), value) {
            (None, _) => unknown.push(format!("--{name}")),
            (Some(FlagType::Boolean), _) => {
                flag_values.insert(name, FlagValue::Boolean(true));
            }
            (Some(FlagType::String), FlagValue::String(text)) => {
                flag_values.insert(name, FlagValue::String(text));
            }
            (Some(FlagType::String), FlagValue::Boolean(_)) => {
                diagnostics.push(error(format!(
                    "Extension flag \"--{name}\" requires a value"
                )));
            }
        }
    }
    if !unknown.is_empty() {
        let plural = if unknown.len() == 1 { "" } else { "s" };
        diagnostics.push(error(format!(
            "Unknown option{plural}: {}",
            unknown.join(", ")
        )));
    }
    diagnostics
}

/// Applies each queued provider registration of the extensions, one at a time in queue
/// order, then empties the queue; a failure becomes a diagnostic and the rest still apply.
fn register_providers<E: ExtensionExecution>(
    extensions: &LoadExtensionsResult<E>,
    model_registry: &ModelRegistry,
) -> Vec<AgentSessionRuntimeDiagnostic> {
    let pending = std::mem::take(&mut *write(
        &extensions.runtime.pending_provider_registrations,
    ));
    pending
        .into_values()
        .filter_map(|registration| {
            let failure = model_registry
                .register_provider(&registration.name, registration.config)
                .err()?;
            let path = registration.extension_path;
            Some(error(format!("Extension \"{path}\" error: {failure}")))
        })
        .collect()
}

/// Native commands that configured credential values name, behind the tools' shell selector.
pub(crate) fn native_config_values() -> ConfigValues {
    Arc::new(ProcessConfigValueOperations::new(|| {
        get_shell_config(None).map(|config| (config.shell, config.args))
    }))
}

/// Creates the services, reloads resources, applies queued provider registrations and
/// applies extension flag values.
///
/// # Errors
/// Returns agent-directory, catalog-load (wrapped as an `io::Error`) or reload failures.
pub async fn create_agent_session_services<E: DefaultEventBus>(
    options: CreateAgentSessionServicesOptions<E>,
) -> io::Result<AgentSessionServices<E>> {
    let cwd = options.cwd;
    let agent_dir = match options.agent_dir {
        Some(agent_dir) => agent_dir,
        None => get_agent_dir()?,
    };
    let auth_storage = options
        .auth_storage
        .unwrap_or_else(|| SharedHandle::new(AuthStorage::create(&get_auth_path(&agent_dir))));
    let settings_manager = options.settings_manager.unwrap_or_else(|| {
        Rc::new(RefCell::new(SettingsManager::create(
            &cwd,
            &agent_dir,
            CONFIG_DIR_NAME,
        )))
    });
    let model_registry = match options.model_registry {
        Some(model_registry) => model_registry,
        None => Arc::new(
            ModelRegistry::create(
                Arc::clone(&auth_storage),
                get_models_path(&agent_dir),
                native_config_values(),
            )
            .map_err(io::Error::other)?,
        ),
    };
    let mut resources = options.resource_loader_options;
    resources.tool_construction.get_or_insert_with(|| {
        let bin_dir = crate::configuration::config::paths::get_bin_dir(&agent_dir);
        maestro_extensions::ToolConstruction {
            tools_manager: Arc::new(
                maestro_tools::ToolsManager::new(bin_dir.clone(), None)
                    .with_offline(options.offline),
            ),
            bin_dir,
        }
    });
    let resource_loader =
        default_resource_loader(&cwd, &agent_dir, &settings_manager, resources).await?;
    model_registry
        .set_base_model_files(resource_loader.get_model_files())
        .map_err(io::Error::other)?;
    let extensions = resource_loader.get_extensions();
    let mut diagnostics = register_providers(&extensions, &model_registry);
    if let Some(values) = options.extension_flag_values {
        diagnostics.extend(apply_extension_flag_values(&extensions, values));
    }
    Ok(AgentSessionServices {
        cwd,
        agent_dir,
        auth_storage,
        settings_manager,
        model_registry,
        resource_loader,
        diagnostics,
        offline: options.offline,
    })
}

/// The default resource loader for `cwd` and `agent_dir` over the shared settings, loaded.
///
/// # Errors
/// Returns the reload failure.
pub(crate) async fn default_resource_loader<E: DefaultEventBus>(
    cwd: &str,
    agent_dir: &str,
    settings_manager: &Rc<RefCell<SettingsManager>>,
    resources: ResourceLoaderOptions<E>,
) -> io::Result<Rc<dyn ResourceLoader<E>>> {
    let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
        cwd: cwd.to_owned(),
        agent_dir: agent_dir.to_owned(),
        settings_manager: Some(Rc::clone(settings_manager)),
        resources,
    });
    loader.reload().await?;
    Ok(Rc::new(loader))
}

/// What [`create_agent_session_from_services`] adds to existing services.
pub struct CreateAgentSessionFromServicesOptions {
    /// The services the session is created over.
    pub services: Rc<AgentSessionServices<Local>>,
    /// The session history.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The start event extensions receive when they are bound.
    pub session_start_event: Option<SessionStartEvent>,
    /// The model; absent means the recorded or default one.
    pub model: Option<Arc<RwLock<Model>>>,
    /// The thinking level; absent means the recorded or default one.
    pub thinking_level: Option<ModelThinkingLevel>,
    /// Models to cycle through, in order.
    pub scoped_models: Vec<ScopedModel>,
    /// The only tools enabled, when present.
    pub tools: Option<Vec<String>>,
    /// The default tools to leave off when no tool list is supplied.
    pub no_tools: Option<NoTools>,
    /// Tool definitions registered outside extensions.
    pub custom_tools: Vec<ToolDefinitionHandle>,
    /// The published theme extensions see while no user interface is bound.
    pub theme: LiveTheme,
}

/// Creates a session over services that already exist, with their directories, credentials,
/// catalog, preferences and resources.
///
/// # Errors
/// Returns the failures of [`create_agent_session`].
pub async fn create_agent_session_from_services(
    options: CreateAgentSessionFromServicesOptions,
) -> Result<CreateAgentSessionResult, DiagnosticErrorInfo> {
    let services = options.services;
    create_agent_session(CreateAgentSessionOptions {
        cwd: Some(services.cwd.clone()),
        agent_dir: Some(services.agent_dir.clone()),
        auth_storage: Some(Arc::clone(&services.auth_storage)),
        model_registry: Some(Arc::clone(&services.model_registry)),
        model: options.model,
        thinking_level: options.thinking_level,
        scoped_models: options.scoped_models,
        no_tools: options.no_tools,
        tools: options.tools,
        custom_tools: options.custom_tools,
        resources: SessionResources::Loader(Rc::clone(&services.resource_loader)),
        session_manager: Some(options.session_manager),
        settings_manager: Some(Rc::clone(&services.settings_manager)),
        session_start_event: options.session_start_event,
        theme: options.theme,
        offline: services.offline,
    })
    .await
}
