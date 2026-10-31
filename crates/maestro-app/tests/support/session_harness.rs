//! A session over the scripted provider with in-memory credentials, catalog, settings and
//! history, plus the recorded corpus it is compared with.
#![cfg(not(target_arch = "wasm32"))]
use indexmap::IndexMap;
use maestro_agent::{Agent, AgentInitialState, AgentOptions};
use maestro_app::agent_session::CompactionReason;
use maestro_app::agent_session::ExtensionRunnerRef;
use maestro_app::agent_session::ToolDefinitionHandle;
use maestro_app::contributions::LoadContributionsResult;
use maestro_app::resources::resource_loader::{
    LoadPromptsResult, LoadThemesResult, ResourceExtensionPaths, ResourceLoader,
};
use maestro_app::{AgentSession, AgentSessionConfig, AgentSessionEvent};
use maestro_catalog::{ModelRegistry, ProviderConfigInput, ProviderModelInput, ScopedModel};
use maestro_credentials::{AuthCredential, AuthStorage, AuthStorageData};
use maestro_extensions::NativeAgentMessage;
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ExtensionAPI, ExtensionContext,
    ExtensionExecution, ExtensionFactory, ExtensionRunner, InputEvent, InputEventResult,
    LoadExtensionsResult, RegisteredToolInput, SessionBeforeCompactEvent,
    SessionBeforeCompactResult, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
};
use maestro_models::system_prompt::ContextFile;
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, Execution, FauxAssistantMessageOptions,
    FauxModelDefinition, FauxProviderRegistration, FauxResponseInput, FauxResponseStep, Local,
    Model, RegisterFauxProviderOptions, StopReason, Threaded, Tool, UserContent,
    faux_assistant_message, register_faux_provider,
};
use maestro_resources::LoadSkillsResult;
use maestro_resources::{SyntheticSourceOptions, create_synthetic_source_info};
use maestro_session::{
    CompactionResult, Presence, SessionManager,
    messages::{CustomAgentMessages, CustomMessageInput},
};
use maestro_settings::SettingsManager;
use maestro_tools::ToolDefinitionRecord;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// An extension that summarizes every compaction as `summary` instead of calling a provider.
pub fn summary_provider(summary: &'static str) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                Box::pin(std::future::ready(Ok(Some(summary_result(event, summary)))))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A supplied summary retaining the compaction preparation's boundary and token count.
pub fn summary_result(
    event: &SessionBeforeCompactEvent,
    summary: &str,
) -> SessionBeforeCompactResult {
    SessionBeforeCompactResult {
        cancel: Presence::Missing,
        compaction: Presence::Present(CompactionResult {
            summary: summary.to_owned(),
            first_kept_entry_id: event.data.preparation.first_kept_entry_id.clone(),
            tokens_before: event.data.preparation.tokens_before,
            details: Presence::Present(json!({})),
        }),
    }
}

/// The recorded corpus file `name`.
pub fn corpus(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/agent_session/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The recorded case `name` of the corpus file `file`.
pub fn case(file: &str, name: &str) -> Value {
    let cases = corpus(file);
    let found = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name);
    found
        .unwrap_or_else(|| panic!("no recorded case {name}"))
        .clone()
}

/// The recorded prompt and queue case `name`.
pub fn expected(name: &str) -> Value {
    corpus("prompt_queue")[name].clone()
}

/// The recorded case `name` of the keyed corpus file `file`.
pub fn recorded(file: &str, name: &str) -> Value {
    let cases = corpus(file);
    cases
        .get(name)
        .unwrap_or_else(|| panic!("no recorded case {name}"))
        .clone()
}

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
    /// The session history; an in-memory one under the scratch project when absent.
    pub session_manager: Option<SessionManager>,
    /// The loader's replacement system prompt.
    pub system_prompt: Option<&'static str>,
    /// The loader's appended prompt texts.
    pub append_system_prompt: Vec<String>,
    /// The agent's own task facility; the session supplies its default when absent.
    pub spawn: Option<maestro_agent::AgentSpawner<Local>>,
    /// The next-turn hook already installed on the supplied agent.
    pub prepare_next_turn: Option<maestro_agent::PrepareNextTurn<CustomAgentMessages, Local>>,
    /// The loaded skills.
    pub skills: Vec<maestro_resources::Skill>,
    /// The loaded prompt templates.
    pub prompts: Vec<maestro_resources::PromptTemplate>,
    /// The disposable root holding the project; a fresh one when absent.
    pub scratch: Option<maestro_test_directory::Scratch>,
    /// SDK tool definitions.
    pub custom_tools: Vec<ToolDefinitionHandle>,
    /// The tools activated first; the default built-in ones when absent.
    pub initial_active_tool_names: Option<Vec<String>>,
    /// The only tool names exposed, when present.
    pub allowed_tool_names: Option<Vec<String>>,
    /// Executable tools in place of the built-in ones.
    pub base_tools_override: Option<IndexMap<String, maestro_agent::SharedAgentTool<Local>>>,
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
            scratch: None,
            custom_tools: Vec::new(),
            initial_active_tool_names: None,
            allowed_tool_names: None,
            base_tools_override: None,
            responses: None,
        }
    }
}

/// A session and the owners the tests observe.
pub struct Harness {
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
    /// The session working directory, below the scratch root.
    pub cwd: String,
    /// The disposable root removed with the harness.
    pub scratch: maestro_test_directory::Scratch,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.session.dispose();
        self.faux.unregister();
    }
}

impl Harness {
    /// The extensions' runner now in effect.
    pub fn runner(&self) -> Rc<ExtensionRunner<Local>> {
        self.session.extension_runner()
    }

    /// A shared copy of the scripted model `id`.
    pub fn model(&self, id: &str) -> Arc<RwLock<Model>> {
        Arc::new(RwLock::new(self.faux.get_model(Some(id)).unwrap().clone()))
    }

    /// The scripted model `id` renamed to `name` with a thinking map.
    pub fn variant(&self, id: &str, name: &str, map: Value) -> Arc<RwLock<Model>> {
        let mut model = serde_json::to_value(self.faux.get_model(Some(id)).unwrap()).unwrap();
        model["id"] = json!(name);
        model["name"] = json!(name);
        model["thinkingLevelMap"] = map;
        Arc::new(RwLock::new(serde_json::from_value(model).unwrap()))
    }

    /// The model `id` under another provider, which has no credentials.
    pub fn elsewhere(&self, id: &str) -> Arc<RwLock<Model>> {
        let model = self.model(id);
        model.write().unwrap().provider = "elsewhere".into();
        model.write().unwrap().id = "x".into();
        model
    }

    /// A scoped entry for `model` with an optional explicit level.
    pub fn scoped(model: Arc<RwLock<Model>>, level: Option<&str>) -> ScopedModel {
        ScopedModel {
            model,
            thinking_level: level.map(|level| serde_json::from_value(json!(level)).unwrap()),
        }
    }
}

