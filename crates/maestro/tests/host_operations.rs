//! Compiled factory calls reach the original owners before session binding.
use maestro_app::{
    DefaultResourceLoader, DefaultResourceLoaderOptions, Local, ResourceLoader,
    ResourceLoaderOptions, SettingsManager, create_event_bus_for,
};
use maestro_test_process::{TestProcess, selection};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn compiled_factory_reads_raw_settings_and_reloads() {
    if !selection().is_child() {
        let output = TestProcess::current("compiled_factory_reads_raw_settings_and_reloads")
            .unwrap()
            .case("raw")
            .env("MAESTRO_HOST_OPERATIONS_REQUEST", "raw")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = crate::support::Root::new();
    let global = json!({"unknown": {"kept": null}, "defaultModel": 17});
    let project = json!({"unknown": false});
    write_settings(&root, &global, &project);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let observed = Rc::new(RefCell::new(Vec::<Value>::new()));
        let (loader, settings) = recorded_loader(&root, local.clone(), observed.clone());
        for _ in 0..2 {
            loader.reload().await.unwrap();
            let extensions = loader.get_extensions();
            assert!(extensions.errors.is_empty(), "{:?}", extensions.errors);
            assert_eq!(observed.take(), vec![json!([global, project])]);
        }
        assert_eq!(
            serde_json::to_value(settings.borrow().get_global_settings()).unwrap(),
            global
        );
    });
}

#[test]
fn compiled_factory_resolves_acquires_and_restores_packages() {
    if !selection().is_child() {
        let output =
            TestProcess::current("compiled_factory_resolves_acquires_and_restores_packages")
                .unwrap()
                .case("packages")
                .env("MAESTRO_HOST_OPERATIONS_REQUEST", "packages")
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = crate::support::Root::new();
    let user_settings = json!({"packages": ["./empty"]});
    write_settings(&root, &user_settings, &json!({}));
    let empty = root.path("agent/empty");
    std::fs::create_dir(&empty).unwrap();
    std::fs::write(empty.join("package.json"), r#"{"maestro":{}}"#).unwrap();
    let user = root.path("agent/git/example.test/team/repo");
    let project = root.path(&format!(
        "project/{}/git/example.test/team/repo",
        maestro_app::configuration::config::CONFIG_DIR_NAME
    ));
    let user_pin = git_fixture::pinned_repository(&user);
    let project_pin = git_fixture::pinned_repository(&project);
    let head = git_fixture::git(&user, &["rev-parse", "HEAD"]);
    std::fs::write(user.join("content"), "dirty").unwrap();
    std::fs::write(user.join("untracked"), "untracked").unwrap();
    let source = "https://example.test/team/repo@pin";
    let observed = observe_once(&root);
    assert_eq!(
        observed,
        vec![json!([
            {"ok": [{"source": "./empty", "identity": format!("local:{}", empty.display()), "scope": "user", "installedPath": empty}]},
            {"ok": null},
            {"ok": [{"source": source, "identity": "git:example.test/team/repo", "scope": "project", "installedPath": project}]},
            {"ok": {"expectedCommit": user_pin, "headCommit": head, "changedPaths": ["content", "untracked"], "clean": false}},
            {"ok": null},
            {"ok": {"expectedCommit": user_pin, "headCommit": user_pin, "changedPaths": [], "clean": true}},
            {"err": "Integrity requires a pinned Git source"},
        ])]
    );
    assert_eq!(
        git_fixture::git(&project, &["rev-parse", "HEAD"]),
        project_pin
    );
    assert_eq!(
        std::fs::read_to_string(user.join("content")).unwrap(),
        "pinned"
    );
    assert!(!user.join("untracked").exists());
    let written = std::fs::read_to_string(root.path("agent/settings.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&written).unwrap(),
        user_settings
    );
}

#[path = "../../maestro-app/tests/host_operations_support/mod.rs"]
mod git_fixture;

/// Loads the compiled factory once, offline, and returns what it emitted.
fn observe_once(root: &crate::support::Root) -> Vec<Value> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let observed = Rc::new(RefCell::new(Vec::<Value>::new()));
        let (loader, _) = recorded_loader(root, local.clone(), observed.clone());
        loader.reload().await.unwrap();
        let extensions = loader.get_extensions();
        assert!(extensions.errors.is_empty(), "{:?}", extensions.errors);
        observed.take()
    })
}

/// Seeds both actual settings files without merging their documents.
fn write_settings(root: &crate::support::Root, global: &Value, project: &Value) {
    std::fs::write(root.path("agent/settings.json"), global.to_string()).unwrap();
    let path = root.path(&format!(
        "project/{}/settings.json",
        maestro_app::configuration::config::CONFIG_DIR_NAME
    ));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, project.to_string()).unwrap();
}

