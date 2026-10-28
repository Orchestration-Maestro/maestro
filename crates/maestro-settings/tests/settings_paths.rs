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

/// Scoped literal edits publish immediately without changing the other scope.
#[test]
fn scoped_paths_keep_null_removal_and_scope_isolation() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let storage = crate::support::ControlledStorage::new();
        let seed = r#"{"custom":{"literal.key":{"remove":1,"keep":2}}}"#;
        storage.inject(SettingsScope::Global, seed);
        storage.inject(SettingsScope::Project, seed);
        let manager = std::cell::RefCell::new(SettingsManager::from_storage(storage.clone()));
        manager
            .borrow_mut()
            .set_key_path(scope, "custom", &["literal.key", "remove"], None);
        manager.borrow_mut().set_key_path(
            scope,
            "custom",
            &["literal.key", "null"],
            Some(Value::Null),
        );
        manager
            .borrow_mut()
            .set_key_path(scope, "absent", &["parent", "leaf"], None);
        let flush = manager.borrow().flush();
        let accepted = match scope {
            SettingsScope::Global => manager.borrow().get_global_settings(),
            SettingsScope::Project => manager.borrow().get_project_settings(),
        };
        assert_eq!(
            accepted.0,
            json!({"custom":{"literal.key":{"keep":2,"null":null}}})
                .as_object()
                .unwrap()
                .clone()
        );
        block_on(flush);
        let other = match scope {
            SettingsScope::Global => SettingsScope::Project,
            SettingsScope::Project => SettingsScope::Global,
        };
        assert_eq!(storage.text(other).as_deref(), Some(seed));
        block_on(manager.borrow_mut().reload());
        let saved: Value = serde_json::from_str(&storage.text(scope).unwrap()).unwrap();
        assert_eq!(saved, Value::Object(accepted.0));
        assert!(manager.borrow_mut().drain_errors().is_empty());
    }
}

/// Independent scoped writers merge into fresh storage rather than stale snapshots.
#[test]
fn scoped_paths_preserve_fresh_document_members() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let storage = crate::support::ControlledStorage::new();
        storage.inject(scope, r#"{"custom":{"left":0,"right":0},"unknown":true}"#);
        let mut first = SettingsManager::from_storage(storage.clone());
        let mut second = SettingsManager::from_storage(storage.clone());
        first.set_key_path(scope, "custom", &["left"], Some(json!(1)));
        block_on(first.flush());
        second.set_key_path(scope, "custom", &["right"], Some(json!(2)));
        block_on(second.flush());
        assert_eq!(
            serde_json::from_str::<Value>(&storage.text(scope).unwrap()).unwrap(),
            json!({"custom":{"left":1,"right":2},"unknown":true})
        );
        second.set_key_path(scope, "custom", &[], None);
        block_on(second.flush());
        assert_eq!(
            serde_json::from_str::<Value>(&storage.text(scope).unwrap()).unwrap(),
            json!({"unknown":true})
        );
        assert!(first.drain_errors().is_empty());
        assert!(second.drain_errors().is_empty());
    }
}

/// Failed scoped writes leave accepted edits visible and report their scope once.
#[test]
fn scoped_paths_drain_persistence_failures() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let storage = crate::support::ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        storage.script_writes(&[false]);
        manager.set_key_path(scope, "theme", &[], Some(json!("accepted")));
        assert_eq!(manager.get_theme().as_deref(), Some("accepted"));
        block_on(manager.flush());
        assert!(storage.text(scope).is_none());
        let errors = manager.drain_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].scope, scope);
        assert_eq!(errors[0].error.to_string(), "controlled failure");
        assert!(manager.drain_errors().is_empty());
        manager.set_key_path(scope, "custom", &[], Some(json!(true)));
        block_on(manager.flush());
        assert_eq!(
            serde_json::from_str::<Value>(&storage.text(scope).unwrap()).unwrap(),
            json!({"theme":"accepted","custom":true})
        );
    }
}

/// Scoped edits cannot bypass the base layer's locked-path refusal.
#[test]
fn scoped_paths_refuse_locked_preferences() {
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let storage = crate::support::ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        manager.publish_base_layer(
            SettingsScope::Global,
            Some(maestro_settings::BaseSettingsLayer {
                settings: crate::support::settings(json!({"custom":{"locked":1}})),
                locked: ["custom.locked".into()].into_iter().collect(),
                source: "config".into(),
                recorded_at: "now".into(),
            }),
        );
        block_on(manager.flush());
        manager.set_key_path(scope, "custom", &[], None);
        block_on(manager.flush());
        assert!(manager.get_global_settings().0.is_empty());
        assert!(manager.get_project_settings().0.is_empty());
        assert!(storage.writes().is_empty());
        let errors = manager.drain_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].scope, scope);
    }
}