/// `provider/id` of a model.
pub fn model_view(model: &Arc<RwLock<Model>>) -> String {
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// Configured-value commands for native tests, shared with the browser-safe fixture.
pub use super::no_commands::no_commands;

/// A loader returning the supplied extensions and no other resources.
pub struct TestResourceLoader {
    /// The loaded extensions.
    pub extensions: Rc<LoadExtensionsResult<Local>>,
    /// The replacement system prompt.
    pub system_prompt: Option<Rc<str>>,
    /// The appended prompt texts.
    pub append_system_prompt: Rc<[String]>,
    /// The loaded skills.
    pub skills: Rc<LoadSkillsResult>,
    /// The loaded prompt templates.
    pub prompts: Rc<LoadPromptsResult>,
    /// Each set of extension resource paths handed to the loader, in order.
    pub extended: Rc<RefCell<Vec<ResourceExtensionPaths>>>,
}

/// A loader returning `extensions` and empty resources.
pub fn create_test_resource_loader(
    extensions: Option<LoadExtensionsResult<Local>>,
) -> TestResourceLoader {
    let extensions = extensions.unwrap_or_else(|| LoadExtensionsResult {
        extensions: Vec::new(),
        errors: Vec::new(),
        runtime: create_extension_runtime::<Local>(),
    });
    TestResourceLoader {
        extensions: Rc::new(extensions),
        system_prompt: None,
        append_system_prompt: Rc::from(Vec::new()),
        skills: Rc::default(),
        prompts: Rc::default(),
        extended: Rc::default(),
    }
}

impl ResourceLoader<Local> for TestResourceLoader {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<Local>> {
        Rc::clone(&self.extensions)
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        Rc::clone(&self.skills)
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        Rc::clone(&self.prompts)
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
        Rc::default()
    }
    fn get_contributions(&self) -> Rc<LoadContributionsResult> {
        Rc::default()
    }
    fn get_agents_files(&self) -> Rc<[ContextFile]> {
        Rc::from(Vec::new())
    }
    fn get_system_prompt(&self) -> Option<Rc<str>> {
        self.system_prompt.clone()
    }
    fn get_append_system_prompt(&self) -> Rc<[String]> {
        Rc::clone(&self.append_system_prompt)
    }
    fn extend_resources(&self, paths: ResourceExtensionPaths) -> io::Result<()> {
        self.extended.borrow_mut().push(paths);
        Ok(())
    }
    fn reload(&self) -> <Local as Execution>::Future<'_, io::Result<()>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

/// Loads `factories` as one extension set, each at its supplied path or `<inline:N>`.
pub async fn create_test_extensions_result(
    factories: Vec<(Option<&str>, ExtensionFactory<Local>)>,
    cwd: &str,
) -> LoadExtensionsResult<Local> {
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(spawn_local(), Rc::new(|_| {}));
    let mut extensions = Vec::new();
    for (index, (path, factory)) in factories.into_iter().enumerate() {
        let path = path.map_or_else(|| format!("<inline:{}>", index + 1), str::to_owned);
        let loaded = load_extension_from_factory(
            factory,
            cwd.to_owned(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        );
        extensions.push(loaded.await.unwrap());
    }
    LoadExtensionsResult {
        extensions,
        errors: Vec::new(),
        runtime,
    }
}

/// The catalog with the scripted provider registered as `auth` configures it.
fn catalog(faux: &FauxProviderRegistration, auth: Auth, window: Option<f64>) -> ModelRegistry {
    let storage = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    match auth {
        Auth::Subscription => storage.set(
            "faux",
            serde_json::from_value::<AuthCredential>(
                json!({"type": "oauth", "access": "", "refresh": "", "expires": 0}),
            )
            .unwrap(),
        ),
        Auth::Key => storage.set_runtime_api_key("faux", "faux-key"),
        Auth::Header | Auth::None | Auth::Command => {}
    }
    let registry = ModelRegistry::in_memory(storage, no_commands()).unwrap();
    let config = match auth {
        Auth::None | Auth::Subscription => return registry,
        Auth::Header => ProviderConfigInput {
            auth_header: Some(true),
            ..ProviderConfigInput::default()
        },
        Auth::Key | Auth::Command => ProviderConfigInput {
            base_url: Some(faux.models[0].base_url.clone()),
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
    registry
        .register_provider("faux", Arc::new(RwLock::new(config)))
        .unwrap();
    registry
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

/// A native tool definition.
pub type Definition = ToolDefinitionRecord<ExtensionContext, Value, Value, (), Local, Local>;

/// A tool definition with prompt text whose execution returns nothing.
pub fn tool_definition(
    name: &str,
    snippet: Option<&str>,
    guidelines: Option<&[&str]>,
) -> Definition {
    ToolDefinitionRecord {
        definition: Tool {
            name: name.into(),
            description: format!("{name} tool"),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        label: name.into(),
        prompt_snippet: snippet.map(str::to_owned),
        prompt_guidelines: guidelines
            .map(|lines| lines.iter().map(|line| (*line).to_owned()).collect()),
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: None,
                    terminate: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// `definition` retained as an SDK tool definition.
pub fn sdk_definition(definition: Definition) -> ToolDefinitionHandle {
    Local::retain_definition(RegisteredToolInput::from(definition))
}

/// The recorded tools: whitespace-heavy snippets, blank and repeated guidelines.
pub fn sdk_tools() -> Vec<ToolDefinitionHandle> {
    [
        tool_definition(
            "alpha",
            Some("  Read\n  files\r\n quickly  "),
            Some(&["  Prefer read ", "", "Prefer read", " Keep it short", "\t"]),
        ),
        tool_definition("beta", Some("   "), None),
        tool_definition("gamma", Some("\u{feff}x\u{85}y"), Some(&["Use gamma"])),
        tool_definition("delta", None, Some(&[])),
    ]
    .into_iter()
    .map(sdk_definition)
    .collect()
}

/// Base time of the recorded messages.
pub const TS: u64 = 1_700_000_000_000;

/// Recorded usage with an optional total.
pub fn usage(tokens: [u64; 4], cost: f64, total_tokens: Option<u64>) -> Value {
    let mut usage = json!({
        "input": tokens[0], "output": tokens[1], "cacheRead": tokens[2], "cacheWrite": tokens[3],
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": cost},
    });
    if let Some(total) = total_tokens {
        usage["totalTokens"] = json!(total);
    }
    usage
}

/// A user message record.
pub fn user(content: &Value, offset: u64) -> Value {
    json!({"role": "user", "content": content, "timestamp": TS + offset})
}

/// An answer record with `text`, then `extra` fields replacing the defaults.
pub fn assistant(text: &str, offset: u64, extra: &Value) -> Value {
    let mut message = json!({
        "role": "assistant", "content": [{"type": "text", "text": text}], "api": "faux",
        "provider": "faux", "model": "plain", "usage": usage([1, 1, 0, 0], 0.0, Some(2)),
        "stopReason": "stop", "timestamp": TS + offset,
    });
    for (key, value) in extra.as_object().unwrap() {
        message[key] = value.clone();
    }
    message
}

/// Tree entry identities by name.
pub type Names = IndexMap<String, String>;

/// The recorded tree: two branches from `a1`, the first ending in a custom message.
pub fn build_tree(manager: &mut SessionManager) -> Names {
    let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
    let mut names = Names::new();
    let mut add = |name: &str, id: String| names.insert(name.to_owned(), id);
    add(
        "u1",
        manager.append_message(user(&json!("first"), 1)).unwrap(),
    );
    add(
        "a1",
        manager
            .append_message(assistant("answer one", 2, &json!({})))
            .unwrap(),
    );
    let second =
        json!([{"type": "text", "text": "second"}, {"type": "text", "text": " part"}, image]);
    add("u2", manager.append_message(user(&second, 3)).unwrap());
    add(
        "a2",
        manager
            .append_message(assistant("answer two", 4, &json!({})))
            .unwrap(),
    );
    let content =
        json!([{"type": "text", "text": "custom "}, image, {"type": "text", "text": "words"}]);
    let content = serde_json::from_value(content).unwrap();
    let custom = manager.append_custom_message_entry("note", content, true, Presence::Missing);
    add("c1", custom.unwrap());
    manager.branch(&names["a1"]).unwrap();
    names.insert(
        "u3".into(),
        manager.append_message(user(&json!("alt"), 5)).unwrap(),
    );
    names.insert(
        "a3".into(),
        manager
            .append_message(assistant("answer alt", 6, &json!({})))
            .unwrap(),
    );
    names
}

/// What a scripted agent is built over.
struct AgentSetup<'a> {
    /// The scripted model the session starts on.
    model: &'a Model,
    /// The context window replacing the scripted one.
    window: Option<f64>,
    /// The agent's own task facility.
    spawn: Option<maestro_agent::AgentSpawner<Local>>,
    /// The stream replacing the scripted provider.
    stream: Option<maestro_agent::StreamFn<Local>>,
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
            let messages = maestro_app::sdk::convert_to_llm_with_block_images(
                &history.read().unwrap(),
                block_images,
            );
            Box::pin(std::future::ready(Ok(messages)))
        })),
        transform_context: Some(maestro_app::sdk::transform_context(&runner)),
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
    stream: Option<maestro_agent::StreamFn<Local>>,
) -> ScriptedAgent {
    let runner_ref: ExtensionRunnerRef<Local> = Rc::new(RefCell::new(None));
    let settings: maestro_settings::Settings =
        serde_json::from_value(std::mem::take(&mut options.settings)).unwrap();
    let settings_manager = Rc::new(RefCell::new(SettingsManager::in_memory(settings)));
    let agent = agent(AgentSetup {
        model: &faux.models[0],
        window: options.context_window,
        spawn: options.spawn.take(),
        stream,
        runner_ref: &runner_ref,
        settings_manager: &settings_manager,
    });
    (runner_ref, settings_manager, agent)
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

/// A loader over the extensions, prompt texts, skills and templates `options` supply, with
/// the extensions loaded in `cwd`; those fields are left empty.
async fn harness_loader(cwd: &str, options: &mut HarnessOptions) -> TestResourceLoader {
    let factories = std::mem::take(&mut options.factories)
        .into_iter()
        .map(|factory| (None, factory));
    let labelled = std::mem::take(&mut options.labelled_factories)
        .into_iter()
        .map(|(path, factory)| (Some(path), factory));
    let extensions = create_test_extensions_result(factories.chain(labelled).collect(), cwd).await;
    TestResourceLoader {
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
    }
}

/// A session over the scripted provider as `options` describe it.
pub async fn harness(mut options: HarnessOptions) -> Harness {
    let scratch = options
        .scratch
        .take()
        .unwrap_or_else(|| maestro_test_directory::Scratch::new().unwrap());
    let cwd = format!("{}/project", scratch.path().display());
    let faux = scripted_provider(&options.models);
    let registry = Arc::new(catalog(&faux, options.auth, options.context_window));
    let requests = ScriptedRequests::default();
    let stream = options
        .responses
        .take()
        .map(|responses| scripted_stream(responses, requests.clone()));
    let (runner_ref, settings_manager, agent) = scripted_agent(&mut options, &faux, stream);
    *agent.prepare_next_turn.write().unwrap() = options.prepare_next_turn.take();
    let session_manager = options
        .session_manager
        .take()
        .unwrap_or_else(|| SessionManager::in_memory(Some(&cwd)).unwrap());
    let loader = harness_loader(&cwd, &mut options).await;
    let extended = Rc::clone(&loader.extended);
    let bin_dir = format!("{}/bin", scratch.path().display());
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
    })
    .unwrap();
    let events = recorded_session_events(&session);
    Harness {
        faux,
        session,
        registry,
        events,
        requests,
        extended,
        cwd,
        scratch,
    }
}

/// The session events of `session` the scripts assert, in their recorded form.
fn recorded_session_events(session: &AgentSession<Local>) -> Rc<RefCell<Vec<Value>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    let _unsubscribe = session.subscribe(Rc::new(move |event| {
        if let Some(view) = session_event_view(event) {
            recorded.borrow_mut().push(view);
        }
    }));
    events
}

/// The recorded form of a session event the scripts assert.
pub fn session_event_view(event: &AgentSessionEvent) -> Option<Value> {
    match event {
        AgentSessionEvent::ThinkingLevelChanged { level } => {
            Some(json!({"type": "thinking_level_changed", "level": level}))
        }
        AgentSessionEvent::SessionInfoChanged { name: Some(name) } => {
            Some(json!({"type": "session_info_changed", "name": name}))
        }
        AgentSessionEvent::SessionInfoChanged { name: None } => {
            Some(json!({"type": "session_info_changed"}))
        }
        AgentSessionEvent::CompactionStart { reason } => {
            Some(json!({"type": "compaction_start", "reason": reason_view(*reason)}))
        }
        AgentSessionEvent::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            error_message,
        } => Some(json!({
            "type": "auto_retry_start",
            "attempt": attempt,
            "maxAttempts": recorded_number(*max_attempts),
            "delayMs": recorded_number(*delay_ms),
            "errorMessage": error_message,
        })),
        AgentSessionEvent::CompactionEnd { reason, result, .. } => {
            Some(compaction_end_view(event, *reason, result.as_ref()))
        }
        AgentSessionEvent::AutoRetryEnd {
            success,
            attempt,
            final_error,
        } => {
            let mut view = json!({
                "type": "auto_retry_end",
                "attempt": attempt,
                "success": success,
            });
            if let Some(error) = final_error {
                view["finalError"] = json!(error);
            }
            Some(view)
        }
        _ => None,
    }
}

/// The recorded form of a compaction end.
fn compaction_end_view(
    event: &AgentSessionEvent,
    reason: CompactionReason,
    result: Option<&CompactionResult>,
) -> Value {
    let AgentSessionEvent::CompactionEnd {
        aborted,
        will_retry,
        error_message,
        ..
    } = event
    else {
        return Value::Null;
    };
    let mut view = json!({
        "type": "compaction_end",
        "reason": reason_view(reason),
        "aborted": aborted,
        "willRetry": will_retry,
        "result": result.map(compaction_result_view),
    });
    if let Some(message) = error_message {
        view["errorMessage"] = json!(message);
    }
    view
}

/// The recorded form of a stored compaction result.
fn compaction_result_view(result: &CompactionResult) -> Value {
    json!({
        "summary": result.summary,
        "firstKeptEntryId": result.first_kept_entry_id,
        "tokensBefore": recorded_number(result.tokens_before),
        "details": result.details,
    })
}

/// What started a compaction, as the recorded cases spell it.
fn reason_view(reason: CompactionReason) -> &'static str {
    match reason {
        CompactionReason::Manual => "manual",
        CompactionReason::Threshold => "threshold",
        CompactionReason::Overflow => "overflow",
    }
}

/// A number as a script writes it: integral values without a fraction.
pub fn recorded_number(value: f64) -> Value {
    if value.is_finite()
        && value.fract() == 0.0
        && let Some(integral) = num_traits::cast::cast::<f64, i64>(value)
    {
        return json!(integral);
    }
    json!(value)
}

/// Completes `body` on a fresh current-thread runtime with a paused clock, inside a task set
/// it receives; the clock moves when a test advances it or when every task waits on a timer.
pub fn on_paused_task_set<F: std::future::Future>(
    body: impl FnOnce(Rc<tokio::task::LocalSet>) -> F,
) -> F::Output {
    let local = Rc::new(tokio::task::LocalSet::new());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    runtime.block_on(local.run_until(body(Rc::clone(&local))))
}

/// Returns once every task of an [`on_paused_task_set`] runtime waits: the paused clock
/// moves only then.
pub async fn until_idle() {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

/// The text of a message record: plain content, joined text blocks, or its summary.
pub fn message_text(record: &Value) -> Value {
    match &record["content"] {
        Value::String(text) => json!(text),
        Value::Array(blocks) => json!(
            blocks
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect::<String>()
        ),
        _ => record.get("summary").cloned().unwrap_or(Value::Null),
    }
}

/// The text a message shows: string content unchanged, or its text blocks in order joined
/// by one newline each; anything without content shows nothing, not its summary.
pub fn get_message_text(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .map(|block| block["text"].as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The text of each user message in the session history, in order.
pub fn get_user_texts(harness: &Harness) -> Vec<String> {
    texts_matching(harness, |message| {
        matches!(message, maestro_agent::AgentMessage::User(_))
    })
}

/// The text of each assistant message in the session history, in order.
pub fn get_assistant_texts(harness: &Harness) -> Vec<String> {
    texts_matching(harness, |message| {
        matches!(message, maestro_agent::AgentMessage::Assistant(_))
    })
}

/// The message fields the prompt and queue checks compare: the role and text, a custom
/// message's own fields, a tool result's error flag and, for a message with images, its
/// content blocks in order.
pub fn delivery_message_view(record: &Value) -> Value {
    let mut view = json!({"role": record["role"], "text": get_message_text(record)});
    if record["role"] == "custom" {
        for key in ["customType", "display", "details"] {
            view[key] = record[key].clone();
        }
    }
    if record["role"] == "toolResult" {
        view["isError"] = record["isError"].clone();
    }
    if let Some(content) = record["content"].as_array()
        && content.iter().any(|part| part["type"] == "image")
    {
        view["content"] = json!(content);
    }
    view
}

/// The delivered history and queue mirrors of `h`.
pub fn delivery_view(h: &Harness) -> Value {
    let messages: Vec<Value> = {
        let messages = h.session.messages();
        let messages = messages.read().unwrap();
        messages
            .iter()
            .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
            .collect()
    };
    json!({
        "users": get_user_texts(h),
        "assistants": get_assistant_texts(h),
        "messages": messages,
        "pending": h.session.pending_message_count(),
        "steering": h.session.get_steering_messages(),
        "followUp": h.session.get_follow_up_messages(),
    })
}

/// The contexts the scripted stream received, as delivery messages, in request order.
pub fn delivery_requests(h: &Harness) -> Value {
    let requests: Vec<Vec<Value>> = h
        .requests
        .0
        .borrow()
        .iter()
        .map(|context| {
            context
                .messages
                .iter()
                .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
                .collect()
        })
        .collect();
    json!(requests)
}

/// The text of each history message `keep` selects, in order.
fn texts_matching(harness: &Harness, keep: fn(&NativeAgentMessage) -> bool) -> Vec<String> {
    let messages = harness.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .filter(|message| keep(message))
        .map(|message| get_message_text(&serde_json::to_value(message).unwrap()))
        .collect()
}

/// A scripted answer with an optional stop reason and error text.
pub fn answer(text: &str, stop: Option<StopReason>, error: Option<&str>) -> AssistantMessage {
    faux_assistant_message(
        text,
        FauxAssistantMessageOptions {
            stop_reason: stop,
            error_message: error.map(str::to_owned),
            ..FauxAssistantMessageOptions::default()
        },
    )
}

/// A scripted answer that records the request's message texts.
pub struct RecordPrompt {
    /// Prompts in request order.
    pub prompts: Arc<Mutex<Vec<String>>>,
    /// The answer.
    pub response: AssistantMessage,
}

impl RecordPrompt {
    /// Records the request and answers.
    fn respond<E: maestro_models::ProviderExecution>(
        &self,
        input: &FauxResponseInput<E>,
    ) -> std::future::Ready<Result<AssistantMessage, DiagnosticErrorInfo>> {
        let texts: Vec<String> = input
            .context
            .messages
            .iter()
            .map(|message| message_text(&serde_json::to_value(message).unwrap()))
            .map(|text| text.as_str().unwrap_or_default().to_owned())
            .collect();
        self.prompts.lock().unwrap().push(texts.join("\n---\n"));
        std::future::ready(Ok(self.response.clone()))
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for RecordPrompt {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for RecordPrompt {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

/// Replaces the agent's live history with the stored history.
pub fn load_history(h: &Harness) {
    let records = h
        .session
        .session_manager()
        .borrow()
        .build_session_context()
        .unwrap()
        .messages;
    let history: Vec<NativeAgentMessage> = serde_json::from_value(json!(records)).unwrap();
    h.session
        .agent()
        .state()
        .write()
        .unwrap()
        .set_messages(history);
}

/// Queues the answers; the returned list collects each request's prompt.
pub fn script_responses(h: &Harness, responses: Vec<AssistantMessage>) -> Arc<Mutex<Vec<String>>> {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let steps = responses.into_iter().map(|response| {
        let prompts = Arc::clone(&prompts);
        FauxResponseStep::Factory(Arc::new(RecordPrompt { prompts, response }))
    });
    h.faux.set_responses(steps.collect());
    prompts
}

/// The agent history as roles and texts.
pub fn history_view(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| {
            let record = serde_json::to_value(message).unwrap();
            json!({"role": record["role"], "text": message_text(&record)})
        })
        .collect()
}

/// Whether code running now could change `session`: its history, its settings and its agent
/// state are all free to borrow mutably.
pub fn free_to_change(session: &AgentSession<Local>) -> bool {
    session.session_manager().try_borrow_mut().is_ok()
        && session.settings_manager().try_borrow_mut().is_ok()
        && session.state().try_write().is_ok()
}

/// Completes `body` on a fresh current-thread runtime inside a task set it receives.
pub fn on_task_set<F: std::future::Future>(
    body: impl FnOnce(Rc<tokio::task::LocalSet>) -> F,
) -> F::Output {
    let local = Rc::new(tokio::task::LocalSet::new());
    super::io_runtime().block_on(local.run_until(body(Rc::clone(&local))))
}

/// A spawner running each task on the current task set.
fn spawn_local() -> maestro_agent::AgentSpawner<Local> {
    Rc::new(|task| drop(tokio::task::spawn_local(task)))
}

/// What the prompt, queue, message and shell cases record beside the session.
#[derive(Clone, Default)]
pub struct Transcript {
    /// Input and before-agent-start events as the recording extension saw them.
    pub extension: Rc<RefCell<Vec<Value>>>,
    /// Failures reported to the extension error listeners.
    pub errors: Rc<RefCell<Vec<Value>>>,
    /// Agent run and message events, and the session's own message and queue
    /// announcements, in arrival order.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// The base system prompt, recorded as `$BASE`.
    pub base: Rc<RefCell<String>>,
}

impl Transcript {
    /// The recorded events of `kind`, in order.
    pub fn events_of(&self, kinds: &[&str]) -> Vec<Value> {
        of_kinds(&self.events.borrow(), kinds)
    }
}

/// The values whose `type` is one of `kinds`, in order.
pub fn of_kinds(values: &[Value], kinds: &[&str]) -> Vec<Value> {
    values
        .iter()
        .filter(|value| kinds.iter().any(|kind| value["type"] == *kind))
        .cloned()
        .collect()
}

/// The input sources the session's handlers saw, in order.
pub fn input_sources(transcript: &Transcript) -> Vec<Value> {
    of_kinds(&transcript.extension.borrow(), &["input"])
        .iter()
        .map(|event| event["source"].clone())
        .collect()
}

/// The extension that records input and before-agent-start events into `transcript`,
/// loaded before the case's own extensions.
pub fn recording_extension(transcript: &Transcript) -> ExtensionFactory<Local> {
    let transcript = transcript.clone();
    Rc::new(move |api| {
        let inputs = Rc::clone(&transcript.extension);
        let starts = Rc::clone(&transcript.extension);
        let base = Rc::clone(&transcript.base);
        let input = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                inputs.borrow_mut().push(json!({
                    "type": "input", "text": event.text, "images": image_count(&event.images),
                    "source": event.source,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let start = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |event, _| {
                let prompt = if event.system_prompt == *base.borrow() {
                    "$BASE".to_owned()
                } else {
                    event.system_prompt.clone()
                };
                starts.borrow_mut().push(json!({
                    "type": "before_agent_start", "prompt": event.prompt,
                    "images": image_count(&event.images), "systemPrompt": prompt,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let registered = input.and(start).map(drop);
        Box::pin(std::future::ready(registered))
    })
}

/// The number of supplied images, or null when none were supplied.
fn image_count(images: &Presence<Vec<maestro_models::ImageContent>>) -> Value {
    match images {
        Presence::Present(images) => json!(images.len()),
        _ => Value::Null,
    }
}

/// A session over `options` with the recording extension loaded first, and its base system
/// prompt, errors, agent events and session announcements recorded.
pub async fn transcribed(options: HarnessOptions) -> (Harness, Transcript) {
    let transcript = Transcript::default();
    let mut factories = vec![recording_extension(&transcript)];
    factories.extend(options.factories);
    let h = harness(HarnessOptions {
        factories,
        ..options
    })
    .await;
    *transcript.base.borrow_mut() = h.session.system_prompt();
    let errors = Rc::clone(&transcript.errors);
    let _unsubscribe = h.runner().on_error(Rc::new(move |error| {
        errors.borrow_mut().push(json!({
            "extensionPath": error.extension_path, "event": error.event, "error": error.error,
        }));
        Ok(())
    }));
    let events = Rc::clone(&transcript.events);
    let _unsubscribe = h.session.subscribe(Rc::new(move |event| match event {
        AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } => events.borrow_mut().push(json!({
            "type": "queue_update", "steering": steering, "followUp": follow_up,
        })),
        AgentSessionEvent::Agent(event) => {
            events
                .borrow_mut()
                .push(agent_event_view(&event.read().unwrap()));
        }
        _ => {}
    }));
    (h, transcript)
}

/// The recorded form of an agent event: run boundaries by type, message events with the
/// message.
fn agent_event_view(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> Value {
    use maestro_agent::AgentEvent;
    match event {
        AgentEvent::AgentStart => json!({"type": "agent_start"}),
        AgentEvent::AgentEnd { .. } => json!({"type": "agent_end"}),
        AgentEvent::MessageStart { message } => {
            json!({"type": "message_start", "message": message_view(&serde_json::to_value(message).unwrap())})
        }
        AgentEvent::MessageEnd { message } => {
            json!({"type": "message_end", "message": message_view(&serde_json::to_value(message).unwrap())})
        }
        _ => json!({"type": "other"}),
    }
}

/// The recorded form of message content: plain text, or blocks with text or image type.
fn content_view(content: &Value) -> Value {
    match content {
        Value::String(text) => json!({"text": text}),
        Value::Array(blocks) => {
            json!({"blocks": blocks.iter().map(|block| if block["type"] == "text" {
            json!({"type": "text", "text": block["text"]})
        } else {
            json!({"type": block["type"], "mimeType": block["mimeType"]})
        }).collect::<Vec<_>>()})
        }
        _ => json!({"absent": true}),
    }
}

/// A recorded field: the value, or null when absent.
fn or_null(record: &Value, field: &str) -> Value {
    record.get(field).cloned().unwrap_or(Value::Null)
}

/// The recorded form of a message record.
pub fn message_view(record: &Value) -> Value {
    match record["role"].as_str() {
        Some("user") => json!({"role": "user", "content": content_view(&record["content"])}),
        Some("assistant") => json!({
            "role": "assistant", "text": message_text(record), "stopReason": record["stopReason"],
            "errorMessage": or_null(record, "errorMessage"),
        }),
        Some("custom") => json!({
            "role": "custom", "customType": record["customType"],
            "content": content_view(&record["content"]), "display": record["display"],
            "details": or_null(record, "details"),
        }),
        Some("bashExecution") => json!({
            "role": "bashExecution", "command": record["command"], "output": record["output"],
            "exitCode": record["exitCode"].as_f64().map_or(Value::Null, recorded_number),
            "cancelled": record["cancelled"],
            "truncated": record["truncated"],
            "fullOutputPath": record.get("fullOutputPath").map(|_| json!({"present": true})),
            "excludeFromContext": or_null(record, "excludeFromContext"),
        }),
        _ => json!({"role": record["role"], "summary": or_null(record, "summary")}),
    }
}

/// The agent history in its recorded form.
pub fn messages_view(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| message_view(&serde_json::to_value(message).unwrap()))
        .collect()
}

/// The stored session entries in their recorded form, numbered by position.
pub fn entries_view(h: &Harness) -> Vec<Value> {
    use maestro_session::session_manager::ReadonlySessionManager;
    let manager = h.session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut view = json!({"type": entry["type"], "id": format!("e{index}")});
            if entry["type"] == "message" {
                view["message"] = message_view(&entry["message"]);
            } else if entry["type"] == "custom_message" {
                view["customType"] = entry["customType"].clone();
                view["content"] = content_view(&entry["content"]);
                view["display"] = entry["display"].clone();
                view["details"] = or_null(entry, "details");
            }
            view
        })
        .collect()
}

/// The queue mirrors, streaming flag and system prompt in their recorded form.
pub fn queue_view(h: &Harness, transcript: &Transcript) -> Value {
    let prompt = h.session.system_prompt();
    json!({
        "steering": h.session.get_steering_messages(),
        "followUp": h.session.get_follow_up_messages(),
        "pendingMessageCount": h.session.pending_message_count(),
        "isStreaming": h.session.is_streaming(),
        "systemPrompt": if prompt == *transcript.base.borrow() { "$BASE".to_owned() } else { prompt },
    })
}

/// A scripted answer that reports its request on `requested` and answers once `release`
/// is notified, so the response streams until the test releases it.
pub struct HeldAnswer {
    /// Notified when the provider is asked.
    pub requested: Arc<tokio::sync::Notify>,
    /// Releases the answer.
    pub release: Arc<tokio::sync::Notify>,
    /// The answer.
    pub response: AssistantMessage,
}

impl HeldAnswer {
    /// Reports the request, then answers once released.
    fn respond(
        &self,
    ) -> impl std::future::Future<Output = Result<AssistantMessage, DiagnosticErrorInfo>> + Send + 'static
    {
        self.requested.notify_one();
        let release = Arc::clone(&self.release);
        let response = self.response.clone();
        async move {
            release.notified().await;
            Ok(response)
        }
    }
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for HeldAnswer {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<Local>) -> Self::Return {
        Box::pin(self.respond())
    }
}

impl maestro_models::Invoke<Threaded, FauxResponseInput<Threaded>> for HeldAnswer {
    type Return = <Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(self.respond())
    }
}

/// The scripted steps: a held first answer `first`, then ordinary answers `rest`; returns
/// the request and release notifiers of the first.
pub fn hold_first_answer(
    h: &Harness,
    first: &str,
    rest: &[&str],
) -> (Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>) {
    let held = HeldAnswer {
        requested: Arc::default(),
        release: Arc::default(),
        response: answer(first, None, None),
    };
    let notifiers = (Arc::clone(&held.requested), Arc::clone(&held.release));
    let mut steps = vec![FauxResponseStep::Factory(Arc::new(held))];
    steps.extend(
        rest.iter()
            .map(|text| FauxResponseStep::Message(Box::new(answer(text, None, None)))),
    );
    h.faux.set_responses(steps);
    notifiers
}

/// `value` with every `(from, to)` spelling replaced, in order, in its strings.
pub fn substituting(value: Value, paths: &[(&str, &str)]) -> Value {
    mapping_strings(value, &|text| {
        paths
            .iter()
            .fold(text, |text, (from, to)| text.replace(from, to))
    })
}

/// `value` with `map` applied to each of its strings.
fn mapping_strings(value: Value, map: &impl Fn(String) -> String) -> Value {
    match value {
        Value::String(text) => json!(map(text)),
        Value::Array(items) => items
            .into_iter()
            .map(|item| mapping_strings(item, map))
            .collect(),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, item)| (key, mapping_strings(item, map)))
                .collect(),
        ),
        other => other,
    }
}

/// `value` with each `(path, placeholder)` path spelled as its placeholder, as the recorded
/// cases spell it. On Windows a string that names a placeholder also spells its native
/// separators `/`, as the recordings do.
pub fn as_recorded(value: Value, paths: &[(&str, &str)]) -> Value {
    mapping_strings(value, &|text| {
        let text = paths.iter().fold(text, |text, (path, placeholder)| {
            text.replace(path, placeholder)
        });
        let named = paths
            .iter()
            .any(|(_, placeholder)| text.contains(placeholder));
        if cfg!(windows) && named {
            text.replace('\\', "/")
        } else {
            text
        }
    })
}

/// `value` with the scratch root spelled `$ROOT`, as the recorded cases spell it.
pub fn with_placeholders(value: Value, h: &Harness) -> Value {
    let root = h.scratch.path().display().to_string();
    as_recorded(value, &[(&root, "$ROOT")])
}

/// The agent run and message events, as the session's listeners would see them.
pub const MESSAGE_EVENTS: [&str; 4] = ["agent_start", "message_start", "message_end", "agent_end"];

/// Asserts what the session produced against `case`: the history, the run and message
/// events, the extension events, the reported errors and the number of provider requests.
/// A run end's messages are compared through the history, since their timestamps are the
/// clock's.
pub fn assert_transcript(h: &Harness, t: &Transcript, case: &Value) {
    assert_eq!(
        with_placeholders(json!(messages_view(h)), h),
        case["messages"]
    );
    let recorded = of_kinds(case["sessionEvents"].as_array().unwrap(), &MESSAGE_EVENTS);
    assert_eq!(
        with_placeholders(json!(t.events_of(&MESSAGE_EVENTS)), h),
        json!(recorded)
    );
    assert_eq!(
        with_placeholders(json!(*t.extension.borrow()), h),
        case["extensionEvents"]
    );
    assert_eq!(
        with_placeholders(json!(*t.errors.borrow()), h),
        case["errorEvents"]
    );
    assert_eq!(json!(h.faux.state.call_count()), case["contexts"]["calls"]);
}

/// The recorded project files: the `review` skill with frontmatter and a padded body, and
/// a `gone` skill whose file is missing.
pub fn recorded_skills(scratch: &maestro_test_directory::Scratch) -> Vec<maestro_resources::Skill> {
    let dir = scratch.path().join("project/skills/review");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("SKILL.md");
    std::fs::write(
        &file,
        "---\ndescription: Review\n---\n\n  Check the diff.  \n",
    )
    .unwrap();
    let base_dir = dir.display().to_string();
    let skill = |name: &str, file_path: String| maestro_resources::Skill {
        name: name.into(),
        description: "Review".into(),
        source_info: create_synthetic_source_info(
            file_path.clone(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
        file_path,
        base_dir: base_dir.clone(),
        disable_model_invocation: false,
    };
    vec![
        skill("review", file.display().to_string()),
        skill("gone", dir.join("missing.md").display().to_string()),
    ]
}

/// A prompt template named `name` with `content`.
pub fn template(name: &str, content: &str) -> maestro_resources::PromptTemplate {
    maestro_resources::PromptTemplate {
        name: name.into(),
        description: String::new(),
        argument_hint: None,
        content: content.into(),
        source_info: create_synthetic_source_info(
            "plan.md".into(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
        file_path: "plan.md".into(),
    }
}

/// The recorded form of an operation's outcome.
pub fn outcome(result: Result<(), DiagnosticErrorInfo>) -> Value {
    match result {
        Ok(()) => json!({"ok": {"undefined": true}}),
        Err(error) => json!({"error": error.message}),
    }
}

/// The stored shell records of `entries` without their position, which also counts the
/// turn messages the agent-event persistence stores.
pub fn shell_entries(entries: &[Value]) -> Vec<Value> {
    entries
        .iter()
        .filter(|entry| entry["message"]["role"] == "bashExecution")
        .map(|entry| json!({"type": entry["type"], "message": entry["message"]}))
        .collect()
}

/// A labelled recorded observation.
pub fn labelled(label: &str, value: impl serde::Serialize) -> Value {
    json!({"label": label, "value": value})
}

/// Starts `text` as a prompt on the current task set.
pub fn start_prompt(
    h: &Harness,
    text: &'static str,
) -> tokio::task::JoinHandle<Result<(), DiagnosticErrorInfo>> {
    let session = Rc::clone(&h.session);
    tokio::task::spawn_local(async move {
        session
            .prompt(text, maestro_app::agent_session::PromptOptions::default())
            .await
    })
}

/// Prompt options whose preflight outcomes are collected in the returned list.
pub fn observed_preflight() -> (
    Rc<RefCell<Vec<bool>>>,
    maestro_app::agent_session::PromptOptions,
) {
    let accepted = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&accepted);
    let options = maestro_app::agent_session::PromptOptions {
        preflight_result: Some(Rc::new(move |ok| record.borrow_mut().push(ok))),
        ..maestro_app::agent_session::PromptOptions::default()
    };
    (accepted, options)
}

/// The extension registering the command `name`, whose handler records its arguments in
/// `heard` and then fails with `failure` when one is given.
pub fn command_extension(
    name: &'static str,
    heard: Rc<RefCell<Option<String>>>,
    failure: Option<&'static str>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let heard = Rc::clone(&heard);
        let handler: maestro_extensions::types::CommandHandler<Local> =
            Rc::new(move |arguments, _| {
                *heard.borrow_mut() = Some(arguments);
                Box::pin(std::future::ready(
                    failure.map_or(Ok(()), |text| Err(text.into())),
                ))
            });
        let options = maestro_extensions::types::RegisteredCommandOptions {
            description: Some(name.into()),
            get_argument_completions: None,
            handler,
        };
        Box::pin(std::future::ready(
            api.register_command(name.into(), options),
        ))
    })
}

/// Shell operations that record each execution, write the scripted chunks, then wait for
/// `release` when holding, and settle with the scripted exit status.
pub struct ScriptedShell {
    /// Each execution's command and directory, in order.
    pub executed: Mutex<Vec<Value>>,
    /// Notified once the chunks are written.
    pub started: Arc<tokio::sync::Notify>,
    /// Lets a holding execution settle.
    pub release: Arc<tokio::sync::Notify>,
    /// The output chunks.
    chunks: Vec<String>,
    /// The exit status.
    exit_code: Option<f64>,
    /// Whether the execution waits for `release`.
    hold: bool,
}

impl ScriptedShell {
    /// Operations writing `chunks` and settling with `exit_code` at once.
    pub fn chunks(chunks: Vec<String>, exit_code: Option<f64>) -> Self {
        Self {
            executed: Mutex::default(),
            started: Arc::default(),
            release: Arc::default(),
            chunks,
            exit_code,
            hold: false,
        }
    }

    /// Operations writing `text`, or nothing when it is empty, and settling at once.
    pub fn output(text: &str, exit_code: Option<f64>) -> Self {
        let chunks = if text.is_empty() {
            Vec::new()
        } else {
            vec![text.to_owned()]
        };
        Self::chunks(chunks, exit_code)
    }

    /// Operations writing `chunks` that settle with `exit_code` only once released.
    pub fn held(chunks: Vec<String>, exit_code: Option<f64>) -> Self {
        Self {
            hold: true,
            ..Self::chunks(chunks, exit_code)
        }
    }
}

impl maestro_tools::BashOperations<Local> for ScriptedShell {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: maestro_tools::BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
                > + 'a,
        >,
    > {
        Box::pin(async move {
            self.executed
                .lock()
                .unwrap()
                .push(json!({"command": command, "cwd": cwd}));
            for chunk in &self.chunks {
                (options.on_data)(chunk.as_bytes());
            }
            self.started.notify_one();
            if self.hold {
                self.release.notified().await;
            }
            Ok(maestro_tools::BashExecResult {
                exit_code: self.exit_code,
            })
        })
    }
}

/// The recorded form of a shell result, with an artifact spelled `$ARTIFACT`.
pub fn bash_result_view(result: &maestro_tools::BashResult) -> Value {
    let mut view = json!({
        "output": result.output, "cancelled": result.cancelled, "truncated": result.truncated,
    });
    if let Some(code) = result.exit_code {
        view["exitCode"] = recorded_number(code);
    }
    if result.full_output_path.is_some() {
        view["fullOutputPath"] = json!("$ARTIFACT");
    }
    view
}

/// Notified once the session has delivered the end of an agent run to its listeners.
pub fn session_run_ended(h: &Harness) -> Rc<tokio::sync::Notify> {
    let ended = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && matches!(
                *event.read().unwrap(),
                maestro_agent::AgentEvent::AgentEnd { .. }
            )
        {
            notify.notify_one();
        }
    })));
    ended
}

/// Notified once an agent run ends.
pub fn agent_ended(h: &Harness) -> Rc<tokio::sync::Notify> {
    let ended = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        if matches!(
            *event.read().unwrap(),
            maestro_agent::AgentEvent::AgentEnd { .. }
        ) {
            notify.notify_one();
        }
        Box::pin(std::future::ready(Ok(())))
    })));
    ended
}

