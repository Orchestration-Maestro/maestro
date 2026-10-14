//! Whole-frame session header and list behavior.
#![cfg(test)]
#[path = "support/session_picker.rs"]
mod session_picker;
#[test]
fn session_header_keeps_independent_constructor_policies() {
    session_picker::header_constructor("session_header_keeps_independent_constructor_policies");
}
#[test]
fn session_header_loading_progress_and_reset() {
    session_picker::header_loading("session_header_loading_progress_and_reset");
}
#[test]
fn session_header_setters_keep_path_and_rename_hints_independent() {
    session_picker::header("session_header_setters_keep_path_and_rename_hints_independent");
}

#[test]
fn session_header_rename_hint_follows_configuration() {
    session_picker::header("session_header_rename_hint_follows_configuration");
}

#[test]
fn session_header_hints_read_live_binding_order() {
    session_picker::header("session_header_hints_read_live_binding_order");
}

#[test]
fn session_header_confirmation_precedes_status_and_hints() {
    session_picker::header("session_header_confirmation_precedes_status_and_hints");
}

#[test]
fn session_header_clips_three_rows_with_cell_width() {
    session_picker::header("session_header_clips_three_rows_with_cell_width");
}

#[test]
fn session_status_expires_only_at_nonzero_caller_delay() {
    session_picker::header("session_status_expires_only_at_nonzero_caller_delay");
}

#[test]
fn session_status_replacement_cancels_previous_callback() {
    session_picker::header("session_status_replacement_cancels_previous_callback");
}
#[test]
fn session_age_uses_elapsed_time_boundaries() {
    session_picker::age("session_age_uses_elapsed_time_boundaries");
}

#[test]
fn session_paths_shorten_home_prefix_without_component_boundary() {
    session_picker::list(
        "session_paths_shorten_home_prefix_without_component_boundary",
        None,
    );
}

#[test]
fn session_threads_sort_stably_and_keep_connector_metadata() {
    session_picker::list(
        "session_threads_sort_stably_and_keep_connector_metadata",
        None,
    );
}

#[test]
fn session_named_filter_roots_children_with_excluded_parents() {
    session_picker::list(
        "session_named_filter_roots_children_with_excluded_parents",
        None,
    );
}

#[test]
fn session_query_selects_threaded_or_flat_existing_search() {
    session_picker::list(
        "session_query_selects_threaded_or_flat_existing_search",
        None,
    );
}

#[test]
fn session_list_setters_reapply_query_and_clamp_selection() {
    session_picker::list(
        "session_list_setters_reapply_query_and_clamp_selection",
        None,
    );
}

#[test]
fn session_empty_rows_keep_exact_scope_and_name_guidance() {
    session_picker::list(
        "session_empty_rows_keep_exact_scope_and_name_guidance",
        None,
    );
}

#[test]
fn session_message_normalization_preserves_name_presence_and_control_rules() {
    session_picker::list(
        "session_message_normalization_preserves_name_presence_and_control_rules",
        None,
    );
}

#[test]
fn session_row_clipping_keeps_cells_and_final_ellipsis() {
    session_picker::list("session_row_clipping_keeps_cells_and_final_ellipsis", None);
}

#[test]
fn session_window_centers_ten_rows_and_reports_scroll_position() {
    session_picker::list(
        "session_window_centers_ten_rows_and_reports_scroll_position",
        None,
    );
}

#[test]
fn session_navigation_clamps_and_routes_confirm_cancel_search() {
    session_picker::list(
        "session_navigation_clamps_and_routes_confirm_cancel_search",
        None,
    );
}

#[test]
fn session_list_routes_live_bindings_and_wrong_modifiers() {
    session_picker::list(
        "session_list_routes_live_bindings_and_wrong_modifiers",
        None,
    );
}

#[test]
fn session_navigation_binding_collision_prefers_up() {
    session_picker::list("session_navigation_binding_collision_prefers_up", None);
}

#[test]
fn session_callbacks_can_replace_clear_and_reenter() {
    session_picker::list("session_callbacks_can_replace_clear_and_reenter", None);
}

#[test]
fn session_render_reads_live_theme_and_invalidation_is_inert() {
    session_picker::list(
        "session_render_reads_live_theme_and_invalidation_is_inert",
        None,
    );
}

#[cfg(unix)]
#[test]
fn session_canonical_aliases_thread_and_mark_active() {
    session_picker::list("session_canonical_aliases_thread_and_mark_active", None);
}

#[test]
fn session_missing_canonical_paths_keep_authored_identity() {
    session_picker::list(
        "session_missing_canonical_paths_keep_authored_identity",
        None,
    );
}

#[test]
fn session_cyclic_parent_links_retain_every_record() {
    session_picker::list("session_cyclic_parent_links_retain_every_record", None);
}

#[cfg(unix)]
#[test]
fn session_canonical_duplicate_records_keep_original_metadata() {
    session_picker::list(
        "session_canonical_duplicate_records_keep_original_metadata",
        None,
    );
}

#[test]
fn session_list_setters_retain_nonempty_search() {
    session_picker::list("session_list_setters_retain_nonempty_search", None);
}

#[test]
fn session_input_submit_uses_live_selected_path() {
    session_picker::list("session_input_submit_uses_live_selected_path", None);
}

#[test]
fn session_parent_field_accepts_strings_without_coercing_values() {
    session_picker::list(
        "session_parent_field_accepts_strings_without_coercing_values",
        None,
    );
}

#[test]
fn session_list_constructor_keeps_sort_and_name_policies_independent() {
    session_picker::list(
        "session_list_constructor_keeps_sort_and_name_policies_independent",
        None,
    );
}

#[test]
fn session_empty_guidance_uses_only_live_enabled_actions() {
    session_picker::list(
        "session_empty_guidance_uses_only_live_enabled_actions",
        None,
    );
}

#[test]
fn session_bold_decorations_read_live_style_policy() {
    session_picker::list("session_bold_decorations_read_live_style_policy", None);
}
#[test]
fn session_status_owner_drop_cancels_pending_work() {
    session_picker::status_drop();
}
