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

/// A project value overrides the global one and ports are independent.
#[test]
fn adapter_reads_the_effective_name_for_each_port() {
    let (mut manager, _storage) = manager_with_both(
        json!({"adapters": {"vector-store": "embedded"}}),
        json!({"adapters": {"vector-store": "server"}}),
    );
    assert_eq!(adapter(&manager, "vector-store").as_deref(), Some("server"));
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

/// A value that is not a name, or an `adapters` entry that is not an object, is refused.
#[test]
fn malformed_adapter_setting_is_refused_with_its_key_and_value() {
    for (document, message) in [
        (
            json!({"adapters": {"embedder": 7}}),
            "Invalid adapters.embedder setting: 7",
        ),
        (
            json!({"adapters": "embedded"}),
            "Invalid adapters setting: \"embedded\"",
        ),
    ] {
        let (manager, _storage) = manager_with(document);
        let refusal = manager
            .get_adapter("embedder")
            .expect_err("a malformed setting is refused");
        assert_eq!(refusal.to_string(), message);
    }
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
        adapter(&manager, "vector-store").as_deref(),
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
        adapter(&manager, "vector-store").as_deref(),
        Some("embedded")
    );
    manager.set_key_path(
        SettingsScope::Project,
        "adapters",
        &["embedder"],
        Some(json!("bge")),
    );
    assert_eq!(adapter(&manager, "embedder").as_deref(), Some("bge"));
}
