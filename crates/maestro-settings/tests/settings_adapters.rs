//! The adapter chosen for each port through the settings manager.
#![cfg(test)]

use crate::support::{block_on, manager, manager_with, manager_with_both, settings};
use maestro_settings::{BaseSettingsLayer, SettingsManager, SettingsScope};
use serde_json::json;

/// The adapter name a well-formed document selects for the port.
fn adapter(manager: &SettingsManager, port: &str) -> Option<String> {
    manager
        .get_adapter(port)
        .expect("a well-formed adapter setting")
}

/// A project value overrides the global one.
#[test]
fn project_adapter_overrides_the_global_adapter() {
    let (manager, _storage) = manager_with_both(
        json!({"adapters": {"vector-store": "embedded"}}),
        json!({"adapters": {"vector-store": "server"}}),
    );
    assert_eq!(adapter(&manager, "vector-store").as_deref(), Some("server"));
}

/// Ports are independent: setting one leaves the others as they were.
#[test]
fn adapter_setting_for_one_port_leaves_the_others_unchanged() {
    let (mut manager, _storage) = manager_with(json!({"adapters": {"vector-store": "server"}}));
    assert_eq!(adapter(&manager, "graph-store"), None);
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["graph-store"],
        Some(json!("local")),
    );
    block_on(manager.flush());
    assert_eq!(adapter(&manager, "graph-store").as_deref(), Some("local"));
    assert_eq!(adapter(&manager, "vector-store").as_deref(), Some("server"));
}

/// Without any `adapters` object every port reads as unset.
#[test]
fn adapter_is_unset_without_the_adapters_object() {
    let (manager, _storage) = manager();
    assert_eq!(adapter(&manager, "vector-store"), None);
}

/// A value that is not a name is refused with its key and value.
#[test]
fn adapter_value_that_is_not_a_name_is_refused_with_its_key_and_value() {
    let (manager, _storage) = manager_with(json!({"adapters": {"embedder": 7}}));
    let refusal = manager
        .get_adapter("embedder")
        .expect_err("a malformed setting is refused");
    assert_eq!(refusal.to_string(), "Invalid adapters.embedder setting: 7");
}

/// An `adapters` entry that is not an object is refused with its value.
#[test]
fn adapters_entry_that_is_not_an_object_is_refused_with_its_value() {
    let (manager, _storage) = manager_with(json!({"adapters": "embedded"}));
    let refusal = manager
        .get_adapter("embedder")
        .expect_err("a malformed setting is refused");
    assert_eq!(
        refusal.to_string(),
        "Invalid adapters setting: \"embedded\""
    );
}

/// A manager whose global base layer locks `adapters.vector-store` to `embedded`.
fn locked_manager() -> SettingsManager {
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
    manager.drain_errors();
    manager
}

/// A lock on one adapter key keeps the base name.
#[test]
fn locked_adapter_key_keeps_the_base_name() {
    let manager = locked_manager();
    assert_eq!(
        adapter(&manager, "vector-store").as_deref(),
        Some("embedded")
    );
}

/// An edit of a locked adapter key is refused and changes nothing.
#[test]
fn locked_adapter_key_refuses_edits() {
    let mut manager = locked_manager();
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
        adapter(&manager, "vector-store").as_deref(),
        Some("embedded")
    );
}

/// A lock on one adapter key leaves the other ports editable.
#[test]
fn locked_adapter_key_leaves_other_ports_editable() {
    let mut manager = locked_manager();
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["embedder"],
        Some(json!("bge")),
    );
    assert_eq!(adapter(&manager, "embedder").as_deref(), Some("bge"));
}
