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
pub(crate) fn layer(value: Value, locked: &[&str]) -> BaseSettingsLayer {
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

#[test]
fn daemon_reload_rereads_global_layer_and_retains_config_on_base_read_failure() {
    use maestro_daemon::settings::{io::reload_config, model::ConfigReloadStatus};
    use maestro_test_directory::Scratch;
    let root = Scratch::new().unwrap();
    let agent = root.path().join("agent");
    let cwd = root.path().join("project");
    let mut manager =
        SettingsManager::create(cwd.to_str().unwrap(), agent.to_str().unwrap(), ".maestro");
    let mut current = load_config(&mut manager, &agent).0.config;
    let mut publisher =
        SettingsManager::create(cwd.to_str().unwrap(), agent.to_str().unwrap(), ".maestro");
    publisher.publish_base_layer(
        SettingsScope::Global,
        Some(layer(
            json!({
                "daemon":{"paneShell":"base-one"},"multiplexer":{"allowNested":true}
            }),
            &["multiplexer.allowNested"],
        )),
    );
    block_on(publisher.flush());
    let report = block_on(reload_config(&mut manager, &mut current, &agent)).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(current.terminal.default_shell, "base-one");
    assert!(current.experimental.allow_nested.0);
    std::fs::write(
        agent.join("settings.json"),
        r#"{"multiplexer":{"allowNested":false}}"#,
    )
    .unwrap();
    let report = block_on(reload_config(&mut manager, &mut current, &agent)).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(current.experimental.allow_nested.0);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("Stored value ignored"))
    );
    let before = current.clone();
    std::fs::write(agent.join("base-settings.json"), "{").unwrap();
    let report = block_on(reload_config(&mut manager, &mut current, &agent)).0;
    assert_eq!(report.status, ConfigReloadStatus::Failed);
    assert_eq!(current, before);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("keeping current config"))
    );
    publisher.publish_base_layer(SettingsScope::Global, None);
    block_on(publisher.flush());
    let report = block_on(reload_config(&mut manager, &mut current, &agent)).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(current.terminal.default_shell, "");
    assert!(!current.experimental.allow_nested.0);
}

/// Reload malformed user input while retaining an unrelated invalid area's old value.
fn assert_malformed_reload(
    user: &Value,
    base: Value,
    path: &str,
    expected: &maestro_daemon::settings::model::Config,
) -> maestro_daemon::settings::model::ConfigReloadReport {
    use maestro_daemon::settings::{io::reload_config, model::ConfigReloadStatus};
    let text = user.to_string();
    let storage = Arc::new(InMemorySettingsStorage::new());
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(text.clone())))
        .unwrap();
    storage
        .write_base_layer(SettingsScope::Global, Some(&layer(base, &[path])))
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.drain_errors();
    let mut current = expected.clone();
    current.update.channel = match expected.update.channel {
        UpdateChannelConfig::Stable => UpdateChannelConfig::Preview,
        UpdateChannelConfig::Preview => UpdateChannelConfig::Stable,
    };
    current.remote.manage_ssh_config = !expected.remote.manage_ssh_config;
    current.update.manifest_check = !expected.update.manifest_check;
    current.experimental.pane_history.0 = !expected.experimental.pane_history.0;
    current.ui.tab_bar_right = if expected.ui.tab_bar_right.is_empty() {
        vec![TabBarRightEntryConfig::Hostname {}]
    } else {
        Vec::new()
    };
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(current, *expected);
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    storage
        .with_lock(SettingsScope::Global, &mut |saved| {
            assert_eq!(saved, Some(text.as_str()));
            Ok(None)
        })
        .unwrap();
    report
}

/// Require one provenance-bearing diagnostic for a restored locked path.
fn assert_lock_diagnostic(
    report: &maestro_daemon::settings::model::ConfigReloadReport,
    path: &str,
) {
    let locks: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|message| message.starts_with("Stored value ignored:"))
        .collect();
    assert_eq!(locks.len(), 1);
    assert!(locks[0].contains(path));
    assert!(locks[0].contains("git:example/config#v1"));
}

#[test]
fn daemon_locks_restore_absent_and_present_values_under_malformed_ancestors() {
    use maestro_daemon::settings::model::Config;
    for (base, channel) in [
        (json!({}), Config::default().update.channel),
        (
            json!({"update":{"channel":"stable"}}),
            UpdateChannelConfig::Stable,
        ),
    ] {
        for malformed in [
            Value::Null,
            json!(false),
            json!(17),
            json!("bad"),
            json!([]),
        ] {
            let mut expected = Config::default();
            expected.update.channel = channel;
            expected.terminal.default_shell = "retained".into();
            let report = assert_malformed_reload(
                &json!({"update":malformed,"daemon":{"paneShell":17}}),
                base.clone(),
                "update.channel",
                &expected,
            );
            assert_lock_diagnostic(&report, "update.channel");
        }
    }
    for (base, manage) in [
        (json!({}), true),
        (
            json!({"multiplexer":{"remote":{"manageSshConfig":false}}}),
            false,
        ),
    ] {
        let mut expected = Config::default();
        expected.remote.manage_ssh_config = manage;
        expected.ui.mouse_capture.0 = false;
        expected.terminal.default_shell = "retained".into();
        let report = assert_malformed_reload(
            &json!({"multiplexer":{"remote":"bad","mouseCapture":false},"daemon":{"paneShell":17}}),
            base,
            "multiplexer.remote.manageSshConfig",
            &expected,
        );
        assert_lock_diagnostic(&report, "multiplexer.remote.manageSshConfig");
    }
}

