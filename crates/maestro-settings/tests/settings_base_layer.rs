//! Read-only base preferences through the settings manager.
#![cfg(test)]

use crate::support::{
    ControlledStorage, assert_number, block_on, manager_with_both, raw_json, settings, text_entries,
};
use maestro_settings::{BaseSettingsLayer, SettingsManager, SettingsScope};
use serde_json::json;

/// A synthetic publisher's accepted document and release metadata.
fn layer(value: serde_json::Value, locked: &[&str]) -> BaseSettingsLayer {
    BaseSettingsLayer {
        settings: settings(value),
        locked: locked.iter().map(|key| (*key).to_owned()).collect(),
        source: "git:example/config#v1".into(),
        recorded_at: "2026-10-27T12:00:00Z".into(),
    }
}

/// The base uses the ordinary shallow merge without entering either raw scope.
#[test]
fn base_layer_uses_one_level_merge_without_changing_raw_scopes() {
    let (mut manager, storage) = manager_with_both(
        json!({"retry":{"maxRetries":4}, "extensions":[], "theme":null}),
        json!({"retry":{"baseDelayMs":7}, "defaultModel":"project"}),
    );
    let global = manager.get_global_settings();
    let project = manager.get_project_settings();
    let base = layer(
        json!({
            "retry":{"enabled":false,"maxRetries":2,"baseDelayMs":3},
            "extensions":["base"], "theme":"base", "defaultModel":"base",
            "defaultProvider":"base-provider"
        }),
        &[],
    );
    manager.publish_base_layer(SettingsScope::Global, Some(base.clone()));
    assert_eq!(manager.get_base_layer(), Some(&base));
    assert_eq!(
        manager.get_default_provider().as_deref(),
        Some("base-provider")
    );
    assert_eq!(manager.get_default_model().as_deref(), Some("project"));
    assert_eq!(manager.get_theme(), None);
    assert!(manager.get_extension_paths().is_empty());
    assert_number(manager.get_retry_settings().max_retries, 4.0);
    assert_number(manager.get_retry_settings().base_delay_ms, 7.0);
    assert!(!manager.get_retry_enabled());
    manager.set_quiet_startup(true);
    block_on(manager.flush());
    assert_eq!(manager.get_project_settings(), project);
    assert_eq!(manager.get_global_settings().0["retry"], global.0["retry"]);
    assert_eq!(
        raw_json(&*storage, SettingsScope::Global),
        json!({
            "retry":{"maxRetries":4}, "extensions":[], "theme":null, "quietStartup":true
        })
    );
    assert_eq!(
        manager.get_default_provider().as_deref(),
        Some("base-provider")
    );
}

/// Locks restore base leaves, including absence, without rewriting stored conflicts.
#[test]
fn stored_locks_restore_base_values_and_engine_defaults() {
    let (mut manager, storage) = manager_with_both(
        json!({"defaultModel":"user","compaction":{"enabled":true}}),
        json!({"defaultModel":"project","compaction":null,"retry":{"maxRetries":9}}),
    );
    let before = [
        raw_json(&*storage, SettingsScope::Global),
        raw_json(&*storage, SettingsScope::Project),
    ];
    manager.publish_base_layer(
        SettingsScope::Global,
        Some(layer(
            json!({"defaultModel":"base","compaction":{"enabled":false}}),
            &["defaultModel", "compaction.enabled", "retry.maxRetries"],
        )),
    );
    assert_eq!(manager.get_default_model().as_deref(), Some("base"));
    assert!(!manager.get_compaction_enabled());
    assert_number(manager.get_retry_settings().max_retries, 3.0);
    let errors = manager.drain_errors();
    assert!(
        errors
            .iter()
            .any(|error| error.error.to_string().contains("defaultModel"))
    );
    assert!(
        errors
            .iter()
            .all(|error| error.error.to_string().contains("git:example/config#v1"))
    );
    block_on(manager.flush());
    assert_eq!(raw_json(&*storage, SettingsScope::Global), before[0]);
    assert_eq!(raw_json(&*storage, SettingsScope::Project), before[1]);
}

