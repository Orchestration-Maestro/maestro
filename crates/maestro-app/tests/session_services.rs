//! Services bound to one session directory: their defaults, provider registrations and
//! extension flag values.
#![cfg(unix)]
use crate::resource_fixtures::{
    Root, factory, module_loader, no_commands, on_task_set, with_environment, with_isolated_home,
};
use indexmap::IndexMap;
use maestro_app::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, AgentSessionServices,
    CreateAgentSessionServicesOptions, ExtensionFactory, FlagValue, ProviderConfig,
    ProviderModelConfig, ResourceKind, ResourceLoaderOptions, create_agent_session_services,
};
use maestro_catalog::{ModelRegistry, ResolvedRequestAuth};
use maestro_credentials::{AuthCredential, AuthStorage, AuthStorageData};
use maestro_models::{Local, ModelCost, ModelInput};
use maestro_settings::{Settings, SettingsManager};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::task::LocalSet;

/// One recorded services case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ServicesCase {
    name: String,
    flag_values: Option<Map<String, Value>>,
    expected: Value,
}

/// The named recorded services case.
fn services_case(name: &str) -> ServicesCase {
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/resources/session_services.json")).unwrap();
    let cases: Vec<ServicesCase> = serde_json::from_value(corpus["cases"].clone()).unwrap();
    cases.into_iter().find(|case| case.name == name).unwrap()
}

/// Services inputs for `root` with in-memory credentials, the agent directory and the
/// supplied inline factories.
fn options(
    root: &Root,
    local: Rc<LocalSet>,
    factories: Vec<ExtensionFactory<Local>>,
) -> CreateAgentSessionServicesOptions<Local> {
    let mut resources = ResourceLoaderOptions::new(local, module_loader());
    resources.extension_factories = factories;
    CreateAgentSessionServicesOptions {
        cwd: root.project(),
        agent_dir: Some(root.agent()),
        auth_storage: Some(Arc::new(AuthStorage::in_memory(AuthStorageData::default()))),
        settings_manager: None,
        model_registry: None,
        extension_flag_values: None,
        resource_loader_options: resources,
        offline: None,
    }
}

/// The recorded projection of a diagnostic list.
fn diagnostics(diagnostics: &[AgentSessionRuntimeDiagnostic]) -> Value {
    let kind = |diagnostic: &AgentSessionRuntimeDiagnostic| match diagnostic.r#type {
        AgentSessionRuntimeDiagnosticType::Info => "info",
        AgentSessionRuntimeDiagnosticType::Warning => "warning",
        AgentSessionRuntimeDiagnosticType::Error => "error",
    };
    (diagnostics.iter())
        .map(|d| json!({"type": kind(d), "message": d.message}))
        .collect()
}

/// A recorded flag value: text or a boolean.
fn flag_value(value: &Value) -> FlagValue {
    value.as_str().map_or_else(
        || FlagValue::Boolean(value.as_bool().unwrap()),
        |text| FlagValue::String(text.to_owned()),
    )
}

/// The case's flag values in recorded order, when it has any.
fn flag_values(case: &ServicesCase) -> Option<IndexMap<String, FlagValue>> {
    let values = case.flag_values.as_ref()?;
    Some(
        values
            .iter()
            .map(|(name, value)| (name.clone(), flag_value(value)))
            .collect(),
    )
}

/// The runtime flag values as recorded.
fn runtime_flags(services: &AgentSessionServices<Local>) -> Value {
    let extensions = services.resource_loader.get_extensions();
    let flags = extensions.runtime.flag_values.read().unwrap();
    (flags.iter())
        .map(|(name, value)| match value {
            FlagValue::Boolean(flag) => (name.clone(), json!(flag)),
            FlagValue::String(text) => (name.clone(), json!(text)),
        })
        .collect::<Map<_, _>>()
        .into()
}

#[test]
fn extension_flag_values_follow_declared_types() {
    with_isolated_home("extension_flag_values_follow_declared_types", |base| {
        for name in [
            "boolean_flag_is_set_true_even_with_text",
            "boolean_flag_with_false_is_set_true",
            "string_flag_takes_text",
            "string_flag_without_text_requires_value",
            "one_unknown_option",
            "several_unknown_options",
            "absent_flag_values_add_nothing",
        ] {
            let case = services_case(name);
            let root = Root::new(base, name);
            let spec = serde_json::from_value(
                json!({"flags": [["bool-flag", "boolean"], ["text-flag", "string"]]}),
            )
            .unwrap();
            let values = flag_values(&case);
            let observed = on_task_set(|local| async move {
                let mut options = options(&root, local, vec![factory(spec)]);
                options.extension_flag_values = values;
                let services = create_agent_session_services(options).await.unwrap();
                json!({
                    "diagnostics": diagnostics(&services.diagnostics),
                    "runtimeFlags": runtime_flags(&services),
                })
            });
            assert_eq!(observed, case.expected, "case {name}");
        }
    });
}

