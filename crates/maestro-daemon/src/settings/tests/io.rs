//! Global workspace preference projection and diagnostics.

use crate::settings::{io::project_global_settings, model::Config};
use maestro_settings::Settings;
use serde_json::json;
use std::path::Path;

#[test]
fn restore_area_loads_without_diagnostics() {
    let raw = Settings(
        json!({"daemon":{"resumeAgentsOnRestore":false}})
            .as_object()
            .unwrap()
            .clone(),
    );
    let loaded = project_global_settings(&raw, Path::new("."));
    assert!(!loaded.config.session.resume_agents_on_restore);
    assert_eq!(loaded.config.ui, Config::default().ui);
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.invalid_sections.is_empty());
}

use crate::settings::io::{config_diagnostic_summary, load_config, reload_config};

fn summary(messages: &[&str]) -> String {
    config_diagnostic_summary(
        &messages
            .iter()
            .map(|message| (*message).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn diagnostic_banner_stays_compact() {
    assert_eq!(config_diagnostic_summary(&[]), None);
    assert_eq!(
        summary(&[
            "bad theme",
            "bad sound",
            "bad sidebar",
            "bad title",
            "bad schedule"
        ]),
        "settings.json; maestro settings check"
    );
}

#[test]
fn diagnostic_banner_identifies_unknown_keys() {
    assert_eq!(
        summary(&["unknown config key multiplexer.typo; ignoring key"]),
        "settings.json has unknown keys; maestro settings check"
    );
}

#[test]
fn diagnostic_banner_handles_mixed_causes() {
    assert_eq!(
        summary(&[
            "unknown config key daemon.typo; ignoring key",
            "invalid ui config"
        ]),
        "settings.json; maestro settings check"
    );
}

#[test]
fn diagnostic_banner_explains_default_fallback() {
    assert_eq!(
        summary(&["config parse error: invalid JSON; using defaults"]),
        "settings.json invalid; using defaults; maestro settings check"
    );
}

#[test]
fn diagnostic_banner_distinguishes_unreadable_settings() {
    assert_eq!(
        summary(&["config read error: denied; using defaults"]),
        "settings.json unreadable; using defaults; maestro settings check"
    );
    assert_eq!(
        summary(&["config read error: denied; keeping current config"]),
        "settings.json unreadable; keeping current config; maestro settings check"
    );
}

#[test]
fn diagnostic_banner_explains_retained_values() {
    assert_eq!(
        summary(&["config parse error: invalid JSON; keeping current config"]),
        "settings.json invalid; keeping current config; maestro settings check"
    );
}

fn project(value: serde_json::Value) -> crate::settings::model::LoadedConfig {
    let expected = value.clone();
    let raw: Settings = serde_json::from_value(value).unwrap();
    let loaded = project_global_settings(&raw, Path::new("."));
    assert_eq!(serde_json::to_value(raw).unwrap(), expected);
    loaded
}

#[test]
fn unknown_members_do_not_hide_valid_siblings() {
    let loaded = project(json!({
        "plugin":[], "keys":{"anything":true},
        "multiplexer":{
            "theme":{"custom":{"accentt":"#ffffff"}},
            "mouseCapture":false, "mouseCaptur":true, "foo.bar":true,"foo.?.bar":false,"z.z":true,
            "toast":{"delivry":"system"},
            "sidebar":{"agents":{"rowsByAgent":{"claude":[["terminal_title"]]}}}
        },
        "daemon":{"scrollbackLimitBytes":42}
    }));
    assert_eq!(
        loaded.diagnostics,
        [
            "unknown config key multiplexer.theme.custom.accentt; ignoring key",
            "unknown config key multiplexer.\"foo.?.bar\"; ignoring key",
            "unknown config key multiplexer.\"foo.bar\"; ignoring key",
            "unknown config key multiplexer.mouseCaptur; ignoring key",
            "unknown config key multiplexer.toast.delivry; ignoring key",
            "unknown config key multiplexer.\"z.z\"; ignoring key",
        ]
    );
    assert!(loaded.invalid_sections.is_empty());
    assert!(!loaded.config.ui.mouse_capture.0);
    assert_eq!(loaded.config.advanced.scrollback_limit_bytes, 42);
    for count in [8, 16] {
        let mut unknown = serde_json::Map::new();
        for index in 0..count {
            unknown.insert(format!("extra{index}"), json!(null));
        }
        let loaded = project(json!({"multiplexer":{"theme":{"custom":unknown}}}));
        assert_eq!(loaded.diagnostics.len(), count);
        assert!(loaded.invalid_sections.is_empty());
    }
}

#[test]
fn invalid_area_discards_its_unknown_key_warnings() {
    let loaded = project(
        json!({"multiplexer":{"mouseCapture":"yes","mouseCaptur":true,"theme":{"name":"nord"}},
        "update":{"channel":false,"typo":true}}),
    );
    assert_eq!(loaded.invalid_sections, ["ui", "update"]);
    assert_eq!(loaded.diagnostics.len(), 2);
    assert!(
        loaded
            .diagnostics
            .iter()
            .all(|message| message.starts_with("invalid "))
    );
    assert_eq!(loaded.config.theme.name.as_deref(), Some("nord"));
    assert!(loaded.config.ui.mouse_capture.0);
}

use super::support::{block_on, capture, manager, put};
use crate::settings::model::ConfigReloadStatus;
use maestro_settings::{SettingsManager, SettingsScope, SettingsStorage};
use std::sync::Arc;

#[test]
fn theme_name_warning_survives_reload() {
    let (mut manager, _) = manager(
        &json!({"multiplexer":{"theme":{"name":"unknown"}}}),
        &json!({}),
    );
    let (loaded, errors) = load_config(&mut manager, Path::new("."));
    assert!(errors.is_empty());
    assert!(loaded.diagnostics[0].contains("unknown theme name multiplexer.theme.name"));
    let mut current = loaded;
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(report.diagnostics[0].contains("using \"catppuccin\""));
    assert_eq!(current.config.theme.name.as_deref(), Some("unknown"));
}

#[test]
fn unreadable_settings_report_startup_and_reload_impact() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    std::fs::create_dir_all(root.join("settings.json")).unwrap();
    let root_text = root.to_str().unwrap();
    let mut manager = SettingsManager::create(root_text, root_text, "project");
    let ((loaded, errors), warning) = capture(|| load_config(&mut manager, root));
    assert!(errors.is_empty());
    assert_eq!(loaded.config, Config::default());
    assert!(loaded.diagnostics[0].starts_with("config read error:"));
    assert!(loaded.diagnostics[0].ends_with("using defaults"));
    assert!(warning.contains("config read error, using defaults"));
    assert!(warning.contains("err="));
    let mut current = project(json!({"daemon":{"paneShell":"kept"}}));
    let prior = current.clone();
    let ((report, errors), warning) =
        capture(|| block_on(reload_config(&mut manager, &mut current, root)));
    assert!(errors.is_empty());
    assert_eq!(report.status, ConfigReloadStatus::Failed);
    assert_eq!(current.config, prior.config);
    assert!(report.diagnostics[0].ends_with("keeping current config"));
    assert!(warning.is_empty());
}

#[test]
fn reload_status_tracks_independent_area_outcomes() {
    let old = json!({"multiplexer":{"mouseCapture":false,"theme":{"name":"nord"}},
        "daemon":{"paneShell":"first","paneHistory":true},"update":{"versionCheck":false}});
    let (mut manager, storage) = manager(&old, &json!({}));
    let mut current = load_config(&mut manager, Path::new(".")).0;
    put(
        &*storage,
        SettingsScope::Global,
        &json!({"multiplexer":{"mouseCapture":"bad","theme":{"name":"dracula"}},
        "daemon":{"paneShell":"second"}})
        .to_string(),
    );
    let report = block_on(reload_config(&mut manager, &mut current, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(!current.config.ui.mouse_capture.0);
    assert_eq!(current.config.theme.name.as_deref(), Some("dracula"));
    assert_eq!(current.config.terminal.default_shell, "second");
    assert!(!current.config.experimental.pane_history.0);
    assert!(current.config.update.version_check);
    put(&*storage, SettingsScope::Global, "{");
    block_on(manager.reload()); // Queue an earlier read failure, not this reload's error.
    put(&*storage, SettingsScope::Global, "{}");
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].scope, SettingsScope::Global);
    assert_eq!(current.config, Config::default());
    assert!(report.diagnostics.is_empty());
}

/// A queued write failure must be returned separately from a fresh global read.
struct FailingWrite {
    storage: maestro_settings::InMemorySettingsStorage,
}

impl SettingsStorage for FailingWrite {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> maestro_settings::SettingsUpdate,
    ) -> Result<(), maestro_settings::SettingsStorageError> {
        self.storage.with_lock(scope, &mut |text| {
            let replacement = update(text)?;
            if replacement.is_some() {
                return Err(std::io::Error::other("controlled write failure").into());
            }
            Ok(None)
        })
    }
}

#[test]
fn queued_write_failure_does_not_poison_reload() {
    let mut manager = SettingsManager::from_storage(Arc::new(FailingWrite {
        storage: maestro_settings::InMemorySettingsStorage::new(),
    }));
    manager.set_global_key("daemon", "paneShell", json!("not-saved"));
    let mut current = project(json!({}));
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].error.to_string(), "controlled write failure");
    assert_eq!(current.config, Config::default());
}

