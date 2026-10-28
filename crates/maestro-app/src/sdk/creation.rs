//! Creating a session with every default filled in.
use super::options::{
    CreateAgentSessionOptions, CreateAgentSessionResult, NoTools, SessionResources,
};
use super::request::build_agent;
use crate::agent_session::{
    AgentSession, AgentSessionConfig, ExtensionRunnerRef, agent_messages, model_level, saved_level,
    thrown,
};
use crate::configuration::config::{
    get_agent_dir, get_docs_path,
    paths::{get_auth_path, get_bin_dir, get_models_path},
};
use crate::runtime::agent_session_services::{default_resource_loader, native_config_values};
use crate::utils::lock::{read, write};
use maestro_catalog::{
    DEFAULT_THINKING_LEVEL, FindInitialModelOptions, ModelRegistry, find_initial_model,
    model_id_comparator, restore_saved_model,
};
use maestro_credentials::{AuthStorage, format_no_models_available_message};
use maestro_models::{DiagnosticErrorInfo, Model, ModelThinkingLevel, clamp_thinking_level};
use maestro_resources::paths::CONFIG_DIR_NAME;
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_settings::SettingsManager;
use maestro_tools::ToolsManager;
use serde_json::Value;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The built-in tools a session starts with when nothing else is asked for.
const DEFAULT_ACTIVE_TOOL_NAMES: [&str; 4] = ["read", "bash", "edit", "write"];

/// The credentials, catalog and preferences a session is created over.
pub(super) struct Services {
    /// The working directory.
    pub(super) cwd: String,
    /// The user configuration directory.
    pub(super) agent_dir: String,
    /// The model catalog.
    pub(super) model_registry: Arc<ModelRegistry>,
    /// The preferences.
    pub(super) settings_manager: Rc<RefCell<SettingsManager>>,
}

/// What the recorded history says about the model and thinking level.
struct Recorded {
    /// Whether the history holds messages.
    has_messages: bool,
    /// Whether the branch holds a thinking-level entry.
    has_thinking_entry: bool,
    /// The recorded model's provider and identity.
    model: Option<(String, String)>,
    /// The recorded thinking level.
    thinking_level: Option<ModelThinkingLevel>,
    /// The recorded messages.
    messages: Vec<Value>,
}

