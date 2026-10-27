//! Persisted global preferences through the daemon configuration boundary.
#![cfg(all(test, not(target_arch = "wasm32")))]

use std::{path::Path, sync::Arc};

use maestro_daemon::settings::{
    io::{load_config, project_global_settings},
    model::UpdateChannelConfig,
    tab_bar::TabBarRightEntryConfig,
};
use maestro_settings::{
    BaseSettingsLayer, InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage,
};
use serde_json::{Value, json};

/// A synthetic accepted release with caller-selected locks.
fn layer(value: Value, locked: &[&str]) -> BaseSettingsLayer {
    BaseSettingsLayer {
        settings: serde_json::from_value(value).unwrap(),
        locked: locked.iter().map(|path| (*path).to_owned()).collect(),
        source: "git:example/config#v1".into(),
        recorded_at: "2026-10-27T12:00:00Z".into(),
    }
}

/// Waits for the existing settings persistence queue.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Persist independent global and project layers over conflicting user preferences.
fn persisted_layers() -> (Arc<InMemorySettingsStorage>, Value) {
    let storage = Arc::new(InMemorySettingsStorage::new());
    let user = json!({
        "multiplexer":{"tabBarRight":[],"allowNested":false,"remote":{"manageSshConfig":true},"mouseCapture":false},
        "daemon":{"resumeAgentsOnRestore":true,"detectionCatalogCheck":true},
        "update":{"versionCheck":true,"channel":"stable"}
    });
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(user.to_string())))
        .unwrap();
    storage
        .with_lock(SettingsScope::Project, &mut |_| {
            Ok(Some(json!({
        "multiplexer":{"allowNested":false,"mouseCapture":true},"daemon":{"paneShell":"project-base"}
    }).to_string()))
        })
        .unwrap();
    let mut publisher = SettingsManager::from_storage(storage.clone());
    assert_eq!(
        load_config(&mut publisher, Path::new(".")).0.config,
        project_global_settings(&publisher.get_global_settings(), Path::new(".")).config
    );
    publisher.publish_base_layer(SettingsScope::Global, Some(layer(json!({
        "multiplexer":{"tabBarRight":[{"type":"hostname"}],"allowNested":true,"remote":{"manageSshConfig":false},"mouseCapture":true},
        "daemon":{"resumeAgentsOnRestore":false,"detectionCatalogCheck":false,"paneShell":"base"},
        "update":{"versionCheck":false,"channel":"preview"}
    }), &[
        "multiplexer.tabBarRight", "multiplexer.allowNested", "multiplexer.remote.manageSshConfig",
        "daemon.resumeAgentsOnRestore", "daemon.detectionCatalogCheck", "update.versionCheck", "update.channel"
    ])));
    publisher.publish_base_layer(
        SettingsScope::Project,
        Some(layer(
            json!({
                "daemon":{"paneShell":"project-base"},"multiplexer":{"allowNested":false}
            }),
            &["daemon.paneShell", "multiplexer.allowNested"],
        )),
    );
    block_on(publisher.flush());
    (storage, user)
}

#[test]
fn daemon_global_locks_win_for_every_governed_key_without_project_settings() {
    let (storage, user) = persisted_layers();
    let mut manager = SettingsManager::from_storage(storage);
    let (loaded, errors) = load_config(&mut manager, Path::new("."));
    assert!(errors.is_empty());
    assert_eq!(
        loaded.config.ui.tab_bar_right,
        vec![TabBarRightEntryConfig::Hostname {}]
    );
    assert!(loaded.config.experimental.allow_nested.0);
    assert!(!loaded.config.remote.manage_ssh_config);
    assert!(!loaded.config.session.resume_agents_on_restore);
    assert!(!loaded.config.update.manifest_check);
    assert!(!loaded.config.update.version_check);
    assert_eq!(loaded.config.update.channel, UpdateChannelConfig::Preview);
    assert!(!loaded.config.ui.mouse_capture.0);
    assert_eq!(loaded.config.terminal.default_shell, "base");
    assert!(loaded.invalid_sections.is_empty());
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|message| message.contains("Stored value ignored")
                && message.contains("git:example/config#v1"))
    );
    assert!(
        !loaded
            .diagnostics
            .iter()
            .any(|message| message.contains("using defaults"))
    );
    assert_eq!(
        manager.get_global_settings().0,
        user.as_object().unwrap().clone()
    );
}
