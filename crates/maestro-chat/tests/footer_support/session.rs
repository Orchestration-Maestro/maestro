//! The interactive-mode test session, built over a catalog read from its directory and
//! in-memory settings and loader.
use maestro_app::resources::resource_loader::ResourceLoader;
use maestro_app::{
    AgentSession, AgentSessionConfig, AgentSessionServices, AuthStorage, ConfigValues,
    CreateAgentSessionRuntimeResult, DefaultResourceLoader, DefaultResourceLoaderOptions, Local,
    ModelRegistry, ModuleLoader, ProcessConfigValueOperations, ResourceLoaderOptions,
    SessionManager, Settings, SettingsManager,
};
use maestro_theme::LiveTheme;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// A memory session over its own services; its resource loader is `loader` applied to the
/// default one.
pub fn session(
    cwd: &str,
    oauth_provider: Option<&str>,
    loader: impl FnOnce(Rc<dyn ResourceLoader<Local>>) -> Rc<dyn ResourceLoader<Local>>,
) -> CreateAgentSessionRuntimeResult {
    session_over(cwd, oauth_provider, loader, None)
}

/// The same session, over a history file under `session_dir` when supplied.
pub fn session_over(
    cwd: &str,
    oauth_provider: Option<&str>,
    loader: impl FnOnce(Rc<dyn ResourceLoader<Local>>) -> Rc<dyn ResourceLoader<Local>>,
    session_dir: Option<&str>,
) -> CreateAgentSessionRuntimeResult {
    session_with_auth(
        cwd,
        Some(Arc::new(AuthStorage::in_memory(credentials(
            oauth_provider,
        )))),
        loader,
        session_dir,
    )
}

/// The same session over the supplied credentials, or an empty in-memory store.
pub fn session_with_auth(
    cwd: &str,
    auth: Option<Arc<AuthStorage>>,
    loader: impl FnOnce(Rc<dyn ResourceLoader<Local>>) -> Rc<dyn ResourceLoader<Local>>,
    session_dir: Option<&str>,
) -> CreateAgentSessionRuntimeResult {
    let auth = auth.unwrap_or_else(|| Arc::new(AuthStorage::in_memory(credentials(None))));
    let services = services(cwd, auth, loader);
    let manager = match session_dir {
        Some(directory) => SessionManager::create(cwd, Some(directory), &services.agent_dir)
            .expect("a session history file"),
        None => return over(cwd, &services),
    };
    over_manager(cwd, &services, manager)
}

/// A memory session over existing `services`, as a replacement session is built.
pub fn over(
    cwd: &str,
    services: &Rc<AgentSessionServices<Local>>,
) -> CreateAgentSessionRuntimeResult {
    let manager = SessionManager::in_memory(Some(cwd)).expect("a memory session");
    over_manager(cwd, services, manager)
}

/// A session over `manager` and the services built for `cwd`.
fn over_manager(
    cwd: &str,
    services: &Rc<AgentSessionServices<Local>>,
    manager: SessionManager,
) -> CreateAgentSessionRuntimeResult {
    let session = AgentSession::new(AgentSessionConfig {
        agent: Rc::default(),
        session_manager: Rc::new(RefCell::new(manager)),
        settings_manager: Rc::clone(&services.settings_manager),
        cwd: cwd.to_owned(),
        bin_dir: String::new(),
        scoped_models: Vec::new(),
        resource_loader: Rc::clone(&services.resource_loader),
        custom_tools: Vec::new(),
        model_registry: Arc::clone(&services.model_registry),
        initial_active_tool_names: None,
        allowed_tool_names: None,
        base_tools_override: None,
        extension_runner_ref: None,
        session_start_event: None,
        tools_manager: Arc::new(maestro_app::agent_session::ToolsManager::new(
            String::new(),
            None,
        )),
        theme: LiveTheme::default(),
    })
    .expect("a memory session");
    CreateAgentSessionRuntimeResult {
        session,
        services: Rc::clone(services),
        extensions_result: services.resource_loader.get_extensions(),
        diagnostics: Vec::new(),
        model_fallback_message: None,
    }
}

/// The credentials, settings, loader and catalog the session is built over; the catalog
/// reads `models.json` in `cwd` when present.
fn services(
    cwd: &str,
    auth: Arc<AuthStorage>,
    wrap: impl FnOnce(Rc<dyn ResourceLoader<Local>>) -> Rc<dyn ResourceLoader<Local>>,
) -> Rc<AgentSessionServices<Local>> {
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let commands: ConfigValues = Arc::new(ProcessConfigValueOperations::new(|| {
        Err(std::io::Error::other("not invoked"))
    }));
    let models = std::path::Path::new(cwd).join("models.json");
    let registry = Arc::new(
        ModelRegistry::create(Arc::clone(&auth), models.display().to_string(), commands)
            .expect("a catalog"),
    );
    let module_loader: ModuleLoader<Local> = Rc::new(|_| Box::pin(async { Err("unused".into()) }));
    let loader = wrap(Rc::new(DefaultResourceLoader::new(
        DefaultResourceLoaderOptions {
            cwd: cwd.to_owned(),
            agent_dir: cwd.to_owned(),
            settings_manager: Some(Rc::clone(&settings)),
            resources: ResourceLoaderOptions::new(
                Rc::new(tokio::task::LocalSet::new()),
                module_loader,
            ),
        },
    )));
    Rc::new(AgentSessionServices {
        cwd: cwd.to_owned(),
        agent_dir: cwd.to_owned(),
        auth_storage: auth,
        settings_manager: settings,
        model_registry: registry,
        resource_loader: loader,
        diagnostics: Vec::new(),
        offline: None,
    })
}

/// Stored credentials, with a subscription for `provider` when supplied.
fn credentials(provider: Option<&str>) -> maestro_app::AuthStorageData {
    let mut data = maestro_app::AuthStorageData::new();
    if let Some(provider) = provider {
        let credential = serde_json::json!({
            "type": "oauth",
            "access": "access",
            "refresh": "refresh",
            "expires": 0
        });
        data.insert(
            provider.to_owned(),
            serde_json::from_value(credential).expect("a stored subscription"),
        );
    }
    data
}