/// Creates a session: fills every absent input with its default, restores the recorded model
/// and thinking level when it can, builds the agent over the providers with the request
/// options and extension hooks, and records the first history entries of a new session.
///
/// The working directory is the explicit one, else the supplied session manager's, else the
/// process directory. The model is the explicit one, else the recorded one when the catalog
/// has it with credentials, else the initial-model choice; the thinking level is the explicit
/// one, else the recorded one, else the settings default, else medium, clamped to the model
/// and off without one. With [`SessionResources::Default`] the default loader is built for
/// the working and agent directories and loaded first.
///
/// # Examples
///
/// A controlled session streams one answer without a network request. The disposable
/// directory owner removes the working and agent directories when the run ends.
///
/// ```
/// use maestro_app::{
///     AgentEvent, AgentSessionEvent, AssistantMessageEvent, AuthStorage, ConfigValueOperations,
///     CreateAgentSessionOptions, Local, ModelRegistry, ModuleLoader, NoTools, PromptOptions,
///     ResourceKind, ResourceLoaderOptions, SessionManager, SessionResources, SettingsManager,
///     create_agent_session,
/// };
/// use maestro_models::{
///     FauxAssistantMessageOptions, FauxResponseStep, RegisterFauxProviderOptions,
///     faux_assistant_message, register_faux_provider,
/// };
/// use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};
///
/// struct NoCommands;
/// impl ConfigValueOperations for NoCommands {
///     fn execute(&self, _: &str) -> Option<Vec<u8>> { None }
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let root = maestro_test_directory::Scratch::new()?;
///     let cwd = root.path().join("project");
///     let agent_dir = root.path().join("agent");
///     std::fs::create_dir_all(&cwd)?;
///     std::fs::create_dir_all(&agent_dir)?;
///     let cwd = cwd.to_string_lossy().into_owned();
///     let agent_dir = agent_dir.to_string_lossy().into_owned();
///     let executor = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
///     let local = Rc::new(tokio::task::LocalSet::new());
///     executor.block_on(local.run_until(async {
///         let provider = register_faux_provider(RegisterFauxProviderOptions::default());
///         provider.set_responses(vec![FauxResponseStep::Message(Box::new(
///             faux_assistant_message("Ready.", FauxAssistantMessageOptions::default()),
///         ))]);
///         let auth = Arc::new(AuthStorage::in_memory(Default::default()));
///         auth.set_runtime_api_key("faux", "controlled-key");
///         let registry = Arc::new(ModelRegistry::in_memory(auth.clone(), Arc::new(NoCommands))?);
///         let module_loader: ModuleLoader<Local> =
///             Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
///         let mut resources = ResourceLoaderOptions::new(Rc::clone(&local), module_loader);
///         resources.disabled.extend([
///             ResourceKind::Extensions, ResourceKind::Skills, ResourceKind::PromptTemplates,
///             ResourceKind::Themes, ResourceKind::ContextFiles,
///         ]);
///         resources.system_prompt = Some("Answer briefly.".into());
///         resources.append_system_prompt = Some(Vec::new());
///         resources.offline = Some(true);
///         let created = create_agent_session(CreateAgentSessionOptions {
///             cwd: Some(cwd.clone()), agent_dir: Some(agent_dir),
///             auth_storage: Some(auth), model_registry: Some(registry),
///             model: Some(Arc::new(RwLock::new(provider.models[0].clone()))),
///             thinking_level: None, scoped_models: Vec::new(),
///             no_tools: Some(NoTools::All), tools: None, custom_tools: Vec::new(),
///             resources: SessionResources::Default(Box::new(resources)),
///             session_manager: Some(Rc::new(RefCell::new(
///                 SessionManager::in_memory(Some(&cwd))?,
///             ))),
///             settings_manager: Some(Rc::new(RefCell::new(
///                 SettingsManager::in_memory(Default::default()),
///             ))),
///             session_start_event: None, theme: Default::default(), offline: Some(true),
///         }).await?;
///         assert!(created.model_fallback_message.is_none());
///         assert!(created.extensions_result.errors.is_empty());
///         let session = created.session;
///         let text = Rc::new(RefCell::new(String::new()));
///         let output = Rc::clone(&text);
///         let ended = Rc::new(tokio::sync::Notify::new());
///         let completion = Rc::clone(&ended);
///         let unsubscribe = session.subscribe(Rc::new(move |event| {
///             if let AgentSessionEvent::Agent(event) = event {
///                 match &*event.read().unwrap() {
///                     AgentEvent::MessageUpdate {
///                         assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. }, ..
///                     } => output.borrow_mut().push_str(delta),
///                     AgentEvent::AgentEnd { .. } => completion.notify_one(),
///                     _ => {}
///                 }
///             }
///         }));
///         let result = session.prompt("Report readiness.", PromptOptions::default()).await;
///         if result.is_ok() { ended.notified().await; }
///         unsubscribe();
///         session.abort().await;
///         session.dispose();
///         provider.unregister();
///         result?;
///         assert_eq!(&*text.borrow(), "Ready.");
///         assert_eq!(provider.state.call_count(), 1);
///         Ok::<(), Box<dyn std::error::Error>>(())
///     }))?;
///     Ok(())
/// }
/// ```
///
/// # Errors
/// Returns agent-directory, catalog, history, resource loading or documentation-path
/// failures, and the failures of [`AgentSession::new`].
pub async fn create_agent_session(
    options: CreateAgentSessionOptions,
) -> Result<CreateAgentSessionResult, DiagnosticErrorInfo> {
    let services = resolve_services(&options)?;
    let session_manager = match options.session_manager {
        Some(manager) => manager,
        None => Rc::new(RefCell::new(
            SessionManager::create(&services.cwd, None, &services.agent_dir).map_err(thrown)?,
        )),
    };
    let resource_loader = resources(&services, options.resources, options.offline).await?;
    admit_requested_tools(
        &services.settings_manager.borrow(),
        options.tools.as_deref(),
    )?;
    services
        .model_registry
        .set_base_model_files(resource_loader.get_model_files())
        .map_err(thrown)?;
    let (agent, runner_ref, model_fallback_message) = prepare_agent(
        &services,
        &session_manager,
        options.model,
        options.thinking_level,
    )?;
    let (allowed_tool_names, initial_active_tool_names) =
        tool_names(options.tools, options.no_tools);
    let construction = resource_loader
        .get_extensions()
        .runtime
        .tool_construction()
        .unwrap_or_else(|| tool_construction(&services.agent_dir, options.offline));
    resource_loader
        .get_extensions()
        .runtime
        .set_tool_construction(construction.clone());
    let session = AgentSession::new(AgentSessionConfig {
        agent,
        session_manager,
        settings_manager: services.settings_manager,
        cwd: services.cwd,
        bin_dir: construction.bin_dir,
        scoped_models: options.scoped_models,
        resource_loader: Rc::clone(&resource_loader),
        custom_tools: options.custom_tools,
        model_registry: services.model_registry,
        initial_active_tool_names: Some(initial_active_tool_names),
        allowed_tool_names,
        base_tools_override: None,
        extension_runner_ref: Some(runner_ref),
        session_start_event: options.session_start_event,
        tools_manager: construction.tools_manager,
        theme: options.theme,
    })?;
    Ok(CreateAgentSessionResult {
        session,
        extensions_result: resource_loader.get_extensions(),
        model_fallback_message,
    })
}

