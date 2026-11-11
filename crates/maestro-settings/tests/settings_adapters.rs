//! The adapter chosen for each port through the settings manager.
#![cfg(test)]

use crate::support::{block_on, manager, manager_with, manager_with_both, settings};
use maestro_settings::{BaseSettingsLayer, SettingsScope};
use serde_json::json;

/// A project value overrides the global one, a wrong type reads as unset and ports are independent.
#[test]
fn adapter_reads_the_effective_name_for_each_port() {
    let (mut manager, _storage) = manager_with_both(
        json!({"adapters": {"vector-store": "embedded", "embedder": 7}}),
        json!({"adapters": {"vector-store": "server"}}),
    );
    assert_eq!(
        manager.get_adapter("vector-store").as_deref(),
        Some("server")
    );
    assert_eq!(manager.get_adapter("embedder"), None);
    assert_eq!(manager.get_adapter("graph-store"), None);
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["graph-store"],
        Some(json!("local")),
    );
    block_on(manager.flush());
    assert_eq!(manager.get_adapter("graph-store").as_deref(), Some("local"));
    assert_eq!(
        manager.get_adapter("vector-store").as_deref(),
        Some("server")
    );
}

/// Without any `adapters` object every port reads as unset.
#[test]
fn adapter_is_unset_without_the_adapters_object() {
    let (manager, _storage) = manager();
    assert_eq!(manager.get_adapter("vector-store"), None);
    let (manager, _storage) = manager_with(json!({"adapters": "embedded"}));
    assert_eq!(manager.get_adapter("vector-store"), None);
}

/// A lock on one adapter key keeps the base name and leaves other ports editable.
#[test]
fn locked_adapter_key_keeps_the_base_name_and_refuses_edits() {
    let (mut manager, _storage) = manager_with(json!({"adapters": {"vector-store": "server"}}));
    manager.publish_base_layer(
        SettingsScope::Global,
        Some(BaseSettingsLayer {
            settings: settings(json!({"adapters": {"vector-store": "embedded"}})),
            locked: ["adapters.vector-store".to_owned()].into(),
            source: "git:example/config#v1".into(),
            recorded_at: "2026-10-27T12:00:00Z".into(),
        }),
    );
    assert_eq!(
        manager.get_adapter("vector-store").as_deref(),
        Some("embedded")
    );
    manager.drain_errors();
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["vector-store"],
        Some(json!("server")),
    );
    let refusal = manager.drain_errors();
    assert_eq!(refusal.len(), 1);
    assert!(
        refusal[0]
            .error
            .to_string()
            .contains("adapters.vector-store")
    );
    assert_eq!(
        manager.get_adapter("vector-store").as_deref(),
        Some("embedded")
    );
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["embedder"],
        Some(json!("bge")),
    );
    assert_eq!(manager.get_adapter("embedder").as_deref(), Some("bge"));
}