/// Every edit route refuses the entire request before memory or persistence changes.
#[test]
fn refused_edits_leave_overrides_pending_edits_and_files_unchanged() {
    let storage = ControlledStorage::new();
    storage.inject(
        SettingsScope::Global,
        r#"{"defaultModel":"stored","editorPaddingX":2}"#,
    );
    let mut manager = SettingsManager::from_storage(storage.clone());
    manager.publish_base_layer(SettingsScope::Global, Some(layer(json!({
        "defaultModel":"base", "compaction":{"enabled":false}, "extensions":[], "editorPaddingX":1
    }), &["defaultModel", "compaction.enabled", "extensions", "editorPaddingX", "autocompleteMaxVisible"])));
    block_on(manager.flush());
    manager.drain_errors();
    manager.apply_overrides(settings(json!({"quietStartup":true})));
    let global = manager.get_global_settings();
    let project = manager.get_project_settings();
    let calls = storage.calls();
    for edit in refused_edits() {
        edit(&mut manager);
        block_on(manager.flush());
        assert_eq!(manager.get_global_settings(), global);
        assert_eq!(manager.get_project_settings(), project);
        assert!(manager.get_quiet_startup());
        assert_number(manager.get_editor_padding_x(), 1.0);
        assert_number(manager.get_autocomplete_max_visible(), 5.0);
        assert_eq!(storage.calls(), calls);
        assert!(storage.writes().is_empty());
        let errors = manager.drain_errors();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0]
                .error
                .to_string()
                .contains("git:example/config#v1")
        );
    }
    storage.inject(
        SettingsScope::Global,
        r#"{"defaultModel":"external","editorPaddingX":2}"#,
    );
    manager.set_global_key_path("compaction", &["reserveTokens"], Some(json!(12)));
    block_on(manager.flush());
    assert_number(manager.get_compaction_reserve_tokens(), 12.0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&storage.text(SettingsScope::Global).unwrap())
            .unwrap(),
        json!({"defaultModel":"external","editorPaddingX":2,"compaction":{"reserveTokens":12}})
    );
}

/// Fresh managers choose only their project's saved layer; clearing reveals global.
#[test]
fn persisted_layers_are_isolated_by_configuration_and_project_root() {
    use crate::support::{Scratch, location};
    let root = Scratch::new().unwrap();
    let agent = location(root.path(), "agent");
    let a = location(root.path(), "a");
    let b = location(root.path(), "b");
    let global = layer(json!({"defaultModel":"global"}), &["defaultModel"]);
    let project = layer(json!({"defaultModel":"project"}), &["defaultModel"]);
    let mut manager = SettingsManager::create(&a, &agent, ".maestro");
    manager.publish_base_layer(SettingsScope::Global, Some(global.clone()));
    manager.publish_base_layer(SettingsScope::Project, Some(project.clone()));
    block_on(manager.flush());
    assert!(manager.drain_errors().is_empty());
    let fresh = SettingsManager::create(&a, &agent, ".maestro");
    assert_eq!(fresh.get_base_layer(), Some(&project));
    let other = SettingsManager::create(&b, &agent, ".maestro");
    assert_eq!(other.get_base_layer(), Some(&global));
    let isolated = SettingsManager::create(&a, &location(root.path(), "other-agent"), ".maestro");
    assert_eq!(isolated.get_base_layer(), None);
    assert!(!root.path().join("a").exists());
    assert!(!root.path().join("b").exists());
    manager.publish_base_layer(SettingsScope::Project, None);
    block_on(manager.flush());
    assert_eq!(
        SettingsManager::create(&a, &agent, ".maestro").get_base_layer(),
        Some(&global)
    );
    manager.publish_base_layer(SettingsScope::Global, None);
    block_on(manager.flush());
    assert_eq!(
        SettingsManager::create(&a, &agent, ".maestro").get_base_layer(),
        None
    );
}