/// Build one authored nested setting without changing any sibling's type.
fn at_path(path: &str, value: serde_json::Value) -> serde_json::Value {
    path.split('.')
        .rev()
        .fold(value, |child, key| json!({key:child}))
}

#[test]
fn area_reload_obeys_json_field_domains() {
    assert_eq!(project(json!({})).config, Config::default());
    let cases: Vec<(&str, serde_json::Value, &str, bool)> =
        serde_json::from_str(include_str!("io_field_domains.json")).unwrap();
    for (path, valid, area, nullable) in cases {
        assert!(
            project(at_path(path, valid.clone()))
                .invalid_sections
                .is_empty(),
            "{path}"
        );
        for wrong in [
            json!(null),
            json!("invalid"),
            json!(1.5),
            json!(true),
            json!({}),
            json!([]),
        ] {
            let accepted = (wrong.is_null() && nullable)
                || (valid.is_string()
                    && wrong.is_string()
                    && matches!(
                        path,
                        "multiplexer.theme.name"
                            | "multiplexer.accent"
                            | "multiplexer.sound.path"
                            | "daemon.paneShell"
                            | "daemon.newPaneCwd"
                            | "daemon.worktreesDirectory"
                    ))
                || (valid.is_boolean() && wrong.is_boolean())
                || (valid.is_array() && wrong.is_array())
                || (valid.is_object() && wrong.is_object());
            let loaded = project(at_path(path, wrong.clone()));
            assert_eq!(
                !loaded.invalid_sections.contains(&area.to_owned()),
                accepted,
                "{path}: {wrong}, {:?}",
                loaded.diagnostics
            );
        }
    }
}

