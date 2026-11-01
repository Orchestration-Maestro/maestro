//! A reload after startup reading new prompt settings into the live resources, compared with
//! the recorded regression case.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{HarnessOptions, harness, on_task_set, recorded};
use maestro_app::{
    CreateAgentSessionFromServicesOptions, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeOptions, CreateAgentSessionRuntimeResult,
    CreateAgentSessionServicesOptions, ResourceKind, ResourceLoaderOptions,
    create_agent_session_from_services, create_agent_session_runtime,
    create_agent_session_services,
};
use maestro_models::{DiagnosticErrorInfo, Local};
use maestro_session::SessionManager;
use serde_json::json;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The runtime factory building one services-backed session over `catalog` and `model`.
fn services_factory(
    catalog: Arc<maestro_catalog::ModelRegistry>,
    model: Arc<std::sync::RwLock<maestro_models::Model>>,
    local: Rc<tokio::task::LocalSet>,
) -> CreateAgentSessionRuntimeFactory {
    Rc::new(move |input| {
        let (catalog, model, local) = (Arc::clone(&catalog), Arc::clone(&model), Rc::clone(&local));
        Box::pin(async move {
            let module_loader: maestro_extensions::ModuleLoader<Local> =
                Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
            let mut options = ResourceLoaderOptions::new(local, module_loader);
            options.disabled.insert(ResourceKind::Skills);
            options.disabled.insert(ResourceKind::Themes);
            let services = Rc::new(
                create_agent_session_services(CreateAgentSessionServicesOptions {
                    cwd: input.cwd,
                    agent_dir: Some(input.agent_dir),
                    auth_storage: None,
                    offline: None,
                    model_registry: Some(catalog),
                    settings_manager: None,
                    extension_flag_values: None,
                    resource_loader_options: options,
                })
                .await
                .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?,
            );
            let created =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services: Rc::clone(&services),
                    session_manager: input.session_manager,
                    session_start_event: input.session_start_event,
                    model: Some(model),
                    thinking_level: None,
                    scoped_models: Vec::new(),
                    tools: None,
                    no_tools: None,
                    custom_tools: Vec::new(),
                    theme: maestro_theme::LiveTheme::default(),
                })
                .await?;
            Ok(CreateAgentSessionRuntimeResult {
                session: created.session,
                diagnostics: services.diagnostics.clone(),
                services,
                extensions_result: created.extensions_result,
                model_fallback_message: created.model_fallback_message,
            })
        })
    })
}

/// The names of the session's live prompt templates.
fn template_names(runtime: &maestro_app::AgentSessionRuntime) -> Vec<String> {
    runtime
        .session()
        .prompt_templates()
        .prompts
        .iter()
        .map(|prompt| prompt.name.clone())
        .collect()
}

#[test]
fn reload_reads_saved_prompt_exclusion_into_live_resources() {
    let expected = recorded(
        "model_navigation",
        "reload_reads_saved_prompt_exclusion_into_live_resources",
    );
    on_task_set(|local| async move {
        let h = harness(HarnessOptions::default()).await;
        let cwd = h.scratch.path().join("cwd");
        let agent = h.scratch.path().join("agent");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(agent.join("prompts")).unwrap();
        std::fs::write(agent.join("prompts/test.md"), "Echo test prompt\n").unwrap();
        let cwd = cwd.to_str().unwrap().to_owned();
        let agent_dir = agent.to_str().unwrap().to_owned();
        let factory =
            services_factory(Arc::clone(&h.registry), h.model("plain"), Rc::clone(&local));
        let manager = SessionManager::create(&cwd, Some(&agent_dir), &agent_dir).unwrap();
        let runtime = create_agent_session_runtime(
            factory,
            CreateAgentSessionRuntimeOptions {
                cwd,
                agent_dir: agent_dir.clone(),
                session_manager: Rc::new(RefCell::new(manager)),
                session_start_event: None,
            },
        )
        .await
        .unwrap();
        let before = template_names(&runtime);
        std::fs::write(
            agent.join("settings.json"),
            "{\"prompts\":[\"-prompts/test.md\"]}\n",
        )
        .unwrap();
        runtime.session().reload().await.unwrap();
        let settings = serde_json::to_value(
            runtime
                .services()
                .settings_manager
                .borrow()
                .get_global_settings(),
        )
        .unwrap();
        let observed = json!({
            "before": before,
            "settings": settings["prompts"],
            "after": template_names(&runtime),
        });
        assert_eq!(observed, expected);
        runtime.dispose().await.unwrap();
        h.session.dispose();
    });
}
