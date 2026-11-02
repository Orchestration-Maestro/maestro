//! Persisted global preferences through the daemon configuration boundary.
#![cfg(all(test, not(target_arch = "wasm32")))]

use std::{path::Path, sync::Arc};

use maestro_daemon::settings::{
    io::{load_config, project_global_settings},
    model::UpdateChannelConfig,
    tab_bar::TabBarRightEntryConfig,
};
use maestro_settings::{
    BaseSettingsLayer, InMemorySettingsStorage, SettingsErrorKind, SettingsManager, SettingsScope,
    SettingsStorage,
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
    let mut current = load_config(&mut manager, &agent).0;
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
    assert_eq!(current.config.terminal.default_shell, "base-one");
    assert!(current.config.experimental.allow_nested.0);
    std::fs::write(
        agent.join("settings.json"),
        r#"{"multiplexer":{"allowNested":false}}"#,
    )
    .unwrap();
    let report = block_on(reload_config(&mut manager, &mut current, &agent)).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(current.config.experimental.allow_nested.0);
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
    assert_eq!(current.config, before.config);
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
    assert_eq!(current.config.terminal.default_shell, "");
    assert!(!current.config.experimental.allow_nested.0);
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
    let accepted = json!({
        "daemon":{"paneShell":expected.terminal.default_shell,
            "detectionCatalogCheck":if path == "daemon.detectionCatalogCheck" { !expected.update.manifest_check } else { expected.update.manifest_check },
            "paneHistory":if path == "daemon.paneHistory" { !expected.experimental.pane_history.0 } else { expected.experimental.pane_history.0 }},
        "update":{"channel":if path == "update.channel" {
            match expected.update.channel {
                UpdateChannelConfig::Stable => "preview", UpdateChannelConfig::Preview => "stable"
            }
        } else { match expected.update.channel {
            UpdateChannelConfig::Stable => "stable", UpdateChannelConfig::Preview => "preview"
        } },"versionCheck":expected.update.version_check},
        "multiplexer":{"mouseCapture":expected.ui.mouse_capture.0,
            "tabBarRight":if path != "multiplexer.tabBarRight" { json!(expected.ui.tab_bar_right) } else if expected.ui.tab_bar_right.is_empty() {
                json!([{"type":"hostname"}])
            } else { json!([]) },
            "remote":{"manageSshConfig":if path == "multiplexer.remote.manageSshConfig" { !expected.remote.manage_ssh_config } else { expected.remote.manage_ssh_config }}}
    });
    let mut current =
        project_global_settings(&serde_json::from_value(accepted).unwrap(), Path::new("."));
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(current.config, *expected);
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
    let mut current = project_global_settings(&settings, Path::new("."));
    let before = current.clone();
    let (report, errors) = block_on(reload_config(manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(report.diagnostics.is_empty());
    assert_eq!(current.config, before.config);
    assert!(!current.config.update.version_check);
    assert_eq!(current.config.terminal.default_shell, "user-shell");
    assert_eq!(
        manager.get_global_settings().0,
        user.as_object().unwrap().clone()
    );
}

#[test]
fn daemon_locked_area_uses_global_base_while_unlocked_invalid_area_keeps_current() {
    use maestro_daemon::settings::{io::reload_config, model::ConfigReloadStatus};
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
    let mut current = project_global_settings(
        &serde_json::from_value(json!({
            "update":{"channel":"preview","versionCheck":false},
            "daemon":{"paneShell":"retained"}
        }))
        .unwrap(),
        Path::new("."),
    );
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(current.config.update.channel, UpdateChannelConfig::Stable);
    assert!(!current.config.update.version_check);
    assert_eq!(current.config.terminal.default_shell, "retained");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("keeping current update settings"))
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
                .any(|message| message.starts_with("invalid "))
        );
    }
}

