//! A session over the scripted provider with in-memory credentials, catalog, settings and
//! history, plus the recorded corpus it is compared with.
#![cfg(not(target_arch = "wasm32"))]
use indexmap::IndexMap;
use maestro_agent::{Agent, AgentInitialState, AgentOptions};
use maestro_app::agent_session::{AgentSession, AgentSessionConfig, AgentSessionEvent};
use maestro_app::resources::resource_loader::{
    LoadPromptsResult, LoadThemesResult, ResourceExtensionPaths, ResourceLoader,
};
use maestro_catalog::{ModelRegistry, ProviderConfigInput, ProviderModelInput, ScopedModel};
use maestro_credentials::{AuthCredential, AuthStorage, AuthStorageData, ConfigValueOperations};
use maestro_extensions::{
    ExtensionContext, ExtensionExecution, ExtensionFactory, ExtensionRunner, LoadExtensionsResult,
    RegisteredTool, RegisteredToolInput, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory, runner::ExtensionRunnerSession,
};
use maestro_models::system_prompt::ContextFile;
use maestro_models::{
    Execution, FauxModelDefinition, FauxProviderRegistration, Local, Model,
    RegisterFauxProviderOptions, Tool, register_faux_provider,
};
use maestro_resources::LoadSkillsResult;
use maestro_resources::{SyntheticSourceOptions, create_synthetic_source_info};
use maestro_session::{Presence, SessionManager, messages::CustomAgentMessages};
use maestro_settings::SettingsManager;
use maestro_tools::ToolDefinitionRecord;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io,
    rc::Rc,
    sync::{Arc, RwLock},
};

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
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            settings: json!({}),
            models: vec![("plain", false), ("think", true), ("extra", true)],
            factories: Vec::new(),
            auth: Auth::Key,
            context_window: None,
            session_manager: None,
            system_prompt: None,
            append_system_prompt: Vec::new(),
        }
    }
}

/// A session and the owners the tests observe.
pub struct Harness {
    /// The scripted provider.
    pub faux: FauxProviderRegistration,
    /// The session under test.
    pub session: AgentSession<Local>,
    /// The catalog the session was given.
    pub registry: Arc<ModelRegistry>,
    /// Thinking and name events the session announced.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// The session working directory, below the scratch root.
    pub cwd: String,
    /// The disposable root removed with the harness.
    pub scratch: maestro_test_directory::Scratch,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.faux.unregister();
    }
}

impl Harness {
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

/// Commands that never produce a value, as a failing shell command does.
pub struct FailingCommands;

impl ConfigValueOperations for FailingCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// A loader returning the supplied extensions and no other resources.
pub struct TestResourceLoader {
    /// The loaded extensions.
    pub extensions: Rc<LoadExtensionsResult<Local>>,
    /// The replacement system prompt.
    pub system_prompt: Option<Rc<str>>,
    /// The appended prompt texts.
    pub append_system_prompt: Rc<[String]>,
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
    }
}

