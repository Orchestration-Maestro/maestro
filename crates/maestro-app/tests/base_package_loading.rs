//! Base factories execute early without changing extension precedence.
#![cfg(unix)]
use crate::resource_fixtures::{ExtensionSpec, factory, on_task_set};
use maestro_app::{
    DefaultResourceLoader, DefaultResourceLoaderOptions, ExtensionFactory, ModuleLoader,
    ResourceLoader, ResourceLoaderOptions,
};
use maestro_models::Local;
use maestro_test_directory::Scratch;
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

/// A factory that records execution before registering colliding capabilities.
fn recording_factory(name: String, calls: Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
    let spec: ExtensionSpec = serde_json::from_value(json!({
        "commands":[["same",name]], "tools":[["same",name]]
    }))
    .unwrap();
    let register = factory(spec);
    Rc::new(move |api| {
        calls.borrow_mut().push(name.clone());
        let registered = register(api.clone());
        let name = name.clone();
        Box::pin(async move {
            registered.await?;
            register_collisions(&api, &name)
        })
    })
}

/// Flags seed different defaults; shortcuts conflict on the same unreserved key.
fn register_collisions(
    api: &maestro_app::ExtensionAPI<Local>,
    name: &str,
) -> Result<(), maestro_extensions::EventError> {
    api.register_flag(
        "same".into(),
        maestro_extensions::types::ExtensionFlagOptions {
            description: Some(name.into()),
            r#type: if name == "base" {
                maestro_extensions::types::FlagType::Boolean
            } else {
                maestro_extensions::types::FlagType::String
            },
            default: Some(maestro_app::FlagValue::String(name.into())),
        },
    )?;
    api.register_shortcut(
        "ctrl+alt+9".into(),
        maestro_extensions::types::ExtensionShortcutOptions {
            description: Some(name.into()),
            handler: Rc::new(|_| Box::pin(async { Ok(()) })),
        },
    )
}

/// The controlled module loader labels each extension by its file stem.
fn recording_modules(calls: Rc<RefCell<Vec<String>>>) -> ModuleLoader<Local> {
    Rc::new(move |path: &str| {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let initialize = recording_factory(name, Rc::clone(&calls));
        Box::pin(async move { Ok(Some(initialize)) })
    })
}

/// Installed base sources run first but retain their lowest package position.
#[test]
fn base_factories_run_first_without_moving_registrations() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    for dir in [&agent, &base, &format!("{cwd}/.maestro/extensions")] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(
        format!("{base}/package.json"),
        json!({"maestro":{"extensions":["base.wasm"]}}).to_string(),
    )
    .unwrap();
    for path in [
        format!("{base}/base.wasm"),
        format!("{cwd}/.maestro/extensions/project.wasm"),
    ] {
        std::fs::write(path, "").unwrap();
    }
    on_task_set(|local| async move {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, recording_modules(Rc::clone(&calls)));
        resources.base_package_sources = vec![base];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(&*calls.borrow(), &["base", "project"]);
        let result = loader.get_extensions();
        let names: Vec<_> = result
            .extensions
            .iter()
            .map(|extension| {
                extension.read().unwrap().commands["same"]
                    .read()
                    .unwrap()
                    .description
                    .clone()
            })
            .collect();
        assert_eq!(names, [Some("project".into()), Some("base".into())]);
        assert_eq!(result.errors.len(), 2);
        assert!(
            result
                .errors
                .iter()
                .all(|error| error.path.ends_with("base.wasm"))
        );
    });
}

/// Writes a local package with one extension and a prompt template.
fn package(path: &str, name: &str) {
    std::fs::create_dir_all(format!("{path}/prompts")).unwrap();
    std::fs::write(format!("{path}/{name}.wasm"), "").unwrap();
    std::fs::write(
        format!("{path}/prompts/{name}.md"),
        format!("{name} prompt"),
    )
    .unwrap();
    std::fs::write(
        format!("{path}/package.json"),
        json!({
            "maestro":{"extensions":[format!("{name}.wasm")], "prompts":["prompts"]}
        })
        .to_string(),
    )
    .unwrap();
}

