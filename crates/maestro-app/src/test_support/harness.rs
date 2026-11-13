//! A session over the scripted provider with in-memory credentials, catalog, settings and
//! history.
use super::events::recorded_session_events;
use super::resource_loader::{
    TestResourceLoader, create_test_extensions_result, create_test_resource_loader,
};
use super::scripted_stream::{Scripted, ScriptedRequests, scripted_stream};
use crate::agent_session::{ExtensionRunnerRef, ToolDefinitionHandle};
use crate::resources::loading::{LoadPromptsResult, ResourceExtensionPaths};
use crate::{AgentSession, AgentSessionConfig};
use indexmap::IndexMap;
use maestro_agent::{
    Agent, AgentInitialState, AgentOptions, AgentSpawner, PrepareNextTurn, SharedAgentTool,
    StreamFn,
};
use maestro_credentials::{AuthCredential, AuthStorage, AuthStorageData, test_support::NoCommands};
use maestro_extensions::{EventError, ExtensionFactory};
use maestro_models::{
    FauxModelDefinition, FauxProviderRegistration, Local, Model, RegisterFauxProviderOptions,
    SharedHandle, register_faux_provider,
};
use maestro_models_registry::{ModelRegistry, ProviderConfigInput, ProviderModelInput};
use maestro_resources::{LoadSkillsResult, PromptTemplate, Skill};
use maestro_session::{SessionManager, message_records::CustomAgentMessages};
use maestro_settings::{Settings, SettingsManager};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// How the scripted provider's credentials are configured.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// A runtime key and a registered provider with a key.
    Key,
    /// A provider registered with only an authorization header and no key.
    Header,
    /// Nothing configured.
    None,
    /// An expired subscription credential and no registered provider.
    Subscription,
    /// A provider whose key comes from a failing command.
    Command,
}

/// What a session is built over.
pub struct HarnessOptions {
    /// Saved preferences.
    pub settings: Value,
    /// Scripted models as identity and reasoning support.
    pub models: Vec<(&'static str, bool)>,
    /// Inline extensions.
    pub factories: Vec<ExtensionFactory<Local>>,
    /// Inline extensions at their own paths, loaded after the others.
    pub labelled_factories: Vec<(&'static str, ExtensionFactory<Local>)>,
    /// Credentials.
    pub auth: Auth,
    /// The context window replacing the scripted one.
    pub context_window: Option<f64>,
    /// The session history; an in-memory one under the project when absent.
    pub session_manager: Option<SessionManager>,
    /// The loader's replacement system prompt.
    pub system_prompt: Option<&'static str>,
    /// The loader's appended prompt texts.
    pub append_system_prompt: Vec<String>,
    /// The agent's own task facility; the session supplies its default when absent.
    pub spawn: Option<AgentSpawner<Local>>,
    /// The next-turn hook already installed on the supplied agent.
    pub prepare_next_turn: Option<PrepareNextTurn<CustomAgentMessages, Local>>,
    /// The loaded skills.
    pub skills: Vec<Skill>,
    /// The loaded prompt templates.
    pub prompts: Vec<PromptTemplate>,
    /// SDK tool definitions.
    pub custom_tools: Vec<ToolDefinitionHandle>,
    /// The tools activated first; the default built-in ones when absent.
    pub initial_active_tool_names: Option<Vec<String>>,
    /// The only tool names exposed, when present.
    pub allowed_tool_names: Option<Vec<String>>,
    /// Executable tools in place of the built-in ones.
    pub base_tools_override: Option<IndexMap<String, SharedAgentTool<Local>>>,
    /// Answers of the scripted stream, which then replaces the scripted provider.
    pub responses: Option<Vec<Scripted>>,
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            settings: json!({}),
            models: vec![("plain", false), ("think", true), ("extra", true)],
            factories: Vec::new(),
            labelled_factories: Vec::new(),
            auth: Auth::Key,
            context_window: None,
            session_manager: None,
            system_prompt: None,
            append_system_prompt: Vec::new(),
            spawn: None,
            prepare_next_turn: None,
            skills: Vec::new(),
            prompts: Vec::new(),
            custom_tools: Vec::new(),
            initial_active_tool_names: None,
            allowed_tool_names: None,
            base_tools_override: None,
            responses: None,
        }
    }
}

