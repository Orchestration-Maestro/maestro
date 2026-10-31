//! Startup construction through real directory-bound application services.
use crate::support::{presenters, startup};
use maestro_app::{
    AgentSessionServices, CreateAgentSessionServicesOptions, Local, ModelResolverOutput,
    ReadonlySessionManager, ResourceKind, ResourceLoaderOptions, SessionManager, SettingsManager,
};
use maestro_cli::startup::main::runtime::{RuntimeCreation, create_runtime};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, rc::Rc, sync::Arc};
use tokio::task::LocalSet;

/// Services and their retained disposable directory.
pub(crate) struct Fixture {
    /// Temporary resources remain alive through session construction.
    scratch: Scratch,
    /// Real application services over a controlled catalog.
    services: Rc<AgentSessionServices<Local>>,
    /// Chosen in-memory history.
    history: Rc<RefCell<SessionManager>>,
    /// Theme available to extension binding.
    theme: maestro_theme::LiveTheme,
    /// Start events observed by an actual loaded extension.
    events: Rc<RefCell<Vec<String>>>,
}
impl Fixture {
    /// Reload actual application services without automatic resource discovery.
    pub(crate) async fn new(local: Rc<LocalSet>, settings: SettingsManager) -> Self {
        Self::build(local, settings, |_| {}).await
    }
    /// Retain extension load failures supplied through the loader's actual override seam.
    async fn with_errors(local: Rc<LocalSet>, settings: SettingsManager) -> Self {
        Self::build(local, settings, refuse_extensions).await
    }
    /// Load an extension that registers the tool [`PROBE_TOOL`].
    async fn with_probe_tool(local: Rc<LocalSet>, settings: SettingsManager) -> Self {
        Self::build(local, settings, |resources| {
            resources.extension_factories.push(register_probe_tool());
        })
        .await
    }
    /// Construct the actual service owner, letting `configure` adjust its resource loading.
    async fn build(
        local: Rc<LocalSet>,
        settings: SettingsManager,
        configure: impl FnOnce(&mut ResourceLoaderOptions<Local>),
    ) -> Self {
        let scratch = Scratch::new().unwrap();
        let cwd = scratch.path().to_str().unwrap();
        let mut resources = ResourceLoaderOptions::<Local>::new(
            local,
            Rc::new(|_| Box::pin(std::future::ready(Ok(None)))),
        );
        resources.disabled = [
            ResourceKind::Extensions,
            ResourceKind::Skills,
            ResourceKind::PromptTemplates,
            ResourceKind::Themes,
            ResourceKind::ContextFiles,
        ]
        .into();
        resources.system_prompt = Some("controlled startup".into());
        let events = Rc::new(RefCell::new(Vec::new()));
        resources.extension_factories.push(record_starts(&events));
        configure(&mut resources);
        let catalog = Arc::new(startup::registry(&[("startup", &["first", "second"])]));
        let services =
            maestro_app::create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: cwd.into(),
                agent_dir: Some(cwd.into()),
                auth_storage: Some(Arc::clone(catalog.auth_storage())),
                settings_manager: Some(Rc::new(RefCell::new(settings))),
                model_registry: Some(catalog),
                extension_flag_values: None,
                resource_loader_options: resources,
                offline: Some(true),
            })
            .await
            .unwrap();
        let theme = presenters::theme(scratch.path());
        theme.init_theme(Some("dark"), None).unwrap();
        let history = Rc::new(RefCell::new(SessionManager::in_memory(Some(cwd)).unwrap()));
        Self {
            scratch,
            services: Rc::new(services),
            history,
            theme: theme.theme(),
            events,
        }
    }
    /// The startup constructor operands retain their original handles.
    pub(crate) fn creation(&self) -> RuntimeCreation {
        RuntimeCreation {
            services: Rc::clone(&self.services),
            session_manager: Rc::clone(&self.history),
            session_start_event: Some(
                serde_json::from_value(
                    json!({"reason":"fork","previousSessionFile":"earlier.jsonl"}),
                )
                .unwrap(),
            ),
            theme: self.theme.clone(),
            model_id_collator: Rc::new(maestro_theme::Collator::new(&|_| None).unwrap()),
        }
    }
    /// Run one actual session construction, retaining warnings in call order.
    async fn create(
        &self,
        tokens: &[&str],
    ) -> (maestro_app::CreateAgentSessionRuntimeResult, Vec<String>) {
        let mut warnings = Vec::new();
        let result = create_runtime(
            &startup::args(tokens),
            self.creation(),
            &mut ModelResolverOutput {
                stdout: &mut Vec::new(),
                stderr: &mut warnings,
                color: false,
            },
        )
        .await
        .unwrap();
        (
            result,
            String::from_utf8(warnings)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect(),
        )
    }
}