/// A provider with one complete model, or a model list without an endpoint.
fn provider(complete: bool) -> ProviderConfig {
    let model = ProviderModelConfig {
        id: if complete { "good-model" } else { "m" }.into(),
        name: "Good".into(),
        api: None,
        base_url: None,
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 1000.0,
        max_tokens: 100.0,
        headers: None,
        compat: None,
    };
    ProviderConfig {
        base_url: complete.then(|| "https://example.test/v1".into()),
        api_key: complete.then(|| "KEY".into()),
        api: complete.then(|| "openai-completions".into()),
        models: Some(vec![model]),
        ..ProviderConfig::default()
    }
}

/// A factory registering `config` as `name`.
fn registering(name: &'static str, complete: bool) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered =
            api.register_provider(name.into(), Arc::new(RwLock::new(provider(complete))));
        Box::pin(async move { registered })
    })
}

#[test]
fn provider_registrations_apply_individually_and_clear() {
    with_isolated_home(
        "provider_registrations_apply_individually_and_clear",
        |base| {
            let case = services_case("provider_registrations_apply_individually");
            let root = Root::new(base, &case.name);
            let observed = on_task_set(|local| async move {
                let factories = vec![
                    registering("good-provider", true),
                    registering("bad-provider", false),
                ];
                let services = create_agent_session_services(options(&root, local, factories))
                    .await
                    .unwrap();
                let extensions = services.resource_loader.get_extensions();
                let pending = extensions
                    .runtime
                    .pending_provider_registrations
                    .read()
                    .unwrap()
                    .len();
                let found = |provider: &str, id: &str| {
                    (services.model_registry.find(provider, id))
                        .map(|model| model.read().unwrap().id.clone())
                };
                json!({
                    "diagnostics": diagnostics(&services.diagnostics),
                    "pending": pending,
                    "goodModel": found("good-provider", "good-model"),
                    "badModel": found("bad-provider", "m"),
                })
            });
            assert_eq!(observed, case.expected);
        },
    );
}

#[test]
fn services_bind_auth_catalog_settings_and_resources() {
    with_isolated_home(
        "services_bind_auth_catalog_settings_and_resources",
        |base| {
            let case = services_case("services_bind_auth_models_settings_and_resources");
            let root = Root::new(base, &case.name);
            let model = json!({
                "id": "file-model", "name": "File", "reasoning": false, "input": ["text"],
                "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
                "contextWindow": 1000, "maxTokens": 100,
            });
            let files = [
                (
                    "agent/auth.json",
                    json!({"custom-provider": {"type": "api_key", "key": "FILE_KEY"}}),
                ),
                (
                    "agent/models.json",
                    json!({"providers": {"custom-provider": {
                        "baseUrl": "https://example.test/v1", "apiKey": "MODELS_KEY",
                        "api": "openai-completions", "models": [model],
                    }}}),
                ),
                (
                    "project/.maestro/settings.json",
                    json!({"defaultThinkingLevel": "low"}),
                ),
            ];
            for (path, document) in files {
                let path = format!("{}/{path}", root.path);
                std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
                std::fs::write(path, document.to_string()).unwrap();
            }
            std::fs::create_dir_all(root.fill("$ROOT/agent/skills")).unwrap();
            let skill = "---\nname: service-skill\ndescription: s\n---\nSkill content here.";
            std::fs::write(root.fill("$ROOT/agent/skills/s.md"), skill).unwrap();
            let observed = on_task_set(|local| async {
                let mut options = options(&root, local, Vec::new());
                options.auth_storage = None;
                let services = create_agent_session_services(options).await.unwrap();
                let api_key = match services.auth_storage.get("custom-provider") {
                    Some(AuthCredential::ApiKey(credential)) => Some(credential.key),
                    _ => None,
                };
                let model = (services
                    .model_registry
                    .find("custom-provider", "file-model"))
                .map(|model| model.read().unwrap().id.clone());
                let settings = services.settings_manager.borrow();
                let skills = services.resource_loader.get_skills();
                root.normalize(&json!({
                    "cwd": services.cwd, "agentDir": services.agent_dir, "apiKey": api_key,
                    "model": model, "registryError": services.model_registry.get_error(),
                    "thinking": settings.get_default_thinking_level().map(Value::from),
                    "skills": skills.skills.iter().map(|s| &s.name).collect::<Vec<_>>(),
                    "diagnostics": diagnostics(&services.diagnostics),
                }))
            });
            assert_eq!(observed, case.expected);
        },
    );
}