/// A failed clear can be retried after storage recovers, without retaining stale locks.
#[test]
fn failed_base_layer_removal_can_be_retried() {
    use crate::support::{Scratch, location};
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let root = Scratch::new().unwrap();
        let agent = location(root.path(), "agent");
        let work = location(root.path(), "work");
        let mut manager = SettingsManager::create(&work, &agent, ".maestro");
        let base = layer(json!({"defaultModel":"base"}), &["defaultModel"]);
        manager.publish_base_layer(scope, Some(base.clone()));
        block_on(manager.flush());
        assert!(manager.drain_errors().is_empty());
        let record = root.path().join(match scope {
            SettingsScope::Global => "agent/base-settings.json",
            SettingsScope::Project => "agent/state/base-settings.json",
        });
        let saved = record.with_extension("saved");
        std::fs::rename(&record, &saved).unwrap();
        std::fs::create_dir(&record).unwrap();
        manager.publish_base_layer(scope, None);
        block_on(manager.flush());
        assert_eq!(manager.get_base_layer(), None);
        assert!(!manager.is_key_path_locked("defaultModel", &[]));
        let errors = manager.drain_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].scope, scope);
        std::fs::remove_dir(&record).unwrap();
        std::fs::rename(&saved, &record).unwrap();
        let stale = SettingsManager::create(&work, &agent, ".maestro");
        assert_eq!(stale.get_base_layer(), Some(&base));
        assert!(stale.is_key_path_locked("defaultModel", &[]));
        manager.publish_base_layer(scope, None);
        block_on(manager.flush());
        assert!(manager.drain_errors().is_empty());
        let fresh = SettingsManager::create(&work, &agent, ".maestro");
        assert_eq!(fresh.get_base_layer(), None);
        assert!(!fresh.is_key_path_locked("defaultModel", &[]));
    }
}

/// A refused parent lookup reports the failed clear and allows retry after recovery.
#[cfg(unix)]
#[test]
fn inaccessible_base_layer_removal_reports_error_and_can_be_retried() {
    use crate::support::{Scratch, location};
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        let root = Scratch::new().unwrap();
        let agent = location(root.path(), "agent");
        let work = location(root.path(), "work");
        let mut manager = SettingsManager::create(&work, &agent, ".maestro");
        let base = layer(json!({"defaultModel":"base"}), &["defaultModel"]);
        manager.publish_base_layer(scope, Some(base.clone()));
        block_on(manager.flush());
        assert!(manager.drain_errors().is_empty());
        let parent = root.path().join(match scope {
            SettingsScope::Global => "agent",
            SettingsScope::Project => "agent/state",
        });
        let record = parent.join("base-settings.json");
        let saved = std::fs::read(&record).unwrap();
        let Some(restore) = maestro_test_directory::deny(&parent, 0o600).unwrap() else {
            continue;
        };
        manager.publish_base_layer(scope, None);
        block_on(manager.flush());
        assert_eq!(manager.get_base_layer(), None);
        assert!(!manager.is_key_path_locked("defaultModel", &[]));
        let errors = manager.drain_errors();
        assert_eq!(
            errors.len(),
            1,
            "{scope:?}: clearing inaccessible storage must fail"
        );
        assert_eq!(errors[0].scope, scope);
        assert_eq!(
            errors[0]
                .error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        drop(restore);
        assert_eq!(std::fs::read(&record).unwrap(), saved);
        let stale = SettingsManager::create(&work, &agent, ".maestro");
        assert_eq!(stale.get_base_layer(), Some(&base));
        assert!(stale.is_key_path_locked("defaultModel", &[]));
        manager.publish_base_layer(scope, None);
        block_on(manager.flush());
        assert!(manager.drain_errors().is_empty());
        let fresh = SettingsManager::create(&work, &agent, ".maestro");
        assert_eq!(fresh.get_base_layer(), None);
        assert!(!fresh.is_key_path_locked("defaultModel", &[]));
    }
}

