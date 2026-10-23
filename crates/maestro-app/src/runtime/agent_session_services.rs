//! Credentials, settings, the model catalog and resources bound to one session working
//! directory.
use crate::configuration::config::{
    get_agent_dir,
    paths::{get_auth_path, get_models_path},
};
use crate::resources::resource_loader::{
    DefaultEventBus, DefaultResourceLoader, DefaultResourceLoaderOptions, ResourceLoader,
    ResourceLoaderOptions,
};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_catalog::ModelRegistry;
use maestro_credentials::AuthStorage;
use maestro_extensions::{
    ExtensionExecution, LoadExtensionsResult,
    types::{FlagType, FlagValue},
};
use maestro_models::{SharedHandle, Threaded};
use maestro_resources::paths::CONFIG_DIR_NAME;
use maestro_settings::SettingsManager;
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
            registered.insert(name.clone(), flag.r#type);
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
            ModelRegistry::create(Arc::clone(&auth_storage), get_models_path(&agent_dir))
                .map_err(io::Error::other)?,
        ),
    };
    let resource_loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
        cwd: cwd.clone(),
        agent_dir: agent_dir.clone(),
        settings_manager: Some(Rc::clone(&settings_manager)),
        resources: options.resource_loader_options,
    });
    resource_loader.reload().await?;
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
        resource_loader: Rc::new(resource_loader),
        diagnostics,
    })
}