/// The retry observations among the delivered events, in the order they arrived:
/// `start:<attempt>` when a retry opens and `end:<success>` when it settles.
pub fn retry_order(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event["type"].as_str() {
            Some("auto_retry_start") => Some(format!("start:{}", event["attempt"])),
            Some("auto_retry_end") => Some(format!("end:{}", event["success"])),
            _ => None,
        })
        .collect()
}

/// Starts `operation` with one poll and drops it unfinished, as a caller that stops
/// observing it.
pub fn start_and_abandon(operation: impl std::future::Future) {
    let mut operation = std::pin::pin!(operation);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(operation.as_mut().poll(&mut context).is_pending());
}

/// A supplied failure carrying every diagnostic field.
pub fn supplied_failure() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("RemoteError".into()),
        message: "connection reset".into(),
        stack: Some("RemoteError: connection reset\n    at remote".into()),
        code: Some(maestro_models::DiagnosticCode::Text("ECONNRESET".into())),
    }
}

/// One answer of the scripted stream: optional thinking, text and tool calls, a failure
/// and input and output usage.
#[derive(Clone, Default)]
pub struct Scripted {
    /// Thinking content.
    pub thinking: Option<String>,
    /// Text content.
    pub text: Option<String>,
    /// Tool calls by name and arguments.
    pub tool_calls: Vec<(String, Value)>,
    /// The failure text; the answer then stops with an error.
    pub error: Option<String>,
    /// Input and output tokens; 100 and 50 when absent.
    pub usage: Option<(f64, f64)>,
    /// Whether the stream sends only the terminal event, so the response arrives whole.
    pub terminal_only: bool,
    /// Whether the answer names the model the request asked for, as a provider's answer does.
    pub names_requested_model: bool,
}