/// Only base factories offer load-time inputs; ordinary factories observe rejection.
fn contributing_modules(
    payload: serde_json::Value,
    calls: Rc<RefCell<Vec<String>>>,
) -> ModuleLoader<Local> {
    Rc::new(move |path: &str| {
        let base = path.ends_with("base.wasm");
        let payload = payload.clone();
        let calls = Rc::clone(&calls);
        let initialize: ExtensionFactory<Local> = Rc::new(move |api| {
            calls
                .borrow_mut()
                .push(if base { "base" } else { "extra" }.into());
            let result = if base {
                api.contribute_load_time(payload.clone())
            } else {
                assert!(api.contribute_load_time(json!({})).is_err());
                Ok(())
            };
            Box::pin(async move { result })
        });
        Box::pin(async move { Ok(Some(initialize)) })
    })
}

/// A base factory's sources and prompt inputs enter the current startup once.
#[test]
fn base_contributions_load_sources_and_prompt_inputs_in_the_same_startup() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let base = format!("{root}/base");
    let extra = format!("{root}/extra");
    let cwd = format!("{root}/project");
    let agent = format!("{root}/agent");
    for dir in [&cwd, &agent] {
        std::fs::create_dir_all(dir).unwrap();
    }
    package(&base, "base");
    package(&extra, "extra");
    let payload = json!({
        "packageSources":[{"source":extra,"scope":"user"},{"source":extra,"scope":"user"}],
        "baseSystemPrompt":"base system", "baseAppendText":["base append"]
    });
    on_task_set(|local| async move {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, contributing_modules(payload, Rc::clone(&calls)));
        resources.base_package_sources = vec![base];
        resources.append_system_prompt = Some(vec!["explicit append".into()]);
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(&*calls.borrow(), &["base", "extra"]);
        assert!(loader.get_extensions().errors.is_empty());
        assert_eq!(loader.get_system_prompt().as_deref(), Some("base system"));
        assert_eq!(
            &*loader.get_append_system_prompt(),
            &["base append", "explicit append"]
        );
        let prompts = loader.get_prompts();
        assert_eq!(prompts.prompts.len(), 2);
        assert_eq!(prompts.prompts[1].name, "extra");
        loader.reload().await.unwrap();
        assert_eq!(&*calls.borrow(), &["base", "extra", "base", "extra"]);
        assert_eq!(loader.get_prompts().prompts.len(), 2);
    });
}

/// A runner over the loader's actual ordered extension inventory.
fn runner(loader: &impl ResourceLoader<Local>) -> maestro_app::ExtensionRunner<Local> {
    let result = loader.get_extensions();
    let auth = std::sync::Arc::new(maestro_credentials::AuthStorage::in_memory(
        maestro_credentials::AuthStorageData::default(),
    ));
    maestro_app::ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(result.extensions.clone())),
        result.runtime.clone(),
        maestro_extensions::runner::ExtensionRunnerSession {
            cwd: "/work".into(),
            session_manager: Rc::new(RefCell::new(
                maestro_session::SessionManager::in_memory(None).unwrap(),
            )),
            model_registry: std::sync::Arc::new(
                maestro_catalog::ModelRegistry::in_memory(
                    auth,
                    crate::resource_fixtures::no_commands(),
                )
                .unwrap(),
            ),
        },
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// Conflict winners and runtime defaults come from list order, not factory execution order.
#[test]
fn base_registration_conflicts_follow_ordinary_extension_order() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    package(&base, "base");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
    std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
    on_task_set(|local| async move {
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, recording_modules(Rc::default()));
        resources.base_package_sources = vec![base];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        let runner = runner(&loader);
        assert_conflict_winners(&runner);
    });
}