/// Valid user values still override unlocked members of a constrained area.
fn assert_valid_locked_area_unchanged(
    manager: &mut SettingsManager,
    storage: &InMemorySettingsStorage,
) {
    use maestro_daemon::settings::{io::reload_config, model::ConfigReloadStatus};
    let user = json!({"update":{"channel":"stable","versionCheck":false},"daemon":{"paneShell":"user-shell"}});
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(user.to_string())))
        .unwrap();
    let settings = serde_json::from_value(user.clone()).unwrap();
    let mut current = project_global_settings(&settings, Path::new(".")).config;
    let before = current.clone();
    let (report, errors) = block_on(reload_config(manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(report.diagnostics.is_empty());
    assert_eq!(current, before);
    assert!(!current.update.version_check);
    assert_eq!(current.terminal.default_shell, "user-shell");
    assert_eq!(
        manager.get_global_settings().0,
        user.as_object().unwrap().clone()
    );
}

#[test]
fn daemon_locked_area_uses_global_base_while_unlocked_invalid_area_keeps_current() {
    use maestro_daemon::settings::{
        io::reload_config,
        model::{Config, ConfigReloadStatus},
    };
    let user = json!({"update":{"versionCheck":"bad"},"daemon":{"paneShell":17}});
    let storage = Arc::new(InMemorySettingsStorage::new());
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(user.to_string())))
        .unwrap();
    storage
        .write_base_layer(
            SettingsScope::Global,
            Some(&layer(
                json!({"update":{"channel":"stable"},"daemon":{"paneShell":"global-base"}}),
                &["update.channel"],
            )),
        )
        .unwrap();
    storage
        .write_base_layer(
            SettingsScope::Project,
            Some(&layer(
                json!({"daemon":{"paneShell":"project-base"}}),
                &["daemon.paneShell"],
            )),
        )
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.drain_errors();
    let mut current = Config::default();
    current.update.channel = UpdateChannelConfig::Preview;
    current.update.version_check = false;
    current.terminal.default_shell = "retained".into();
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(current.update.channel, UpdateChannelConfig::Stable);
    assert!(current.update.version_check);
    assert_eq!(current.terminal.default_shell, "retained");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("locked base")
                && message.contains("until")
                && message.contains("fixed"))
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("keeping current terminal settings"))
    );
    assert_eq!(
        manager.get_global_settings().0,
        user.as_object().unwrap().clone()
    );
    assert_valid_locked_area_unchanged(&mut manager, &storage);
}

#[test]
fn daemon_locked_area_fallback_covers_ui_and_flattened_source_namespaces() {
    use maestro_daemon::settings::model::Config;
    let mut ui = Config::default();
    ui.terminal.default_shell = "retained".into();
    let mut ui_present = ui.clone();
    ui_present.ui.tab_bar_right = vec![TabBarRightEntryConfig::Hostname {}];
    let mut catalog = ui.clone();
    catalog.update.manifest_check = false;
    let mut history = ui.clone();
    history.experimental.pane_history.0 = true;
    for (user, base, path, expected) in [
        (
            json!({"multiplexer":{"sidebarWidth":"bad"},"daemon":{"paneShell":17}}),
            json!({}),
            "multiplexer.tabBarRight",
            ui,
        ),
        (
            json!({"multiplexer":{"sidebarWidth":"bad"},"daemon":{"paneShell":17}}),
            json!({"multiplexer":{"tabBarRight":[{"type":"hostname"}]}}),
            "multiplexer.tabBarRight",
            ui_present,
        ),
        (
            json!({"update":{"channel":17},"daemon":{"paneShell":17}}),
            json!({"daemon":{"detectionCatalogCheck":false}}),
            "daemon.detectionCatalogCheck",
            catalog,
        ),
        (
            json!({"multiplexer":{"allowNested":17},"daemon":{"paneShell":17}}),
            json!({"daemon":{"paneHistory":true}}),
            "daemon.paneHistory",
            history,
        ),
    ] {
        let report = assert_malformed_reload(&user, base, path, &expected);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("locked base")
                    && message.contains("until")
                    && message.contains("fixed"))
        );
    }
}