impl Scripted {
    /// A text answer.
    pub fn text(text: &str) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::default()
        }
    }

    /// A failed answer reporting `error`.
    pub fn failure(error: &str) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }

    /// A failed answer from the requested model reporting a context overflow.
    pub fn context_overflow() -> Self {
        Self {
            names_requested_model: true,
            ..Self::failure("prompt is too long: 213462 tokens > 200000 maximum")
        }
    }

    /// A call of the tool named `echo` with `text`.
    pub fn echo_call(text: &str) -> Self {
        Self {
            tool_calls: vec![("echo".into(), json!({"text": text}))],
            ..Self::default()
        }
    }
}

/// The requests the scripted stream received, in order.
#[derive(Clone, Default)]
pub struct ScriptedRequests(pub Rc<RefCell<Vec<maestro_models::Context>>>);

/// The stream answering request `n` with `responses[n % len]`, each block streamed with its
/// start, its deltas and its end.
fn scripted_stream(
    responses: Vec<Scripted>,
    requests: ScriptedRequests,
) -> maestro_agent::StreamFn<Local> {
    Rc::new(move |model, context, _, _| {
        let index = requests.0.borrow().len() % responses.len();
        requests.0.borrow_mut().push(context);
        let stream = maestro_models::AssistantMessageEventStream::new();
        let response = &responses[index];
        let mut message = scripted_message(response);
        if response.names_requested_model {
            message.provider = model.provider;
            message.model = model.id;
        }
        stream_with_deltas(&stream, message, response.terminal_only);
        Box::pin(std::future::ready(Ok(stream)))
    })
}

