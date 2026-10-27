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
        "commands":[["same",name]], "flags":[["same","boolean"]], "tools":[["same","tool"]]
    }))
    .unwrap();
    let register = factory(spec);
    Rc::new(move |api| {
        calls.borrow_mut().push(name.clone());
        register(api)
    })
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
