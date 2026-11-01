//! Workspace preference saves use captured settings edits before live application.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::config_support;

use config_support::{block_on, fixture, put};
use maestro_daemon::settings::model::{
    AgentPanelSortConfig, ConfigReloadStatus, StatusIndicatorStyle, ToastDelivery,
};
use maestro_settings::{SettingsScope, SettingsStorage};
use serde_json::Value;
use std::path::Path;

/// Reads the exact stored global document without consulting the manager's cache.
fn saved(storage: &dyn SettingsStorage) -> Value {
    let mut saved = None;
    storage
        .with_lock(SettingsScope::Global, &mut |text| {
            saved = Some(serde_json::from_str(text.unwrap()).unwrap());
            Ok(None)
        })
        .unwrap();
    saved.unwrap()
}

#[test]
fn daemon_settings_save_toast_delivery_persists_then_applies_live_config() {
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, r#"{"multiplexer":{"onboarding":false}}"#);
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Off);
    let (report, errors) =
        block_on(app.save_toast_delivery(&mut manager, Path::new("."), ToastDelivery::Terminal));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(errors.is_empty());
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Terminal);
    assert_eq!(
        saved(&*storage)["multiplexer"]["toast"]["delivery"],
        "terminal"
    );
    assert_eq!(saved(&*storage)["multiplexer"]["onboarding"], false);
    assert!(app.state.config_diagnostic.is_none());
    assert!(app.state.toast.is_none());
}

#[test]
fn daemon_save_status_indicators_persists_then_applies_live_config() {
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(app.config.ui.status_indicators, StatusIndicatorStyle::Dots);
    let (report, errors) = block_on(app.save_status_indicators(
        &mut manager,
        Path::new("."),
        StatusIndicatorStyle::Symbols,
    ));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(errors.is_empty());
    assert_eq!(
        app.config.ui.status_indicators,
        StatusIndicatorStyle::Symbols
    );
    assert_eq!(
        saved(&*storage)["multiplexer"]["statusIndicators"],
        "symbols"
    );
    assert!(app.state.config_diagnostic.is_none());
}

#[test]
fn daemon_save_agent_panel_sort_persists_then_applies_live_config() {
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(app.state.agent_panel_sort, AgentPanelSortConfig::Spaces);
    let (report, errors) = block_on(app.save_agent_panel_sort(
        &mut manager,
        Path::new("."),
        AgentPanelSortConfig::Priority,
    ));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(errors.is_empty());
    assert_eq!(app.state.agent_panel_sort, AgentPanelSortConfig::Priority);
    assert_eq!(
        saved(&*storage)["multiplexer"]["agentPanelSort"],
        "priority"
    );
    assert!(app.state.config_diagnostic.is_none());
}

/// A healthy accepted snapshot whose later write attempts fail before replacement.
struct RejectWrites(std::sync::Arc<maestro_settings::InMemorySettingsStorage>);

impl SettingsStorage for RejectWrites {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> maestro_settings::SettingsUpdate,
    ) -> Result<(), maestro_settings::SettingsStorageError> {
        self.0.with_lock(scope, update)
    }

    fn with_write_lock(
        &self,
        _: SettingsScope,
        _: &mut dyn FnMut(Option<&str>) -> maestro_settings::SettingsUpdate,
    ) -> Result<(), maestro_settings::SettingsStorageError> {
        Err("controlled write failure".into())
    }
}

/// Proves failed writes cannot publish their cached manager value into live daemon state.
fn assert_failed_overlay_retains_live_area() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let (mut app, _, storage) = fixture();
    let mut manager =
        maestro_settings::SettingsManager::from_storage(Arc::new(RejectWrites(storage.clone())));
    let before = app.config.clone();
    let started = Instant::now();
    let (report, errors) =
        block_on(app.save_toast_delivery(&mut manager, Path::new("."), ToastDelivery::Terminal));
    assert_eq!(report.status, ConfigReloadStatus::Failed);
    assert_eq!(errors.len(), 1);
    assert_eq!(app.config, before);
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Off);
    assert_eq!(saved(&*storage), serde_json::json!({}));
    assert_eq!(
        app.state.config_diagnostic.as_deref(),
        Some("failed to save toast setting: controlled write failure")
    );
    let deadline = app.config_diagnostic_deadline.unwrap();
    assert!(deadline >= started + Duration::from_secs(5));
    assert!(deadline <= Instant::now() + Duration::from_secs(5));
    assert!(!app.take_config_reloaded_from_disk());
}

#[test]
fn overlay_save_keeps_unrelated_global_settings() {
    use serde_json::json;
    let (mut app, _, storage) = fixture();
    put(&*storage, &json!({"theme":"chat", "multiplexer":{"onboarding":false,"toast":{"enabled":true,"delivery":"off","clipboard":{"enabled":true}}}}).to_string());
    let mut first = maestro_settings::SettingsManager::from_storage(storage.clone());
    let mut second = maestro_settings::SettingsManager::from_storage(storage.clone());
    first.set_global_key_path(
        "multiplexer",
        &["toast", "clipboard", "enabled"],
        Some(json!(false)),
    );
    first.set_default_model("retained-model".into());
    block_on(first.flush());
    let (report, errors) =
        block_on(app.save_toast_delivery(&mut second, Path::new("."), ToastDelivery::Terminal));
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(errors.is_empty());
    assert_eq!(
        saved(&*storage),
        json!({"theme":"chat","defaultModel":"retained-model","multiplexer":{"onboarding":false,"toast":{"delivery":"terminal","clipboard":{"enabled":false}}}})
    );
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Terminal);
    assert!(!app.state.toast_config.clipboard.enabled);
    assert_failed_overlay_retains_live_area();
}

#[test]
fn preference_save_helpers_retain_sibling_fields_and_apply_manual_theme_policy() {
    use serde_json::json;
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, &json!({"multiplexer":{"theme":{"autoSwitch":true,"custom":{"accent":"#010203"}},"sound":{"enabled":false},"onboarding":true}}).to_string());
    block_on(manager.reload());
    let report = block_on(app.save_theme(&mut manager, Path::new("."), "gruvbox")).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(app.state.theme_name, "gruvbox");
    assert!(!app.state.theme_runtime.auto_switch);
    assert_eq!(
        saved(&*storage)["multiplexer"]["theme"],
        json!({"name":"gruvbox","autoSwitch":false,"custom":{"accent":"#010203"}})
    );
    assert_eq!(
        block_on(app.save_sound(&mut manager, Path::new("."), true))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert!(app.state.sound.enabled);
    assert_eq!(
        block_on(app.save_agent_border_labels(&mut manager, Path::new("."), true))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert!(app.config.ui.show_agent_labels_on_pane_borders.0);
    assert_eq!(
        block_on(app.mark_onboarding_complete(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(saved(&*storage)["multiplexer"]["onboarding"], false);
    assert_eq!(
        saved(&*storage)["multiplexer"]["theme"]["custom"]["accent"],
        "#010203"
    );
}