/// Selects one application tool owner before loading factories.
fn tool_construction(
    agent_dir: &str,
    offline: Option<bool>,
) -> maestro_extensions::ToolConstruction {
    let bin_dir = get_bin_dir(agent_dir);
    maestro_extensions::ToolConstruction {
        tools_manager: Arc::new(ToolsManager::new(bin_dir.clone(), None).with_offline(offline)),
        bin_dir,
    }
}

/// Reports an explicit SDK refusal through the settings diagnostic channel.
fn admit_sdk_request(
    settings: &SettingsManager,
    request: &maestro_settings::DecisionRequest<'_>,
) -> Result<(), DiagnosticErrorInfo> {
    settings.decide(request).map_err(|message| {
        settings.report_refusal(maestro_settings::SettingsScope::Global, message.clone());
        DiagnosticErrorInfo::new(Some("ConfigurationRefusal".into()), message)
    })
}

/// Admits explicit SDK tools before history creation or live tool publication.
fn admit_requested_tools(
    settings: &SettingsManager,
    tools: Option<&[String]>,
) -> Result<(), DiagnosticErrorInfo> {
    tools.map_or(Ok(()), |tools| {
        admit_sdk_request(settings, &maestro_settings::DecisionRequest::Tools(tools))
    })
}

/// Admits explicit selections while retaining the requested, unclamped thinking level.
fn admit_requested_selection(
    settings: &SettingsManager,
    model: Option<&Arc<RwLock<Model>>>,
    level: Option<&ModelThinkingLevel>,
) -> Result<(), DiagnosticErrorInfo> {
    let requested = level.cloned().unwrap_or_else(|| {
        settings
            .get_default_thinking_level()
            .as_ref()
            .map_or(DEFAULT_THINKING_LEVEL, model_level)
    });
    let requested = saved_level(&requested);
    let identity = model.map(|model| {
        let model = read(model);
        (model.provider.clone(), model.id.clone())
    });
    let request = if let Some((provider, model_id)) = &identity {
        maestro_settings::DecisionRequest::Model {
            provider,
            model_id,
            thinking_level: requested.as_str(),
        }
    } else if level.is_some() {
        maestro_settings::DecisionRequest::ThinkingLevel(requested.as_str())
    } else {
        return Ok(());
    };
    admit_sdk_request(settings, &request)
}

/// Loads resources only after their application-owned construction services are installed.
async fn resources(
    services: &Services,
    resources: SessionResources,
    offline: Option<bool>,
) -> Result<
    Rc<dyn crate::resources::resource_loader::ResourceLoader<maestro_models::Local>>,
    DiagnosticErrorInfo,
> {
    match resources {
        SessionResources::Loader(loader) => Ok(loader),
        SessionResources::Default(mut resources) => {
            resources
                .tool_construction
                .get_or_insert_with(|| tool_construction(&services.agent_dir, offline));
            default_resource_loader(
                &services.cwd,
                &services.agent_dir,
                &services.settings_manager,
                *resources,
            )
            .await
            .map_err(thrown)
        }
    }
}