#[test]
fn malformed_namespaces_and_positional_records_retain_their_areas() {
    let (mut manager, storage) = manager(&json!({"multiplexer":{"onboarding":false}}), &json!({}));
    let mut current = load_config(&mut manager, Path::new(".")).0;
    put(&*storage, SettingsScope::Global, r#"{"multiplexer":null}"#);
    let report = block_on(reload_config(&mut manager, &mut current, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(current.config.onboarding, Some(false));
    for path in [
        "multiplexer.theme",
        "multiplexer.theme.custom",
        "multiplexer.sidebar",
        "multiplexer.sidebar.agents",
        "multiplexer.toast",
        "multiplexer.toast.clipboard",
        "multiplexer.sound",
        "multiplexer.sound.agents",
        "multiplexer.remote",
    ] {
        assert!(
            !project(at_path(path, json!([])))
                .invalid_sections
                .is_empty(),
            "{path}"
        );
    }
    for path in [
        "multiplexer.sidebar.agents.rows",
        "multiplexer.sidebar.spaces.rows",
        "multiplexer.sidebar.agents.rowsByAgent.claude",
    ] {
        let loaded = project(at_path(path, json!([[["workspace", null, null, null]]])));
        assert_eq!(loaded.invalid_sections, ["ui"], "{path}");
    }
}

#[test]
fn catalog_check_is_owned_by_the_daemon_namespace_only() {
    let loaded = project(
        json!({"update":{"detectionCatalogCheck":false},"multiplexer":{"paneHistory":true}}),
    );
    assert!(loaded.config.update.manifest_check);
    assert!(!loaded.config.experimental.pane_history.0);
    assert_eq!(
        loaded.diagnostics,
        [
            "unknown config key multiplexer.paneHistory; ignoring key",
            "unknown config key update.detectionCatalogCheck; ignoring key",
        ]
    );
    let loaded = project(json!({"update":{"detectionCatalogCheck":false,"channel":false}}));
    assert_eq!(loaded.invalid_sections, ["update"]);
    assert_eq!(loaded.diagnostics.len(), 1);
    assert!(!loaded.diagnostics[0].starts_with("unknown config key"));
}

#[test]
fn object_valued_string_enums_reject_projection() {
    for (path, variant, area) in [
        ("multiplexer.hostCursor", "native", "ui"),
        ("multiplexer.agentPanelSort", "priority", "ui"),
        ("multiplexer.statusIndicators", "symbols", "ui"),
        ("multiplexer.sidebarCollapsedMode", "hidden", "ui"),
        ("multiplexer.tabBarPosition", "bottom", "ui"),
        ("multiplexer.toast.delivery", "system", "ui"),
        ("multiplexer.toast.clipboard.position", "top-left", "ui"),
        ("multiplexer.toast.workspace.position", "top-left", "ui"),
        ("multiplexer.sound.agents.claude", "off", "ui"),
        ("multiplexer.cjkImeCursorShape", "bar", "experimental"),
        ("daemon.paneShellMode", "login", "terminal"),
        ("update.channel", "preview", "update"),
    ] {
        let valid = at_path(path, json!(variant));
        assert!(project(valid).invalid_sections.is_empty(), "{path}");
        let wrong = at_path(path, json!({variant: null}));
        let loaded = project(wrong);
        assert_eq!(loaded.invalid_sections, [area], "{path}");
        assert!(loaded.diagnostics[0].starts_with(&format!("invalid {area} config:")));
    }
    let loaded = project(json!({"multiplexer":{"tabBarRight":[{"type":"text","text":"ready"}]}}));
    assert!(loaded.invalid_sections.is_empty());
    assert_eq!(
        loaded.config.ui.tab_bar_right,
        [crate::settings::tab_bar::TabBarRightEntryConfig::Text {
            text: "ready".into()
        }]
    );
}

#[test]
fn object_host_cursor_retains_previous_ui_on_reload() {
    let (mut manager, storage) = manager(
        &json!({"multiplexer":{"hostCursor":"drawn","sidebarWidth":31}}),
        &json!({}),
    );
    let mut current = load_config(&mut manager, Path::new(".")).0;
    let previous = current.config.ui.clone();
    put(
        &*storage,
        SettingsScope::Global,
        r#"{"multiplexer":{"hostCursor":{"native":null},"sidebarWidth":40}}"#,
    );
    let (report, errors) = block_on(reload_config(&mut manager, &mut current, Path::new(".")));
    assert!(errors.is_empty());
    assert_eq!(current.config.ui, previous);
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.starts_with("invalid ui config:"))
    );
}