#[test]
fn daemon_startup_keeps_base_locks_when_user_document_is_unavailable() {
    use maestro_test_directory::Scratch;
    for unreadable in [false, true] {
        let root = Scratch::new().unwrap();
        let agent = root.path().join("agent");
        let mut publisher = SettingsManager::create(".", agent.to_str().unwrap(), ".maestro");
        publisher.publish_base_layer(
            SettingsScope::Global,
            Some(layer(
                json!({"daemon":{"resumeAgentsOnRestore":false,"paneShell":"base-shell"}}),
                &["daemon.resumeAgentsOnRestore"],
            )),
        );
        block_on(publisher.flush());
        let path = agent.join("settings.json");
        if unreadable {
            std::fs::create_dir(&path).unwrap();
        } else {
            std::fs::write(&path, "{").unwrap();
        }
        let mut manager = SettingsManager::create(".", agent.to_str().unwrap(), ".maestro");
        let (loaded, errors) = load_config(&mut manager, &agent);
        assert!(errors.is_empty());
        assert!(!loaded.config.session.resume_agents_on_restore);
        assert_eq!(loaded.config.terminal.default_shell, "base-shell");
        assert!(
            loaded
                .diagnostics
                .iter()
                .any(|message| message.contains("config ") && message.contains("using defaults"))
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_invalid_area_retains_unlocked_values_through_lock_changes() {
    use crate::config_support::{block_on, fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"mouseCapture":false,"tabBarRight":[]}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    for entries in [json!([{"type":"hostname"}]), json!([])] {
        storage
            .write_base_layer(
                SettingsScope::Global,
                Some(&layer(
                    json!({"multiplexer":{"tabBarRight":entries}}),
                    &["multiplexer.tabBarRight"],
                )),
            )
            .unwrap();
        put(
            &*storage,
            r#"{"multiplexer":{"sidebarWidth":"bad","mouseCapture":true}}"#,
        );
        let (report, errors) = block_on(app.reload_config(&mut manager, Path::new(".")));
        assert!(errors.is_empty());
        assert!(!app.config.ui.mouse_capture.0);
        assert_eq!(
            serde_json::to_value(&app.config.ui.tab_bar_right).unwrap(),
            entries
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.starts_with("invalid ui config:"))
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_first_reload_retains_startup_area_document() {
    use crate::config_support::put;
    use maestro_daemon::{api::event_hub::EventHub, app::App};
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(&*storage, r#"{"multiplexer":{"mouseCapture":false}}"#);
    let mut manager = SettingsManager::from_storage(storage.clone());
    let loaded = load_config(&mut manager, Path::new(".")).0;
    let manifests = Arc::new(std::sync::RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = crate::support::launch::launch_inputs(&[], manifests);
    let mut app = App::from_loaded(&loaded, || 0, launch, receiver, EventHub::default());
    storage
        .write_base_layer(
            SettingsScope::Global,
            Some(&layer(
                json!({"multiplexer":{"tabBarRight":[{"type":"hostname"}]}}),
                &["multiplexer.tabBarRight"],
            )),
        )
        .unwrap();
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(!app.config.ui.mouse_capture.0);
    assert_eq!(
        app.config.ui.tab_bar_right,
        vec![TabBarRightEntryConfig::Hostname {}]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_runtime_rejected_documents_do_not_replace_accepted_areas() {
    use crate::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"mouseCapture":false},"daemon":{"headlessCols":100}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarMinWidth":50,"sidebarMaxWidth":30},"daemon":{"headlessCols":0}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    storage.write_base_layer(SettingsScope::Global, Some(&layer(
        json!({"multiplexer":{"tabBarRight":[{"type":"hostname"}]},"daemon":{"headlessRows":47}}),
        &["multiplexer.tabBarRight", "daemon.headlessRows"],
    ))).unwrap();
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarWidth":"bad"},"daemon":{"headlessCols":"bad"}}"#,
    );
    let (report, errors) = block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(errors.is_empty());
    assert!(!report.diagnostics.is_empty());
    assert!(!app.config.ui.mouse_capture.0);
    assert_eq!(
        app.config.ui.tab_bar_right,
        vec![TabBarRightEntryConfig::Hostname {}]
    );
    assert_eq!(app.config.server.headless_cols, 100);
    assert_eq!(app.config.server.headless_rows, 47);
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_no_layer_rejected_ui_does_not_reset_scroll() {
    use crate::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, "{}");
    block_on(app.reload_config(&mut manager, Path::new(".")));
    app.state.agent_panel_scroll = 7;
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert_eq!(app.state.agent_panel_scroll, 7);
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_typed_constructor_retains_ui_on_first_invalid_no_layer_reload() {
    use crate::config_support::{fixture, put};
    let (mut app, mut manager, storage) = fixture();
    app.config.ui.mouse_capture.0 = false;
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":"bad"}}"#);
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(!app.config.ui.mouse_capture.0);
}

/// Attempts to replace a locked channel while resolving an unlocked daemon setting.
struct ResolveDaemonGlobal;

impl maestro_settings::SettingsDecider for ResolveDaemonGlobal {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, settings: &mut maestro_settings::Settings) -> Vec<String> {
        settings.0["update"]["channel"] = json!("stable");
        settings.0["daemon"]["paneShell"] = json!("resolved-global");
        Vec::new()
    }
}

#[test]
fn daemon_global_view_resolves_stored_values_and_restores_locks_after_decider() {
    let (storage, user) = persisted_layers();
    let mut manager = SettingsManager::from_storage(storage);
    manager
        .register_decider(Arc::new(ResolveDaemonGlobal))
        .unwrap();
    let settings = manager.effective_global_settings();
    assert_eq!(settings.0["daemon"]["paneShell"], json!("resolved-global"));
    assert_eq!(settings.0["update"]["channel"], json!("preview"));
    assert_eq!(settings.0["multiplexer"]["mouseCapture"], json!(false));
    assert_eq!(
        serde_json::to_value(manager.get_global_settings()).unwrap(),
        user
    );
    assert!(
        manager
            .drain_errors()
            .iter()
            .all(|error| error.kind == SettingsErrorKind::StoredValueIgnored)
    );
    let (loaded, errors) = load_config(&mut manager, Path::new("."));
    assert!(errors.is_empty());
    assert_eq!(loaded.config.terminal.default_shell, "resolved-global");
    assert_eq!(loaded.config.update.channel, UpdateChannelConfig::Preview);
}

#[test]
fn daemon_startup_fallback_resolves_stored_values_and_restores_locks_after_decider() {
    use maestro_test_directory::Scratch;
    let root = Scratch::new().unwrap();
    let agent = root.path().join("agent");
    let mut publisher = SettingsManager::create(".", agent.to_str().unwrap(), ".maestro");
    publisher.publish_base_layer(
        SettingsScope::Global,
        Some(layer(
            json!({"daemon":{"paneShell":"base-shell"},"update":{"channel":"preview"}}),
            &["update.channel"],
        )),
    );
    block_on(publisher.flush());
    std::fs::write(agent.join("settings.json"), "{").unwrap();
    let mut manager = SettingsManager::create(".", agent.to_str().unwrap(), ".maestro");
    manager
        .register_decider(Arc::new(ResolveDaemonGlobal))
        .unwrap();
    let (loaded, errors) = load_config(&mut manager, &agent);
    assert!(errors.is_empty());
    assert_eq!(loaded.config.terminal.default_shell, "resolved-global");
    assert_eq!(loaded.config.update.channel, UpdateChannelConfig::Preview);
}

/// Resolves an unlocked catalog check without touching the area a reload rejects.
struct ResolveCatalogCheck;

impl maestro_settings::SettingsDecider for ResolveCatalogCheck {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, settings: &mut maestro_settings::Settings) -> Vec<String> {
        settings.0.entry("daemon").or_insert_with(|| json!({}))["detectionCatalogCheck"] =
            json!(false);
        Vec::new()
    }
}

#[test]
fn daemon_area_recovery_resolves_stored_values_and_restores_locks_after_decider() {
    use maestro_daemon::settings::io::{AcceptedAreas, reload_config};
    let storage = Arc::new(InMemorySettingsStorage::new());
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(json!({"update":{"versionCheck":"bad"}}).to_string()))
        })
        .unwrap();
    storage
        .write_base_layer(
            SettingsScope::Global,
            Some(&layer(
                json!({"update":{"channel":"stable"},"daemon":{"detectionCatalogCheck":true}}),
                &["update.channel"],
            )),
        )
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage);
    manager
        .register_decider(Arc::new(ResolveCatalogCheck))
        .unwrap();
    let mut current =
        project_global_settings(&maestro_settings::Settings::default(), Path::new("."));
    current.accepted_areas = AcceptedAreas::default();
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert!(!report.diagnostics.is_empty());
    assert!(!current.config.update.manifest_check);
    assert_eq!(current.config.update.channel, UpdateChannelConfig::Stable);
}