/// Clearing an absent record creates neither directories nor record or lock files.
#[test]
fn clearing_missing_base_layers_leaves_storage_untouched() {
    use crate::support::{Scratch, file_storage, location};
    use maestro_settings::SettingsStorage;
    let root = Scratch::new().unwrap();
    let storage = file_storage(root.path());
    let mut manager = SettingsManager::create(
        &location(root.path(), "work"),
        &location(root.path(), "agent"),
        ".maestro",
    );
    for scope in [SettingsScope::Global, SettingsScope::Project] {
        manager.publish_base_layer(scope, None);
        block_on(manager.flush());
        assert!(manager.drain_errors().is_empty());
        storage.write_base_layer(scope, None).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

/// Reload retains the layer and failed scopes, while successful loads discard overrides.
#[test]
fn reload_keeps_base_locks_and_independent_failed_scope_rules() {
    let storage = ControlledStorage::new();
    storage.inject(
        SettingsScope::Global,
        r#"{"theme":"old","defaultModel":"stored"}"#,
    );
    storage.inject(SettingsScope::Project, r#"{"quietStartup":true}"#);
    let mut manager = SettingsManager::from_storage(storage.clone());
    let base = layer(json!({"defaultModel":"base"}), &["defaultModel"]);
    manager.publish_base_layer(SettingsScope::Global, Some(base.clone()));
    block_on(manager.flush());
    manager.drain_errors();
    manager.apply_overrides(settings(json!({"theme":"runtime"})));
    storage.inject(SettingsScope::Global, "invalid");
    storage.inject(SettingsScope::Project, r#"{"quietStartup":false}"#);
    block_on(manager.reload());
    assert_eq!(manager.get_base_layer(), Some(&base));
    assert_eq!(manager.get_default_model().as_deref(), Some("base"));
    assert_eq!(manager.get_theme().as_deref(), Some("old"));
    assert!(!manager.get_quiet_startup());
    assert!(
        manager
            .drain_errors()
            .iter()
            .any(|error| error.scope == SettingsScope::Global)
    );
    manager.set_theme("pending".into());
    assert!(storage.writes().is_empty());
    storage.inject(SettingsScope::Global, r#"{"theme":"fresh"}"#);
    storage.inject(SettingsScope::Project, "invalid");
    block_on(manager.reload());
    assert_eq!(manager.get_theme().as_deref(), Some("fresh"));
    assert!(!manager.get_quiet_startup());
    assert_eq!(manager.get_default_model().as_deref(), Some("base"));
}

/// Queries match literal components and include both ancestors and descendants.
#[test]
fn lock_queries_cover_ancestors_descendants_and_not_siblings() {
    let (mut manager, _) = manager_with_both(json!({}), json!({}));
    assert!(!manager.is_key_path_locked("retry", &[]));
    manager.publish_base_layer(
        SettingsScope::Global,
        Some(layer(json!({}), &["retry.provider.timeoutMs"])),
    );
    for keys in [
        vec![],
        vec!["provider"],
        vec!["provider", "timeoutMs"],
        vec!["provider", "timeoutMs", "child"],
    ] {
        assert!(manager.is_key_path_locked("retry", &keys));
    }
    assert!(!manager.is_key_path_locked("retry", &["maxRetries"]));
    assert!(!manager.is_key_path_locked("retry", &["provider", "timeout"]));
    assert!(!manager.is_key_path_locked("retry.provider.timeoutMs", &[]));
}

/// Explicit requests that each address at least one locked preference.
fn refused_edits() -> [fn(&mut SettingsManager); 10] {
    [
        |m| m.set_default_model("other".into()),
        |m| m.set_default_model_and_provider("other".into(), "other".into()),
        |m| m.set_compaction_enabled(true),
        |m| m.set_global_key_path("compaction", &[], None),
        |m| {
            m.set_global_key_paths(
                "compaction",
                &[
                    (&["reserveTokens"], Some(json!(2))),
                    (&["enabled"], Some(json!(true))),
                ],
            );
        },
        |m| {
            m.apply_overrides(settings(
                json!({"defaultModel":"other","quietStartup":false}),
            ));
        },
        |m| m.apply_overrides(settings(json!({"compaction":null}))),
        |m| m.set_project_extension_paths(text_entries(&["other"])),
        |m| m.set_editor_padding_x(f64::NAN),
        |m| m.set_autocomplete_max_visible(f64::NAN),
    ]
}
