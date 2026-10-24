//! A session runtime over the scripted provider: the factory that creates each session of
//! a runtime, and a runtime over a persisted session with chosen extensions.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, TestResourceLoader, create_test_extensions_result,
    create_test_resource_loader, harness,
};
use maestro_app::{
    AgentSessionRuntime, AgentSessionServices, CreateAgentSessionOptions,
    CreateAgentSessionRuntimeFactory, CreateAgentSessionRuntimeOptions,
    CreateAgentSessionRuntimeResult, SessionResources, create_agent_session,
    create_agent_session_runtime,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::{ExtensionFactory, LoadExtensionsResult};
use maestro_models::Local;
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_settings::SettingsManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// What a runtime under test records: replacement steps and factory calls.
#[derive(Clone, Default)]
pub struct Recorded {
    /// Invalidation, rebinding and continuation steps, in order.
    pub order: Rc<RefCell<Vec<String>>>,
    /// Each factory call's directory, session file and start event.
    pub factory_calls: Rc<RefCell<Vec<Value>>>,
}

/// The recorded form of a start event, with its event type.
pub fn start_event_view(event: &maestro_extensions::SessionStartEvent) -> Value {
    let mut view = json!({"type": "session_start"});
    for (key, value) in serde_json::to_value(event).unwrap().as_object().unwrap() {
        view[key] = value.clone();
    }
    view
}

/// The factory creating each runtime over the fixture's catalog with `extensions` loaded.
pub fn factory(
    h: &Harness,
    extensions: &Rc<LoadExtensionsResult<Local>>,
    recorded: &Recorded,
) -> CreateAgentSessionRuntimeFactory {
    let (registry, model) = (Arc::clone(&h.registry), h.model("faux-1"));
    let (extensions, calls) = (Rc::clone(extensions), Rc::clone(&recorded.factory_calls));
    Rc::new(move |options: CreateAgentSessionRuntimeOptions| {
        let (registry, model, extensions) = (
            Arc::clone(&registry),
            Arc::clone(&model),
            Rc::clone(&extensions),
        );
        calls.borrow_mut().push(json!({
            "cwd": options.cwd,
            "session_file": options.session_manager.borrow().get_session_file(),
            "session_start_event": options.session_start_event.as_ref().map(start_event_view),
        }));
        Box::pin(async move {
            let settings = Rc::new(RefCell::new(SettingsManager::in_memory(
                maestro_settings::Settings::default(),
            )));
            let loader: Rc<dyn maestro_app::resources::resource_loader::ResourceLoader<Local>> =
                Rc::new(TestResourceLoader {
                    extensions,
                    ..create_test_resource_loader(None)
                });
            let created = create_agent_session(CreateAgentSessionOptions {
                cwd: Some(options.cwd.clone()),
                agent_dir: Some(options.agent_dir.clone()),
                auth_storage: None,
                model_registry: Some(Arc::clone(&registry)),
                model: Some(model),
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
                }),
                diagnostics: Vec::new(),
                extensions_result: created.extensions_result,
                model_fallback_message: created.model_fallback_message,
            })
        })
    })
}

/// A runtime over a persisted session at `<root>/<label>` with `factories` loaded, and what
/// it records.
pub async fn runtime_for(
    label: &str,
    factories: Vec<(Option<&str>, ExtensionFactory<Local>)>,
) -> (Harness, AgentSessionRuntime, Recorded) {
    let h = harness(HarnessOptions {
        models: vec![("faux-1", true)],
        auth: Auth::Key,
        ..HarnessOptions::default()
    })
    .await;
    let root = h.scratch.path().display().to_string();
    let cwd = format!("{root}/{label}");
    std::fs::create_dir_all(&cwd).unwrap();
    let extensions = Rc::new(create_test_extensions_result(factories, &cwd).await);
    let recorded = Recorded::default();
    let manager = SessionManager::create(&cwd, Some(&format!("{cwd}/sessions")), &root).unwrap();
    let manager = Rc::new(RefCell::new(manager));
    let create = factory(&h, &extensions, &recorded);
    let options = |manager: &Rc<RefCell<SessionManager>>| CreateAgentSessionRuntimeOptions {
        cwd: cwd.clone(),
        agent_dir: root.clone(),
        session_manager: Rc::clone(manager),
        session_start_event: None,
    };
    // The recorded cases created a session over the history before the runtime took it.
    drop(create(options(&manager)).await.unwrap());
    recorded.factory_calls.borrow_mut().clear();
    let runtime = create_agent_session_runtime(create, options(&manager))
        .await
        .unwrap();
    (h, runtime, recorded)
}
