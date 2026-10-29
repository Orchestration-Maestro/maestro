//! Publisher decisions through the settings manager's public edit seam.
#![cfg(test)]

use crate::support::{ControlledStorage, assert_number, block_on, settings, text_entries};
use maestro_settings::{
    DecisionRequest, SettingsDecider, SettingsErrorKind, SettingsManager, SettingsScope,
};
use serde_json::json;
use std::sync::Arc;

/// Refuses explicit requests without changing the stored-value view.
struct Refuse;
impl SettingsDecider for Refuse {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Err("release policy refuses this edit".into())
    }
}

/// Refused edits preserve raw scopes, overrides and the persistence queue.
#[test]
fn refusing_decider_preserves_settings_and_pending_edits() {
    let storage = ControlledStorage::new();
    storage.inject(SettingsScope::Global, r#"{"defaultModel":"stored"}"#);
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.register_decider(Arc::new(Refuse)).unwrap();
    manager.apply_overrides(settings(json!({"defaultModel":"override"})));
    manager.set_default_model_and_provider("new-provider".into(), "new-model".into());
    manager.set_global_key_path("compaction", &["enabled"], Some(json!(false)));
    manager.set_project_skill_paths(text_entries(&["disabled"]));
    manager.set_editor_padding_x(f64::NAN);
    block_on(manager.flush());
    assert_eq!(manager.get_default_model().as_deref(), Some("stored"));
    assert_eq!(manager.get_default_provider(), None);
    assert!(manager.get_compaction_enabled());
    assert!(manager.get_project_settings().0.is_empty());
    assert_number(manager.get_editor_padding_x(), 0.0);
    assert!(storage.writes().is_empty());
    let errors = manager.drain_errors();
    assert_eq!(errors.len(), 5);
    assert!(
        errors
            .iter()
            .all(|error| error.error.to_string() == "release policy refuses this edit")
    );
    assert_eq!(
        storage.text(SettingsScope::Global).as_deref(),
        Some(r#"{"defaultModel":"stored"}"#)
    );
}

/// Resolves a synthetic stored default while accepting explicit edits.
struct Resolve;
impl SettingsDecider for Resolve {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, view: &mut maestro_settings::Settings) -> Vec<String> {
        if view.0.get("defaultModel") == Some(&json!("accepted")) {
            return Vec::new();
        }
        view.0.insert("defaultModel".into(), json!("accepted"));
        vec!["stored default resolved by configuration release".into()]
    }
}

/// Registration, edits, overrides and reload resolve only the effective view.
#[test]
fn sole_decider_resolves_rebuilds_without_rewriting_stored_values() {
    let storage = ControlledStorage::new();
    storage.inject(SettingsScope::Global, r#"{"defaultModel":"stored"}"#);
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.register_decider(Arc::new(Resolve)).unwrap();
    assert_eq!(manager.get_default_model().as_deref(), Some("accepted"));
    assert!(manager.register_decider(Arc::new(Refuse)).is_err());
    assert_resolution_diagnostic(&mut manager);
    manager.apply_overrides(settings(json!({"defaultModel":"runtime"})));
    assert_eq!(manager.get_default_model().as_deref(), Some("accepted"));
    assert_resolution_diagnostic(&mut manager);
    manager.set_quiet_startup(true);
    block_on(manager.flush());
    assert_eq!(manager.get_default_model().as_deref(), Some("accepted"));
    assert_eq!(manager.get_global_settings().0["defaultModel"], "stored");
    assert_eq!(storage.writes().len(), 1);
    assert_eq!(
        storage.text(SettingsScope::Global).unwrap(),
        "{\n  \"defaultModel\": \"stored\",\n  \"quietStartup\": true\n}"
    );
    manager.drain_errors();
    storage.inject(SettingsScope::Global, r#"{"defaultModel":"external"}"#);
    block_on(manager.reload());
    assert_eq!(manager.get_default_model().as_deref(), Some("accepted"));
    assert_resolution_diagnostic(&mut manager);
    assert_eq!(storage.writes().len(), 1);
}

/// Requires one successful stored-value resolution rather than a refusal.
fn assert_resolution_diagnostic(manager: &mut SettingsManager) {
    let errors = manager.drain_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind, SettingsErrorKind::StoredValueIgnored);
}

/// Attempts to replace a value that the base layer locks.
struct ReplaceLockedTheme;
impl SettingsDecider for ReplaceLockedTheme {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, view: &mut maestro_settings::Settings) -> Vec<String> {
        assert_eq!(view.0["theme"], json!("dark"));
        view.0.insert("theme".into(), json!("light"));
        Vec::new()
    }
}

/// Stored decisions cannot weaken a lock or rewrite either raw file.
#[test]
fn stored_decisions_preserve_locked_values_and_raw_files() {
    use maestro_settings::BaseSettingsLayer;
    let storage = ControlledStorage::new();
    let global = r#"{ "theme": "stored", "unknown": [false, 3] }"#;
    let project = r#"{"theme":"project","untouched":true}"#;
    storage.inject(SettingsScope::Global, global);
    storage.inject(SettingsScope::Project, project);
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.publish_base_layer(
        SettingsScope::Global,
        Some(BaseSettingsLayer {
            settings: settings(json!({"theme":"dark"})),
            locked: ["theme".into()].into(),
            source: "configuration-release".into(),
            recorded_at: "accepted".into(),
        }),
    );
    manager
        .register_decider(Arc::new(ReplaceLockedTheme))
        .unwrap();
    assert_eq!(manager.get_theme().as_deref(), Some("dark"));
    block_on(manager.reload());
    assert_eq!(manager.get_theme().as_deref(), Some("dark"));
    manager.apply_overrides(settings(json!({"quietStartup":true})));
    assert_eq!(manager.get_theme().as_deref(), Some("dark"));
    block_on(manager.flush());
    assert!(storage.writes().is_empty());
    assert_eq!(storage.text(SettingsScope::Global).as_deref(), Some(global));
    assert_eq!(
        storage.text(SettingsScope::Project).as_deref(),
        Some(project)
    );
}

/// Recomputed lists retain the admitted key and scope instead of granting new edits.
#[test]
fn recomputed_prepared_list_preserves_key_and_scope() {
    use maestro_settings::SettingsChange;
    for (scope, path, accepted) in [
        (SettingsScope::Global, "packages", true),
        (SettingsScope::Project, "packages", false),
        (SettingsScope::Global, "skills", false),
    ] {
        let mut manager = SettingsManager::from_storage(ControlledStorage::new());
        let changes = vec![SettingsChange {
            path: vec!["packages".into()],
            value: Some(json!(["npm:new"])),
        }];
        let request = DecisionRequest::SettingsEdit {
            scope: SettingsScope::Global,
            changes: &changes,
        };
        let edit = manager
            .prepare_operation(&request, SettingsScope::Global, changes.clone())
            .unwrap();
        let result =
            edit.with_recomputed_list(scope, &[path], vec![json!("npm:late"), json!("npm:new")]);
        assert_eq!(result.is_ok(), accepted);
        if let Ok(edit) = result {
            manager.apply_prepared_edit(edit);
            assert_eq!(
                manager.get_global_settings().0.get("packages"),
                Some(&json!(["npm:late", "npm:new"]))
            );
        } else {
            assert!(manager.get_global_settings().0.is_empty());
        }
    }
}

/// Numeric setters offer their normalized values and preserve typed NaN results.
#[test]
fn numeric_preference_admission_keeps_normalization_and_nan() {
    for (input, padding, height, width) in [
        (-1.5, json!(0.0), json!(3.0), json!(1.0)),
        (2.75, json!(2.0), json!(3.0), json!(2.0)),
        (50.5, json!(3.0), json!(20.0), json!(50.0)),
        (f64::NAN, json!(null), json!(null), json!(null)),
    ] {
        let mut manager = SettingsManager::in_memory(maestro_settings::Settings::default());
        manager.set_editor_padding_x(input);
        manager.set_autocomplete_max_visible(input);
        manager.set_image_width_cells(input);
        let raw = manager.get_global_settings().0;
        assert_eq!(raw["editorPaddingX"], padding);
        assert_eq!(raw["autocompleteMaxVisible"], height);
        assert_eq!(raw["terminal"]["imageWidthCells"], width);
        assert_eq!(manager.get_editor_padding_x().is_nan(), input.is_nan());
        assert_eq!(
            manager.get_autocomplete_max_visible().is_nan(),
            input.is_nan()
        );
        manager.register_decider(Arc::new(Refuse)).unwrap();
        manager.set_editor_padding_x(1.0);
        manager.set_autocomplete_max_visible(5.0);
        manager.set_image_width_cells(60.0);
        assert_eq!(manager.get_global_settings().0, raw);
        assert_eq!(manager.get_editor_padding_x().is_nan(), input.is_nan());
        assert_eq!(
            manager.get_autocomplete_max_visible().is_nan(),
            input.is_nan()
        );
    }
}
