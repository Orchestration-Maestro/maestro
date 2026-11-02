//! Global configuration edits remain governed independently of project layers.
#![cfg(all(test, target_os = "linux"))]

use crate::{config_support, daemon_base_settings::layer};
use config_support::{TestApp, block_on, fixture};
use maestro_daemon::settings::model::{ConfigReloadStatus, StatusIndicatorStyle};
use maestro_settings::{Settings, SettingsErrorKind, SettingsManager, SettingsScope};
use maestro_test_directory::Scratch;
use serde_json::json;

/// A native global preferences file with an independent persisted project layer.
fn governed_app() -> (TestApp, SettingsManager, Scratch) {
    let root = Scratch::new().unwrap();
    let path = root.path().to_str().unwrap();
    let mut publisher = SettingsManager::create(path, path, ".maestro");
    publisher.set_global_key_path("multiplexer", &["statusIndicators"], Some(json!("symbols")));
    publisher.publish_base_layer(
        SettingsScope::Global,
        Some(layer(
            json!({
                "multiplexer":{"statusIndicators":"symbols"}
            }),
            &["multiplexer.statusIndicators"],
        )),
    );
    publisher.publish_base_layer(
        SettingsScope::Project,
        Some(layer(
            json!({
                "multiplexer":{"sound":{"enabled":false}}
            }),
            &["multiplexer.sound.enabled"],
        )),
    );
    block_on(publisher.flush());
    let mut manager = SettingsManager::create(path, path, ".maestro");
    let (mut app, _, _) = fixture();
    let report = block_on(app.reload_config(&mut manager, root.path())).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    (app, manager, root)
}

#[test]
fn daemon_refuses_global_locked_edit_with_project_layer_without_file_or_live_change() {
    let (mut app, mut manager, root) = governed_app();
    let file = root.path().join("settings.json");
    let bytes = std::fs::read(&file).unwrap();
    let global = manager.get_global_settings();
    let project = manager.get_project_settings();
    let config = app.config.clone();
    let (report, errors) =
        block_on(app.save_status_indicators(&mut manager, root.path(), StatusIndicatorStyle::Dots));
    assert_eq!(report.status, ConfigReloadStatus::Failed);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind, SettingsErrorKind::Failure);
    assert!(report.diagnostics[0].contains("multiplexer.statusIndicators"));
    assert!(report.diagnostics[0].contains("git:example/config#v1"));
    assert!(report.diagnostics[0].contains("2026-10-27T12:00:00Z"));
    assert_eq!(app.config, config);
    assert_eq!(manager.get_global_settings(), global);
    assert_eq!(manager.get_project_settings(), project);
    block_on(manager.flush());
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}

#[test]
fn daemon_global_save_ignores_project_only_lock_but_runtime_overrides_do_not() {
    let (mut app, mut manager, root) = governed_app();
    let (report, errors) = block_on(app.save_sound(&mut manager, root.path(), true));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(
        errors
            .iter()
            .all(|error| error.kind == SettingsErrorKind::StoredValueIgnored)
    );
    assert!(app.state.sound.enabled);
    assert_eq!(
        manager.get_global_settings().0["multiplexer"]["sound"]["enabled"],
        true
    );
    let saved = std::fs::read(root.path().join("settings.json")).unwrap();
    manager.apply_overrides(
        serde_json::from_value::<Settings>(json!({
            "multiplexer":{"sound":{"enabled":true}}
        }))
        .unwrap(),
    );
    let errors = manager.drain_errors();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].error.to_string().contains("Edit refused"));
    block_on(manager.flush());
    assert_eq!(
        std::fs::read(root.path().join("settings.json")).unwrap(),
        saved
    );
    assert_eq!(
        app.config.ui.status_indicators,
        StatusIndicatorStyle::Symbols
    );
    assert!(!root.path().join(".maestro/settings.json").exists());
}