impl ResourceLoader<Local> for TestResourceLoader {
    fn get_extensions(&self) -> Rc<LoadExtensionsResult<Local>> {
        Rc::clone(&self.extensions)
    }
    fn get_skills(&self) -> Rc<LoadSkillsResult> {
        Rc::default()
    }
    fn get_prompts(&self) -> Rc<LoadPromptsResult> {
        Rc::default()
    }
    fn get_themes(&self) -> Rc<LoadThemesResult> {
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
    fn extend_resources(&self, _: ResourceExtensionPaths) -> io::Result<()> {
        Ok(())
    }
    fn reload(&self) -> <Local as Execution>::Future<'_, io::Result<()>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

/// Loads `factories` as one extension set, each at `<inline:N>`.
pub async fn create_test_extensions_result(
    factories: Vec<ExtensionFactory<Local>>,
    cwd: &str,
) -> LoadExtensionsResult<Local> {
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|task| drop(tokio::task::spawn_local(task))),
        Rc::new(|_| {}),
    );
    let mut extensions = Vec::new();
    for (index, factory) in factories.into_iter().enumerate() {
        let path = format!("<inline:{}>", index + 1);
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
    let registry = ModelRegistry::in_memory(storage).unwrap();
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
            parameters: json!({"type": "object", "properties": {}}),
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

/// `definition` registered as supplied by the SDK, under `<sdk:name>`.
pub fn sdk_registered(definition: Definition) -> RegisteredTool<Local> {
    let source = create_synthetic_source_info(
        format!("<sdk:{}>", definition.definition.name),
        SyntheticSourceOptions {
            source: "sdk".into(),
            scope: None,
            origin: None,
            base_dir: None,
        },
    );
    RegisteredTool {
        definition: Local::retain_definition(RegisteredToolInput::from(definition)),
        source_info: Arc::new(RwLock::new(source)),
    }
}

/// The recorded tools: whitespace-heavy snippets, blank and repeated guidelines.
pub fn sdk_tools() -> IndexMap<String, RegisteredTool<Local>> {
    let tools = [
        tool_definition(
            "alpha",
            Some("  Read\n  files\r\n quickly  "),
            Some(&["  Prefer read ", "", "Prefer read", " Keep it short", "\t"]),
        ),
        tool_definition("beta", Some("   "), None),
        tool_definition("gamma", Some("\u{feff}x\u{85}y"), Some(&["Use gamma"])),
        tool_definition("delta", None, Some(&[])),
    ];
    tools
        .into_iter()
        .map(|definition| {
            (
                definition.definition.name.clone(),
                sdk_registered(definition),
            )
        })
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

/// A local agent on the first scripted model, with the window replaced when given.
fn agent(model: &Model, window: Option<f64>) -> Rc<Agent<CustomAgentMessages, Local>> {
    let mut initial = model.clone();
    initial.context_window = window.unwrap_or(initial.context_window);
    Rc::new(Agent::new_local(AgentOptions {
        spawn: Some(Rc::new(|task| drop(tokio::task::spawn_local(task)))),
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(initial))),
            system_prompt: Some("base".into()),
            ..AgentInitialState::default()
        },
        ..AgentOptions::default()
    }))
}

/// The runner over the loaded extensions, the session history and the catalog.
fn runner(
    extensions: &LoadExtensionsResult<Local>,
    cwd: &str,
    session_manager: &Rc<RefCell<SessionManager>>,
    registry: &Arc<ModelRegistry>,
) -> Rc<ExtensionRunner<Local>> {
    Rc::new(ExtensionRunner::<Local>::new(
        Local::share(RwLock::new(extensions.extensions.clone())),
        extensions.runtime.clone(),
        ExtensionRunnerSession {
            cwd: cwd.to_owned(),
            session_manager: session_manager.clone(),
            model_registry: Arc::clone(registry),
        },
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// A session over the scripted provider as `options` describe it.
pub async fn harness(options: HarnessOptions) -> Harness {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let cwd = format!("{}/project", scratch.path().display());
    let faux = register_faux_provider(RegisterFauxProviderOptions {
        models: Some(
            options
                .models
                .iter()
                .map(|&(id, reasoning)| FauxModelDefinition {
                    id: id.into(),
                    reasoning: Some(reasoning),
                    ..FauxModelDefinition::default()
                })
                .collect(),
        ),
        ..RegisterFauxProviderOptions::default()
    });
    let registry = Arc::new(catalog(&faux, options.auth, options.context_window));
    let agent = agent(&faux.models[0], options.context_window);
    let session_manager =
        Rc::new(RefCell::new(options.session_manager.unwrap_or_else(|| {
            SessionManager::in_memory(Some(&cwd)).unwrap()
        })));
    let extensions = create_test_extensions_result(options.factories, &cwd).await;
    let runner = runner(&extensions, &cwd, &session_manager, &registry);
    let settings: maestro_settings::Settings = serde_json::from_value(options.settings).unwrap();
    let session = AgentSession::new(AgentSessionConfig {
        agent,
        session_manager,
        settings_manager: Rc::new(RefCell::new(SettingsManager::in_memory(settings))),
        cwd: cwd.clone(),
        scoped_models: Vec::new(),
        resource_loader: Rc::new(TestResourceLoader {
            system_prompt: options.system_prompt.map(Rc::from),
            append_system_prompt: Rc::from(options.append_system_prompt),
            ..create_test_resource_loader(Some(extensions))
        }),
        model_registry: Arc::clone(&registry),
        extension_runner: runner,
        config_value_operations: Arc::new(FailingCommands),
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    let _unsubscribe = session.subscribe(Rc::new(move |event| {
        if let Some(view) = session_event_view(event) {
            recorded.borrow_mut().push(view);
        }
    }));
    Harness {
        faux,
        session,
        registry,
        events,
        cwd,
        scratch,
    }
}

/// The recorded form of a thinking or name event.
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
        _ => None,
    }
}

/// Runs `body` on a current-thread runtime inside a task set.
pub fn on_task_set<F: std::future::Future>(body: F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, body)
}
