//! Isolated composition through the same settings-owned constraints.
#![cfg(test)]

use crate::settings_manager::{
    BaseSettingsLayer, DecisionRequest, Settings, SettingsDecider, SettingsManager, SettingsScope,
};
use serde_json::json;
use std::sync::Arc;

/// Reads locked input, then attempts to replace it while resolving an ordinary value.
struct ResolveView;
impl SettingsDecider for ResolveView {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, view: &mut Settings) -> Vec<String> {
        assert_eq!(view.0["theme"], json!("dark"));
        view.0.insert("theme".into(), json!("light"));
        view.0.insert("resolved".into(), json!(true));
        Vec::new()
    }
}

/// A global-only view excludes project data and preserves locks after resolution.
#[test]
fn compose_effective_settings_resolves_global_without_project_values() {
    let mut manager = SettingsManager::in_memory(Settings(
        json!({"theme":"user","globalOnly":1})
            .as_object()
            .unwrap()
            .clone(),
    ));
    manager.project.settings = json!({"theme":"project","projectOnly":2})
        .as_object()
        .unwrap()
        .clone();
    let layer = BaseSettingsLayer {
        settings: Settings(json!({"theme":"dark"}).as_object().unwrap().clone()),
        locked: ["theme".into()].into(),
        source: "configuration-release".into(),
        recorded_at: "accepted".into(),
    };
    manager.publish_base_layer(SettingsScope::Global, Some(layer.clone()));
    manager.register_decider(Arc::new(ResolveView)).unwrap();
    let view = manager.compose_effective_settings(
        Some(&layer),
        layer.settings.0.clone(),
        &[(SettingsScope::Global, &manager.global.settings)],
    );
    assert_eq!(view["theme"], json!("dark"));
    assert_eq!(view["globalOnly"], json!(1));
    assert_eq!(view["resolved"], json!(true));
    assert!(!view.contains_key("projectOnly"));
    assert_eq!(manager.global.settings["theme"], json!("user"));
    assert_eq!(manager.project.settings["theme"], json!("project"));
}