/// Drive supplied services on the application's real local task set.
pub(crate) fn run<F: Future<Output = ()>>(name: &str, body: impl FnOnce(Rc<LocalSet>) -> F) {
    if crate::support::missing_cwd::isolated(name) {
        return;
    }
    let local = Rc::new(LocalSet::new());
    startup::runtime().block_on(local.run_until(body(Rc::clone(&local))));
}

#[test]
fn created_session_receives_every_selected_option() {
    run(
        "created_session_receives_every_selected_option",
        |local| async move {
            let fixture = Fixture::new(local, startup::settings(&[])).await;
            let (created, warnings) = fixture
                .create(&[
                    "--model",
                    "startup/first",
                    "--thinking",
                    "high",
                    "--models",
                    "startup/*",
                    "--tools",
                    "read",
                ])
                .await;
            assert!(warnings.is_empty());
            assert!(Rc::ptr_eq(
                created.session.session_manager(),
                &fixture.history
            ));
            assert!(Rc::ptr_eq(&created.services, &fixture.services));
            assert!(Rc::ptr_eq(
                created.session.settings_manager(),
                &fixture.services.settings_manager
            ));
            assert!(Arc::ptr_eq(
                created.session.model_registry(),
                &fixture.services.model_registry
            ));
            assert_eq!(created.session.model().read().unwrap().id, "first");
            assert_eq!(
                created.session.thinking_level(),
                maestro_app::ThinkingLevel::High
            );
            assert_eq!(created.session.scoped_models().len(), 2);
            assert_eq!(created.session.get_active_tool_names(), ["read"]);
            assert_eq!(
                created.services.cwd,
                fixture.scratch.path().to_str().unwrap()
            );
            created
                .session
                .bind_extensions(maestro_app::agent_session::ExtensionBindings::default())
                .await
                .unwrap();
            assert_eq!(
                fixture
                    .events
                    .borrow()
                    .iter()
                    .map(|event| serde_json::from_str::<Value>(event).unwrap())
                    .collect::<Vec<_>>(),
                [json!({"reason":"fork","previousSessionFile":"earlier.jsonl"})]
            );
            created.session.dispose();
        },
    );
}

/// The active and the registered tool names of a session created with `flag` over a loaded
/// extension that registers [`PROBE_TOOL`].
async fn probe_tool_names(local: &Rc<LocalSet>, flag: &str) -> (Vec<String>, Vec<String>) {
    let fixture = Fixture::with_probe_tool(Rc::clone(local), startup::settings(&[])).await;
    let (created, warnings) = fixture.create(&[flag]).await;
    assert!(warnings.is_empty());
    let registered = created
        .session
        .get_all_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    let active = created.session.get_active_tool_names();
    created.session.dispose();
    (active, registered)
}

#[test]
fn disabled_tool_policies_keep_extension_registered_tools_by_policy() {
    run(
        "disabled_tool_policies_keep_extension_registered_tools_by_policy",
        |local| async move {
            let (active, registered) = probe_tool_names(&local, "--no-tools").await;
            assert!(active.is_empty() && registered.is_empty());
            let (active, registered) = probe_tool_names(&local, "--no-builtin-tools").await;
            assert_eq!(active, [PROBE_TOOL]);
            for name in [PROBE_TOOL, "read"] {
                assert!(registered.iter().any(|known| known == name), "{name}");
            }
        },
    );
}