/// An independent resource loader with the real compiled factory and captured observations.
fn recorded_loader(
    root: &crate::support::Root,
    local: Rc<tokio::task::LocalSet>,
    observed: Rc<RefCell<Vec<Value>>>,
) -> (DefaultResourceLoader<Local>, Rc<RefCell<SettingsManager>>) {
    let (options, settings) = recorded_options(root, local, observed);
    (DefaultResourceLoader::new(options), settings)
}

/// The SDK and direct loader share the same real component inputs.
fn recorded_options(
    root: &crate::support::Root,
    local: Rc<tokio::task::LocalSet>,
    observed: Rc<RefCell<Vec<Value>>>,
) -> (
    DefaultResourceLoaderOptions<Local>,
    Rc<RefCell<SettingsManager>>,
) {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|text| panic!("{text}")),
    );
    bus.on(
        "host-operations",
        Rc::new(move |data, _| {
            observed.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
    let cwd = root.path("project").to_str().unwrap().to_owned();
    let agent_dir = root.path("agent").to_str().unwrap().to_owned();
    let settings = Rc::new(RefCell::new(SettingsManager::create(
        &cwd,
        &agent_dir,
        maestro_app::configuration::config::CONFIG_DIR_NAME,
    )));
    let mut resources =
        ResourceLoaderOptions::<Local>::new(local, Rc::new(|_| Box::pin(async { Ok(None) })));
    resources.event_bus = Some(bus);
    resources.offline = Some(true);
    resources
        .extension_factories
        .push(maestro_extensions_wasmtime::artifact_factory(
            std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
            crate::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "host-operation-test".into(),
                agent_dir: agent_dir.clone(),
            },
        ));
    let options = DefaultResourceLoaderOptions {
        cwd,
        agent_dir,
        settings_manager: Some(settings.clone()),
        resources,
    };
    (options, settings)
}

#[test]
fn sdk_component_reads_original_settings_on_creation_and_reload() {
    if !selection().is_child() {
        let output =
            TestProcess::current("sdk_component_reads_original_settings_on_creation_and_reload")
                .unwrap()
                .case("sdk")
                .env("MAESTRO_HOST_OPERATIONS_REQUEST", "sdk")
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = crate::support::Root::new();
    let global = json!({"unknown": {"kept": null}, "defaultModel": 17, "packages": ["./empty"]});
    let packages = sdk_package(&root);
    let project = json!({});
    write_settings(
        &root,
        &json!({"diskOnly": true}),
        &json!({"diskProject": true}),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let observed = Rc::new(RefCell::new(Vec::<Value>::new()));
        let (options, settings) = recorded_options(&root, local.clone(), observed.clone());
        *settings.borrow_mut() =
            SettingsManager::in_memory(serde_json::from_value(global.clone()).unwrap());
        let created = maestro_app::create_agent_session(sdk_options(options, settings.clone()))
            .await
            .unwrap();
        assert!(
            created.extensions_result.errors.is_empty(),
            "{:?}",
            created.extensions_result.errors
        );
        assert_eq!(
            observed.take(),
            vec![json!({"settings": [global, project], "packages": packages})]
        );
        settings
            .borrow_mut()
            .set_global_key("unknown", "originalOwner", json!(true));
        created.session.reload().await.unwrap();
        let mut reloaded = global.clone();
        reloaded["unknown"]["originalOwner"] = json!(true);
        assert_eq!(
            observed.take(),
            vec![json!({"settings": [reloaded, project], "packages": packages})]
        );
    });
}

/// SDK creation uses its default loader with the supplied live settings owner.
fn sdk_options(
    options: DefaultResourceLoaderOptions<Local>,
    settings: Rc<RefCell<SettingsManager>>,
) -> maestro_app::CreateAgentSessionOptions {
    maestro_app::CreateAgentSessionOptions {
        cwd: Some(options.cwd),
        agent_dir: Some(options.agent_dir),
        auth_storage: None,
        model_registry: None,
        model: None,
        thinking_level: None,
        scoped_models: Vec::new(),
        no_tools: Some(maestro_app::NoTools::All),
        tools: None,
        custom_tools: Vec::new(),
        resources: maestro_app::SessionResources::Default(Box::new(options.resources)),
        session_manager: None,
        settings_manager: Some(settings),
        session_start_event: None,
        theme: maestro_theme::LiveTheme::default(),
        offline: Some(true),
    }
}

/// An empty package visible only through the SDK's original in-memory settings.
fn sdk_package(root: &crate::support::Root) -> Value {
    let empty = root.path("agent/empty");
    std::fs::create_dir(&empty).unwrap();
    std::fs::write(empty.join("package.json"), r#"{"maestro":{}}"#).unwrap();
    json!([{"source": "./empty", "identity": format!("local:{}", empty.display()), "scope": "user", "installedPath": empty}])
}