/// Reports a successful resolution without rejecting the supplied preferences.
struct ResolutionDiagnostic;

impl maestro_settings::SettingsDecider for ResolutionDiagnostic {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, _: &mut maestro_settings::Settings) -> Vec<String> {
        vec!["stored preferences resolved".into()]
    }
}

/// A successful resolution keeps valid startup preferences and reports its message.
#[test]
fn daemon_startup_resolution_diagnostic_preserves_user_preferences() {
    let storage = Arc::new(InMemorySettingsStorage::new());
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(r#"{"daemon":{"paneShell":"user-shell"}}"#.into()))
        })
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage);
    manager
        .register_decider(Arc::new(ResolutionDiagnostic))
        .unwrap();
    let (loaded, errors) = load_config(&mut manager, Path::new("."));
    assert!(errors.is_empty());
    assert_eq!(loaded.config.terminal.default_shell, "user-shell");
    assert_eq!(loaded.diagnostics, vec!["stored preferences resolved"]);
}

/// A successful resolution is a diagnostic rather than a failed live reload.
#[test]
fn daemon_reload_resolution_diagnostic_applies_user_preferences() {
    use maestro_daemon::settings::{io::reload_config, model::ConfigReloadStatus};
    let storage = Arc::new(InMemorySettingsStorage::new());
    let mut manager = SettingsManager::from_storage(storage.clone());
    let mut current = load_config(&mut manager, Path::new(".")).0;
    manager
        .register_decider(Arc::new(ResolutionDiagnostic))
        .unwrap();
    manager.drain_errors();
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(r#"{"daemon":{"paneShell":"user-shell"}}"#.into()))
        })
        .unwrap();
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(current.config.terminal.default_shell, "user-shell");
    assert_eq!(report.diagnostics, vec!["stored preferences resolved"]);
}