#[test]
fn startup_settings_keep_typed_reads_and_raw_values() {
    for value in [
        Value::Null,
        json!(3),
        json!({}),
        json!([]),
        json!(false),
        json!(true),
        json!(["startup/first"]),
        json!("authored"),
    ] {
        let manager = startup::settings(&[
            ("sessionDir", value.clone()),
            ("defaultProvider", value.clone()),
            ("defaultModel", value.clone()),
            ("theme", value.clone()),
            ("quietStartup", value.clone()),
            ("enabledModels", value.clone()),
            ("images", json!({"autoResize":value.clone()})),
        ]);
        assert_eq!(manager.get_session_dir().as_deref(), value.as_str());
        assert_eq!(manager.get_default_provider().as_deref(), value.as_str());
        assert_eq!(manager.get_default_model().as_deref(), value.as_str());
        assert_eq!(manager.get_theme().as_deref(), value.as_str());
        assert_eq!(
            manager.get_quiet_startup(),
            value.as_bool().unwrap_or(false)
        );
        assert_eq!(
            manager.get_image_auto_resize(),
            value.as_bool().unwrap_or(true)
        );
        assert_eq!(manager.get_enabled_models().is_some(), value.is_array());
        let raw = manager.get_global_settings();
        for field in [
            "sessionDir",
            "defaultProvider",
            "defaultModel",
            "theme",
            "quietStartup",
            "enabledModels",
        ] {
            assert_eq!(raw.0[field], value);
        }
        assert_eq!(raw.0["images"]["autoResize"], value);
    }
}

#[test]
fn api_key_requires_a_selected_cli_or_scoped_model() {
    run(
        "api_key_requires_a_selected_cli_or_scoped_model",
        |local| async move {
            let fixture = Fixture::new(local, startup::settings(&[])).await;
            for (tokens, diagnostic) in [
                (vec!["--api-key", "controlled"], true),
                (vec!["--api-key", ""], false),
                (
                    vec!["--api-key", "controlled", "--model", "startup/first"],
                    false,
                ),
                (
                    vec!["--api-key", "controlled", "--models", "startup/second"],
                    false,
                ),
            ] {
                fixture
                    .services
                    .auth_storage
                    .remove_runtime_api_key("startup");
                let (created, _) = fixture.create(&tokens).await;
                assert_eq!(created.diagnostics.iter().any(|d| d.message == "--api-key requires a model to be specified via --model, --provider/--model, or --models"),diagnostic);
                if tokens.contains(&"--model") || tokens.contains(&"--models") {
                    let key = fixture
                        .services
                        .auth_storage
                        .get_api_key("startup", false, &startup::NoCommands)
                        .await
                        .unwrap();
                    assert!(
                        key.is_some_and(|value| value == "controlled"),
                        "the selected provider receives the runtime override"
                    );
                }
                created.session.dispose();
            }
        },
    );
}

#[test]
fn thinking_is_reapplied_only_for_cli_override() {
    run(
        "thinking_is_reapplied_only_for_cli_override",
        |local| async move {
            let fixture = Fixture::new(local, startup::settings(&[])).await;
            for (tokens, level) in [
                (
                    vec!["--model", "startup/first", "--thinking", "high"],
                    maestro_app::ThinkingLevel::High,
                ),
                (
                    vec!["--model", "startup/first:low"],
                    maestro_app::ThinkingLevel::Low,
                ),
                (
                    vec!["--models", "startup/first:low"],
                    maestro_app::ThinkingLevel::Low,
                ),
                (
                    vec!["--models", "startup/first:high"],
                    maestro_app::ThinkingLevel::High,
                ),
            ] {
                let (created, _) = fixture.create(&tokens).await;
                assert_eq!(created.session.thinking_level(), level);
                created.session.dispose();
            }
        },
    );
}