/// Checks conflict resolution through the runner, independently of factory order.
fn assert_conflict_winners(runner: &maestro_app::ExtensionRunner<Local>) {
    assert_eq!(
        runner.get_flags()["same"]
            .read()
            .unwrap()
            .description
            .as_deref(),
        Some("project")
    );
    assert_eq!(
        runner.get_flag_values()["same"],
        maestro_app::FlagValue::String("project".into())
    );
    assert_eq!(
        runner
            .get_tool_definition::<serde_json::Value, serde_json::Value, ()>("same")
            .unwrap()
            .read()
            .unwrap()
            .definition
            .description,
        "project"
    );
    let shortcuts = runner.get_shortcuts(&Vec::new());
    assert_eq!(
        shortcuts["ctrl+alt+9"]
            .read()
            .unwrap()
            .description
            .as_deref(),
        Some("base")
    );
    let commands: Vec<_> = runner
        .get_registered_commands()
        .into_iter()
        .map(|command| (command.invocation_name, command.command.description))
        .collect();
    assert_eq!(
        commands,
        [
            ("same:1".into(), Some("project".into())),
            ("same:2".into(), Some("base".into()))
        ]
    );
}

/// Both base factories try to publish an independently labelled settings layer.
fn publishing_modules() -> ModuleLoader<Local> {
    Rc::new(|path: &str| {
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let initialize: ExtensionFactory<Local> = Rc::new(move |api| {
            let layer = maestro_settings::BaseSettingsLayer {
                settings: maestro_settings::Settings(
                    serde_json::from_value(json!({"theme":name})).unwrap(),
                ),
                locked: ["theme".into()].into_iter().collect(),
                source: name.clone(),
                recorded_at: "2026-10-27T00:00:00Z".into(),
            };
            let result =
                api.contribute_load_time(json!({"baseLayers":[{"scope":"user","layer":layer}]}));
            Box::pin(async move { result })
        });
        Box::pin(async move { Ok(Some(initialize)) })
    })
}

/// A second publisher is diagnosed without replacing the first accepted layer.
#[test]
fn second_base_layer_publisher_is_reported_without_application() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let one = format!("{root}/one");
    let two = format!("{root}/two");
    for dir in [&agent, &cwd] {
        std::fs::create_dir_all(dir).unwrap();
    }
    package(&one, "one");
    package(&two, "two");
    on_task_set(|local| async move {
        let settings = Rc::new(RefCell::new(maestro_settings::SettingsManager::in_memory(
            maestro_settings::Settings::default(),
        )));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, publishing_modules());
        resources.base_package_sources = vec![one, two];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: Some(Rc::clone(&settings)),
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(settings.borrow().get_base_layer().unwrap().source, "one");
        assert_eq!(settings.borrow().get_theme().as_deref(), Some("one"));
        assert_eq!(
            settings.borrow().get_global_settings().0,
            serde_json::Map::new()
        );
        let result = loader.get_extensions();
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].path.ends_with("two.wasm"));
        assert!(result.errors[0].error.contains("one.wasm"));
        assert!(result.errors[0].error.contains("publisher"));
        loader.reload().await.unwrap();
        assert_eq!(loader.get_extensions().errors.len(), 1);
        assert_eq!(settings.borrow().get_base_layer().unwrap().source, "one");
    });
}