/// The agent over the resolved model and thinking level, with the resumed messages or the
/// first entries of a new history recorded; with its runner slot and the model fallback
/// message.
fn prepare_agent(
    services: &Services,
    session_manager: &Rc<RefCell<SessionManager>>,
    explicit_model: Option<Arc<RwLock<Model>>>,
    explicit_level: Option<ModelThinkingLevel>,
) -> Result<PreparedAgent, DiagnosticErrorInfo> {
    admit_requested_selection(
        &services.settings_manager.borrow(),
        explicit_model.as_ref(),
        explicit_level.as_ref(),
    )?;
    let recorded = recorded(&session_manager.borrow())?;
    let (model, model_fallback_message) = resolve_model(services, explicit_model, &recorded)?;
    let thinking_level = resolve_thinking_level(
        &services.settings_manager.borrow(),
        explicit_level,
        recorded
            .has_messages
            .then(|| recorded.thinking_level.clone())
            .flatten(),
        model.as_ref(),
    );
    let runner_ref: ExtensionRunnerRef = Rc::default();
    let agent = build_agent(
        services,
        &session_manager.borrow(),
        &runner_ref,
        model.clone(),
        thinking_level.clone(),
    );
    let messages = record_initial_entries(
        &mut session_manager.borrow_mut(),
        recorded,
        model.as_ref(),
        &thinking_level,
    )?;
    if let Some(messages) = messages {
        write(agent.state()).set_messages(messages);
    }
    Ok((agent, runner_ref, model_fallback_message))
}

/// The supplied directories, credentials, catalog and preferences, each absent one created.
fn resolve_services(options: &CreateAgentSessionOptions) -> Result<Services, DiagnosticErrorInfo> {
    let cwd = match (&options.cwd, &options.session_manager) {
        (Some(cwd), _) => cwd.clone(),
        (None, Some(manager)) => manager.borrow().get_cwd().to_owned(),
        (None, None) => std::env::current_dir()
            .map_err(thrown)?
            .to_string_lossy()
            .into_owned(),
    };
    let agent_dir = match &options.agent_dir {
        Some(agent_dir) => agent_dir.clone(),
        None => get_agent_dir().map_err(thrown)?,
    };
    let auth_storage = options
        .auth_storage
        .clone()
        .unwrap_or_else(|| Arc::new(AuthStorage::create(&get_auth_path(&agent_dir))));
    let model_registry = match &options.model_registry {
        Some(registry) => Arc::clone(registry),
        None => Arc::new(
            ModelRegistry::create(
                auth_storage,
                get_models_path(&agent_dir),
                native_config_values(),
            )
            .map_err(thrown)?,
        ),
    };
    let settings_manager = options.settings_manager.clone().unwrap_or_else(|| {
        Rc::new(RefCell::new(SettingsManager::create(
            &cwd,
            &agent_dir,
            CONFIG_DIR_NAME,
        )))
    });
    Ok(Services {
        cwd,
        agent_dir,
        model_registry,
        settings_manager,
    })
}

/// The recorded messages, model and thinking level of the selected branch.
fn recorded(session_manager: &SessionManager) -> Result<Recorded, DiagnosticErrorInfo> {
    let context = session_manager.build_session_context().map_err(thrown)?;
    let has_thinking_entry = session_manager
        .get_branch(None)
        .map_err(thrown)?
        .iter()
        .any(|entry| entry["type"] == "thinking_level_change");
    let text = |value: &maestro_session::Presence<Value>| {
        value.present().and_then(Value::as_str).map(str::to_owned)
    };
    let model = context
        .model
        .as_ref()
        .and_then(|model| Some((text(&model.provider)?, text(&model.model_id)?)));
    let thinking_level = has_thinking_entry
        .then(|| text(&context.thinking_level))
        .flatten()
        .map(|level| level.parse().unwrap_or(ModelThinkingLevel::Off));
    Ok(Recorded {
        has_messages: !context.messages.is_empty(),
        has_thinking_entry,
        model,
        thinking_level,
        messages: context.messages,
    })
}

/// A chosen model and the message explaining a failed restoration or a missing model.
type ResolvedModel = (Option<Arc<RwLock<Model>>>, Option<String>);

/// An agent with its runner slot and the model fallback message.
type PreparedAgent = (
    Rc<maestro_agent::Agent<maestro_session::messages::CustomAgentMessages, maestro_models::Local>>,
    ExtensionRunnerRef,
    Option<String>,
);

