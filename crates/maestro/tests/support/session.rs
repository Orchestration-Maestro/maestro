//! SDK construction shared by native component composition tests.
use maestro_app::{
    AgentSession, CreateAgentSessionOptions, ExtensionBindings, Local, NoTools,
    ResourceLoaderOptions, SessionResources,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Build an isolated SDK session with the production component factory, the application's
/// native effects and no ambient auth.
pub async fn component_session(
    root: &super::Root,
    local: Rc<tokio::task::LocalSet>,
    component: String,
    no_tools: Option<NoTools>,
    widgets: Rc<dyn maestro_app::ExtensionWidgets>,
) -> Rc<AgentSession> {
    let io = maestro_app::extension_native_io::extension_native_io(&local);
    let mut resources =
        ResourceLoaderOptions::<Local>::new(local, Rc::new(|_| Box::pin(async { Ok(None) })));
    resources
        .extension_factories
        .push(maestro_extensions_wasmtime::artifact_factory_with_io(
            component,
            widgets,
            maestro_extensions_wasmtime::Installation {
                version: env!("CARGO_PKG_VERSION").into(),
                agent_dir: root.path("agent").display().to_string(),
            },
            io,
            None,
        ));
    resource_session(root, resources, no_tools).await
}

/// Builds the same isolated SDK session from caller-supplied component effects.
pub async fn resource_session(
    root: &super::Root,
    mut resources: ResourceLoaderOptions<Local>,
    no_tools: Option<NoTools>,
) -> Rc<AgentSession> {
    resources.system_prompt = Some(String::new());
    resources.append_system_prompt = Some(Vec::new());
    resources.offline = Some(true);
    let cwd = root.path("project").display().to_string();
    let created = maestro_app::create_agent_session(CreateAgentSessionOptions {
        cwd: Some(cwd.clone()), agent_dir: Some(root.path("agent").display().to_string()),
        auth_storage: Some(Arc::new(maestro_app::AuthStorage::in_memory(maestro_app::AuthStorageData::default()))),
        model_registry: None,
        model: Some(Arc::new(std::sync::RwLock::new(serde_json::from_value(json!({
            "id":"registered-case","name":"registered","api":"guest-owned-stream",
            "provider":"compiled-stream","baseUrl":"https://example.invalid","reasoning":false,"input":["text"],
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":10000,"maxTokens":10
        })).unwrap()))),
        thinking_level: None, scoped_models: Vec::new(), no_tools,
        tools: None, custom_tools: Vec::new(), resources: SessionResources::Default(Box::new(resources)),
        session_manager: Some(Rc::new(RefCell::new(maestro_app::SessionManager::in_memory(Some(&cwd)).unwrap()))),
        settings_manager: Some(Rc::new(RefCell::new(maestro_app::SettingsManager::in_memory(maestro_app::Settings::default())))),
        session_start_event: None, theme: maestro_theme::LiveTheme::default(), offline: Some(true),
    }).await.unwrap();
    assert!(created.extensions_result.errors.is_empty());
    created
        .session
        .bind_extensions(ExtensionBindings::default())
        .await
        .unwrap();
    created.session
}