/// A session and the owners the tests observe, with the owner `S` of its disposable root.
pub struct Harness<S> {
    /// The scripted provider.
    pub faux: FauxProviderRegistration,
    /// The session under test.
    pub session: Rc<AgentSession<Local>>,
    /// The catalog the session was given.
    pub registry: Arc<ModelRegistry>,
    /// Thinking and name events the session announced.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// The requests the scripted stream received.
    pub requests: ScriptedRequests,
    /// Each set of extension resource paths handed to the loader, in order.
    pub extended: Rc<RefCell<Vec<ResourceExtensionPaths>>>,
    /// The session working directory, below the disposable root.
    pub cwd: String,
    /// The owner of the disposable root, released with the harness.
    pub scratch: S,
}

impl<S> Drop for Harness<S> {
    fn drop(&mut self) {
        self.session.dispose();
        self.faux.unregister();
    }
}

impl<S> Harness<S> {
    /// A shared copy of the scripted model `id`, when the scripted provider serves it.
    #[must_use]
    pub fn model(&self, id: &str) -> Option<Arc<RwLock<Model>>> {
        let model = self.faux.get_model(Some(id))?;
        Some(Arc::new(RwLock::new(model.clone())))
    }
}

/// A session over the scripted provider as `options` describe it, with its project below the
/// directory `scratch` names.
///
/// # Errors
/// Returns the first setup step that failed: the catalog, the saved preferences, the
/// history, an inline extension or the session itself.
pub async fn harness<S: AsRef<Path>>(
    scratch: S,
    options: HarnessOptions,
) -> Result<Harness<S>, EventError> {
    let root = scratch.as_ref().display().to_string();
    let faux = scripted_provider(&options.models);
    match session(&root, &faux, options).await {
        Ok(built) => Ok(Harness {
            faux,
            events: recorded_session_events(&built.session),
            session: built.session,
            registry: built.registry,
            requests: built.requests,
            extended: built.extended,
            cwd: built.cwd,
            scratch,
        }),
        Err(error) => {
            faux.unregister();
            Err(error)
        }
    }
}

/// The session of a harness and the owners built beside it.
struct Built {
    /// The session under test.
    session: Rc<AgentSession<Local>>,
    /// The catalog the session was given.
    registry: Arc<ModelRegistry>,
    /// The requests the scripted stream received.
    requests: ScriptedRequests,
    /// Each set of extension resource paths handed to the loader, in order.
    extended: Rc<RefCell<Vec<ResourceExtensionPaths>>>,
    /// The session working directory.
    cwd: String,
}

/// The session `options` describe over `faux`, with its project below `root`.
async fn session(
    root: &str,
    faux: &FauxProviderRegistration,
    mut options: HarnessOptions,
) -> Result<Built, EventError> {
    let cwd = format!("{root}/project");
    let registry = Arc::new(catalog(faux, options.auth, options.context_window)?);
    let requests = ScriptedRequests::default();
    let stream = options
        .responses
        .take()
        .map(|responses| scripted_stream(responses, requests.clone()));
    let (runner_ref, settings_manager, agent) = scripted_agent(&mut options, faux, stream)?;
    *maestro_lock::write(&agent.prepare_next_turn) = options.prepare_next_turn.take();
    let session_manager = match options.session_manager.take() {
        Some(manager) => manager,
        None => SessionManager::in_memory(Some(&cwd))?,
    };
    let loader = harness_loader(&cwd, &mut options).await?;
    let extended = Rc::clone(&loader.extended);
    let bin_dir = format!("{root}/bin");
    let session = AgentSession::new(AgentSessionConfig {
        agent,
        session_manager: Rc::new(RefCell::new(session_manager)),
        settings_manager,
        cwd: cwd.clone(),
        bin_dir: bin_dir.clone(),
        scoped_models: Vec::new(),
        resource_loader: Rc::new(loader),
        custom_tools: options.custom_tools,
        model_registry: Arc::clone(&registry),
        initial_active_tool_names: options.initial_active_tool_names,
        allowed_tool_names: options.allowed_tool_names,
        base_tools_override: options.base_tools_override,
        extension_runner_ref: Some(runner_ref),
        session_start_event: None,
        tools_manager: Arc::new(maestro_tools::ToolsManager::new(bin_dir, None)),
        theme: maestro_theme::LiveTheme::default(),
    })?;
    Ok(Built {
        session,
        registry,
        requests,
        extended,
        cwd,
    })
}