/// Session services read contributed model files below the user's file in the same startup.
#[test]
fn session_services_apply_contributed_models_before_user_models() {
    if maestro_test_process::run_alone(
        "session_services_apply_contributed_models_before_user_models",
    )
    .unwrap()
    {
        return;
    }
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    let models = format!("{base}/models.json");
    for dir in [&agent, &cwd] {
        std::fs::create_dir_all(dir).unwrap();
    }
    package(&base, "base");
    std::fs::write(&models, json!({"providers":{"openai":{"modelOverrides":{"gpt-4o":{"contextWindow":12345,"cost":{"input":1,"output":2}}}}}}).to_string()).unwrap();
    std::fs::write(
        format!("{agent}/models.json"),
        json!({"providers":{"openai":{"modelOverrides":{"gpt-4o":{"cost":{"input":3}}}}}})
            .to_string(),
    )
    .unwrap();
    on_task_set(|local| async move {
        let mut resources: ResourceLoaderOptions<Local> = ResourceLoaderOptions::new(
            local,
            contributing_modules(json!({"modelFiles":[models]}), Rc::default()),
        );
        resources.base_package_sources = vec![base];
        let services = maestro_app::create_agent_session_services(
            maestro_app::CreateAgentSessionServicesOptions {
                cwd,
                agent_dir: Some(agent),
                auth_storage: Some(std::sync::Arc::new(
                    maestro_credentials::AuthStorage::in_memory(
                        maestro_credentials::AuthStorageData::default(),
                    ),
                )),
                settings_manager: None,
                model_registry: None,
                extension_flag_values: None,
                resource_loader_options: resources,
                offline: Some(true),
            },
        )
        .await
        .unwrap();
        assert!(services.resource_loader.get_extensions().errors.is_empty());
        let model = services.model_registry.find("openai", "gpt-4o").unwrap();
        let model = model.read().unwrap();
        assert_eq!(model.context_window.to_bits(), 12345.0_f64.to_bits());
        assert_eq!(model.cost.input.to_bits(), 3.0_f64.to_bits());
        assert_eq!(model.cost.output.to_bits(), 2.0_f64.to_bits());
    });
}

/// Registers distinct first defaults and replacement metadata in each factory.
fn reading_flag_modules(observations: Rc<RefCell<Vec<serde_json::Value>>>) -> ModuleLoader<Local> {
    Rc::new(move |path: &str| {
        let base = path.ends_with("base.wasm");
        let observations = Rc::clone(&observations);
        let initialize: ExtensionFactory<Local> = Rc::new(move |api| {
            let first = if base { "base first" } else { "project first" };
            for default in [first, "replacement"] {
                api.register_flag(
                    "shared".into(),
                    maestro_extensions::types::ExtensionFlagOptions {
                        description: None,
                        r#type: maestro_extensions::types::FlagType::String,
                        default: Some(maestro_app::FlagValue::String(default.into())),
                    },
                )
                .unwrap();
            }
            api.register_flag(
                "optional".into(),
                maestro_extensions::types::ExtensionFlagOptions {
                    description: None,
                    r#type: maestro_extensions::types::FlagType::String,
                    default: base.then(|| maestro_app::FlagValue::String("base fallback".into())),
                },
            )
            .unwrap();
            if !base {
                observations.borrow_mut().push(json!([
                    api.get_flag("shared").unwrap(),
                    api.get_flag("optional").unwrap()
                ]));
            }
            Box::pin(async { Ok(()) })
        });
        Box::pin(async move { Ok(Some(initialize)) })
    })
}

/// Ordinary factories see their own first defaults before later base defaults are restored.
#[test]
fn ordinary_factories_read_their_first_registered_flag_defaults() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    package(&base, "base");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
    std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
    on_task_set(|local| async move {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, reading_flag_modules(Rc::clone(&observed)));
        resources.base_package_sources = vec![base];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(&*observed.borrow(), &[json!(["project first", null])]);
        let values = loader.get_extensions();
        let flags = values.runtime.flag_values.read().unwrap();
        assert_eq!(
            flags["shared"],
            maestro_app::FlagValue::String("project first".into())
        );
        assert_eq!(
            flags["optional"],
            maestro_app::FlagValue::String("base fallback".into())
        );
    });
}

