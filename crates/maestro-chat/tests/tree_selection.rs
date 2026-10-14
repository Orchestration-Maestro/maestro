//! Public session-tree navigation behavior.
#![cfg(test)]
mod support;

/// Runs the hidden model-leaf selection and ordered visible cycle.
#[test]
fn selects_visible_parent_of_model_leaf() {
    support::group("selects_visible_parent_of_model_leaf");
}

/// Hidden thinking changes retain their visible parent.
#[test]
fn selects_visible_parent_of_thinking_leaf() {
    support::group("selects_visible_parent_of_thinking_leaf");
}

/// User filtering selects the nearest visible user ancestor.
#[test]
fn user_filter_chooses_visible_parent() {
    support::group("user_filter_chooses_visible_parent");
}

/// Returning to default keeps the visible user selection.
#[test]
fn default_filter_keeps_last_visible_choice() {
    support::group("default_filter_keeps_last_visible_choice");
}

/// An empty label view retains the earlier selection.
#[test]
fn empty_label_filter_restores_selection() {
    support::group("empty_label_filter_restores_selection");
}

/// Repeated empty toggles do not erase the bookmark.
#[test]
fn repeated_empty_filters_keep_bookmark() {
    support::group("repeated_empty_filters_keep_bookmark");
}

/// Initial targets and modes retain their constructor precedence.
#[test]
fn initial_selection_obeys_explicit_target_and_filter() {
    support::group("initial_selection_obeys_explicit_target_and_filter");
}

/// Replays the attributed public constructor observations.
#[test]
fn missing_targets_fall_back_to_last_visible_row() {
    support::group("missing_targets_fall_back_to_last_visible_row");
}

/// Replays the attributed public constructor observations.
#[test]
fn active_subtrees_precede_stable_siblings() {
    support::group("active_subtrees_precede_stable_siblings");
}

/// Replays the attributed ordered filtering observations.
#[test]
fn active_roots_keep_nonactive_root_order() {
    support::group("active_roots_keep_nonactive_root_order");
}

/// Replays the attributed ordered filtering observations.
#[test]
fn repeated_id_occurrences_stay_in_navigation() {
    support::group("repeated_id_occurrences_stay_in_navigation");
}

/// Replays the attributed ordered filtering observations.
#[test]
fn five_modes_keep_their_entry_kinds() {
    support::group("five_modes_keep_their_entry_kinds");
}

/// Assistant content and stop reasons determine visibility before modes.
#[test]
fn assistant_visibility_uses_content_stop_reason_and_leaf() {
    support::group("assistant_visibility_uses_content_stop_reason_and_leaf");
}

/// Replays the attributed input and content observations.
#[test]
fn labeled_mode_tests_presence_not_text() {
    support::group("labeled_mode_tests_presence_not_text");
}

/// Replays the attributed input and content observations.
#[test]
fn whitespace_content_uses_shared_scalar_set() {
    support::group("whitespace_content_uses_shared_scalar_set");
}

/// Replays the attributed input and content observations.
#[test]
fn malformed_text_fails_only_when_read() {
    support::group("malformed_text_fails_only_when_read");
}

/// Replays the attributed input and content observations.
#[test]
fn navigation_wraps_and_pages_clamp() {
    support::group("navigation_wraps_and_pages_clamp");
}

/// Direct toggles and both cycle directions retain selection.
#[test]
fn filter_shortcuts_toggle_and_cycle() {
    support::group("filter_shortcuts_toggle_and_cycle");
}

/// Replays the attributed configurable input observations.
#[test]
fn configured_keys_replace_defaults_and_keep_priority() {
    support::group("configured_keys_replace_defaults_and_keep_priority");
}

/// Replays the attributed configurable input observations.
#[test]
fn registry_replacement_is_observed_on_next_input() {
    support::group("registry_replacement_is_observed_on_next_input");
}

/// Replays the attributed configurable input observations.
#[test]
fn empty_navigation_confirms_nothing_and_cancels() {
    support::group("empty_navigation_confirms_nothing_and_cancels");
}

/// Callbacks report the current selection without changing it.
#[test]
fn callbacks_report_current_selection_without_mutating_it() {
    support::group("callbacks_report_current_selection_without_mutating_it");
}

/// Replays the attributed retained history observations.
#[test]
fn deep_histories_keep_navigation_iterative() {
    support::group("deep_histories_keep_navigation_iterative");
}

/// Replays the attributed retained history observations.
#[test]
fn selected_node_retains_shared_record_identity() {
    support::group("selected_node_retains_shared_record_identity");
}

/// Replays the attributed retained history observations.
#[test]
fn opaque_entry_fields_survive_selection() {
    support::group("opaque_entry_fields_survive_selection");
}

/// An empty view avoids walking a self-parent leaf.
#[test]
fn self_parent_hidden_leaf_has_no_selection() {
    support::group("self_parent_hidden_leaf_has_no_selection");
}

/// Empty filtering avoids walking a retained self-parent selection.
#[test]
fn all_to_empty_filter_keeps_self_parent_bookmark() {
    support::group("all_to_empty_filter_keeps_self_parent_bookmark");
}

/// A hidden ancestry cycle falls back to the last unrelated visible row.
#[test]
fn hidden_parent_cycle_falls_back_to_last_visible() {
    support::group("hidden_parent_cycle_falls_back_to_last_visible");
}
