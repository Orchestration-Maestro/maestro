//! Session-tree panel and label behavior through public components.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "tree_label_support/mod.rs"]
mod tree_label_support;

#[test]
fn panel_composes_borders_title_help_search_and_padding() {
    tree_label_support::group("panel_composes_borders_title_help_search_and_padding");
}

#[test]
fn label_editor_opens_from_raw_optional_labels() {
    tree_label_support::group("label_editor_opens_from_raw_optional_labels");
}

#[test]
fn label_timestamp_toggle_preserves_view_and_appends_status() {
    tree_label_support::group("label_timestamp_toggle_preserves_view_and_appends_status");
}

#[test]
fn panel_styles_flag_follows_the_toolkit_decoration_mode() {
    tree_label_support::group("panel_styles_flag_follows_the_toolkit_decoration_mode");
}

#[test]
fn panel_height_sets_the_list_page_distance() {
    tree_label_support::group("panel_height_sets_the_list_page_distance");
}

#[test]
fn panel_options_forward_selection_and_filter() {
    tree_label_support::group("panel_options_forward_selection_and_filter");
}

#[test]
fn panel_search_line_tracks_the_retained_query() {
    tree_label_support::group("panel_search_line_tracks_the_retained_query");
}

#[test]
fn panel_help_captures_keys_while_editor_hints_read_live_keys() {
    tree_label_support::group("panel_help_captures_keys_while_editor_hints_read_live_keys");
}

#[test]
fn panel_live_theme_updates_dynamic_children_not_cached_heading() {
    tree_label_support::group("panel_live_theme_updates_dynamic_children_not_cached_heading");
}

#[test]
fn label_save_trims_the_shared_whitespace_set() {
    tree_label_support::group("label_save_trims_the_shared_whitespace_set");
}

#[test]
fn label_cancel_keeps_metadata_and_does_not_notify() {
    tree_label_support::group("label_cancel_keeps_metadata_and_does_not_notify");
}

#[test]
fn label_save_without_callback_still_updates_then_closes() {
    tree_label_support::group("label_save_without_callback_still_updates_then_closes");
}

#[test]
fn label_callback_sees_updated_row_before_editor_closes() {
    tree_label_support::group("label_callback_sees_updated_row_before_editor_closes");
}

#[test]
fn label_callback_can_reenter_without_borrowing_conflicts() {
    tree_label_support::group("label_callback_can_reenter_without_borrowing_conflicts");
}

#[test]
fn label_callback_failure_leaves_editor_open_after_update() {
    tree_label_support::group("label_callback_failure_leaves_editor_open_after_update");
}

#[test]
fn label_editor_has_fresh_cursor_undo_kill_and_paste_on_reopen() {
    tree_label_support::group("label_editor_has_fresh_cursor_undo_kill_and_paste_on_reopen");
}

#[test]
fn label_edit_does_not_replace_query_selection_or_folds() {
    tree_label_support::group("label_edit_does_not_replace_query_selection_or_folds");
}

#[test]
fn label_focus_reaches_the_input_cursor_through_the_writer() {
    tree_label_support::group("label_focus_reaches_the_input_cursor_through_the_writer");
    tree_label_support::writer();
}

#[test]
fn label_editor_keys_take_priority_over_navigation_and_search() {
    tree_label_support::group("label_editor_keys_take_priority_over_navigation_and_search");
}

#[test]
fn label_editor_confirm_wins_a_binding_collision() {
    tree_label_support::group("label_editor_confirm_wins_a_binding_collision");
}

#[test]
fn label_editor_disabled_confirmation_keeps_editing() {
    tree_label_support::group("label_editor_disabled_confirmation_keeps_editing");
}

#[test]
fn label_update_changes_only_first_navigation_occurrence() {
    tree_label_support::group("label_update_changes_only_first_navigation_occurrence");
}

#[test]
fn label_update_missing_id_has_no_effect_or_clock_read() {
    tree_label_support::group("label_update_missing_id_has_no_effect_or_clock_read");
}

#[test]
fn label_update_uses_explicit_or_current_timestamp_and_clears_it() {
    tree_label_support::group("label_update_uses_explicit_or_current_timestamp_and_clears_it");
}

#[test]
fn label_update_does_not_rebuild_the_current_filtered_view() {
    tree_label_support::group("label_update_does_not_rebuild_the_current_filtered_view");
}

#[test]
fn label_timestamp_toggle_obeys_live_keys_and_action_priority() {
    let (_, definition) = maestro_app::presentation_data::keybindings::KEYBINDINGS
        .iter()
        .find(|(id, _)| id == "app.tree.toggleLabelTimestamp")
        .unwrap();
    assert_eq!(
        definition.default_keys,
        maestro_tui::KeybindingKeys::Single("shift+t".into())
    );
    assert_eq!(
        definition.description.as_deref(),
        Some("Toggle tree label timestamps")
    );
    tree_label_support::group("label_timestamp_toggle_obeys_live_keys_and_action_priority");
}

#[test]
fn label_timestamp_uses_local_calendar_branches() {
    if !maestro_test_process::local_zone_follows_tz() {
        return;
    }
    tree_label_support::group("label_timestamp_uses_local_calendar_branches");
}

#[test]
fn label_timestamp_respects_local_daylight_transitions() {
    if !maestro_test_process::local_zone_follows_tz() {
        return;
    }
    tree_label_support::group("label_timestamp_respects_local_daylight_transitions");
}

#[test]
fn label_timestamp_reads_the_clock_at_each_visible_render() {
    tree_label_support::group("label_timestamp_reads_the_clock_at_each_visible_render");
}

#[test]
fn label_clock_can_reenter_the_retained_list() {
    tree_label_support::group("label_clock_can_reenter_the_retained_list");
}

#[test]
fn label_timestamp_requires_truthy_label_and_timestamp() {
    tree_label_support::group("label_timestamp_requires_truthy_label_and_timestamp");
}

#[test]
fn label_timestamp_omits_unparseable_date_without_hiding_label() {
    tree_label_support::group("label_timestamp_omits_unparseable_date_without_hiding_label");
}

#[test]
fn label_timestamp_hidden_invalid_date_is_not_read() {
    tree_label_support::group("label_timestamp_hidden_invalid_date_is_not_read");
}

#[test]
fn empty_tree_cancels_at_one_hundred_milliseconds() {
    tree_label_support::group("empty_tree_cancels_at_one_hundred_milliseconds");
}

#[test]
fn filtered_empty_tree_never_schedules_constructor_cancel() {
    tree_label_support::group("filtered_empty_tree_never_schedules_constructor_cancel");
}

#[test]
fn panel_container_edits_do_not_replace_the_retained_list() {
    tree_label_support::group("panel_container_edits_do_not_replace_the_retained_list");
}

#[test]
fn panel_construction_preserves_reached_list_errors() {
    tree_label_support::error("panel_construction_preserves_reached_list_errors");
}

#[test]
fn panel_try_render_preserves_reached_row_errors() {
    tree_label_support::error("panel_try_render_preserves_reached_row_errors");
}