/// The answer `response` describes.
fn scripted_message(response: &Scripted) -> AssistantMessage {
    use maestro_models::{AssistantContent, TextContent, ThinkingContent, ToolCall};
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut content = Vec::new();
    if let Some(thinking) = &response.thinking {
        content.push(AssistantContent::Thinking(ThinkingContent {
            thinking: thinking.clone(),
            thinking_signature: None,
            redacted: None,
        }));
    }
    if let Some(text) = &response.text {
        content.push(AssistantContent::Text(TextContent::new(text.clone())));
    }
    for (name, arguments) in &response.tool_calls {
        let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        content.push(AssistantContent::ToolCall(ToolCall {
            id: format!("faux_tc_{call}"),
            name: name.clone(),
            arguments: arguments.clone(),
            thought_signature: None,
        }));
    }
    if content.is_empty() && response.error.is_none() {
        content.push(AssistantContent::Text(TextContent::new(String::new())));
    }
    let stop = match (&response.error, response.tool_calls.is_empty()) {
        (Some(_), _) => "error",
        (None, false) => "toolUse",
        (None, true) => "stop",
    };
    let (input, output) = response.usage.unwrap_or((100.0, 50.0));
    serde_json::from_value(json!({
        "role": "assistant", "content": content, "api": "anthropic-messages",
        "provider": "faux", "model": "faux-1", "stopReason": stop,
        "errorMessage": response.error, "timestamp": maestro_models::timestamp_now(),
        "usage": {"input": input, "output": output, "cacheRead": 0, "cacheWrite": 0,
            "totalTokens": input + output,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
    }))
    .unwrap()
}

/// Pushes `message` onto `stream` block by block, each with its start, deltas of up to four
/// characters and its end, then its terminal event; only the terminal event when
/// `terminal_only`.
fn stream_with_deltas(
    stream: &maestro_models::AssistantMessageEventStream,
    message: AssistantMessage,
    terminal_only: bool,
) {
    if !terminal_only {
        stream_blocks(stream, &message);
    }
    stream_terminal(stream, message);
}

/// Pushes the start of `message`, then each block with its start, deltas of up to four
/// characters and its end.
fn stream_blocks(stream: &maestro_models::AssistantMessageEventStream, message: &AssistantMessage) {
    use maestro_models::{AssistantContent, AssistantMessageEvent as Event};
    let mut partial = AssistantMessage {
        content: Vec::new(),
        ..message.clone()
    };
    let snapshot = |partial: &AssistantMessage| Arc::new(RwLock::new(partial.clone()));
    stream.push(Event::Start {
        partial: snapshot(&partial),
    });
    for (index, block) in message.content.iter().enumerate() {
        partial.content.push(block.clone());
        let text = match block {
            AssistantContent::Thinking(thinking) => thinking.thinking.clone(),
            AssistantContent::Text(text) => text.text.clone(),
            AssistantContent::ToolCall(call) => serde_json::to_string(&call.arguments).unwrap(),
        };
        let characters: Vec<char> = text.chars().collect();
        let mut chunks: Vec<String> = characters
            .chunks(4)
            .map(|chunk| chunk.iter().collect())
            .collect();
        if chunks.is_empty() {
            chunks.push(String::new());
        }
        stream.push(block_event(block, index, None, &snapshot(&partial)));
        for chunk in chunks {
            stream.push(block_event(block, index, Some(chunk), &snapshot(&partial)));
        }
        stream.push(block_end(block, index, &snapshot(&partial)));
    }
}

/// Pushes the terminal event of `message`: its completion, or its failure.
fn stream_terminal(
    stream: &maestro_models::AssistantMessageEventStream,
    message: AssistantMessage,
) {
    use maestro_models::AssistantMessageEvent as Event;
    let failed = matches!(message.stop_reason, StopReason::Error | StopReason::Aborted);
    let reason = message.stop_reason.clone();
    let message = Arc::new(RwLock::new(message));
    stream.push(if failed {
        let reason = if reason == StopReason::Aborted {
            maestro_models::ErrorReason::Aborted
        } else {
            maestro_models::ErrorReason::Error
        };
        Event::Error {
            reason,
            error: message,
        }
    } else {
        let reason = match reason {
            StopReason::ToolUse => maestro_models::DoneReason::ToolUse,
            StopReason::Length => maestro_models::DoneReason::Length,
            _ => maestro_models::DoneReason::Stop,
        };
        Event::Done { reason, message }
    });
}

/// The start of `block`, or one of its deltas.
fn block_event(
    block: &maestro_models::AssistantContent,
    content_index: usize,
    delta: Option<String>,
    partial: &maestro_models::SharedAssistantMessage,
) -> maestro_models::AssistantMessageEvent {
    use maestro_models::{AssistantContent as Block, AssistantMessageEvent as Event};
    let partial = Arc::clone(partial);
    match (block, delta) {
        (Block::Thinking(_), None) => Event::ThinkingStart {
            content_index,
            partial,
        },
        (Block::Thinking(_), Some(delta)) => Event::ThinkingDelta {
            content_index,
            delta,
            partial,
        },
        (Block::Text(_), None) => Event::TextStart {
            content_index,
            partial,
        },
        (Block::Text(_), Some(delta)) => Event::TextDelta {
            content_index,
            delta,
            partial,
        },
        (Block::ToolCall(_), None) => Event::ToolcallStart {
            content_index,
            partial,
        },
        (Block::ToolCall(_), Some(delta)) => Event::ToolcallDelta {
            content_index,
            delta,
            partial,
        },
    }
}

/// The end of `block`.
fn block_end(
    block: &maestro_models::AssistantContent,
    content_index: usize,
    partial: &maestro_models::SharedAssistantMessage,
) -> maestro_models::AssistantMessageEvent {
    use maestro_models::{AssistantContent as Block, AssistantMessageEvent as Event};
    let partial = Arc::clone(partial);
    match block {
        Block::Thinking(thinking) => Event::ThinkingEnd {
            content_index,
            content: thinking.thinking.clone(),
            partial,
        },
        Block::Text(text) => Event::TextEnd {
            content_index,
            content: text.text.clone(),
            partial,
        },
        Block::ToolCall(call) => Event::ToolcallEnd {
            content_index,
            tool_call: call.clone(),
            partial,
        },
    }
}

/// The event type of an agent event, as the session's listeners name it.
pub fn agent_event_type(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> &'static str {
    use maestro_agent::AgentEvent;
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
    }
}