/// Multiple entrypoints of one base package share its publication identity.
#[test]
fn one_base_package_can_publish_from_multiple_entrypoints() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/one");
    for dir in [&agent, &cwd] {
        std::fs::create_dir_all(dir).unwrap();
    }
    package(&base, "one");
    std::fs::write(format!("{base}/one-extra.wasm"), "").unwrap();
    std::fs::write(
        format!("{base}/package.json"),
        json!({"maestro":{"extensions":["one.wasm","one-extra.wasm"]}}).to_string(),
    )
    .unwrap();
    on_task_set(|local| async move {
        let settings = Rc::new(RefCell::new(maestro_settings::SettingsManager::in_memory(
            maestro_settings::Settings::default(),
        )));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, publishing_modules());
        resources.base_package_sources = vec![base];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: Some(Rc::clone(&settings)),
            resources,
        });
        loader.reload().await.unwrap();
        assert!(loader.get_extensions().errors.is_empty());
        assert_eq!(
            settings.borrow().get_base_layer().unwrap().source,
            "one-extra"
        );
    });
}

/// Offers inputs before returning the controlled factory failure.
fn failing_contributor(payload: serde_json::Value) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let result = api
            .contribute_load_time(payload.clone())
            .and(Err("factory failed".into()));
        Box::pin(async move { result })
    })
}

/// A failed base factory cannot install sources or persist a layer it offered earlier.
#[test]
fn failed_base_factories_do_not_apply_their_load_time_inputs() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    let extra = format!("{root}/extra");
    for dir in [&agent, &cwd] {
        std::fs::create_dir_all(dir).unwrap();
    }
    package(&base, "base");
    package(&extra, "extra");
    let payload = json!({"packageSources":[{"source":extra,"scope":"user"}], "baseSystemPrompt":"failed",
        "baseLayers":[{"scope":"user","layer":{"settings":{"theme":"failed"},"locked":[],"source":"failed","recorded_at":"now"}}]});
    on_task_set(|local| async move {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&calls);
        let modules: ModuleLoader<Local> = Rc::new(move |path: &str| {
            let initialize = if path.ends_with("base.wasm") {
                failing_contributor(payload.clone())
            } else {
                recording_factory("extra".into(), Rc::clone(&seen))
            };
            Box::pin(async move { Ok(Some(initialize)) })
        });
        let settings = Rc::new(RefCell::new(maestro_settings::SettingsManager::in_memory(
            maestro_settings::Settings::default(),
        )));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, modules);
        resources.base_package_sources = vec![base];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: Some(Rc::clone(&settings)),
            resources,
        });
        loader.reload().await.unwrap();
        assert!(settings.borrow().get_base_layer().is_none());
        assert!(loader.get_system_prompt().is_none());
        assert!(calls.borrow().is_empty());
        let result = loader.get_extensions();
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].error.contains("factory failed"));
    });
}

/// Explicit flag values use the first extension's declared kind, not a later base kind.
#[test]
fn extension_flag_values_use_the_first_registered_kind() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    package(&base, "base");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
    std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
    on_task_set(|local| async move {
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, recording_modules(Rc::default()));
        resources.base_package_sources = vec![base];
        let services = maestro_app::create_agent_session_services(
            maestro_app::CreateAgentSessionServicesOptions {
                cwd,
                agent_dir: Some(agent),
                auth_storage: Some(std::sync::Arc::new(
                    maestro_credentials::AuthStorage::in_memory(
                        maestro_credentials::AuthStorageData::default(),
                    ),
                )),
                settings_manager: None,
                model_registry: None,
                extension_flag_values: Some(indexmap::IndexMap::from([(
                    "same".into(),
                    maestro_app::FlagValue::String("chosen".into()),
                )])),
                resource_loader_options: resources,
                offline: Some(true),
            },
        )
        .await
        .unwrap();
        assert!(services.diagnostics.is_empty());
        assert_eq!(
            services
                .resource_loader
                .get_extensions()
                .runtime
                .flag_values
                .read()
                .unwrap()["same"],
            maestro_app::FlagValue::String("chosen".into())
        );
    });
}

