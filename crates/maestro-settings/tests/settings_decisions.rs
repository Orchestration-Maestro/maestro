//! Publisher decisions through the settings manager's public edit seam.
#![cfg(test)]

use crate::support::{ControlledStorage, assert_number, block_on, settings, text_entries};
use maestro_settings::{DecisionRequest, SettingsDecider, SettingsManager, SettingsScope};
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
