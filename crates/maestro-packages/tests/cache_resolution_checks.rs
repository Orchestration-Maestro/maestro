//! Explicit remote source admission through supplied effects.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::cache_support;
/// Replays missing npm acquires in selected scope.
#[test]
fn missing_npm_acquires_in_selected_scope() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "npm_missing_user",
            "npm_missing_project",
            "npm_missing_temporary",
        ])
        .await;
    });
}

/// Replays missing git clones in selected scope.
#[test]
fn missing_git_clones_in_selected_scope() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "git_missing_user",
            "git_missing_project",
            "git_missing_temporary",
        ])
        .await;
    });
}

/// Replays installed unpinned packages avoid version probes.
#[test]
fn installed_unpinned_packages_avoid_version_probes() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "npm_existing_user",
            "npm_existing_project",
            "npm_existing_temporary",
        ])
        .await;
    });
}

/// Replays pinned npm uses literal version equality.
#[test]
fn pinned_npm_uses_literal_version_equality() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "pin_user_2.0.0",
            "pin_user_1.0.0",
            "pin_project_2.0.0",
            "pin_project_1.0.0",
            "pin_temporary_2.0.0",
            "pin_temporary_1.0.0",
        ])
        .await;
    });
}

/// Replays npm tags and ranges remain literal pins.
#[test]
fn npm_tags_and_ranges_remain_literal_pins() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "pin_literal_@scope/pkg@2.0.0",
            "pin_literal_pkg@^2.0.0",
            "pin_literal_pkg@latest",
            "pin_literal_pkg@beta",
            "pin_literal_pkg@~2.0.0",
            "pin_literal_pkg@2.x",
        ])
        .await;
    });
}

/// Replays pin manifest failures reinstall without registry queries.
#[test]
fn pin_manifest_failures_reinstall_without_registry_queries() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "pin_manifest_0_21",
            "pin_manifest_0_22",
            "pin_manifest_0_23",
            "pin_manifest_0_24",
            "pin_manifest_0_25",
            "pin_manifest_0_26",
            "pin_manifest_0_27",
            "pin_manifest_0_28",
            "pin_manifest_0_29",
            "pin_manifest_0_30",
            "pin_manifest_0_31",
            "pin_manifest_0_32",
            "pin_manifest_0_33",
            "pin_manifest_missing",
            "pin_manifest_directory",
        ])
        .await;
    });
}

/// Replays installed versions are not trimmed.
#[test]
fn installed_versions_are_not_trimmed() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "pin_version_36",
            "pin_version_37",
            "pin_version_38",
            "pin_version_39",
        ])
        .await;
    });
}

/// Replays source spec boundaries reuse shared parser.
#[test]
fn source_spec_boundaries_reuse_shared_parser() {
    crate::runtime::run(async {
        let mut cases = vec!["spec_40", "spec_41", "spec_42"];
        // The recorded newline in this case's cache directory name is a forbidden file name
        // on Windows, where the same boundary is covered by the other cases.
        if !cfg!(windows) {
            cases.push("spec_43");
        }
        cache_support::cases(&cases).await;
    });
}

/// Replays offline values gate missing acquisition without trimming.
#[test]
fn offline_values_gate_missing_acquisition_without_trimming() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "offline_npm_44",
            "offline_git_45",
            "offline_npm_46",
            "offline_git_47",
            "offline_npm_48",
            "offline_git_49",
            "offline_npm_50",
            "offline_git_51",
            "offline_npm_52",
            "offline_git_53",
            "offline_npm_54",
            "offline_git_55",
            "offline_npm_56",
            "offline_git_57",
            "offline_npm_58",
            "offline_git_59",
            "offline_npm_60",
            "offline_git_61",
            "offline_npm_62",
            "offline_git_63",
            "offline_npm_64",
            "offline_git_65",
            "offline_npm_66",
            "offline_git_67",
        ])
        .await;
    });
}

/// Replays offline mismatched pins are skipped not returned.
#[test]
fn offline_mismatched_pins_are_skipped_not_returned() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "offline_pin_user",
            "offline_pin_project",
            "offline_pin_temporary",
        ])
        .await;
    });
}

/// Replays installed git outside temporary scope never refreshes.
#[test]
fn installed_git_outside_temporary_scope_never_refreshes() {
    crate::runtime::run(async {
        cache_support::cases(&["git_keep_user", "git_keep_project"]).await;
    });
}

/// Replays pinned git cache reuse and missing checkout keep refs.
#[test]
fn pinned_git_cache_reuse_and_missing_checkout_keep_refs() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "git_pin_existing_main",
            "git_pin_existing_feature/branch",
            "git_pin_existing_v1.0.0",
            "git_pin_missing_main",
            "git_pin_missing_feature/branch",
            "git_pin_missing_v1.0.0",
        ])
        .await;
    });
}

/// Replays offline cached git returns resources without progress.
#[test]
fn offline_cached_git_returns_resources_without_progress() {
    crate::runtime::run(async {
        cache_support::cases(&["git_offline_1", "git_offline_TRUE", "git_offline_yes"]).await;
    });
}

