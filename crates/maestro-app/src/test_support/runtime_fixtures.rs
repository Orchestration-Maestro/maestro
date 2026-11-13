//! A session runtime over the scripted provider: the factory that creates each session of
//! a runtime, and a runtime over file-backed or in-memory history with chosen extensions.
use super::harness::{Auth, Harness, HarnessOptions, harness};
use super::records::answer;
use super::resource_loader::{
    TestResourceLoader, create_test_extensions_result, create_test_resource_loader,
};
use crate::resources::loading::{LoadPromptsResult, ResourceLoader};
use crate::{
    AgentSessionRuntime, AgentSessionServices, CreateAgentSessionOptions,
    CreateAgentSessionRuntimeFactory, CreateAgentSessionRuntimeOptions,
    CreateAgentSessionRuntimeResult, SessionResources, create_agent_session,
    create_agent_session_runtime,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::{EventError, ExtensionFactory, LoadExtensionsResult, SessionStartEvent};
use maestro_models::{DiagnosticErrorInfo, Local, Model};
use maestro_models_registry::ModelRegistry;
use maestro_resources::{LoadSkillsResult, PromptTemplate, Skill};
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_settings::{Settings, SettingsManager};
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// What a runtime under test records: replacement steps and factory calls.
#[derive(Clone, Default)]
pub struct Recorded {
    /// Invalidation, rebinding and continuation steps, in order.
    pub order: Rc<RefCell<Vec<String>>>,
    /// Each factory call's directory, session file and start event.
    pub factory_calls: Rc<RefCell<Vec<Value>>>,
}

/// The recorded form of a start event, with its event type.
fn start_event_view(event: &SessionStartEvent) -> Value {
    let mut view = Map::from_iter([("type".to_owned(), json!("session_start"))]);
    if let Value::Object(fields) = json!(event) {
        view.extend(fields);
    }
    Value::Object(view)
}

/// The resources every session of a runtime loads.
#[derive(Clone)]
pub struct Resources {
    /// Whether the caller supplies the first model instead of restoring history.
    pub bootstrap_model: bool,
    /// The loaded extensions, shared by every session.
    pub extensions: Rc<LoadExtensionsResult<Local>>,
    /// Extensions loaded afresh for each session, as a runtime factory loads them.
    pub factories: Vec<(Option<&'static str>, ExtensionFactory<Local>)>,
    /// The loaded skills.
    pub skills: Rc<LoadSkillsResult>,
    /// The loaded prompt templates.
    pub prompts: Rc<LoadPromptsResult>,
}

impl Resources {
    /// Only `extensions`.
    #[must_use]
    pub fn of(extensions: &Rc<LoadExtensionsResult<Local>>) -> Self {
        Self {
            bootstrap_model: true,
            extensions: Rc::clone(extensions),
            factories: Vec::new(),
            skills: Rc::default(),
            prompts: Rc::default(),
        }
    }
}

/// The extensions a session over `cwd` loads: the shared set, or the factories loaded
/// afresh.
async fn session_extensions(
    resources: &Resources,
    cwd: &str,
) -> Result<Rc<LoadExtensionsResult<Local>>, EventError> {
    if resources.factories.is_empty() {
        return Ok(Rc::clone(&resources.extensions));
    }
    let loaded = create_test_extensions_result(resources.factories.clone(), cwd).await?;
    Ok(Rc::new(loaded))
}

/// The factory creating each runtime over the fixture's catalog with `resources` loaded;
/// nothing when the scripted provider does not serve `faux-1`.
#[must_use]
pub fn factory<S>(
    h: &Harness<S>,
    resources: &Resources,
    recorded: &Recorded,
) -> Option<CreateAgentSessionRuntimeFactory> {
    let (registry, model) = (Arc::clone(&h.registry), h.model("faux-1")?);
    let (resources, calls) = (resources.clone(), Rc::clone(&recorded.factory_calls));
    Some(Rc::new(move |options: CreateAgentSessionRuntimeOptions| {
        calls.borrow_mut().push(json!({
            "cwd": options.cwd,
            "session_file": options.session_manager.borrow().get_session_file(),
            "session_start_event": options.session_start_event.as_ref().map(start_event_view),
        }));
        let (registry, model) = (Arc::clone(&registry), Arc::clone(&model));
        Box::pin(created(options, registry, model, resources.clone()))
    }))
}

/// One runtime of the factory: a session over `registry` starting on `model` with
/// `resources` loaded, and its services.
async fn created(
    options: CreateAgentSessionRuntimeOptions,
    registry: Arc<ModelRegistry>,
    model: Arc<RwLock<Model>>,
    resources: Resources,
) -> Result<CreateAgentSessionRuntimeResult, DiagnosticErrorInfo> {
    let extensions = session_extensions(&resources, &options.cwd)
        .await
        .map_err(|error| error.to_string())?;
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let loader: Rc<dyn ResourceLoader<Local>> = Rc::new(TestResourceLoader {
        extensions,
        skills: resources.skills,
        prompts: resources.prompts,
        ..create_test_resource_loader(None)
    });
    let created = create_agent_session(CreateAgentSessionOptions {
        cwd: Some(options.cwd.clone()),
        agent_dir: Some(options.agent_dir.clone()),
        auth_storage: None,
        model_registry: Some(Arc::clone(&registry)),
        model: resources.bootstrap_model.then_some(model),
        thinking_level: None,
        scoped_models: Vec::new(),
        no_tools: None,
        tools: None,
        custom_tools: Vec::new(),
        resources: SessionResources::Loader(Rc::clone(&loader)),
        session_manager: Some(options.session_manager),
        settings_manager: Some(Rc::clone(&settings)),
        session_start_event: options.session_start_event,
        theme: maestro_theme::LiveTheme::default(),
        offline: None,
    })
    .await?;
    Ok(CreateAgentSessionRuntimeResult {
        session: created.session,
        services: Rc::new(AgentSessionServices {
            cwd: options.cwd,
            agent_dir: options.agent_dir,
            auth_storage: Arc::new(AuthStorage::in_memory(AuthStorageData::default())),
            settings_manager: settings,
            model_registry: registry,
            resource_loader: loader,
            diagnostics: Vec::new(),
            offline: None,
        }),
        diagnostics: Vec::new(),
        extensions_result: created.extensions_result,
        model_fallback_message: created.model_fallback_message,
    })
}

/// What a test runtime loads besides the scripted provider.
pub struct RuntimeOptions {
    /// Whether the runtime stores its session in a file.
    pub persisted: bool,
    /// Whether the caller supplies the first model instead of restoring history.
    pub bootstrap_model: bool,
    /// Inline extensions, each at its supplied path or `<inline:N>`.
    pub factories: Vec<(Option<&'static str>, ExtensionFactory<Local>)>,
    /// Scripted models as identity and reasoning support.
    pub models: Vec<(&'static str, bool)>,
    /// The scripted provider's credentials.
    pub auth: Auth,
    /// The loaded skills.
    pub skills: Vec<Skill>,
    /// The loaded prompt templates.
    pub prompts: Vec<PromptTemplate>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            persisted: true,
            bootstrap_model: true,
            factories: Vec::new(),
            models: vec![("faux-1", true)],
            auth: Auth::Key,
            skills: Vec::new(),
            prompts: Vec::new(),
        }
    }
}

/// A runtime over a persisted session at `<root>/<label>` with `factories` loaded, and what
/// it records; `scratch` names the root.
///
/// # Errors
/// Returns the first setup step that failed, as [`runtime_with`] reports it.
pub async fn runtime_for<S: AsRef<Path>>(
    scratch: S,
    label: &str,
    factories: Vec<(Option<&'static str>, ExtensionFactory<Local>)>,
) -> Result<(Harness<S>, AgentSessionRuntime, Recorded), EventError> {
    let options = RuntimeOptions {
        factories,
        ..RuntimeOptions::default()
    };
    runtime_with(scratch, label, options).await
}

/// A runtime over file-backed or in-memory history at `<root>/<label>`, and what it
/// records; `scratch` names the root.
///
/// # Errors
/// Returns the first setup step that failed: the session harness, the session directory,
/// an inline extension, the history or the runtime itself.
pub async fn runtime_with<S: AsRef<Path>>(
    scratch: S,
    label: &str,
    options: RuntimeOptions,
) -> Result<(Harness<S>, AgentSessionRuntime, Recorded), EventError> {
    let root = scratch.as_ref().display().to_string();
    let cwd = format!("{root}/{label}");
    let models = HarnessOptions {
        models: options.models,
        auth: options.auth,
        ..HarnessOptions::default()
    };
    let h = harness(scratch, models).await?;
    std::fs::create_dir_all(&cwd)?;
    let resources = Resources {
        bootstrap_model: options.bootstrap_model,
        factories: options.factories,
        skills: Rc::new(LoadSkillsResult {
            skills: options.skills,
            diagnostics: Vec::new(),
        }),
        prompts: Rc::new(LoadPromptsResult {
            prompts: options.prompts,
            diagnostics: Vec::new(),
        }),
        ..Resources::of(&Rc::new(
            create_test_extensions_result(Vec::new(), &cwd).await?,
        ))
    };
    let recorded = Recorded::default();
    let manager = if options.persisted {
        SessionManager::create(&cwd, Some(&format!("{cwd}/sessions")), &root)?
    } else {
        SessionManager::in_memory(Some(&cwd))?
    };
    let manager = Rc::new(RefCell::new(manager));
    let create = factory(&h, &resources, &recorded).ok_or("the runtime needs model faux-1")?;
    let options = |manager: &Rc<RefCell<SessionManager>>| CreateAgentSessionRuntimeOptions {
        cwd: cwd.clone(),
        agent_dir: root.clone(),
        session_manager: Rc::clone(manager),
        session_start_event: None,
    };
    // The recorded cases created a session over the history before the runtime took it.
    drop(create(options(&manager)).await?);
    recorded.factory_calls.borrow_mut().clear();
    let runtime = create_agent_session_runtime(create, options(&manager)).await?;
    Ok((h, runtime, recorded))
}

/// A session stored in `sessions` for `cwd`, holding one exchange.
///
/// # Errors
/// Returns the failure to create the session file or to append a message.
pub fn stored_in(cwd: &str, sessions: &str, root: &str) -> std::io::Result<SessionManager> {
    let mut manager = SessionManager::create(cwd, Some(sessions), root)?;
    manager.append_message(json!({"role": "user", "content": "kept", "timestamp": 0}))?;
    manager.append_message(json!(answer("stored", None, None)))?;
    Ok(manager)
}