#[test]
fn runtime_empty_catalog_keeps_inert_model() {
    run(
        "runtime_empty_catalog_keeps_inert_model",
        |local| async move {
            let mut fixture = Fixture::new(local, startup::settings(&[])).await;
            Rc::get_mut(&mut fixture.services).unwrap().model_registry =
                Arc::new(startup::registry(&[]));
            let (created, _) = fixture.create(&[]).await;
            let model = created.session.model();
            let model = model.read().unwrap();
            assert_eq!(
                (model.provider.as_str(), model.id.as_str()),
                ("unknown", "unknown")
            );
            assert_eq!(
                created.session.thinking_level(),
                maestro_app::ThinkingLevel::Off
            );
            assert!(
                created
                    .model_fallback_message
                    .as_deref()
                    .is_some_and(|message| !message.is_empty())
            );
            assert!(
                !fixture
                    .history
                    .borrow()
                    .get_entries()
                    .iter()
                    .any(|entry| serde_json::to_value(entry).unwrap()["type"] == "model_change")
            );
            created.session.dispose();
        },
    );
}

#[test]
fn runtime_diagnostics_keep_all_producer_groups() {
    run(
        "runtime_diagnostics_keep_all_producer_groups",
        |local| async move {
            let stored = Scratch::new().unwrap();
            let root = stored.path().to_str().unwrap();
            let settings = SettingsManager::create(root, root, ".maestro");
            let mut fixture = Fixture::with_errors(local, settings).await;
            Rc::get_mut(&mut fixture.services).unwrap().diagnostics =
                vec![maestro_app::AgentSessionRuntimeDiagnostic {
                    r#type: maestro_app::AgentSessionRuntimeDiagnosticType::Info,
                    message: "service notice".into(),
                }];
            std::fs::write(stored.path().join("settings.json"), "{").unwrap();
            fixture
                .services
                .settings_manager
                .borrow_mut()
                .reload_from_storage();
            let (created, _) = fixture.create(&["--model", "missing-model"]).await;
            let messages: Vec<_> = created
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect();
            assert_eq!(messages[0], "service notice");
            assert!(messages[1].contains("runtime creation, global settings"));
            assert_eq!(
                &messages[2..4],
                [
                    "Failed to load extension \"first\": first refusal",
                    "Failed to load extension \"second\": second refusal"
                ]
            );
            assert!(messages[4].contains("missing-model"));
            assert!(
                fixture
                    .services
                    .settings_manager
                    .borrow_mut()
                    .drain_errors()
                    .is_empty()
            );
            created.session.dispose();
        },
    );
}

/// Replace the loaded extensions' failures with two refusals through the loader's override seam.
fn refuse_extensions(resources: &mut ResourceLoaderOptions<Local>) {
    resources.extensions_override = Some(Box::new(|mut loaded| {
        loaded.errors = ["first", "second"]
            .map(|path| maestro_app::ExtensionLoadError {
                path: path.into(),
                error: format!("{path} refusal"),
            })
            .into();
        loaded
    }));
}

/// The name of the tool [`register_probe_tool`] registers, distinct from every built-in tool.
const PROBE_TOOL: &str = "extension_probe";

/// An extension registering [`PROBE_TOOL`] through the real registration owner.
fn register_probe_tool() -> maestro_app::ExtensionFactory<Local> {
    Rc::new(|api| {
        let tool =
            maestro_app::local::ToolDefinition::<maestro_app::ExtensionContext, Value, Value, ()> {
                definition: serde_json::from_value(json!({
                    "name": PROBE_TOOL,
                    "description": "probe",
                    "parameters": {"type": "object", "properties": {}},
                }))
                .unwrap(),
                label: PROBE_TOOL.into(),
                prompt_snippet: None,
                prompt_guidelines: None,
                render_shell: None,
                prepare_arguments: None,
                execution_mode: None,
                execute: Rc::new(|_, _, _, _, _| {
                    Box::pin(std::future::ready(Ok(maestro_app::AgentToolResult {
                        content: vec![],
                        details: None,
                        terminate: None,
                        usage: None,
                    })))
                }),
                render_call: None,
                render_result: None,
            };
        Box::pin(std::future::ready(api.register_tool(tool)))
    })
}

/// Record events through the real extension registration and dispatch owners.
fn record_starts(events: &Rc<RefCell<Vec<String>>>) -> maestro_app::ExtensionFactory<Local> {
    let record = Rc::clone(events);
    Rc::new(move |api| {
        let record = Rc::clone(&record);
        let registered = api.on::<maestro_app::SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |event, _| {
                record
                    .borrow_mut()
                    .push(serde_json::to_string(event).unwrap());
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}