/// Replays temporary git refresh reuses targeted update pipeline.
#[test]
fn temporary_git_refresh_reuses_targeted_update_pipeline() {
    crate::runtime::run(async {
        cache_support::cases(&["git_changed"]).await;
    });
}

/// Replays unchanged git refresh preserves checkout and skips dependencies.
#[test]
fn unchanged_git_refresh_preserves_checkout_and_skips_dependencies() {
    crate::runtime::run(async {
        cache_support::cases(&["git_unchanged"]).await;
    });
}

/// Replays refresh failures returning the recovered cache.
#[test]
fn refresh_failures_return_recovered_cache() {
    crate::runtime::run(async {
        cache_support::cases(&[
            "git_fail_upstream",
            "git_fail_fetch",
            "git_fail_head",
            "git_fail_reset",
            "git_fail_clean",
            "git_fail_dependencies",
        ])
        .await;
    });
}

/// Replays refresh catches progress callback failures.
#[test]
fn refresh_catches_progress_callback_failures() {
    crate::runtime::run(async {
        cache_support::cases(&["callback_start", "callback_complete", "callback_error"]).await;
    });
}

/// Replays failed missing acquisition rejects before later sources.
#[test]
fn failed_missing_acquisition_rejects_before_later_sources() {
    crate::runtime::run(async {
        cache_support::cases(&["missing_fail_npm", "missing_fail_git"]).await;
    });
}

/// Replays missing resolution emits no install progress.
#[test]
fn missing_resolution_emits_no_install_progress() {
    crate::runtime::run(async {
        cache_support::cases(&["npm_no_progress"]).await;
    });
}

/// Replays git dependencies use live npm command after clone.
#[test]
fn git_dependencies_use_live_npm_command_after_clone() {
    crate::runtime::run(async {
        cache_support::cases(&["git_live_command"]).await;
    });
}

/// Replays temporary npm preserves existing root files.
#[test]
fn temporary_npm_preserves_existing_root_files() {
    crate::runtime::run(async {
        cache_support::cases(&["npm_root_files"]).await;
    });
}

/// Replays cache keys hash authored identity before path join.
#[test]
fn cache_keys_hash_authored_identity_before_path_join() {
    crate::runtime::run(async {
        cache_support::cases(&["key_99", "key_100", "key_101", "key_102", "key_103"]).await;
    });
}

/// Replays successful acquisition does not require new contents.
#[test]
fn successful_acquisition_does_not_require_new_contents() {
    crate::runtime::run(async {
        cache_support::cases(&["no_contents_npm", "no_contents_git"]).await;
    });
}

/// Replays successful install collects without rechecking version.
#[test]
fn successful_install_collects_without_rechecking_version() {
    crate::runtime::run(async {
        cache_support::cases(&["pin_after_install"]).await;
    });
}

/// Replays remote malformed resource lists are ignored after acquisition.
#[test]
fn remote_malformed_resource_lists_are_ignored_after_acquisition() {
    crate::runtime::run(async {
        cache_support::cases(&["remote_manifest_bad"]).await;
    });
}

/// Replays explicit source order keeps first resource metadata.
#[test]
fn explicit_source_order_keeps_first_resource_metadata() {
    crate::runtime::run(async {
        cache_support::cases(&["source_order", "source_order_authored_alias"]).await;
    });
}

/// Replays cache identity is shared across refs and managers.
#[test]
fn cache_identity_is_shared_across_refs_and_managers() {
    crate::runtime::run(async {
        cache_support::cases(&["cache_reuse"]).await;
    });
}

/// Replays local temporary resolution never reads remote state.
#[test]
fn local_temporary_resolution_never_reads_remote_state() {
    crate::runtime::run(async {
        cache_support::cases(&["local_temporary"]).await;
    });
}

/// Replays empty sources do not observe environment or commands.
#[test]
fn empty_sources_do_not_observe_environment_or_commands() {
    crate::runtime::run(async {
        cache_support::cases(&["empty"]).await;
    });
}

/// Replays root lookup failure wins before offline skip.
#[test]
fn root_lookup_failure_wins_before_offline_skip() {
    crate::runtime::run(async {
        cache_support::cases(&["root_error"]).await;
    });
}

/// Replays invalid npm command fails after cache preparation.
#[test]
fn invalid_npm_command_fails_after_cache_preparation() {
    crate::runtime::run(async {
        cache_support::cases(&["invalid_command"]).await;
    });
}

/// Replays temporary cache is not removed when manager is dropped.
#[test]
fn temporary_cache_is_not_removed_when_manager_is_dropped() {
    crate::runtime::run(async {
        cache_support::cases(&["cache_persists"]).await;
    });
}

/// A local Git remote changes cached files and a failed fetch retains the new checkout.
#[cfg(unix)]
#[test]
fn native_cached_git_refresh_changes_files_and_survives_failure() {
    cache_support::native::refresh();
}
