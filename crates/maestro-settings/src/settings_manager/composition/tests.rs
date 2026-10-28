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

/// Resolves presentation values without editing their raw nonfinite preferences.
struct ResolvePresentation;
impl SettingsDecider for ResolvePresentation {
    fn decide(&self, _: &DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }

    fn resolve(&self, view: &mut Settings) -> Vec<String> {
        match view.0.get("resolution").and_then(serde_json::Value::as_str) {
            Some("finite") => {
                view.0.insert("editorPaddingX".into(), json!(2));
                view.0.insert("autocompleteMaxVisible".into(), json!(8));
            }
            Some("remove") => {
                view.0.shift_remove("editorPaddingX");
                view.0.shift_remove("autocompleteMaxVisible");
            }
            _ => {}
        }
        Vec::new()
    }
}

/// Resolver replacements and removals supersede typed NaN, including runtime composition.
#[test]
fn resolved_presentation_values_supersede_nonfinite_metadata() {
    for resolution in ["finite", "remove", "unchanged"] {
        for runtime in [false, true] {
            let mut manager = SettingsManager::in_memory(Settings::default());
            manager.set_editor_padding_x(f64::NAN);
            manager.set_autocomplete_max_visible(f64::NAN);
            let overrides = Settings(
                json!({"resolution": resolution})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
            if !runtime {
                manager
                    .global
                    .settings
                    .insert("resolution".into(), json!(resolution));
            }
            manager
                .register_decider(Arc::new(ResolvePresentation))
                .unwrap();
            if runtime {
                manager.apply_overrides(overrides);
            }
            match resolution {
                "finite" => {
                    assert!((manager.get_editor_padding_x() - 2.0).abs() < f64::EPSILON);
                    assert!((manager.get_autocomplete_max_visible() - 8.0).abs() < f64::EPSILON);
                }
                "remove" => {
                    assert!((manager.get_editor_padding_x() - 0.0).abs() < f64::EPSILON);
                    assert!((manager.get_autocomplete_max_visible() - 5.0).abs() < f64::EPSILON);
                }
                _ => {
                    assert!(manager.get_editor_padding_x().is_nan());
                    assert!(manager.get_autocomplete_max_visible().is_nan());
                }
            }
            assert_eq!(
                manager.get_global_settings().0["editorPaddingX"],
                json!(null)
            );
            assert_eq!(
                manager.get_global_settings().0["autocompleteMaxVisible"],
                json!(null)
            );
        }
    }
}
