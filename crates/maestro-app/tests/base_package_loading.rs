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
            r#type: maestro_extensions::types::FlagType::String,
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