/// Without an agent directory the services use the environment-selected one.
#[test]
fn omitted_agent_dir_uses_the_environment_root() {
    let variables = [("MAESTRO_CODING_AGENT_DIR", "$BASE/case/env-agent")];
    with_environment(
        "omitted_agent_dir_uses_the_environment_root",
        &variables,
        |base| {
            let root = Root::new(base, "case");
            let agent_dir = on_task_set(|local| async {
                let mut options = options(&root, local, Vec::new());
                options.agent_dir = None;
                options
                    .resource_loader_options
                    .disabled
                    .insert(ResourceKind::Extensions);
                create_agent_session_services(options)
                    .await
                    .unwrap()
                    .agent_dir
            });
            assert_eq!(
                root.normalize(&json!({"agentDir": agent_dir})),
                json!({"agentDir": "$ROOT/env-agent"})
            );
        },
    );
}

/// Supplied credentials, settings and catalog are the services' own handles.
#[test]
fn supplied_services_are_reused() {
    with_isolated_home("supplied_services_are_reused", |base| {
        let root = Root::new(base, "case");
        let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
        let settings = Rc::new(RefCell::new(
            SettingsManager::in_memory(Settings::default()),
        ));
        let catalog = Arc::new(ModelRegistry::in_memory(Arc::clone(&auth), no_commands()).unwrap());
        on_task_set(|local| async {
            let mut options = options(&root, local, Vec::new());
            options.auth_storage = Some(Arc::clone(&auth));
            options.settings_manager = Some(Rc::clone(&settings));
            options.model_registry = Some(Arc::clone(&catalog));
            let services = create_agent_session_services(options).await.unwrap();
            assert!(Arc::ptr_eq(&services.auth_storage, &auth));
            assert!(Rc::ptr_eq(&services.settings_manager, &settings));
            assert!(Arc::ptr_eq(&services.model_registry, &catalog));
        });
    });
}

/// A catalog the services create themselves runs configured key commands.
#[test]
fn native_services_resolve_configured_command_keys() {
    with_isolated_home("native_services_resolve_configured_command_keys", |base| {
        let root = Root::new(base, "case");
        let document = json!({"providers": {"p": {
            "baseUrl": "https://example.test/v1",
            "api": "openai-completions",
            "apiKey": "!printf resolved",
            "models": [{"id": "m"}],
        }}});
        std::fs::write(root.fill("$ROOT/agent/models.json"), document.to_string()).unwrap();
        let resolved = on_task_set(|local| async {
            let services = create_agent_session_services(options(&root, local, Vec::new()))
                .await
                .unwrap();
            let model = services.model_registry.find("p", "m").unwrap();
            services
                .model_registry
                .get_api_key_and_headers(&model)
                .await
        });
        let ResolvedRequestAuth::Ok {
            api_key, headers, ..
        } = resolved
        else {
            panic!("configured command key must resolve")
        };
        assert!(api_key.as_deref().eq(&Some("resolved")));
        assert!(headers.is_none());
    });
}

#[test]
fn extension_catalog_defaults_are_available_during_factory_loading() {
    with_isolated_home(
        "extension_catalog_defaults_are_available_during_factory_loading",
        |base| {
            let root = Root::new(base, "constructor-defaults");
            std::fs::create_dir_all(root.agent()).unwrap();
            std::fs::write(
                format!("{}/auth.json", root.agent()),
                r#"{"default-provider":{"type":"api_key","key":"default-auth"}}"#,
            )
            .unwrap();
            std::fs::write(format!("{}/models.json", root.agent()), r#"{"providers":{"default-provider":{"baseUrl":"https://default.invalid","apiKey":"model-key","api":"openai-completions","models":[{"id":"default-model","name":"Default","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16}]}}}"#).unwrap();
            let observed = Rc::new(RefCell::new(None));
            let capture = observed.clone();
            let factory: ExtensionFactory<Local> = Rc::new(move |api| {
                let capture = capture.clone();
                Box::pin(async move {
                    let auth = api.create_auth_storage(None)?;
                    let catalog = api.create_model_registry(auth.clone(), None)?;
                    let model = catalog
                        .find("default-provider", "default-model")
                        .ok_or("default model missing")?;
                    assert_eq!(
                        catalog
                            .get_api_key_for_provider("default-provider")
                            .await?
                            .as_deref(),
                        Some("default-auth")
                    );
                    auth.set_runtime_api_key("default-provider", "later-key");
                    *capture.borrow_mut() = Some(
                        json!({"model": model.read().unwrap().id, "key":catalog.get_api_key_for_provider("default-provider").await?}),
                    );
                    Ok(())
                })
            });
            on_task_set(|local| async move {
                let services = create_agent_session_services(options(&root, local, vec![factory]))
                    .await
                    .unwrap();
                assert!(
                    services.resource_loader.get_extensions().errors.is_empty(),
                    "{:?}",
                    services.resource_loader.get_extensions().errors
                );
            });
            assert_eq!(
                *observed.borrow(),
                Some(json!({"model":"default-model", "key":"later-key"}))
            );
        },
    );
}