/// Every session event `session` delivers from now on, in order: agent events by type, with
/// a stream update's own type and delta, and the session's announcements in their recorded
/// form.
pub fn delivered_events(session: &AgentSession<Local>) -> Rc<RefCell<Vec<Value>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    drop(session.subscribe(Rc::new(move |event| {
        let view = match event {
            AgentSessionEvent::Agent(event) => Some(delivered_agent_event(&event.read().unwrap())),
            other => session_event_view(other),
        };
        recorded.borrow_mut().extend(view);
    })));
    events
}

/// The delivered form of an agent event.
fn delivered_agent_event(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> Value {
    let mut view = json!({"type": agent_event_type(event)});
    if let maestro_agent::AgentEvent::MessageUpdate {
        assistant_message_event,
        ..
    } = event
    {
        let update = serde_json::to_value(assistant_message_event).unwrap();
        view["update"] = update["type"].clone();
        view["delta"] = update.get("delta").cloned().unwrap_or(Value::Null);
    }
    view
}

/// A displayed custom `note` message that only records `text`.
pub fn custom_note(text: &str) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "note".into(),
        content: UserContent::Text(text.into()),
        display: true,
        details: Presence::Missing,
    }
}

/// An executable tool that reports its entry and answers `released` once released.
pub fn waiting_tool(
    started: Rc<tokio::sync::Notify>,
    release: Rc<tokio::sync::Notify>,
) -> maestro_agent::SharedAgentTool<Local> {
    text_tool("wait", "Wait for release", "Wait", "released", move || {
        started.notify_one();
        let release = Rc::clone(&release);
        Box::pin(async move { release.notified().await })
    })
}

