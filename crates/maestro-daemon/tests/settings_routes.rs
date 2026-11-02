//! Global-only preferences through the real settings-owner seam.
#![cfg(all(test, not(target_arch = "wasm32")))]

#[cfg(test)]
mod tests {

    use std::{
        path::Path,
        sync::{Arc, Mutex},
    };

    use maestro_daemon::settings::{
        io::{load_config, reload_config},
        model::{Config, ConfigReloadStatus},
    };
    use maestro_settings::{
        InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage,
        SettingsStorageError, SettingsUpdate,
    };
    use serde_json::json;

    #[test]
    fn workspace_preferences_ignore_project_scope() {
        let storage = Arc::new(InMemorySettingsStorage::new());
        for (scope, value) in [
            (
                SettingsScope::Global,
                json!({"multiplexer":{"mouseCapture":false},"daemon":{"paneShell":"global"},"update":{"versionCheck":false},"theme":"global-chat"}),
            ),
            (
                SettingsScope::Project,
                json!({"multiplexer":{"mouseCapture":true},"daemon":{"paneShell":"project"},"update":{"versionCheck":true},"theme":"project-chat"}),
            ),
        ] {
            storage
                .with_lock(scope, &mut |_| Ok(Some(value.to_string())))
                .unwrap();
        }
        let mut manager = SettingsManager::from_storage(storage.clone());
        let (loaded, errors) = load_config(&mut manager, Path::new("."));
        assert!(errors.is_empty());
        assert!(!loaded.config.ui.mouse_capture.0);
        assert_eq!(loaded.config.terminal.default_shell, "global");
        assert!(!loaded.config.update.version_check);
        assert_eq!(manager.get_theme().as_deref(), Some("project-chat"));
        storage
            .with_lock(SettingsScope::Project, &mut |_| Ok(Some("{".into())))
            .unwrap();
        let mut current = loaded;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let (report, errors) =
            runtime.block_on(reload_config(&mut manager, &mut current, Path::new(".")));
        assert_eq!(report.status, ConfigReloadStatus::Applied);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].scope, SettingsScope::Project);
        assert_eq!(current.config.terminal.default_shell, "global");
        assert!(!current.config.ui.mouse_capture.0);
        assert!(!current.config.update.version_check);
    }

    /// Read failure is supplied independently of malformed stored JSON.
    struct ControlledStorage(Mutex<Result<String, std::io::ErrorKind>>);

    impl SettingsStorage for ControlledStorage {
        fn with_lock(
            &self,
            scope: SettingsScope,
            update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
        ) -> Result<(), SettingsStorageError> {
            if scope == SettingsScope::Project {
                update(None)?;
                return Ok(());
            }
            let state = self.0.lock().unwrap();
            match &*state {
                Ok(text) => {
                    update(Some(text))?;
                    Ok(())
                }
                Err(kind) => Err(std::io::Error::new(*kind, "controlled read failure").into()),
            }
        }
    }

    #[test]
    fn malformed_reload_keeps_every_workspace_area() {
        let storage = Arc::new(ControlledStorage(Mutex::new(Ok("{".into()))));
        let mut manager = SettingsManager::from_storage(storage.clone());
        let output = maestro_test_tracing::Output::default();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer({
                let output = output.clone();
                move || output.clone()
            })
            .finish();
        let (loaded, errors) = maestro_test_tracing::with_default(subscriber, || {
            load_config(&mut manager, Path::new("."))
        });
        assert!(errors.is_empty());
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.diagnostics[0].starts_with("config parse error:"));
        let warning = output.text();
        assert!(warning.contains("config parse error, using defaults"));
        assert!(warning.contains("err="));
        *storage.0.lock().unwrap() = Ok(json!({
        "multiplexer":{"onboarding":false,"theme":{"name":"nord"},"mouseCapture":false,"allowNested":true,"remote":{"manageSshConfig":false}},
        "daemon":{"paneShell":"chosen","resumeAgentsOnRestore":false,"headlessCols":70,"headlessRows":30,"worktreesDirectory":"chosen-path","scrollbackLimitBytes":20,"paneHistory":true,"detectionCatalogCheck":false},
        "update":{"channel":"preview","versionCheck":false}
    }).to_string());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut current = loaded;
        let report = runtime
            .block_on(reload_config(&mut manager, &mut current, Path::new(".")))
            .0;
        assert_eq!(report.status, ConfigReloadStatus::Applied);
        let previous = current.clone();
        for input in [Ok("{".into()), Err(std::io::ErrorKind::PermissionDenied)] {
            *storage.0.lock().unwrap() = input;
            output.clear();
            let subscriber = tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer({
                    let output = output.clone();
                    move || output.clone()
                })
                .finish();
            let (report, errors) = maestro_test_tracing::with_default(subscriber, || {
                runtime.block_on(reload_config(&mut manager, &mut current, Path::new(".")))
            });
            assert!(errors.is_empty());
            assert_eq!(report.status, ConfigReloadStatus::Failed);
            assert!(report.diagnostics[0].ends_with("keeping current config"));
            assert_eq!(current.config, previous.config);
            assert!(output.text().is_empty());
        }
    }
}
