//! Factory-time operations use the loader's settings, including in threaded factories.
use maestro_app::{
    DefaultResourceLoader, DefaultResourceLoaderOptions, ResourceLoader, ResourceLoaderOptions,
};
use maestro_extensions::host_operations::{HostOperation, OperationScope};
use maestro_models::Threaded;
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::json;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[test]
fn threaded_factory_reads_original_raw_settings_before_runner_binding() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let cwd = scratch.path().to_str().unwrap().to_owned();
        let storage = Arc::new(InMemorySettingsStorage::new());
        for (scope, value) in [
            (
                SettingsScope::Global,
                json!({"unknown": {"kept": null}, "defaultModel": 17}),
            ),
            (SettingsScope::Project, json!({"unknown": false})),
        ] {
            storage
                .with_lock(scope, &mut |_| Ok(Some(value.to_string())))
                .unwrap();
        }
        let settings = Rc::new(RefCell::new(SettingsManager::from_storage(storage)));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let mut resources = ResourceLoaderOptions::<Threaded>::new(
            local.clone(),
            Arc::new(|_| Box::pin(async { Ok(None) })),
        );
        resources.extension_factories.push(Arc::new(move |api| {
            let seen = captured.clone();
            Box::pin(observe_settings(api, seen))
        }));
        let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
            cwd: cwd.clone(),
            agent_dir: cwd,
            settings_manager: Some(settings.clone()),
            resources,
        });
        loader.reload().await.unwrap();
        assert!(
            loader.get_extensions().errors.is_empty(),
            "{:?}",
            loader.get_extensions().errors
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                json!({"unknown": {"kept": null}, "defaultModel": 17}),
                json!({"unknown": false})
            ]
        );
        loader.reload().await.unwrap();
        assert_eq!(seen.lock().unwrap().len(), 4);
    });
}

/// Reads both raw documents from a threaded factory before any session is bound.
async fn observe_settings(
    api: maestro_extensions::ExtensionAPI<Threaded>,
    seen: Arc<Mutex<Vec<serde_json::Value>>>,
) -> Result<(), maestro_extensions::EventError> {
    for scope in [OperationScope::User, OperationScope::Project] {
        let value = api
            .host_operation(HostOperation::ReadSettings(scope))
            .await
            .map_err(maestro_extensions::EventError::from)?;
        seen.lock().unwrap().push(value);
    }
    Ok(())
}

/// A loader over caller-supplied raw settings and disposable installed contents.
struct Probe {
    /// Retains the native fixture root.
    _scratch: maestro_test_directory::Scratch,
    /// The exact owner the loader receives.
    settings: Rc<RefCell<SettingsManager>>,
    /// Project source base.
    cwd: String,
    /// User source base.
    agent_dir: String,
}
impl Probe {
    /// Creates both scopes without writing or changing ambient settings.
    fn new(user: serde_json::Value, project: serde_json::Value) -> Self {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let cwd = scratch.path().join("project");
        let agent_dir = scratch.path().join("agent");
        std::fs::create_dir(&cwd).unwrap();
        std::fs::create_dir(&agent_dir).unwrap();
        let storage = Arc::new(InMemorySettingsStorage::new());
        for (scope, value) in [
            (SettingsScope::Global, user),
            (SettingsScope::Project, project),
        ] {
            storage
                .with_lock(scope, &mut |_| Ok(Some(value.to_string())))
                .unwrap();
        }
        Self {
            _scratch: scratch,
            settings: Rc::new(RefCell::new(SettingsManager::from_storage(storage))),
            cwd: cwd.to_str().unwrap().into(),
            agent_dir: agent_dir.to_str().unwrap().into(),
        }
    }
    /// Calls operations from a local factory, without an SDK session or bound runner.
    fn run(&self, operations: Vec<HostOperation>) -> Vec<Result<serde_json::Value, String>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let local = Rc::new(tokio::task::LocalSet::new());
        let observed = Rc::new(RefCell::new(Vec::new()));
        local.block_on(&runtime, async {
            let mut resources = ResourceLoaderOptions::<maestro_models::Local>::new(
                local.clone(),
                Rc::new(|_| Box::pin(async { Ok(None) })),
            );
            let captured = observed.clone();
            resources.extension_factories.push(Rc::new(move |api| {
                Box::pin(record_operations(api, operations.clone(), captured.clone()))
            }));
            let loader = DefaultResourceLoader::new(DefaultResourceLoaderOptions {
                cwd: self.cwd.clone(),
                agent_dir: self.agent_dir.clone(),
                settings_manager: Some(self.settings.clone()),
                resources,
            });
            loader.reload().await.unwrap();
            assert!(
                loader.get_extensions().errors.is_empty(),
                "{:?}",
                loader.get_extensions().errors
            );
        });
        observed.take()
    }
}
/// Records native failures as values so the factory itself can finish successfully.
async fn record_operations(
    api: maestro_extensions::ExtensionAPI<maestro_models::Local>,
    operations: Vec<HostOperation>,
    observed: Rc<RefCell<Vec<Result<serde_json::Value, String>>>>,
) -> Result<(), maestro_extensions::EventError> {
    for operation in operations {
        let result = api.host_operation(operation).await;
        observed.borrow_mut().push(result);
    }
    Ok(())
}

#[test]
fn local_factory_resolves_empty_packages_with_project_precedence() {
    let probe = Probe::new(
        json!({"packages":["https://example.test/team/repo@v1", "./empty"]}),
        json!({"packages":[{"source":"https://example.test/team/repo@v2", "extensions":[]}]}),
    );
    let project = std::path::Path::new(&probe.cwd)
        .join(maestro_app::configuration::config::CONFIG_DIR_NAME)
        .join("git/example.test/team/repo");
    let empty = std::path::Path::new(&probe.agent_dir).join("empty");
    for path in [&project, &empty] {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("package.json"), r#"{"maestro":{}}"#).unwrap();
    }
    let result = probe.run(vec![HostOperation::ResolvePackages]);
    assert_eq!(
        result,
        vec![Ok(json!([
            {"source":"https://example.test/team/repo@v2", "identity":"git:example.test/team/repo", "scope":"project", "installedPath":project},
            {"source":"./empty", "identity":format!("local:{}", empty.display()), "scope":"user", "installedPath":empty},
        ]))]
    );
}

#[path = "host_operations_support/mod.rs"]
mod git_command;

#[test]
fn factory_acquisition_selects_exact_pin_and_scope_without_editing_settings() {
    let probe = Probe::new(json!({"untouched": null}), json!({"unknown": false}));
    let before = probe.settings.borrow().get_global_settings();
    let project =
        std::path::Path::new(&probe.cwd).join(maestro_app::configuration::config::CONFIG_DIR_NAME);
    for (scope, root) in [
        (
            OperationScope::User,
            std::path::PathBuf::from(&probe.agent_dir),
        ),
        (OperationScope::Project, project),
    ] {
        let target = root.join("git/example.test/team/repo");
        let pin = git_command::pinned_repository(&target);
        let result = probe.run(vec![HostOperation::Acquire(
            "https://example.test/team/repo@pin".into(),
            scope,
        )]);
        assert_eq!(result, vec![Ok(serde_json::Value::Null)]);
        assert_eq!(git_command::git(&target, &["rev-parse", "HEAD"]), pin);
        assert_eq!(
            std::fs::read_to_string(target.join("content")).unwrap(),
            "pinned"
        );
    }
    assert_eq!(probe.settings.borrow().get_global_settings(), before);
}