/// The scripted provider serving `models` as identity and reasoning support.
fn scripted_provider(models: &[(&'static str, bool)]) -> FauxProviderRegistration {
    register_faux_provider(RegisterFauxProviderOptions {
        models: Some(
            models
                .iter()
                .map(|&(id, reasoning)| FauxModelDefinition {
                    id: id.into(),
                    reasoning: Some(reasoning),
                    ..FauxModelDefinition::default()
                })
                .collect(),
        ),
        ..RegisterFauxProviderOptions::default()
    })
}

/// The first model the scripted provider serves.
fn first_model(faux: &FauxProviderRegistration) -> Result<&Model, EventError> {
    faux.models
        .first()
        .ok_or_else(|| "the scripted provider serves no model".into())
}

/// The catalog with the scripted provider registered as `auth` configures it.
fn catalog(
    faux: &FauxProviderRegistration,
    auth: Auth,
    window: Option<f64>,
) -> Result<ModelRegistry, EventError> {
    let storage = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    match auth {
        Auth::Subscription => storage.set(
            "faux",
            serde_json::from_value::<AuthCredential>(
                json!({"type": "oauth", "access": "", "refresh": "", "expires": 0}),
            )?,
        ),
        Auth::Key => storage.set_runtime_api_key("faux", "faux-key"),
        Auth::Header | Auth::None | Auth::Command => {}
    }
    let registry = ModelRegistry::in_memory(storage, SharedHandle::new(NoCommands))?;
    let config = match auth {
        Auth::None | Auth::Subscription => return Ok(registry),
        Auth::Header => ProviderConfigInput {
            auth_header: Some(true),
            ..ProviderConfigInput::default()
        },
        Auth::Key | Auth::Command => ProviderConfigInput {
            base_url: Some(first_model(faux)?.base_url.clone()),
            api_key: Some(
                if auth == Auth::Command {
                    "!exit 3"
                } else {
                    "faux-key"
                }
                .into(),
            ),
            api: Some(faux.api.clone()),
            models: Some(
                faux.models
                    .iter()
                    .map(|model| model_input(model, window))
                    .collect(),
            ),
            ..ProviderConfigInput::default()
        },
    };
    registry.register_provider("faux", Arc::new(RwLock::new(config)))?;
    Ok(registry)
}

/// The catalog record of a scripted model.
fn model_input(model: &Model, window: Option<f64>) -> ProviderModelInput {
    ProviderModelInput {
        id: model.id.clone(),
        name: model.name.clone(),
        api: Some(model.api.clone()),
        base_url: Some(model.base_url.clone()),
        reasoning: model.reasoning,
        thinking_level_map: None,
        input: model.input.clone(),
        cost: model.cost.clone(),
        context_window: window.unwrap_or(model.context_window),
        max_tokens: model.max_tokens,
        headers: None,
        compat: None,
    }
}

/// What a scripted agent is built over.
struct AgentSetup<'a> {
    /// The scripted model the session starts on.
    model: &'a Model,
    /// The context window replacing the scripted one.
    window: Option<f64>,
    /// The agent's own task facility.
    spawn: Option<AgentSpawner<Local>>,
    /// The stream replacing the scripted provider.
    stream: Option<StreamFn<Local>>,
    /// The slot always holding the runner currently in effect.
    runner_ref: &'a ExtensionRunnerRef<Local>,
    /// The preferences the message conversion reads.
    settings_manager: &'a Rc<RefCell<SettingsManager>>,
}

/// A local agent on the first scripted model, running the application's own message
/// conversion and context hook.
fn agent(setup: AgentSetup<'_>) -> Rc<Agent<CustomAgentMessages, Local>> {
    let mut initial = setup.model.clone();
    initial.context_window = setup.window.unwrap_or(initial.context_window);
    let runner = Rc::clone(setup.runner_ref);
    let converting = Rc::clone(setup.settings_manager);
    Rc::new(Agent::new_local(AgentOptions {
        spawn: setup.spawn,
        stream_fn: setup.stream,
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(initial))),
            system_prompt: Some("base".into()),
            ..AgentInitialState::default()
        },
        convert_to_llm: Some(Rc::new(move |history| {
            let block_images = converting.borrow().get_block_images();
            let messages = crate::sdk::convert_to_llm_with_block_images(
                &maestro_lock::read(&history),
                block_images,
            );
            Box::pin(std::future::ready(Ok(messages)))
        })),
        transform_context: Some(crate::sdk::transform_context(&runner)),
        ..AgentOptions::default()
    }))
}

