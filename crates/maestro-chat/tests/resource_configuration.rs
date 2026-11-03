//! Resource configuration through its retained public component.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::keybindings;
#[path = "resource_configuration_support/mod.rs"]
mod resource_configuration_support;
use crate::selector;

#[test]
#[cfg(not(windows))]
fn resource_groups_keep_scope_labels_and_kind_order() {
    resource_configuration_support::group("resource_groups_keep_scope_labels_and_kind_order");
}

#[test]
#[cfg(not(windows))]
fn mixed_scopes_have_consistent_group_order() {
    resource_configuration_support::group("mixed_scopes_have_consistent_group_order");
}

#[test]
#[cfg(not(windows))]
fn resource_names_use_basename_rules_and_stable_locale_order() {
    resource_configuration_support::group(
        "resource_names_use_basename_rules_and_stable_locale_order",
    );
}

#[test]
#[cfg(not(windows))]
fn resource_navigation_skips_headings_and_clamps() {
    resource_configuration_support::group("resource_navigation_skips_headings_and_clamps");
}

#[test]
#[cfg(not(windows))]
fn resource_page_keys_use_flat_window_offsets() {
    resource_configuration_support::group("resource_page_keys_use_flat_window_offsets");
}

#[test]
#[cfg(not(windows))]
fn resource_filter_matches_name_kind_and_full_path() {
    resource_configuration_support::group("resource_filter_matches_name_kind_and_full_path");
}

#[test]
#[cfg(not(windows))]
fn resource_search_preserves_query_whitespace_semantics() {
    resource_configuration_support::group("resource_search_preserves_query_whitespace_semantics");
}

#[test]
#[cfg(not(windows))]
fn top_level_toggles_write_only_selected_scope_and_kind() {
    resource_configuration_support::group("top_level_toggles_write_only_selected_scope_and_kind");
}

#[test]
#[cfg(not(windows))]
fn resource_patterns_replace_exact_signed_entries_in_order() {
    resource_configuration_support::group(
        "resource_patterns_replace_exact_signed_entries_in_order",
    );
}

#[test]
#[cfg(not(windows))]
fn package_toggles_match_first_source_and_preserve_members() {
    resource_configuration_support::group(
        "package_toggles_match_first_source_and_preserve_members",
    );
}

#[test]
#[cfg(not(windows))]
fn package_patterns_use_supplied_base_or_parent_fallback() {
    resource_configuration_support::group("package_patterns_use_supplied_base_or_parent_fallback");
}

#[test]
#[cfg(not(windows))]
fn temporary_toggles_use_the_existing_user_scope_branch() {
    resource_configuration_support::group("temporary_toggles_use_the_existing_user_scope_branch");
}

#[test]
#[cfg(not(windows))]
fn toggle_notifications_observe_committed_rows_and_settings() {
    resource_configuration_support::group(
        "toggle_notifications_observe_committed_rows_and_settings",
    );
}

#[test]
#[cfg(not(windows))]
fn resource_callback_slots_are_replaceable_and_reentrant() {
    resource_configuration_support::group("resource_callback_slots_are_replaceable_and_reentrant");
}

#[test]
#[cfg(not(windows))]
fn item_updates_keep_shared_identity_and_filter_replay() {
    resource_configuration_support::group("item_updates_keep_shared_identity_and_filter_replay");
}

#[test]
#[cfg(not(windows))]
fn detached_item_updates_keep_first_matching_path_and_kind() {
    resource_configuration_support::group(
        "detached_item_updates_keep_first_matching_path_and_kind",
    );
}

#[test]
#[cfg(not(windows))]
fn config_selector_retains_its_focus_list_and_child_array() {
    resource_configuration_support::group("config_selector_retains_its_focus_list_and_child_array");
}

#[test]
#[cfg(not(windows))]
fn malformed_resource_lists_keep_unknown_values_while_toggling() {
    resource_configuration_support::group(
        "malformed_resource_lists_keep_unknown_values_while_toggling",
    );
}

#[test]
#[cfg(not(windows))]
fn malformed_package_sources_keep_unknown_data_while_toggling() {
    resource_configuration_support::group(
        "malformed_package_sources_keep_unknown_data_while_toggling",
    );
}

#[test]
#[cfg(not(windows))]
fn resource_scope_writes_survive_flush_and_reload() {
    resource_configuration_support::group("resource_scope_writes_survive_flush_and_reload");
}

#[test]
#[cfg(not(windows))]
fn resource_chrome_and_rows_keep_exact_text_and_visible_styles() {
    resource_configuration_support::group(
        "resource_chrome_and_rows_keep_exact_text_and_visible_styles",
    );
}

#[test]
#[cfg(not(windows))]
fn resource_input_uses_live_bindings_and_literal_space() {
    resource_configuration_support::group("resource_input_uses_live_bindings_and_literal_space");
}

#[test]
#[cfg(not(windows))]
fn resource_tabs_expand_without_losing_inverse_spaces() {
    resource_configuration_support::group("resource_tabs_expand_without_losing_inverse_spaces");
}

#[test]
#[cfg(windows)]
fn resource_windows_paths_keep_authored_relative_patterns() {
    resource_configuration_support::group("resource_windows_paths_keep_authored_relative_patterns");
}

#[test]
fn config_container_propagates_child_failures_and_invalidation() {
    resource_configuration_support::protocol::container();
}

#[test]
#[cfg(unix)]
fn resource_relative_paths_observe_live_directory_and_preserve_failures() {
    resource_configuration_support::native::paths();
}

#[test]
fn refused_resource_toggles_preserve_rows_files_and_callbacks() {
    resource_configuration_support::protocol::refused_toggles();
}