/// Emits a diagnostic only when recovery resolves the empty-user base view.
struct FallbackDiagnostic;

impl maestro_settings::SettingsDecider for FallbackDiagnostic {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, settings: &mut maestro_settings::Settings) -> Vec<String> {
        if settings.0.contains_key("multiplexer") {
            Vec::new()
        } else {
            vec!["fallback preferences resolved".into(); 2]
        }
    }
}

/// Supplies a malformed UI area so its recovery alone emits a resolution message.
fn fallback_diagnostic_manager() -> SettingsManager {
    let storage = Arc::new(InMemorySettingsStorage::new());
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(r#"{"multiplexer":{"sidebarWidth":"bad"}}"#.into()))
        })
        .unwrap();
    storage
        .write_base_layer(SettingsScope::Global, Some(&layer(json!({}), &[])))
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage);
    manager
        .register_decider(Arc::new(FallbackDiagnostic))
        .unwrap();
    manager
}

/// Startup returns each fallback diagnostic once and leaves none queued.
#[test]
fn daemon_startup_collects_fallback_only_resolution_diagnostics() {
    let mut manager = fallback_diagnostic_manager();
    let (loaded, errors) = load_config(&mut manager, Path::new("."));
    assert!(errors.is_empty());
    assert_eq!(
        loaded
            .diagnostics
            .iter()
            .filter(|message| *message == "fallback preferences resolved")
            .count(),
        1
    );
    assert!(manager.drain_errors().is_empty());
}

/// Live recovery returns each fallback diagnostic once and leaves none queued.
#[test]
fn daemon_live_load_collects_fallback_only_resolution_diagnostics() {
    use maestro_daemon::settings::io::{AcceptedAreas, load_live_config};
    let mut manager = fallback_diagnostic_manager();
    let (loaded, errors) = block_on(load_live_config(
        &mut manager,
        &AcceptedAreas::default(),
        Path::new("."),
    ));
    assert!(errors.is_empty());
    let loaded = loaded.unwrap();
    assert_eq!(
        loaded
            .diagnostics
            .iter()
            .filter(|message| *message == "fallback preferences resolved")
            .count(),
        1
    );
    assert!(manager.drain_errors().is_empty());
}
