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
