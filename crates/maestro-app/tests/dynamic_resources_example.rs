//! Shipped resources through session discovery and reload, using the component's shared result.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{Harness, HarnessOptions, harness, on_task_set};
use maestro_app::{
    AgentSession, AgentSessionServices, CreateAgentSessionFromServicesOptions,
    DefaultResourceLoader, DefaultResourceLoaderOptions, ExtensionBindings, NoTools,
    ResourceLoader, ResourceLoaderOptions, create_agent_session_from_services,
};
use maestro_extensions::{
    ExtensionFactory, Local, ResourcesDiscoverEvent, ResourcesDiscoverResult,
};
use maestro_resources::SourceInfo;
use std::{cell::RefCell, fs, path::Path, rc::Rc, sync::Arc};

/// The same discovery result checked against the built component, at this installation.
fn factory(directory: &Path) -> ExtensionFactory<Local> {
    let result: ResourcesDiscoverResult = serde_json::from_str(
        &include_str!("../../maestro-extensions-wasmtime/tests/dynamic_resources_expected.json")
            .replace(
                "$DIRECTORY",
                &directory.to_string_lossy().replace('\\', "/"),
            ),
    )
    .unwrap();
    Rc::new(move |api| {
        let result = result.clone();
        Box::pin(async move {
            api.on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
                "resources_discover".into(),
                Rc::new(move |_, _| Box::pin(std::future::ready(Ok(Some(result.clone()))))),
            )
        })
    })
}

/// Copies the real shipped assets into a disposable installation.
fn install(directory: &Path) -> String {
    fs::create_dir_all(directory).unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/extensions/dynamic-resources");
    for leaf in ["SKILL.md", "dynamic.md", "dynamic.json"] {
        fs::copy(source.join(leaf), directory.join(leaf)).unwrap();
    }
    let artifact = directory.join("resources.wasm");
    fs::write(&artifact, []).unwrap();
    artifact.to_str().unwrap().into()
}

/// A session over the real resource loader and the controlled model catalog.
async fn session(h: &Harness, loader: Rc<DefaultResourceLoader<Local>>) -> Rc<AgentSession<Local>> {
    let services = Rc::new(AgentSessionServices {
        cwd: h.cwd.clone(),
        agent_dir: h.scratch.path().join("agent").to_str().unwrap().into(),
        auth_storage: Arc::new(maestro_app::AuthStorage::in_memory(
            maestro_credentials::AuthStorageData::default(),
        )),
        settings_manager: Rc::new(RefCell::new(maestro_app::SettingsManager::in_memory(
            maestro_settings::Settings::default(),
        ))),
        model_registry: h.registry.clone(),
        resource_loader: loader,
        diagnostics: Vec::new(),
        offline: Some(true),
    });
    create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
        services,
        session_manager: Rc::new(RefCell::new(
            maestro_app::SessionManager::in_memory(Some(&h.cwd)).unwrap(),
        )),
        session_start_event: None,
        model: Some(h.model("plain")),
        thinking_level: None,
        scoped_models: Vec::new(),
        tools: None,
        no_tools: Some(NoTools::Builtin),
        custom_tools: Vec::new(),
        theme: maestro_theme::LiveTheme::default(),
    })
    .await
    .unwrap()
    .session
}

/// Every resource retains its installed path and extension provenance.
fn source(info: &SourceInfo, directory: &Path, leaf: &str) {
    assert_eq!(Path::new(&info.path), directory.join(leaf));
    assert_eq!(info.source, "extension:resources.wasm");
    assert_eq!(Path::new(info.base_dir.as_ref().unwrap()), directory);
}