/// Explicit base sources bootstrap while ordinary discovery is disabled.
#[test]
fn disabled_extension_discovery_keeps_base_sources_at_package_precedence() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    let cli = format!("{root}/cli.wasm");
    package(&base, "base");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
    std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
    std::fs::write(&cli, "").unwrap();
    on_task_set(|local| async move {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, recording_modules(Rc::clone(&calls)));
        resources.base_package_sources = vec![base];
        resources.additional_extension_paths = vec![cli];
        resources
            .disabled
            .insert(maestro_app::ResourceKind::Extensions);
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(&*calls.borrow(), &["base", "cli"]);
        let result = loader.get_extensions();
        let names: Vec<_> = result
            .extensions
            .iter()
            .map(|extension| {
                extension.read().unwrap().commands["same"]
                    .read()
                    .unwrap()
                    .description
                    .clone()
            })
            .collect();
        assert_eq!(names, [Some("cli".into()), Some("base".into())]);
    });
}

/// Counts executions while using the same flag-reading factory at every path.
fn counted_flag_modules(
    observations: Rc<RefCell<Vec<serde_json::Value>>>,
    calls: Rc<RefCell<Vec<String>>>,
) -> ModuleLoader<Local> {
    let modules = reading_flag_modules(observations);
    Rc::new(move |path: &str| {
        let loaded = modules(path);
        let name = std::path::Path::new(path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let calls = Rc::clone(&calls);
        Box::pin(async move {
            let Some(factory) = loaded.await? else {
                return Ok(None);
            };
            let initialize: ExtensionFactory<Local> = Rc::new(move |api| {
                calls.borrow_mut().push(name.clone());
                factory(api)
            });
            Ok(Some(initialize))
        })
    })
}

/// Reads the visible defaults after path extensions and before the final snapshot.
fn inline_flag_reader(
    observations: Rc<RefCell<Vec<serde_json::Value>>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered = ["shared", "optional"].into_iter().try_for_each(|name| {
            api.register_flag(
                name.into(),
                maestro_extensions::types::ExtensionFlagOptions {
                    description: None,
                    r#type: maestro_extensions::types::FlagType::String,
                    default: None,
                },
            )
        });
        let result = registered.and_then(|()| {
            observations
                .borrow_mut()
                .push(json!([api.get_flag("shared")?, api.get_flag("optional")?]));
            Ok(())
        });
        Box::pin(async move { result })
    })
}

/// One physical base extension reuses its explicit ordinary slot and visible defaults.
#[test]
fn aliased_base_entrypoints_load_once_at_their_ordinary_position() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let base = format!("{root}/base");
    let alias = format!("{root}/alias");
    package(&base, "base");
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
    std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
    std::os::unix::fs::symlink(&base, &alias).unwrap();
    on_task_set(|local| async move {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::new(RefCell::new(Vec::new()));
        let mut resources: ResourceLoaderOptions<Local> = ResourceLoaderOptions::new(
            local,
            counted_flag_modules(Rc::clone(&observed), Rc::clone(&calls)),
        );
        resources.base_package_sources = vec![base];
        resources.additional_extension_paths = vec![format!("{alias}/base.wasm")];
        resources.extension_factories = vec![inline_flag_reader(Rc::clone(&observed))];
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        loader.reload().await.unwrap();
        assert_eq!(&*calls.borrow(), &["base", "project"]);
        assert_eq!(
            &*observed.borrow(),
            &[
                json!(["base first", "base fallback"]),
                json!(["base first", "base fallback"])
            ]
        );
        assert_eq!(loader.get_extensions().extensions.len(), 3);
        assert_eq!(
            runner(&loader).get_flags()["shared"]
                .read()
                .unwrap()
                .default,
            Some(maestro_app::FlagValue::String("replacement".into()))
        );
        assert_eq!(
            loader.get_extensions().runtime.flag_values.read().unwrap()["shared"],
            maestro_app::FlagValue::String("base first".into())
        );
    });
}