/// The explicit model, else the recorded one when it can be restored, else the initial-model
/// choice; with the message explaining a failed restoration or a missing model.
fn resolve_model(
    services: &Services,
    explicit: Option<Arc<RwLock<Model>>>,
    recorded: &Recorded,
) -> Result<ResolvedModel, DiagnosticErrorInfo> {
    if explicit.is_some() {
        return Ok((explicit, None));
    }
    let registry = &services.model_registry;
    let mut fallback_message = None;
    if recorded.has_messages
        && let Some((provider, model_id)) = &recorded.model
    {
        let stored = maestro_settings::Settings(
            serde_json::json!({"defaultProvider": provider, "defaultModel": model_id})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        );
        let resolved = services
            .settings_manager
            .borrow()
            .resolve_stored_values(&stored);
        if resolved.0.get("defaultProvider") == stored.0.get("defaultProvider")
            && resolved.0.get("defaultModel") == stored.0.get("defaultModel")
            && let Some(model) =
                restore_saved_model(registry, provider, model_id).map_err(thrown)?
        {
            return Ok((Some(model), None));
        }
        fallback_message = Some(format!("Could not restore model {provider}/{model_id}"));
    }
    let settings = services.settings_manager.borrow();
    let compare = model_id_comparator(&|key| std::env::var(key).ok()).map_err(thrown)?;
    let initial = find_initial_model(
        FindInitialModelOptions {
            cli_provider: None,
            cli_model: None,
            scoped_models: &[],
            is_continuing: recorded.has_messages,
            default_provider: settings.get_default_provider().as_deref(),
            default_model_id: settings.get_default_model().as_deref(),
            default_thinking_level: settings
                .get_default_thinking_level()
                .as_ref()
                .map(model_level),
            model_registry: registry,
        },
        &compare,
    )
    .map_err(thrown)?;
    let Some(model) = initial.model else {
        let docs = get_docs_path().map_err(thrown)?;
        return Ok((None, Some(format_no_models_available_message(&docs))));
    };
    let fallback_message = fallback_message.map(|message| {
        let chosen = read(&model);
        format!("{message}. Using {}/{}", chosen.provider, chosen.id)
    });
    Ok((Some(model), fallback_message))
}

/// The explicit level, else the recorded one of a resumed history, else the settings
/// default, else medium; clamped to `model`, and off without one.
fn resolve_thinking_level(
    settings: &SettingsManager,
    explicit: Option<ModelThinkingLevel>,
    recorded: Option<ModelThinkingLevel>,
    model: Option<&Arc<RwLock<Model>>>,
) -> ModelThinkingLevel {
    let recorded = if explicit.is_none() {
        recorded.map(|level| resolve_recorded_thinking(settings, &level))
    } else {
        None
    };
    let level = explicit.or(recorded).unwrap_or_else(|| {
        settings
            .get_default_thinking_level()
            .as_ref()
            .map_or(DEFAULT_THINKING_LEVEL, model_level)
    });
    model.map_or(ModelThinkingLevel::Off, |model| {
        clamp_thinking_level(&read(model), level)
    })
}

/// Resolves a recorded level through stored-value policy before capability clamping.
fn resolve_recorded_thinking(
    settings: &SettingsManager,
    level: &ModelThinkingLevel,
) -> ModelThinkingLevel {
    let stored = maestro_settings::Settings(
        [(
            String::from("defaultThinkingLevel"),
            Value::String(saved_level(level).as_str().into()),
        )]
        .into_iter()
        .collect(),
    );
    settings
        .resolve_stored_values(&stored)
        .0
        .get("defaultThinkingLevel")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_THINKING_LEVEL)
}

/// Records the model and thinking level a new history starts with, or the thinking level a
/// resumed one lacks; returns the resumed messages.
fn record_initial_entries(
    session_manager: &mut SessionManager,
    recorded: Recorded,
    model: Option<&Arc<RwLock<Model>>>,
    thinking_level: &ModelThinkingLevel,
) -> Result<Option<Vec<maestro_extensions::NativeAgentMessage>>, DiagnosticErrorInfo> {
    if !recorded.has_messages
        && let Some(model) = model
    {
        let model = read(model);
        session_manager
            .append_model_change(&model.provider, &model.id)
            .map_err(thrown)?;
    }
    if !recorded.has_messages || !recorded.has_thinking_entry {
        session_manager
            .append_thinking_level_change(saved_level(thinking_level).as_str())
            .map_err(thrown)?;
    }
    recorded
        .has_messages
        .then(|| agent_messages(recorded.messages))
        .transpose()
}

/// The allowlist and the initially active tools: the supplied list for both, else none or
/// only extension and SDK tools as `no_tools` says, else the default built-in tools.
fn tool_names(
    tools: Option<Vec<String>>,
    no_tools: Option<NoTools>,
) -> (Option<Vec<String>>, Vec<String>) {
    match (tools, no_tools) {
        (Some(tools), _) => (Some(tools.clone()), tools),
        (None, Some(NoTools::All)) => (Some(Vec::new()), Vec::new()),
        (None, Some(NoTools::Builtin)) => (None, Vec::new()),
        (None, None) => (None, DEFAULT_ACTIVE_TOOL_NAMES.map(str::to_owned).to_vec()),
    }
}
