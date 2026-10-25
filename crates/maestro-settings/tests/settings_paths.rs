//! Captured leaf edits preserve independently accepted sibling writes.
#![cfg(test)]

use crate::support::block_on;
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn independent_nested_writers_preserve_fresh_sibling_changes() {
    let root = crate::support::Scratch::new().unwrap();
    let stores: [maestro_settings::SettingsStorageHandle; 2] = [
        Arc::new(InMemorySettingsStorage::new()),
        Arc::new(crate::support::file_storage(root.path())),
    ];
    for storage in stores {
        let seed = json!({"multiplexer":{"toast":{"delivery":"off","clipboard":{"enabled":true}}},"other":"kept"});
        storage
            .with_lock(SettingsScope::Global, &mut |_| Ok(Some(seed.to_string())))
            .unwrap();
        let mut first = SettingsManager::from_storage(storage.clone());
        let mut second = SettingsManager::from_storage(storage.clone());
        first.set_global_key_path(
            "multiplexer",
            &["toast", "clipboard", "enabled"],
            Some(json!(false)),
        );
        block_on(first.flush());
        second.set_global_key_path(
            "multiplexer",
            &["toast", "delivery"],
            Some(json!("terminal")),
        );
        block_on(second.flush());
        let mut saved = None;
        storage
            .with_lock(SettingsScope::Global, &mut |text| {
                saved = Some(serde_json::from_str::<Value>(text.unwrap()).unwrap());
                Ok(None)
            })
            .unwrap();
        assert_eq!(
            saved.unwrap(),
            json!({"multiplexer":{"toast":{"delivery":"terminal","clipboard":{"enabled":false}}},"other":"kept"})
        );
        assert!(first.drain_errors().is_empty());
        assert!(second.drain_errors().is_empty());
    }
}

#[test]
fn nested_removal_preserves_fresh_siblings_and_absent_parents() {
    let storage = Arc::new(InMemorySettingsStorage::new());
    let seed = json!({"multiplexer":{"toast":{"enabled":true,"clipboard":{"enabled":true}}}});
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(seed.to_string())))
        .unwrap();
    let mut first = SettingsManager::from_storage(storage.clone());
    let mut second = SettingsManager::from_storage(storage);
    first.set_global_key_path(
        "multiplexer",
        &["toast", "clipboard", "enabled"],
        Some(json!(false)),
    );
    block_on(first.flush());
    second.set_global_key_path("multiplexer", &["toast", "enabled"], None);
    second.set_global_key_path("absent", &["parent", "leaf"], None);
    block_on(second.flush());
    block_on(second.reload());
    assert_eq!(
        second.get_global_settings().0,
        json!({"multiplexer":{"toast":{"clipboard":{"enabled":false}}}})
            .as_object()
            .unwrap()
            .clone()
    );
    assert!(second.drain_errors().is_empty());
}

#[test]
fn literal_paths_repair_only_edited_parents_and_retain_null_values() {
    let storage = Arc::new(InMemorySettingsStorage::new());
    let mut manager = SettingsManager::from_storage(storage);
    manager.set_global_key_path("multiplexer", &[], Some(json!(false)));
    manager.set_global_key_path(
        "multiplexer",
        &["toast", "clip.board", "enabled"],
        Some(Value::Null),
    );
    manager.set_global_key_path("other", &[], Some(json!({"kept":true})));
    block_on(manager.flush());
    block_on(manager.reload());
    assert_eq!(
        manager.get_global_settings().0["multiplexer"],
        json!({"toast":{"clip.board":{"enabled":null}}})
    );
    assert_eq!(
        manager.get_global_settings().0["other"],
        json!({"kept":true})
    );
    manager.set_global_key_path("multiplexer", &[], None);
    block_on(manager.flush());
    block_on(manager.reload());
    assert!(!manager.get_global_settings().0.contains_key("multiplexer"));
    assert_eq!(
        manager.get_global_settings().0["other"],
        json!({"kept":true})
    );
}

#[test]
fn whole_object_key_edits_still_replace_the_addressed_value() {
    let storage = Arc::new(InMemorySettingsStorage::new());
    let mut manager = SettingsManager::from_storage(storage);
    manager.set_global_key(
        "multiplexer",
        "toast",
        json!({"clipboard":true,"delivery":"off"}),
    );
    manager.set_global_key("multiplexer", "toast", json!({"delivery":"terminal"}));
    block_on(manager.flush());
    block_on(manager.reload());
    assert_eq!(
        manager.get_global_settings().0["multiplexer"]["toast"],
        json!({"delivery":"terminal"})
    );
}

#[test]
fn nested_batch_publishes_one_complete_storage_update() {
    let storage = crate::support::ControlledStorage::new();
    storage.inject(
        SettingsScope::Global,
        r#"{"multiplexer":{"theme":{"name":"catppuccin","autoSwitch":true}}}"#,
    );
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.set_global_key_paths(
        "multiplexer",
        &[
            (&["theme", "name"], Some(json!("gruvbox"))),
            (&["theme", "autoSwitch"], Some(json!(false))),
        ],
    );
    block_on(manager.flush());
    let writes = storage.writes();
    assert_eq!(
        writes.len(),
        1,
        "readers must not observe half of a settings change"
    );
    let saved: Value = serde_json::from_str(&writes[0].1).unwrap();
    assert_eq!(
        saved["multiplexer"]["theme"],
        json!({"name":"gruvbox","autoSwitch":false})
    );
}