/// An executable tool named `name` that calls `on_execute` and answers `ok`.
pub fn callback_tool(
    name: &str,
    on_execute: impl Fn() + 'static,
) -> maestro_agent::SharedAgentTool<Local> {
    text_tool(name, "Run a callback", name, "ok", move || {
        on_execute();
        Box::pin(std::future::ready(()))
    })
}

/// An executable tool with no parameters that awaits the work `run` starts, then answers
/// `text`.
fn text_tool(
    name: &str,
    description: &str,
    label: &str,
    text: &'static str,
    run: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static,
) -> maestro_agent::SharedAgentTool<Local> {
    let tool = maestro_agent::AgentToolRecord::<Local> {
        definition: Tool {
            name: name.into(),
            description: description.into(),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        description: None,
        label: label.into(),
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _| {
            let work = run();
            Box::pin(async move {
                work.await;
                Ok(maestro_agent::AgentToolResult {
                    content: vec![maestro_models::UserBlock::Text(
                        maestro_models::TextContent::new(text.into()),
                    )],
                    details: Some(json!({})),
                    terminate: None,
                })
            })
        }),
        execution_mode: None,
    };
    Rc::new(RwLock::new(tool))
}

/// Notified once `session` announces the start of a retry.
pub fn retry_started(session: &AgentSession<Local>) -> Rc<tokio::sync::Notify> {
    let started = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&started);
    drop(session.subscribe(Rc::new(move |event| {
        if matches!(event, AgentSessionEvent::AutoRetryStart { .. }) {
            notify.notify_one();
        }
    })));
    started
}

