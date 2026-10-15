//! Application-selected settings qualification.
#![cfg(test)]
#![cfg(unix)]
/// Shared recorded queries and assertions.
#[path = "support/default_configuration.rs"]
pub mod fixture;

#[test]
fn external_enabled_models_survive_thinking_write() {
    fixture::settings(
        "external_enabled_models_survive_thinking_write",
        fixture::Group::ExternalEnabledModelsSurviveThinkingWrite,
    );
}

#[test]
fn external_custom_settings_survive_theme_write() {
    fixture::settings(
        "external_custom_settings_survive_theme_write",
        fixture::Group::ExternalCustomSettingsSurviveThemeWrite,
    );
}

#[test]
fn global_edits_win_over_external_same_field() {
    fixture::settings(
        "global_edits_win_over_external_same_field",
        fixture::Group::GlobalEditsWinOverExternalSameField,
    );
}

#[test]
fn local_extensions_remain_local() {
    fixture::settings(
        "local_extensions_remain_local",
        fixture::Group::LocalExtensionsRemainLocal,
    );
}

#[test]
fn filtered_package_records_remain_intact() {
    fixture::settings(
        "filtered_package_records_remain_intact",
        fixture::Group::FilteredPackageRecordsRemainIntact,
    );
}

#[test]
fn reload_accepts_external_global_changes() {
    fixture::settings(
        "reload_accepts_external_global_changes",
        fixture::Group::ReloadAcceptsExternalGlobalChanges,
    );
}

#[test]
fn reload_retains_previous_invalid_scope() {
    fixture::settings(
        "reload_retains_previous_invalid_scope",
        fixture::Group::ReloadRetainsPreviousInvalidScope,
    );
}

#[test]
fn load_errors_drain_once_for_both_scopes() {
    fixture::settings(
        "load_errors_drain_once_for_both_scopes",
        fixture::Group::LoadErrorsDrainOnceForBothScopes,
    );
}

#[test]
fn reading_missing_project_config_creates_nothing() {
    fixture::settings(
        "reading_missing_project_config_creates_nothing",
        fixture::Group::ReadingMissingProjectConfigCreatesNothing,
    );
}

#[test]
fn writing_project_config_creates_selected_directory() {
    fixture::settings(
        "writing_project_config_creates_selected_directory",
        fixture::Group::WritingProjectConfigCreatesSelectedDirectory,
    );
}

#[test]
fn shell_prefix_loads_from_selected_root() {
    fixture::settings(
        "shell_prefix_loads_from_selected_root",
        fixture::Group::ShellPrefixLoadsFromSelectedRoot,
    );
}

#[test]
fn missing_shell_prefix_stays_absent() {
    fixture::settings(
        "missing_shell_prefix_stays_absent",
        fixture::Group::MissingShellPrefixStaysAbsent,
    );
}

#[test]
fn shell_prefix_survives_unrelated_save() {
    fixture::settings(
        "shell_prefix_survives_unrelated_save",
        fixture::Group::ShellPrefixSurvivesUnrelatedSave,
    );
}

#[test]
fn missing_session_directory_stays_absent() {
    fixture::settings(
        "missing_session_directory_stays_absent",
        fixture::Group::MissingSessionDirectoryStaysAbsent,
    );
}

#[test]
fn global_session_directory_is_returned() {
    fixture::settings(
        "global_session_directory_is_returned",
        fixture::Group::GlobalSessionDirectoryIsReturned,
    );
}

#[test]
fn project_session_directory_overrides_global() {
    fixture::settings(
        "project_session_directory_overrides_global",
        fixture::Group::ProjectSessionDirectoryOverridesGlobal,
    );
}

#[test]
fn session_directory_tilde_uses_isolated_home() {
    fixture::settings(
        "session_directory_tilde_uses_isolated_home",
        fixture::Group::SessionDirectoryTildeUsesIsolatedHome,
    );
}

#[test]
fn nested_scope_merge_and_writes_preserve_unedited_members() {
    fixture::settings(
        "nested_scope_merge_and_writes_preserve_unedited_members",
        fixture::Group::NestedScopeMergeAndWritesPreserveUneditedMembers,
    );
}

#[test]
fn defaults_and_owned_reads_survive_application_root_selection() {
    fixture::settings(
        "defaults_and_owned_reads_survive_application_root_selection",
        fixture::Group::DefaultsAndOwnedReadsSurviveApplicationRootSelection,
    );
}

#[test]
fn empty_session_and_shell_values_remain_present() {
    fixture::settings(
        "empty_session_and_shell_values_remain_present",
        fixture::Group::EmptySessionAndShellValuesRemainPresent,
    );
}

#[test]
fn session_directory_path_boundaries_use_the_existing_owner() {
    fixture::settings(
        "session_directory_path_boundaries_use_the_existing_owner",
        fixture::Group::SessionDirectoryPathBoundariesUseTheExistingOwner,
    );
}

#[test]
fn invalid_scope_reload_preserves_other_healthy_scope() {
    fixture::settings(
        "invalid_scope_reload_preserves_other_healthy_scope",
        fixture::Group::InvalidScopeReloadPreservesOtherHealthyScope,
    );
}

#[test]
fn typed_wrong_values_fall_back_without_rewriting_raw_settings() {
    fixture::settings(
        "typed_wrong_values_fall_back_without_rewriting_raw_settings",
        fixture::Group::TypedWrongValuesFallBackWithoutRewritingRawSettings,
    );
}