/// Missing layer keys cannot remove accepted settings; explicit null still can.
#[test]
fn base_layer_removal_requires_an_explicit_null() {
    for explicit_null in [false, true] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path().to_string_lossy();
        let agent = format!("{root}/agent");
        let cwd = format!("{root}/project");
        let base = format!("{root}/base");
        for dir in [&agent, &cwd] {
            std::fs::create_dir_all(dir).unwrap();
        }
        package(&base, "base");
        on_task_set(|local| async move {
            let settings = Rc::new(RefCell::new(maestro_settings::SettingsManager::in_memory(
                maestro_settings::Settings::default(),
            )));
            let layer = maestro_settings::BaseSettingsLayer {
                settings: maestro_settings::Settings(
                    serde_json::from_value(json!({"theme":"locked"})).unwrap(),
                ),
                locked: ["theme".into()].into_iter().collect(),
                source: "accepted".into(),
                recorded_at: "now".into(),
            };
            settings
                .borrow_mut()
                .publish_base_layer(maestro_settings::SettingsScope::Global, Some(layer.clone()));
            let input = if explicit_null {
                json!({"scope":"user","layer":null})
            } else {
                json!({"scope":"user"})
            };
            let mut resources: ResourceLoaderOptions<Local> = ResourceLoaderOptions::new(
                local,
                contributing_modules(json!({"baseLayers":[input]}), Rc::default()),
            );
            resources.base_package_sources = vec![base];
            let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
                cwd,
                agent_dir: agent,
                settings_manager: Some(Rc::clone(&settings)),
                resources,
            });
            loader.reload().await.unwrap();
            let result = loader.get_extensions();
            if explicit_null {
                assert!(result.errors.is_empty());
                assert!(settings.borrow().get_base_layer().is_none());
                assert!(!settings.borrow().is_key_path_locked("theme", &[]));
            } else {
                assert_eq!(result.errors.len(), 1);
                assert!(result.errors[0].path.ends_with("base.wasm"));
                assert!(result.errors[0].error.contains("missing field `layer`"));
                assert_eq!(settings.borrow().get_base_layer(), Some(&layer));
                assert!(settings.borrow().is_key_path_locked("theme", &[]));
                assert_eq!(settings.borrow().get_theme().as_deref(), Some("locked"));
            }
        });
    }
}

/// Records registration and removal collisions without publishing load-time inputs.
fn provider_modules() -> ModuleLoader<Local> {
    Rc::new(|path: &str| {
        let base = path.ends_with("base.wasm");
        let initialize: ExtensionFactory<Local> = Rc::new(move |api| {
            let endpoint = if base { "base" } else { "project" };
            for name in [
                "shared",
                if base {
                    "removed-by-project"
                } else {
                    "removed-by-base"
                },
            ] {
                api.register_provider(
                    name.into(),
                    std::sync::Arc::new(std::sync::RwLock::new(
                        maestro_extensions::types::ProviderConfig {
                            base_url: Some(endpoint.into()),
                            ..Default::default()
                        },
                    )),
                )
                .unwrap();
            }
            api.unregister_provider(if base {
                "removed-by-base"
            } else {
                "removed-by-project"
            })
            .unwrap();
            Box::pin(async { Ok(()) })
        });
        Box::pin(async move { Ok(Some(initialize)) })
    })
}

/// Queued registrations and removals follow ordinary positions, including canonical reuse.
#[test]
fn base_provider_operations_follow_ordinary_extension_order() {
    for explicit_alias in [false, true] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path().to_string_lossy();
        let agent = format!("{root}/agent");
        let cwd = format!("{root}/project");
        let base = format!("{root}/base");
        let alias = format!("{root}/alias");
        package(&base, "base");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::create_dir_all(format!("{cwd}/.maestro/extensions")).unwrap();
        std::fs::write(format!("{cwd}/.maestro/extensions/project.wasm"), "").unwrap();
        std::os::unix::fs::symlink(&base, &alias).unwrap();
        on_task_set(|local| async move {
            let mut resources: ResourceLoaderOptions<Local> =
                ResourceLoaderOptions::new(local, provider_modules());
            resources.base_package_sources = vec![base];
            if explicit_alias {
                resources.additional_extension_paths = vec![format!("{alias}/base.wasm")];
            }
            let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
                cwd,
                agent_dir: agent,
                settings_manager: None,
                resources,
            });
            loader.reload().await.unwrap();
            let result = loader.get_extensions();
            assert!(result.errors.is_empty());
            assert_provider_operations(&result, explicit_alias);
        });
    }
}