/// The extension whose before-agent-start handler answers every prompt with the system
/// prompt `prompt`.
pub fn system_prompt_extension(prompt: &'static str) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |_, _| {
                Box::pin(std::future::ready(Ok(Some(BeforeAgentStartEventResult {
                    message: Presence::Missing,
                    system_prompt: Presence::Present(prompt.to_owned()),
                }))))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// Prompts `text` and waits until the session has delivered the run's end.
pub async fn prompt_to_end(
    h: &Harness,
    text: &str,
    options: maestro_app::agent_session::PromptOptions,
) {
    let ended = session_run_ended(h);
    h.session.prompt(text, options).await.unwrap();
    h.session.agent().wait_for_idle().await;
    ended.notified().await;
}

/// The echo tool, in place of the built-in tools: it marks `executed` and answers `echoed`.
pub fn echo_tools(
    executed: Rc<RefCell<bool>>,
) -> IndexMap<String, maestro_agent::SharedAgentTool<Local>> {
    let tool = maestro_agent::AgentToolRecord::<Local> {
        definition: Tool {
            name: "echo".into(),
            description: "Echo back".into(),
            parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}})
                .into(),
        },
        description: None,
        label: "Echo".into(),
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _| {
            *executed.borrow_mut() = true;
            let echoed = maestro_models::TextContent::new("echoed".into());
            Box::pin(std::future::ready(Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(echoed)],
                details: Some(json!({})),
                terminate: None,
            })))
        }),
        execution_mode: None,
    };
    IndexMap::from([("echo".to_owned(), Rc::new(RwLock::new(tool)))])
}

/// The extension whose input handler answers every input with `answer`.
pub fn input_extension(
    answer: impl Fn(&InputEvent) -> Result<Option<InputEventResult>, &'static str> + 'static,
) -> ExtensionFactory<Local> {
    let answer = Rc::new(answer);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let registered = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                let result = answer(event).map_err(Into::into);
                Box::pin(std::future::ready(result))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}
