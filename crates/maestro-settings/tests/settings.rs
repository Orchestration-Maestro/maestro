mod support;
use maestro_settings::*;
use serde_json::json;
use support::*;

#[test]
fn precedence_retains_leaf_origins() {
    support::precedence_retains_leaf_origins(memory);
    support::precedence_retains_leaf_origins(controlled);
    support::precedence_retains_leaf_origins(file);
}

#[test]
fn recursive_objects_preserve_unmentioned_descendants() {
    support::recursive_objects_preserve_unmentioned_descendants(memory);
    support::recursive_objects_preserve_unmentioned_descendants(controlled);
    support::recursive_objects_preserve_unmentioned_descendants(file);
}

#[test]
fn scalars_arrays_and_null_replace() {
    support::scalars_arrays_and_null_replace(memory);
    support::scalars_arrays_and_null_replace(controlled);
    support::scalars_arrays_and_null_replace(file);
}

#[test]
fn resource_lists_replace_without_discovery() {
    support::resource_lists_replace_without_discovery(memory);
    support::resource_lists_replace_without_discovery(controlled);
    support::resource_lists_replace_without_discovery(file);
}

#[test]
fn literal_segments_do_not_split_keys() {
    support::literal_segments_do_not_split_keys(memory);
    support::literal_segments_do_not_split_keys(controlled);
    support::literal_segments_do_not_split_keys(file);
}

#[test]
fn manifest_locks_freeze_values_and_subtrees() {
    support::manifest_locks_freeze_values_and_subtrees(memory);
    support::manifest_locks_freeze_values_and_subtrees(controlled);
    support::manifest_locks_freeze_values_and_subtrees(file);
}

#[test]
fn equal_restatement_changes_origin_not_lock_authority() {
    support::equal_restatement_changes_origin_not_lock_authority(memory);
    support::equal_restatement_changes_origin_not_lock_authority(controlled);
    support::equal_restatement_changes_origin_not_lock_authority(file);
}

#[test]
fn locks_reject_ancestor_replacement_and_omission() {
    support::locks_reject_ancestor_replacement_and_omission(memory);
    support::locks_reject_ancestor_replacement_and_omission(controlled);
    support::locks_reject_ancestor_replacement_and_omission(file);
}

#[test]
fn array_locks_are_atomic() {
    support::array_locks_are_atomic(memory);
    support::array_locks_are_atomic(controlled);
    support::array_locks_are_atomic(file);
}

#[test]
fn invalid_and_missing_manifest_locks_fail() {
    support::invalid_and_missing_manifest_locks_fail(memory);
    support::invalid_and_missing_manifest_locks_fail(controlled);
    support::invalid_and_missing_manifest_locks_fail(file);
}

#[test]
fn masked_conflicts_fail_at_their_layer() {
    support::masked_conflicts_fail_at_their_layer(memory);
    support::masked_conflicts_fail_at_their_layer(controlled);
    support::masked_conflicts_fail_at_their_layer(file);
}

#[test]
fn all_caller_origins_use_the_same_guard() {
    support::all_caller_origins_use_the_same_guard(memory);
    support::all_caller_origins_use_the_same_guard(controlled);
    support::all_caller_origins_use_the_same_guard(file);
}

#[test]
fn rejected_updates_preserve_values_origins_and_storage() {
    support::rejected_updates_preserve_values_origins_and_storage(memory);
    support::rejected_updates_preserve_values_origins_and_storage(controlled);
    support::rejected_updates_preserve_values_origins_and_storage(file);
}

#[test]
fn ordinary_settings_cannot_change_lock_authority() {
    support::ordinary_settings_cannot_change_lock_authority(memory);
    support::ordinary_settings_cannot_change_lock_authority(controlled);
    support::ordinary_settings_cannot_change_lock_authority(file);
}

#[test]
fn settings_use_injected_storage_without_caller_policy() {
    caller_sequence(memory);
    caller_sequence(controlled);
    caller_sequence(file);
    let maps = std::sync::Arc::new(std::sync::Mutex::new([
        map(json!({"keep":1})),
        map(json!({"project":2})),
    ]));
    let fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut s = Settings::new(
        map(json!({})),
        ManifestSettings {
            values: map(json!({"frozen":1})),
            locks: vec![path(&["frozen"])],
        },
        Box::new(ControlledStorage {
            maps: maps.clone(),
            fail: fail.clone(),
        }),
    )
    .unwrap();
    let before = s.resolve();
    fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        s.reload(),
        Err(SettingsError::Storage {
            scope: SettingsScope::User
        })
    );
    assert_eq!(s.resolve(), before);
    assert_eq!(
        s.set(
            SettingsTarget::Stored(SettingsScope::User),
            &path(&["keep"]),
            json!(3)
        ),
        Err(SettingsError::Storage {
            scope: SettingsScope::Project
        })
    );
    assert_eq!(s.resolve(), before);
    assert_eq!(maps.lock().unwrap()[0], map(json!({"keep":1})));
    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    maps.lock().unwrap()[1] = map(json!({"frozen":2}));
    assert!(matches!(
        s.reload(),
        Err(SettingsError::LockConflict {
            attempted_by: SettingsOrigin::Project,
            ..
        })
    ));
    assert_eq!(s.resolve(), before);
}

#[test]
fn memory_initial_values_and_updates_survive_reload() {
    support::memory_initial_values_and_updates_survive_reload(memory);
    support::memory_initial_values_and_updates_survive_reload(controlled);
    support::memory_initial_values_and_updates_survive_reload(file);
}

#[test]
fn overrides_are_ephemeral_and_reload_restores_stored_values() {
    support::overrides_are_ephemeral_and_reload_restores_stored_values(memory);
    support::overrides_are_ephemeral_and_reload_restores_stored_values(controlled);
    support::overrides_are_ephemeral_and_reload_restores_stored_values(file);
}

#[test]
fn snapshots_are_detached() {
    support::snapshots_are_detached(memory);
    support::snapshots_are_detached(controlled);
    support::snapshots_are_detached(file);
}

#[test]
fn defaults_cover_omitted_explicit_and_disabled() {
    support::defaults_cover_omitted_explicit_and_disabled(memory);
    support::defaults_cover_omitted_explicit_and_disabled(controlled);
    support::defaults_cover_omitted_explicit_and_disabled(file);
}
