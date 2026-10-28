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
    let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
        cwd,
        agent_dir,
        settings_manager: Some(settings.clone()),
        resources,
    });
    (loader, settings)
}