/// The scripted agent of `options`, its runner slot and its preferences.
type ScriptedAgent = (
    ExtensionRunnerRef<Local>,
    Rc<RefCell<SettingsManager>>,
    Rc<Agent<CustomAgentMessages, Local>>,
);

/// The scripted agent of `options`, its runner slot and its preferences.
fn scripted_agent(
    options: &mut HarnessOptions,
    faux: &FauxProviderRegistration,
    stream: Option<StreamFn<Local>>,
) -> Result<ScriptedAgent, EventError> {
    let runner_ref: ExtensionRunnerRef<Local> = Rc::new(RefCell::new(None));
    let settings: Settings = serde_json::from_value(std::mem::take(&mut options.settings))?;
    let settings_manager = Rc::new(RefCell::new(SettingsManager::in_memory(settings)));
    let agent = agent(AgentSetup {
        model: first_model(faux)?,
        window: options.context_window,
        spawn: options.spawn.take(),
        stream,
        runner_ref: &runner_ref,
        settings_manager: &settings_manager,
    });
    Ok((runner_ref, settings_manager, agent))
}

/// A loader over the extensions, prompt texts, skills and templates `options` supply, with
/// the extensions loaded in `cwd`; those fields are left empty.
async fn harness_loader(
    cwd: &str,
    options: &mut HarnessOptions,
) -> Result<TestResourceLoader, EventError> {
    let factories = std::mem::take(&mut options.factories)
        .into_iter()
        .map(|factory| (None, factory));
    let labelled = std::mem::take(&mut options.labelled_factories)
        .into_iter()
        .map(|(path, factory)| (Some(path), factory));
    let extensions =
        create_test_extensions_result(factories.chain(labelled).collect(), cwd).await?;
    Ok(TestResourceLoader {
        system_prompt: options.system_prompt.map(Rc::from),
        append_system_prompt: Rc::from(std::mem::take(&mut options.append_system_prompt)),
        skills: Rc::new(LoadSkillsResult {
            skills: std::mem::take(&mut options.skills),
            ..LoadSkillsResult::default()
        }),
        prompts: Rc::new(LoadPromptsResult {
            prompts: std::mem::take(&mut options.prompts),
            ..LoadPromptsResult::default()
        }),
        ..create_test_resource_loader(Some(extensions))
    })
}