/// Checks the exact queue consumed by session services and runner binding.
fn assert_provider_operations(
    result: &maestro_extensions::LoadExtensionsResult<Local>,
    explicit_alias: bool,
) {
    let pending = result
        .runtime
        .pending_provider_registrations
        .read()
        .unwrap();
    let operations: Vec<_> = pending
        .values()
        .map(|entry| {
            (
                entry.name.clone(),
                entry.config.read().unwrap().base_url.clone().unwrap(),
            )
        })
        .collect();
    let expected = if explicit_alias {
        vec![
            ("shared", "base"),
            ("shared", "project"),
            ("removed-by-base", "project"),
        ]
    } else {
        vec![
            ("shared", "project"),
            ("shared", "base"),
            ("removed-by-project", "base"),
        ]
    };
    assert_eq!(
        operations,
        expected
            .into_iter()
            .map(|(name, url)| (name.into(), url.into()))
            .collect::<Vec<_>>()
    );
}

/// Failed factories retain provider registrations and removals in base-source order.
#[test]
fn disabled_discovery_replays_failed_base_provider_operations() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy();
    let agent = format!("{root}/agent");
    let cwd = format!("{root}/project");
    let sources: Vec<_> = ["first", "failed", "last"]
        .map(|name| format!("{root}/{name}"))
        .into();
    for (source, name) in sources.iter().zip(["project", "base", "project"]) {
        package(source, name);
    }
    std::fs::create_dir_all(&agent).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    on_task_set(|local| async move {
        let mut resources: ResourceLoaderOptions<Local> =
            ResourceLoaderOptions::new(local, failing_provider_modules());
        resources.base_package_sources = sources;
        resources
            .disabled
            .insert(maestro_app::ResourceKind::Extensions);
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd,
            agent_dir: agent,
            settings_manager: None,
            resources,
        });
        for _ in 0..2 {
            loader.reload().await.unwrap();
            let result = loader.get_extensions();
            assert_eq!(result.errors.len(), 1);
            assert!(result.errors[0].error.contains("base factory failed"));
            assert_eq!(result.extensions.len(), 2);
            let pending = result
                .runtime
                .pending_provider_registrations
                .read()
                .unwrap();
            let operations: Vec<_> = pending
                .values()
                .map(|entry| {
                    (
                        entry.name.clone(),
                        entry.config.read().unwrap().base_url.clone().unwrap(),
                    )
                })
                .collect();
            assert_eq!(
                operations,
                [
                    ("shared".into(), "project".into()),
                    ("shared".into(), "base".into()),
                    ("shared".into(), "project".into()),
                    ("removed-by-base".into(), "project".into()),
                ]
            );
        }
    });
}

/// Registers the controlled provider operations before failing the middle base factory.
fn failing_provider_modules() -> ModuleLoader<Local> {
    let modules = provider_modules();
    Rc::new(move |path| {
        let loaded = modules(path);
        let failed = path.ends_with("base.wasm");
        Box::pin(async move {
            Ok(Some(fail_after_provider_operations(
                loaded.await?.unwrap(),
                failed,
            )))
        })
    })
}

/// Wraps a factory with a failure after its provider operations have completed.
fn fail_after_provider_operations(
    initialize: ExtensionFactory<Local>,
    failed: bool,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let initialized = initialize(api);
        Box::pin(async move {
            initialized.await?;
            if failed {
                return Err("base factory failed".into());
            }
            Ok(())
        })
    })
}