/// All three parsed resources expose their contents, not merely existing files.
fn resources(loader: &DefaultResourceLoader<Local>, directory: &Path, reloaded: bool) {
    let skills = loader.get_skills();
    assert!(skills.diagnostics.is_empty(), "{:?}", skills.diagnostics);
    assert_eq!(skills.skills.len(), 1);
    let skill = &skills.skills[0];
    assert_eq!(skill.name, "dynamic-resources");
    assert_eq!(
        skill.description,
        if reloaded {
            "Reloaded skill"
        } else {
            "Example skill loaded from resources_discover"
        }
    );
    source(&skill.source_info, directory, "SKILL.md");
    assert!(
        fs::read_to_string(&skill.file_path)
            .unwrap()
            .contains("# Dynamic Resources Skill")
    );
    let prompts = loader.get_prompts();
    assert!(prompts.diagnostics.is_empty(), "{:?}", prompts.diagnostics);
    assert_eq!(prompts.prompts.len(), 1);
    let prompt = &prompts.prompts[0];
    assert_eq!(prompt.name, "dynamic");
    assert_eq!(
        prompt.description,
        "Example prompt template loaded from resources_discover"
    );
    assert_eq!(
        prompt.content,
        if reloaded {
            "Reloaded prompt."
        } else {
            "Summarize the current repository structure and mention any build or test commands."
        }
    );
    source(&prompt.source_info, directory, "dynamic.md");
    theme(loader, directory, reloaded);
}

/// Theme metadata is refreshed with the skill and prompt.
fn theme(loader: &DefaultResourceLoader<Local>, directory: &Path, reloaded: bool) {
    let themes = loader.get_themes();
    assert!(themes.diagnostics.is_empty(), "{:?}", themes.diagnostics);
    assert_eq!(themes.themes.len(), 1);
    let theme = &themes.themes[0];
    assert_eq!(
        theme.name(),
        Some(if reloaded {
            "dynamic-reloaded"
        } else {
            "dynamic-resources"
        })
    );
    assert_eq!(
        Path::new(theme.source_path().unwrap()),
        directory.join("dynamic.json")
    );
    source(
        &theme.source_info().unwrap().borrow(),
        directory,
        "dynamic.json",
    );
}

/// Binding and reload load all sibling resources from two installations, never from cwd.
#[test]
fn dynamic_resources_discovery_and_reload_load_shipped_files() {
    on_task_set(|local| async move {
        let h = harness(HarnessOptions::default()).await;
        fs::create_dir_all(&h.cwd).unwrap();
        for name in ["original", "relocated # Ω"] {
            let directory = h.scratch.path().join(name);
            let artifact = install(&directory);
            let factory = factory(&directory);
            let mut options = ResourceLoaderOptions::<Local>::new(
                local.clone(),
                Rc::new(move |_| Box::pin(std::future::ready(Ok(Some(factory.clone()))))),
            );
            options.additional_extension_paths = vec![artifact];
            options.offline = Some(true);
            let loader = Rc::new(DefaultResourceLoader::new(DefaultResourceLoaderOptions {
                cwd: h.cwd.clone(),
                agent_dir: h.scratch.path().join("agent").to_str().unwrap().into(),
                settings_manager: None,
                resources: options,
            }));
            loader.reload().await.unwrap();
            let session = session(&h, loader.clone()).await;
            session
                .bind_extensions(ExtensionBindings {
                    on_error: Some(Rc::new(|error| panic!("{error:?}"))),
                    ..ExtensionBindings::default()
                })
                .await
                .unwrap();
            resources(&loader, &directory, false);
            for (leaf, from, to) in [
                (
                    "SKILL.md",
                    "Example skill loaded from resources_discover",
                    "Reloaded skill",
                ),
                (
                    "dynamic.md",
                    "Summarize the current repository structure and mention any build or test commands.",
                    "Reloaded prompt.",
                ),
                (
                    "dynamic.json",
                    "\"name\": \"dynamic-resources\"",
                    "\"name\": \"dynamic-reloaded\"",
                ),
            ] {
                let path = directory.join(leaf);
                fs::write(&path, fs::read_to_string(&path).unwrap().replace(from, to)).unwrap();
            }
            session.reload().await.unwrap();
            resources(&loader, &directory, true);
            session.dispose();
        }
    });
}
